//! Request admission and deadlines for HTTP routers.
use axum::{body::Body, extract::Request, http::StatusCode, middleware::Next, response::Response};
use std::{sync::Arc, time::Duration};
use tokio::sync::Semaphore;

#[derive(Debug, Clone)]
pub struct StreamPermit(pub Arc<tokio::sync::OwnedSemaphorePermit>);

#[derive(Debug, Clone)]
pub struct Limits {
    ordinary: Arc<Semaphore>,
    streams: Arc<Semaphore>,
    timeout: Duration,
    stream_path: Option<&'static str>,
}

impl Limits {
    #[must_use]
    pub fn new(
        in_flight: usize,
        stream_connections: usize,
        timeout: Duration,
        stream_path: Option<&'static str>,
    ) -> Self {
        Self {
            ordinary: Arc::new(Semaphore::new(in_flight)),
            streams: Arc::new(Semaphore::new(stream_connections)),
            timeout,
            stream_path,
        }
    }
}

pub async fn enforce(
    axum::extract::State(limits): axum::extract::State<Limits>,
    request: Request<Body>,
    next: Next,
) -> Response {
    let streaming = limits.stream_path.is_some_and(|path| request.uri().path() == path);
    let permits = if streaming { &limits.streams } else { &limits.ordinary };
    let Ok(permit) = Arc::clone(permits).try_acquire_owned() else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };
    if streaming {
        let mut request = request;
        request.extensions_mut().insert(StreamPermit(Arc::new(permit)));
        return next.run(request).await;
    }
    let response = tokio::time::timeout(limits.timeout, next.run(request))
        .await
        .unwrap_or_else(|_| StatusCode::GATEWAY_TIMEOUT.into_response());
    drop(permit);
    response
}

use axum::response::IntoResponse;
