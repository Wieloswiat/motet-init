//! Browser-session World creation and public-scene procedures.

use std::collections::BTreeSet;
use std::sync::Arc;

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use futures::{StreamExt as _, TryStreamExt as _, stream};
use motet_xrpc::server::{Input, Service};
use motet_xrpc::{Access, Caller, Kind, Method, XrpcError};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::api::State;
use crate::auth::{self, pds::PdsError};
use crate::writes::client::ClientError;

const RESERVE_WORLD: &str = "at.motet.pilot.tenor.reserveWorld";
const CREATE_WORLD: &str = "at.motet.pilot.tenor.createWorld";
const OPEN_SCENE: &str = "at.motet.pilot.play.openScene";

const BEGIN: Method = Method {
    nsid: "at.motet.pilot.beginWorld",
    kind: Kind::Procedure,
    access: Access::AppviewSession,
    max_body: 16 * 1024,
};
const FINISH: Method = Method {
    nsid: "at.motet.pilot.finishWorld",
    kind: Kind::Procedure,
    access: Access::AppviewSession,
    max_body: 32 * 1024,
};
const OPEN: Method = Method {
    nsid: "at.motet.pilot.openScene",
    kind: Kind::Procedure,
    access: Access::AppviewSession,
    max_body: 256 * 1024,
};

/// Register the three procedures with the same browser-session boundary.
#[must_use]
pub fn register(service: Service<Arc<State>>) -> Service<Arc<State>> {
    service
        .method(BEGIN, begin_world)
        .method(FINISH, finish_world)
        .method(OPEN, open_scene)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BeginInput {
    handle: String,
    name: String,
    visibility: Visibility,
    rotation_key: String,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
enum Visibility {
    Public,
    Private,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct Reserved {
    reservation: String,
    operation: Value,
    signing_bytes: String,
    expires_at: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Begun<'a> {
    #[serde(flatten)]
    reserved: Reserved,
    tenor: &'a str,
}

async fn begin_world(state: Arc<State>, caller: Caller, input: Input) -> Result<Response, XrpcError> {
    require_session(&caller)?;
    let session = auth::session_cookie(&input.headers)?;
    let request: BeginInput = serde_json::from_slice(&input.body).map_err(invalid)?;
    if request.name.len() > 640 {
        return Err(invalid("World name exceeds 640 bytes"));
    }
    let target = state.client.default_target();
    let token = state
        .auth
        .pds()
        .service_token(session, target.audience(), RESERVE_WORLD)
        .await
        .map_err(pds_error)?;
    // The browser offers no recovery-delegation choice. Do not silently give
    // the tenor a rotation key that could recover the owner's identity.
    let request = json!({
        "handle": request.handle,
        "name": request.name,
        "visibility": request.visibility,
        "rotationKey": request.rotation_key,
        "tenorRotationKey": false,
    });
    let response = state
        .client
        .post(target, &token, RESERVE_WORLD, &request)
        .await
        .map_err(client_error)?;
    let reserved: Reserved = serde_json::from_value(response).map_err(|_| upstream())?;
    if reserved.reservation.is_empty() || reserved.signing_bytes.is_empty() || !reserved.operation.is_object() {
        return Err(upstream());
    }
    Ok(Json(Begun {
        reserved,
        tenor: target.audience(),
    })
    .into_response())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FinishInput {
    reservation: String,
    signed_operation: Value,
}

#[derive(Deserialize, Serialize)]
struct Created {
    did: String,
    handle: String,
}

async fn finish_world(state: Arc<State>, caller: Caller, input: Input) -> Result<Response, XrpcError> {
    require_session(&caller)?;
    let session = auth::session_cookie(&input.headers)?;
    let request: FinishInput = serde_json::from_slice(&input.body).map_err(invalid)?;
    let target = state.client.default_target();
    let token = state
        .auth
        .pds()
        .service_token(session, target.audience(), CREATE_WORLD)
        .await
        .map_err(pds_error)?;
    // The tenor remembers the reservation and signed genesis. Sending them
    // unchanged makes a caller's retry safe after an uncertain response.
    let request = json!({
        "reservation": request.reservation,
        "signedOperation": request.signed_operation,
    });
    let response = state
        .client
        .post(target, &token, CREATE_WORLD, &request)
        .await
        .map_err(client_error)?;
    let created: Created = serde_json::from_value(response).map_err(|_| upstream())?;
    if created.did.is_empty() || created.handle.is_empty() {
        return Err(upstream());
    }
    Ok(Json(created).into_response())
}

#[derive(Deserialize)]
struct OpenInput {
    world: String,
    title: String,
    #[serde(default)]
    cast: Vec<String>,
}

#[derive(Deserialize, Serialize)]
struct Opened {
    scene: String,
    uri: String,
    cid: String,
}

async fn open_scene(state: Arc<State>, caller: Caller, input: Input) -> Result<Response, XrpcError> {
    require_session(&caller)?;
    let session = auth::session_cookie(&input.headers)?;
    let request: OpenInput = serde_json::from_slice(&input.body).map_err(invalid)?;
    if request.title.len() > 1200 {
        return Err(invalid("scene title exceeds 1200 bytes"));
    }
    if request.cast.len() > 100 {
        return Err(invalid("cast exceeds 100 members"));
    }
    let target = state.client.world_target(&request.world).await.map_err(client_error)?;
    let client = &state.client;
    let identifiers: BTreeSet<_> = request.cast.into_iter().collect();
    let cast: BTreeSet<String> = stream::iter(identifiers)
        .map(|identifier| async move { client.member_did(&identifier).await })
        .buffer_unordered(8)
        .try_collect()
        .await
        .map_err(client_error)?;
    let token = state
        .auth
        .pds()
        .service_token(session, target.audience(), OPEN_SCENE)
        .await
        .map_err(pds_error)?;
    // No client operation key exists in this lexicon. A new request must not
    // collapse into an earlier, deliberately identical scene.
    let request = json!({
        "op": Uuid::now_v7().to_string(),
        "world": request.world,
        "title": request.title,
        "cast": cast,
    });
    let response = state
        .client
        .post(&target, &token, OPEN_SCENE, &request)
        .await
        .map_err(client_error)?;
    let opened: Opened = serde_json::from_value(response).map_err(|_| upstream())?;
    if atproto_record::record_key::check_record_key(&opened.scene).is_err()
        || opened.uri.is_empty()
        || opened.cid.is_empty()
    {
        return Err(upstream());
    }
    Ok(Json(opened).into_response())
}

fn require_session(caller: &Caller) -> Result<(), XrpcError> {
    if matches!(caller, Caller::AppviewSession { .. }) {
        Ok(())
    } else {
        Err(XrpcError::AuthRequired)
    }
}

fn invalid(error: impl std::fmt::Display) -> XrpcError {
    XrpcError::InvalidRequest(error.to_string())
}

fn forbidden() -> XrpcError {
    XrpcError::Protocol {
        status: StatusCode::FORBIDDEN,
        name: "Forbidden",
        message: "the signed-in member is not permitted to perform this operation".into(),
    }
}

fn upstream() -> XrpcError {
    XrpcError::Protocol {
        status: StatusCode::BAD_GATEWAY,
        name: "UpstreamFailure",
        message: "the PDS or tenor could not complete the request".into(),
    }
}

fn pds_error(error: PdsError) -> XrpcError {
    match error {
        PdsError::Session => XrpcError::AuthRequired,
        PdsError::Status(StatusCode::FORBIDDEN) => forbidden(),
        other => {
            tracing::warn!(error = %other, "PDS write grant failed");
            upstream()
        }
    }
}

fn client_error(error: ClientError) -> XrpcError {
    match error {
        ClientError::NotFound => XrpcError::NotFound,
        ClientError::Forbidden => forbidden(),
        ClientError::InvalidRequest(message) => XrpcError::InvalidRequest(message),
        ClientError::Unauthorized | ClientError::Upstream => {
            tracing::warn!(error = %error, "tenor request failed");
            upstream()
        }
    }
}
