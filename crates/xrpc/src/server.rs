//! Serving: one route for every method, dispatched through the [`Registry`].
//!
//! A handler can only be added together with its [`Method`] declaration, so
//! the registry and the routes can't drift apart. Each request is checked in
//! the same order, before any handler code runs:
//!
//! 1. the method exists and the HTTP verb matches its kind;
//! 2. the caller is established from the request's credentials;
//! 3. the registry authorizes that caller for that method;
//! 4. only then is the body read, up to the method's `max_body`.
//!
//! An unauthenticated request never gets as far as having its body read.

use std::collections::BTreeMap;
use std::sync::Arc;

use axum::Router;
use axum::body::{Body, Bytes};
use axum::extract::{Path, RawQuery, State};
use axum::http::{HeaderMap, Method as Verb};
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
}

type Handler<S> = Arc<dyn Fn(S, Caller, Input) -> BoxFuture<'static, Result<Response, XrpcError>> + Send + Sync>;

/// Establishes who is calling from a request's headers. Returns
/// [`Caller::Anonymous`] when there are no credentials, and an error when
/// there are credentials that don't verify: a bad token is never anonymous.
pub type Authenticate = Arc<dyn Fn(&HeaderMap) -> BoxFuture<'static, Result<Caller, XrpcError>> + Send + Sync>;

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
    verb: Verb,
    headers: HeaderMap,
    RawQuery(query): RawQuery,
    body: Body,
) -> Response {
    match serve(&shared, &nsid, &verb, &headers, query.as_deref(), body).await {
        Ok(response) => response,
        Err(error) => error.into_response(),
    }
}

async fn serve<S: Clone + Send + Sync + 'static>(
    shared: &Shared<S>,
    nsid: &str,
    verb: &Verb,
    headers: &HeaderMap,
    query: Option<&str>,
    body: Body,
) -> Result<Response, XrpcError> {
    let kind = match *verb {
        Verb::GET => Kind::Query,
        Verb::POST => Kind::Procedure,
        _ => return Err(XrpcError::InvalidRequest("XRPC uses GET and POST only".into())),
    };
    let caller = (shared.authenticate)(headers).await?;
    let method = shared.registry.authorize(nsid, kind, &caller)?;
    let handler = shared
        .handlers
        .get(method.nsid)
        .ok_or(XrpcError::MethodNotImplemented)?;
    let body = axum::body::to_bytes(body, method.max_body)
        .await
        .map_err(|_| XrpcError::PayloadTooLarge(method.max_body))?;
    let params = parse_query(query)?;
    handler(shared.state.clone(), caller, Input { params, body }).await
}

fn parse_query(query: Option<&str>) -> Result<Vec<(String, String)>, XrpcError> {
    let Some(query) = query else { return Ok(Vec::new()) };
    serde_urlencoded::from_str(query).map_err(|_| XrpcError::InvalidRequest("malformed query string".into()))
}
