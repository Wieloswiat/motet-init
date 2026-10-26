//! Serving: one route for every method, dispatched through the [`Registry`].
//!
//! A handler can only be added together with its [`Method`] declaration, so
//! the registry and the routes can't drift apart. Each request is checked in
//! the same order, before any handler code runs:
//!
//! 1. the method exists and the HTTP verb matches its kind;
//! 2. the caller is established from the request's credentials;
//! 3. the registry authorizes that caller for that method;
//! 4. only then is a WebSocket upgrade extracted or the bounded body read.
//!
//! An unauthenticated request never gets as far as having its body read.

use std::collections::BTreeMap;
use std::sync::Arc;

use axum::Router;
use axum::body::Bytes;
use axum::extract::ws::WebSocketUpgrade;
use axum::extract::{FromRequestParts, Path, Request, State};
use axum::http::{HeaderMap, HeaderValue, Method as Verb, StatusCode, Uri, header};
use axum::response::{IntoResponse, Response};
use axum::routing::any;
use futures::future::BoxFuture;

use crate::{Caller, Kind, Method, Registry, RegistryError, XrpcError};

/// What a handler receives besides its state and caller.
#[derive(Debug, Default)]
pub struct Input {
    /// Query parameters, in order, repeated keys kept (lexicon arrays).
    pub params: Vec<(String, String)>,
    /// The request body, already bounded by the method's `max_body`.
    pub body: Bytes,
    /// Original headers for checking resource-bound proofs after parsing the
    /// request. Authentication has already verified the credential itself.
    pub headers: HeaderMap,
    /// An upgrade extracted only after the registry authorized this caller.
    /// Subscription handlers consume it; ordinary requests leave it absent.
    pub websocket: Option<WebSocketUpgrade>,
}

type Handler<S> = Arc<dyn Fn(S, Caller, Input) -> BoxFuture<'static, Result<Response, XrpcError>> + Send + Sync>;

/// The original HTTP context used to bind credentials to this request.
///
/// The URI is the transport URI; authenticators use their configured public
/// origin, not untrusted forwarded headers, when validating `DPoP` `htu`.
#[derive(Clone, Copy, Debug)]
pub struct AuthenticationContext<'a> {
    pub headers: &'a HeaderMap,
    pub method: &'a Verb,
    pub uri: &'a Uri,
}

/// Establishes who is calling from their credentials and HTTP context.
/// Returns [`Caller::Anonymous`] only when no credentials are present:
/// credentials that fail verification must return an error.
pub type Authenticate =
    Arc<dyn for<'a> Fn(AuthenticationContext<'a>) -> BoxFuture<'a, Result<Caller, XrpcError>> + Send + Sync>;

/// Makes token-authenticated responses readable by independent browser clients.
/// Do not apply this policy to cookie-authenticated login or consent routes.
pub fn public_cors(response: &mut Response) {
    let headers = response.headers_mut();
    headers.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, HeaderValue::from_static("*"));
    headers.remove(header::ACCESS_CONTROL_ALLOW_CREDENTIALS);
    headers.append(
        header::ACCESS_CONTROL_EXPOSE_HEADERS,
        HeaderValue::from_static("DPoP-Nonce, WWW-Authenticate"),
    );
}

const CORS_REQUEST_HEADERS: &str = "authorization,dpop,content-type,accept,accept-language,content-language,atproto-proxy,atproto-space-audience,atproto-space-want-mac,signature,signature-input";

/// A set of XRPC methods with their handlers, built into an axum router.
pub struct Service<S> {
    methods: Vec<Method>,
    handlers: BTreeMap<&'static str, Handler<S>>,
    authenticate: Authenticate,
}

impl<S> std::fmt::Debug for Service<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Service")
            .field("methods", &self.methods)
            .finish_non_exhaustive()
    }
}

impl<S: Clone + Send + Sync + 'static> Service<S> {
    pub fn new(authenticate: Authenticate) -> Self {
        Self {
            methods: Vec::new(),
            handlers: BTreeMap::new(),
            authenticate,
        }
    }

    /// Declares `method` and the handler that serves it.
    #[must_use]
    pub fn method<F, Fut>(mut self, method: Method, handler: F) -> Self
    where
        F: Fn(S, Caller, Input) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<Response, XrpcError>> + Send + 'static,
    {
        let handler: Handler<S> = Arc::new(move |state, caller, input| Box::pin(handler(state, caller, input)));
        self.handlers.insert(method.nsid, handler);
        self.methods.push(method);
        self
    }

    /// The router serving `/xrpc/{nsid}`. Fails if a method was declared twice.
    pub fn into_router(self, state: S) -> Result<Router, RegistryError> {
        let shared = Arc::new(Shared {
            registry: Registry::new(self.methods)?,
            handlers: self.handlers,
            authenticate: self.authenticate,
            state,
        });
        Ok(Router::new()
            .route("/xrpc/{nsid}", any(dispatch::<S>))
            .with_state(shared))
    }
}

struct Shared<S> {
    registry: Registry,
    handlers: BTreeMap<&'static str, Handler<S>>,
    authenticate: Authenticate,
    state: S,
}

async fn dispatch<S: Clone + Send + Sync + 'static>(
    State(shared): State<Arc<Shared<S>>>,
    Path(nsid): Path<String>,
    request: Request,
) -> Response {
    let preflight = request.method() == Verb::OPTIONS;
    let mut response = match serve(&shared, &nsid, request).await {
        Ok(response) => response,
        Err(error) => error.into_response(),
    };
    public_cors(&mut response);
    if preflight {
        response.headers_mut().append(
            header::VARY,
            HeaderValue::from_static("Access-Control-Request-Method, Access-Control-Request-Headers"),
        );
    }
    response
}

async fn serve<S: Clone + Send + Sync + 'static>(
    shared: &Shared<S>,
    nsid: &str,
    request: Request,
) -> Result<Response, XrpcError> {
    let (mut parts, body) = request.into_parts();
    if parts.method == Verb::OPTIONS {
        return preflight(&shared.registry, nsid, &parts.headers);
    }
    let kind = match parts.method {
        Verb::GET => Kind::Query,
        Verb::POST => Kind::Procedure,
        _ => return Err(XrpcError::InvalidRequest("XRPC uses GET and POST only".into())),
    };
    let method = shared.registry.resolve(nsid, kind)?;
    let caller = (shared.authenticate)(AuthenticationContext {
        headers: &parts.headers,
        method: &parts.method,
        uri: &parts.uri,
    })
    .await?;
    let method = Registry::authorize_method(method, &caller)?;
    let handler = shared
        .handlers
        .get(method.nsid)
        .ok_or(XrpcError::MethodNotImplemented)?;
    // Extracting an upgrade consumes its transport capability. Keep it behind
    // the same authorization boundary as ordinary request bodies.
    let websocket = if parts.headers.contains_key(header::UPGRADE) {
        Some(
            WebSocketUpgrade::from_request_parts(&mut parts, &())
                .await
                .map_err(|error| XrpcError::InvalidRequest(error.to_string()))?,
        )
    } else {
        None
    };
    let body = axum::body::to_bytes(body, method.max_body)
        .await
        .map_err(|_| XrpcError::PayloadTooLarge(method.max_body))?;
    let params = parse_query(parts.uri.query())?;
    handler(
        shared.state.clone(),
        caller,
        Input {
            params,
            body,
            headers: parts.headers,
            websocket,
        },
    )
    .await
}

fn preflight(registry: &Registry, nsid: &str, headers: &HeaderMap) -> Result<Response, XrpcError> {
    let invalid = || XrpcError::InvalidRequest("invalid XRPC CORS preflight".into());
    let mut origins = headers.get_all(header::ORIGIN).iter();
    match (origins.next(), origins.next()) {
        (Some(origin), None) if !origin.as_bytes().is_empty() => {}
        _ => return Err(invalid()),
    }
    let mut methods = headers.get_all(header::ACCESS_CONTROL_REQUEST_METHOD).iter();
    let (kind, verb) = match (methods.next().map(HeaderValue::as_bytes), methods.next()) {
        (Some(b"GET"), None) => (Kind::Query, "GET"),
        (Some(b"POST"), None) => (Kind::Procedure, "POST"),
        _ => return Err(invalid()),
    };
    registry.resolve(nsid, kind)?;
    for requested in headers.get_all(header::ACCESS_CONTROL_REQUEST_HEADERS) {
        for name in requested.to_str().map_err(|_| invalid())?.split(',') {
            if !CORS_REQUEST_HEADERS
                .split(',')
                .any(|allowed| allowed.eq_ignore_ascii_case(name.trim()))
            {
                return Err(invalid());
            }
        }
    }
    let mut response = StatusCode::NO_CONTENT.into_response();
    response
        .headers_mut()
        .insert(header::ACCESS_CONTROL_ALLOW_METHODS, HeaderValue::from_static(verb));
    response.headers_mut().insert(
        header::ACCESS_CONTROL_ALLOW_HEADERS,
        HeaderValue::from_static(CORS_REQUEST_HEADERS),
    );
    Ok(response)
}

fn parse_query(query: Option<&str>) -> Result<Vec<(String, String)>, XrpcError> {
    let Some(query) = query else { return Ok(Vec::new()) };
    serde_urlencoded::from_str(query).map_err(|_| XrpcError::InvalidRequest("malformed query string".into()))
}
