//! The tenor's XRPC methods.
//!
//! Every method is declared here with who may call it; see `motet-xrpc` for
//! how that is enforced. The tenor trusts nothing about Motet: a request from
//! Motet's API is service auth from the author's PDS like any other client's,
//! so a self-hosted tenor serves Motet and independent clients the same way.
//!
//! Method names avoid opensocial.group's verbs (`admit` there means letting
//! someone into a group), hence `play.submitAdmission`.

use std::sync::Arc;

use axum::Json;
use axum::response::{IntoResponse, Response};
use motet_rules::world::{Outcome, Refusal};
use motet_rules::{Cid, Did, OperationId, StrongRef};
use motet_xrpc::server::{Authenticate, Input, Service};
use motet_xrpc::{Access, Caller, Kind, Method, RegistryError, XrpcError};
use serde::{Deserialize, Serialize};

use crate::admission::{self, Decision, Records};
use crate::shard::{Handle, Rejected};

/// The service fragment a World's DID document gives its tenor endpoint;
/// service-auth tokens for a World name `did#motet_tenor` as their audience.
pub const SERVICE_FRAGMENT: &str = "motet_tenor";

pub const SUBMIT_ADMISSION: Method = Method {
    nsid: "at.motet.pilot.play.submitAdmission",
    kind: Kind::Procedure,
    access: Access::Service,
    max_body: 4096,
};

/// What the API handlers share.
#[derive(Debug)]
pub struct State<R> {
    pub shard: Handle,
    pub records: R,
}

pub fn router<R: Records + 'static>(
    state: Arc<State<R>>,
    authenticate: Authenticate,
) -> Result<axum::Router, RegistryError> {
    Service::new(authenticate)
        .method(SUBMIT_ADMISSION, submit_admission::<R>)
        .into_router(state)
}

/// Whether a token's audience is a World on this tenor: `did#motet_tenor`.
/// Which Worlds are hosted is checked against the request, by the shard.
#[must_use]
pub fn accepts_audience(aud: &str) -> bool {
    aud.split_once('#')
        .is_some_and(|(did, fragment)| did.starts_with("did:") && fragment == SERVICE_FRAGMENT)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SubmitAdmission {
    op: String,
    world: String,
    post: StrongRef,
    replaces: Option<String>,
}

#[derive(Serialize)]
#[serde(tag = "status", rename_all = "camelCase")]
enum Submitted {
    /// Durably admitted, at this place.
    Admitted { epoch: u32, seq: u64 },
    /// Durably refused; the author can be told why.
    Refused { reason: String },
    /// Not decided yet: the record isn't readable (yet). Retry with the same
    /// operation ID, or let sync bring it.
    Pending,
}

async fn submit_admission<R: Records>(
    state: Arc<State<R>>,
    caller: Caller,
    input: Input,
) -> Result<Response, XrpcError> {
    let Caller::Service { did, audience, .. } = caller else {
        return Err(XrpcError::AuthRequired);
    };
    let body: SubmitAdmission =
        serde_json::from_slice(&input.body).map_err(|e| XrpcError::InvalidRequest(e.to_string()))?;
    // The token was minted for one World; it opens nothing in another.
    if audience.split_once('#').map(|(world, _)| world) != Some(body.world.as_str()) {
        return Err(XrpcError::AuthRequired);
    }
    // Only a post's author asks for its admission. A DID in the body is never
    // the caller: the post URI's authority must be the token's issuer.
    if !body.post.uri.as_str().starts_with(&format!("at://{did}/")) {
        return Err(XrpcError::AuthRequired);
    }
    let request = admission::Request {
        op: OperationId::new(format!("{did}/{}", body.op)),
        world: Did::new(body.world),
        post: body.post,
        replaces: body.replaces.map(Cid::new),
    };
    let decision = admission::admit(&state.shard, &state.records, request)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "admission failed");
            XrpcError::Internal
        })?;
    let submitted = match decision {
        Decision::Decided(Ok(Outcome::Admitted(a))) => Submitted::Admitted {
            epoch: a.place.epoch,
            seq: a.place.seq,
        },
        Decision::Decided(Ok(Outcome::Refused(refusal))) => Submitted::Refused {
            reason: refusal_code(&refusal).to_owned(),
        },
        Decision::Decided(Err(Rejected::UnknownWorld(_))) | Decision::OtherWorld => return Err(XrpcError::NotFound),
        Decision::Decided(Err(Rejected::OperationReused(_))) => {
            return Err(XrpcError::InvalidRequest(
                "operation ID reused for a different request".into(),
            ));
        }
        Decision::Decided(Ok(other)) => {
            tracing::error!(?other, "admission produced an unexpected outcome");
            return Err(XrpcError::Internal);
        }
        Decision::Decided(Err(Rejected::AlreadyBegun(_))) => return Err(XrpcError::Internal),
        Decision::NotFound | Decision::Unreachable => Submitted::Pending,
    };
    Ok(Json(submitted).into_response())
}

/// Stable, machine-readable refusal reasons for the lexicon's `knownValues`.
fn refusal_code(refusal: &Refusal) -> &'static str {
    match refusal {
        Refusal::UnknownScene => "UnknownScene",
        Refusal::SceneClosed => "SceneClosed",
        Refusal::NotInCast => "NotInCast",
        Refusal::Suspended => "Suspended",
        Refusal::AlreadyAdmitted => "AlreadyAdmitted",
        Refusal::StaleEdit { .. } => "StaleEdit",
        Refusal::SceneChanged => "SceneChanged",
    }
}
