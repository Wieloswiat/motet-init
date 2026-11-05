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

use crate::api::State;
use crate::auth::{self, pds::PdsError};
use crate::writes::client::{ClientError, Target};

const RESERVE_WORLD: &str = "at.motet.pilot.tenor.reserveWorld";
const CREATE_WORLD: &str = "at.motet.pilot.tenor.createWorld";
const OPEN_SCENE: &str = "at.motet.pilot.play.openScene";
const SET_CAST: &str = "at.motet.pilot.play.setSceneCast";
const CLOSE_SCENE: &str = "at.motet.pilot.play.closeScene";
const GET_CONTROLS: &str = "at.motet.pilot.play.getSceneControls";

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

const CAST: Method = Method {
    nsid: "at.motet.pilot.setSceneCast",
    kind: Kind::Procedure,
    access: Access::AppviewSession,
    max_body: 256 * 1024,
};
const CLOSE: Method = Method {
    nsid: "at.motet.pilot.closeScene",
    kind: Kind::Procedure,
    access: Access::AppviewSession,
    max_body: 4096,
};

const CONTROLS: Method = Method {
    nsid: "at.motet.pilot.getSceneControls",
    kind: Kind::Query,
    access: Access::AppviewSession,
    max_body: 0,
};

/// Register World and scene procedures with the same browser-session boundary.
#[must_use]
pub fn register(service: Service<Arc<State>>) -> Service<Arc<State>> {
    service
        .method(BEGIN, begin_world)
        .method(FINISH, finish_world)
        .method(OPEN, open_scene)
        .method(CAST, set_scene_cast)
        .method(CLOSE, close_scene)
        .method(CONTROLS, get_scene_controls)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BeginInput {
    op: String,
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
    tenor_url: &'a str,
}

async fn begin_world(state: Arc<State>, caller: Caller, input: Input) -> Result<Response, XrpcError> {
    require_session(&caller)?;
    let session = auth::session_cookie(&input.headers)?;
    let request: BeginInput = serde_json::from_slice(&input.body).map_err(invalid)?;
    atproto_record::record_key::check_record_key(&request.op).map_err(invalid)?;
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
        "op": request.op,
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
        tenor_url: target.endpoint(),
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
    op: String,
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
    atproto_record::record_key::check_record_key(&request.op).map_err(invalid)?;
    if request.title.len() > 1200 {
        return Err(invalid("scene title exceeds 1200 bytes"));
    }
    if request.cast.len() > 100 {
        return Err(invalid("cast exceeds 100 members"));
    }
    let target = state.client.world_target(&request.world).await.map_err(client_error)?;
    let cast = resolve_cast(&state.client, request.cast).await?;
    let token = state
        .auth
        .pds()
        .service_token(session, target.audience(), OPEN_SCENE)
        .await
        .map_err(pds_error)?;
    let request = json!({
        "op": request.op,
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

async fn resolve_cast(
    client: &crate::writes::client::Client,
    identifiers: Vec<String>,
) -> Result<BTreeSet<String>, XrpcError> {
    let identifiers: BTreeSet<_> = identifiers.into_iter().collect();
    stream::iter(identifiers)
        .map(|identifier| async move { client.member_did(&identifier).await })
        .buffer_unordered(8)
        .try_collect()
        .await
        .map_err(client_error)
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct SceneInput {
    world: String,
    scene: String,
    op: String,
    swap_record: String,
}

impl SceneInput {
    fn validate(&self) -> Result<(), XrpcError> {
        atproto_record::record_key::check_record_key(&self.op).map_err(invalid)?;
        atproto_record::record_key::check_record_key(&self.scene).map_err(invalid)?;
        self.swap_record.parse::<atproto_dasl::Cid>().map_err(invalid)?;
        Ok(())
    }
}

#[derive(Deserialize)]
struct CastInput {
    #[serde(flatten)]
    scene: SceneInput,
    cast: Vec<String>,
}

async fn set_scene_cast(state: Arc<State>, caller: Caller, input: Input) -> Result<Response, XrpcError> {
    require_session(&caller)?;
    let request: CastInput = serde_json::from_slice(&input.body).map_err(invalid)?;
    request.scene.validate()?;
    if request.cast.len() > 100 {
        return Err(invalid("cast exceeds 100 members"));
    }
    let target = state
        .client
        .world_target(&request.scene.world)
        .await
        .map_err(client_error)?;
    let cast = resolve_cast(&state.client, request.cast).await?;
    let body = json!({
        "world": request.scene.world,
        "scene": request.scene.scene,
        "op": request.scene.op,
        "swapRecord": request.scene.swap_record,
        "cast": cast,
    });
    change_scene(&state, &input, &target, &request.scene.world, SET_CAST, &body).await
}

async fn close_scene(state: Arc<State>, caller: Caller, input: Input) -> Result<Response, XrpcError> {
    require_session(&caller)?;
    let request: SceneInput = serde_json::from_slice(&input.body).map_err(invalid)?;
    request.validate()?;
    let target = state.client.world_target(&request.world).await.map_err(client_error)?;
    let body = serde_json::to_value(&request).map_err(invalid)?;
    change_scene(&state, &input, &target, &request.world, CLOSE_SCENE, &body).await
}

async fn change_scene(
    state: &State,
    input: &Input,
    target: &Target,
    world: &str,
    method: &str,
    body: &Value,
) -> Result<Response, XrpcError> {
    let session = auth::session_cookie(&input.headers)?;
    // Each attempt gets a fresh single-use token; the caller keeps its operation and swap unchanged.
    let token = state
        .auth
        .pds()
        .service_token(session, target.audience(), method)
        .await
        .map_err(pds_error)?;
    let response = state
        .client
        .post(target, &token, method, body)
        .await
        .map_err(client_error)?;
    let changed: Opened = serde_json::from_value(response).map_err(|_| upstream())?;
    if body.get("scene").and_then(Value::as_str) != Some(changed.scene.as_str())
        || changed.uri != format!("at://{world}/at.motet.pilot.scene/{}", changed.scene)
        || changed.cid.parse::<atproto_dasl::Cid>().is_err()
    {
        return Err(upstream());
    }
    Ok(Json(changed).into_response())
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct Controls {
    set_cast: bool,
    close: bool,
}

async fn get_scene_controls(state: Arc<State>, caller: Caller, input: Input) -> Result<Response, XrpcError> {
    require_session(&caller)?;
    let [(name, world)] = input.params.as_slice() else {
        return Err(invalid("world is required and must be the only parameter"));
    };
    if name != "world" {
        return Err(invalid("world is required"));
    }
    let session = auth::session_cookie(&input.headers)?;
    let target = state.client.world_target(world).await.map_err(client_error)?;
    let token = state
        .auth
        .pds()
        .service_token(session, target.audience(), GET_CONTROLS)
        .await
        .map_err(pds_error)?;
    let response = state
        .client
        .query(&target, &token, GET_CONTROLS, &[("world", world)])
        .await
        .map_err(client_error)?;
    let controls: Controls = serde_json::from_value(response).map_err(|_| upstream())?;
    Ok((
        [(axum::http::header::CACHE_CONTROL, "private, no-store")],
        Json(controls),
    )
        .into_response())
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
        ClientError::InvalidSwap => XrpcError::Protocol {
            status: StatusCode::BAD_REQUEST,
            name: "InvalidSwap",
            message: "the scene changed since the supplied swapRecord; reload before making a new change".into(),
        },
        ClientError::SceneClosed => XrpcError::Protocol {
            status: StatusCode::BAD_REQUEST,
            name: "SceneClosed",
            message: "the scene is already closed".into(),
        },
        ClientError::Unauthorized | ClientError::Upstream => {
            tracing::warn!(error = %error, "tenor request failed");
            upstream()
        }
    }
}
