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
use std::time::SystemTime;

use axum::Json;
use axum::extract::State as AxumState;
use axum::http::{HeaderMap, header, uri::Authority};
use axum::response::{IntoResponse, Response};
use motet_rules::world::{Outcome, Refusal};
use motet_rules::{Cid, Did, OperationId, StrongRef};
use motet_xrpc::server::{Authenticate, Input, Service};
use motet_xrpc::{Access, Caller, Kind, Method, RegistryError, XrpcError};
use serde::{Deserialize, Serialize};

use crate::admission::{self, Decision, Records};
use crate::identity::{self, Hosting, IdentityError};
use crate::shard::{Handle, Rejected};

/// The service fragment a World's DID document gives its tenor endpoint.
/// Service auth accepts its bare DID or exact `did#motet_tenor` audience.
pub const SERVICE_FRAGMENT: &str = "motet_tenor";

pub const SUBMIT_ADMISSION: Method = Method {
    nsid: "at.motet.pilot.play.submitAdmission",
    kind: Kind::Procedure,
    access: Access::Service,
    max_body: 4096,
};

pub const RESERVE_WORLD: Method = Method {
    nsid: "at.motet.pilot.tenor.reserveWorld",
    kind: Kind::Procedure,
    access: Access::Service,
    max_body: 4096,
};

pub const CREATE_WORLD: Method = Method {
    nsid: "at.motet.pilot.tenor.createWorld",
    kind: Kind::Procedure,
    access: Access::Service,
    max_body: 16 * 1024,
};

pub const RESOLVE_HANDLE: Method = Method {
    nsid: "com.atproto.identity.resolveHandle",
    kind: Kind::Query,
    access: Access::Public,
    max_body: 0,
};

pub const DESCRIBE_SERVER: Method = Method {
    nsid: "com.atproto.server.describeServer",
    kind: Kind::Query,
    access: Access::Public,
    max_body: 0,
};

/// What the API handlers share.
#[derive(Debug)]
pub struct State<R> {
    pub shard: Handle,
    pub records: R,
    pub hosting: Arc<Hosting>,
}

#[derive(Debug, thiserror::Error)]
pub enum RouterError {
    #[error(transparent)]
    Registry(#[from] RegistryError),
    #[error(transparent)]
    OAuth(#[from] crate::oauth::Error),
}

pub async fn router<R: Records + 'static>(
    state: Arc<State<R>>,
    authenticate: Authenticate,
) -> Result<axum::Router, RouterError> {
    let spaces = Arc::new(crate::spaces::http::State {
        shard: state.shard.clone(),
        resolver: Arc::new(motet_net::identity::DidResolver::new(
            state.hosting.fetcher().clone(),
            state.hosting.plc_directory(),
        )),
        fetcher: state.hosting.fetcher().clone(),
    });
    let oauth = crate::oauth::Server::open(
        state.shard.clone(),
        state.hosting.endpoint().into(),
        state.hosting.fetcher().clone(),
        Arc::clone(&spaces.resolver),
    )
    .await?;
    let authenticate = crate::oauth::http::authenticate(authenticate, Arc::clone(&oauth));
    let authenticate = crate::spaces::http::authenticate(authenticate, Arc::clone(&spaces));
    let service = Service::new(authenticate)
        .method(SUBMIT_ADMISSION, submit_admission::<R>)
        .method(RESERVE_WORLD, reserve_world::<R>)
        .method(CREATE_WORLD, create_world::<R>)
        .method(RESOLVE_HANDLE, resolve_handle::<R>)
        .method(DESCRIBE_SERVER, describe_server::<R>);
    // Anyone may resolve a hosted handle; only the Host header selects it.
    let well_known = axum::Router::new()
        .route("/.well-known/atproto-did", axum::routing::get(handle_did::<R>))
        .with_state(Arc::clone(&state));
    let service = crate::spaces::http::register(crate::sync::methods(service), &spaces);
    let service = crate::oauth::writes::register(service, &state.shard);
    // Discovery is public. PAR/token verify client + DPoP proofs; browser
    // endpoints require one-use state, a bound cookie and form CSRF tokens.
    let oauth_documents = axum::Router::new()
        .route(
            "/.well-known/oauth-authorization-server",
            axum::routing::get(crate::oauth::http::authorization_metadata),
        )
        .route(
            "/.well-known/oauth-protected-resource",
            axum::routing::get(crate::oauth::http::protected_metadata),
        )
        .route(
            "/oauth/client-metadata.json",
            axum::routing::get(crate::oauth::http::client_metadata),
        )
        .route("/oauth/jwks.json", axum::routing::get(crate::oauth::http::jwks))
        .layer(axum::middleware::from_fn(crate::oauth::http::cors_get));
    let oauth_protocol = axum::Router::new()
        .route("/oauth/par", axum::routing::post(crate::oauth::http::par))
        .route("/oauth/token", axum::routing::post(crate::oauth::http::token))
        .layer(axum::middleware::from_fn(crate::oauth::http::cors_post));
    // Cookie-bearing pages do not grant cross-origin access. Public clients
    // use only the non-credentialed discovery, PAR, token and XRPC surfaces.
    let oauth_routes = axum::Router::new()
        .route("/oauth/authorize", axum::routing::get(crate::oauth::http::authorize))
        .route("/oauth/login", axum::routing::post(crate::oauth::http::login))
        .route("/oauth/callback", axum::routing::get(crate::oauth::http::callback))
        .route("/oauth/consent", axum::routing::post(crate::oauth::http::consent))
        .merge(oauth_documents)
        .merge(oauth_protocol)
        .layer(axum::extract::DefaultBodyLimit::max(16 * 1024))
        .with_state(oauth);
    Ok(service
        .into_router(state)?
        .merge(well_known)
        .merge(oauth_routes)
        .layer(axum::middleware::from_fn(crate::spaces::http::vary_mac)))
}

/// Accepts the tenor's bare DID or creation audience, and per-World audiences.
/// Each handler binds the audience to its own scope before performing any work.
#[must_use]
pub fn accepts_audience(aud: &str, tenor_audience: &str) -> bool {
    aud == tenor_audience
        || tenor_audience
            .split_once('#')
            .is_some_and(|(did, fragment)| aud == did && fragment == SERVICE_FRAGMENT)
        || (!aud.contains('#') && aud.starts_with("did:plc:"))
        || aud.split_once('#').is_some_and(|(did, fragment)| {
            did.starts_with("did:plc:") && matches!(fragment, SERVICE_FRAGMENT | "atproto_pds")
        })
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
    if audience == state.hosting.audience()
        || audience == state.hosting.service_did()
        || (audience != body.world && audience.split_once('#') != Some((body.world.as_str(), SERVICE_FRAGMENT)))
    {
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
    // The rules transaction also queues publication. Acknowledging admission
    // does not wait for the publisher, and a process exit cannot lose its work.
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

fn creation_owner(caller: Caller, hosting: &Hosting) -> Result<Did, XrpcError> {
    match caller {
        Caller::Service { did, audience, .. }
            if audience == hosting.audience() || audience == hosting.service_did() =>
        {
            Ok(Did::new(did))
        }
        _ => Err(XrpcError::AuthRequired),
    }
}

async fn reserve_world<R: Records>(state: Arc<State<R>>, caller: Caller, input: Input) -> Result<Response, XrpcError> {
    let owner = creation_owner(caller, &state.hosting)?;
    let request = serde_json::from_slice(&input.body).map_err(|e| XrpcError::InvalidRequest(e.to_string()))?;
    let reserved = state
        .shard
        .run(identity::ReserveWorld {
            owner,
            request,
            hosting: Arc::clone(&state.hosting),
            now: SystemTime::now(),
        })
        .await
        .map_err(IdentityError::from)
        .map_err(identity_error)?
        .map_err(identity_error)?;
    Ok(Json(reserved).into_response())
}

async fn create_world<R: Records>(state: Arc<State<R>>, caller: Caller, input: Input) -> Result<Response, XrpcError> {
    let owner = creation_owner(caller, &state.hosting)?;
    let request = serde_json::from_slice(&input.body).map_err(|e| XrpcError::InvalidRequest(e.to_string()))?;
    let info = identity::create(&state.shard, &state.hosting, owner, request)
        .await
        .map_err(identity_error)?;
    Ok(Json(serde_json::json!({ "did": info.did, "handle": info.handle })).into_response())
}

async fn resolve_handle<R: Records>(
    state: Arc<State<R>>,
    _caller: Caller,
    input: Input,
) -> Result<Response, XrpcError> {
    let mut handles = input.params.iter().filter(|(key, _)| key == "handle");
    let Some((_, handle)) = handles.next() else {
        return Err(XrpcError::InvalidRequest("handle is required".into()));
    };
    if handles.next().is_some() {
        return Err(XrpcError::InvalidRequest("handle must appear once".into()));
    }
    let did = identity::resolve_handle(state.shard.reader(), handle)
        .await
        .map_err(identity_error)?;
    Ok(Json(serde_json::json!({ "did": did })).into_response())
}

async fn describe_server<R: Records>(
    state: Arc<State<R>>,
    _caller: Caller,
    _input: Input,
) -> Result<Response, XrpcError> {
    Ok(Json(serde_json::json!({
        "did": state.hosting.service_did(),
        "availableUserDomains": state.hosting.domains(),
        "inviteCodeRequired": false,
    }))
    .into_response())
}

async fn handle_did<R: Records>(
    AxumState(state): AxumState<Arc<State<R>>>,
    headers: HeaderMap,
) -> Result<String, XrpcError> {
    let authority: Authority = headers
        .get(header::HOST)
        .and_then(|value| value.to_str().ok())
        .and_then(|host| host.parse().ok())
        .ok_or_else(|| XrpcError::InvalidRequest("a valid Host header is required".into()))?;
    identity::resolve_handle(state.shard.reader(), authority.host())
        .await
        .map(|did| did.as_str().to_owned())
        .map_err(identity_error)
}

#[expect(
    clippy::needless_pass_by_value,
    reason = "Result::map_err consumes the identity error"
)]
fn identity_error(error: IdentityError) -> XrpcError {
    match error {
        IdentityError::NotOwner => XrpcError::AuthRequired,
        IdentityError::HandleNotFound | IdentityError::ReservationNotFound => XrpcError::NotFound,
        IdentityError::InvalidHandle
        | IdentityError::HandleTaken
        | IdentityError::InvalidRotationKey
        | IdentityError::ReservationExpired
        | IdentityError::InvalidOperation => XrpcError::InvalidRequest(error.to_string()),
        _ => {
            tracing::error!(%error, "World identity operation failed");
            XrpcError::Internal
        }
    }
}
