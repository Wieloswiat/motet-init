//! Request bounds protect handler capacity and preserve long-lived subscriptions.
use std::{sync::Arc, time::Duration};

use axum::{
    body::Body,
    http::{Request, StatusCode},
    response::IntoResponse,
};
use motet_xrpc::{
    Access, Caller, Kind, Method,
    limits::{self, Limits},
    server::{Authenticate, Input, Service},
};
use tokio::sync::{Barrier, Notify};
use tower::ServiceExt;

fn auth() -> Authenticate {
    Arc::new(|_| Box::pin(async { Ok(Caller::Anonymous) }))
}
fn method(nsid: &'static str) -> Method {
    Method {
        nsid,
        kind: Kind::Query,
        access: Access::Public,
        max_body: 0,
    }
}
fn bounded(gate: &Arc<Notify>, barrier: Option<Arc<Barrier>>, permits: usize, timeout: Duration) -> axum::Router {
    Service::new(auth())
        .method(method("test.slow"), {
            let gate = Arc::clone(gate);
            move |(), _, _: Input| {
                let gate = Arc::clone(&gate);
                async move {
                    gate.notified().await;
                    Ok("done".into_response())
                }
            }
        })
        .method(method("com.atproto.sync.subscribeRepos"), {
            let gate = Arc::clone(gate);
            move |(), _, _: Input| {
                let gate = Arc::clone(&gate);
                let barrier = barrier.clone();
                async move {
                    if let Some(barrier) = barrier {
                        barrier.wait().await;
                    }
                    gate.notified().await;
                    Ok("stream".into_response())
                }
            }
        })
        .into_router(())
        .expect("test router has unique method declarations")
        .layer(axum::middleware::from_fn_with_state(
            Limits::new(permits, 1, timeout, Some("/xrpc/com.atproto.sync.subscribeRepos")),
            limits::enforce,
        ))
}

/// A slow ordinary request returns 504; cancellation must not be disguised as success.
#[tokio::test]
async fn ordinary_handler_exceeding_deadline_gets_gateway_timeout() {
    let gate = Arc::new(Notify::new());
    let app = bounded(&gate, None, 2, Duration::from_millis(1));
    let response = app
        .oneshot(Request::get("/xrpc/test.slow").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::GATEWAY_TIMEOUT);
}

/// Admission is non-queuing: once the sole permit is held, a second request is immediately shed.
#[tokio::test]
async fn excess_requests_are_shed_while_first_handler_is_blocked() {
    let gate = Arc::new(Notify::new());
    let app = bounded(&gate, None, 1, Duration::from_secs(5));
    let (entered_tx, entered_rx) = tokio::sync::oneshot::channel();
    let first_app = app.clone();
    let first = tokio::spawn(async move {
        let response = first_app
            .oneshot(Request::get("/xrpc/test.slow").body(Body::empty()).unwrap())
            .await
            .unwrap();
        let _ = entered_tx.send(());
        response
    });
    // The blocked handler occupies the request slot; the second call must not wait for it.
    tokio::task::yield_now().await;
    let rejected = app
        .oneshot(Request::get("/xrpc/test.slow").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(rejected.status(), StatusCode::SERVICE_UNAVAILABLE);
    gate.notify_waiters();
    let _ = entered_rx.await;
    assert_eq!(first.await.unwrap().status(), StatusCode::OK);
}

/// The named firehose route bypasses ordinary deadlines and consumes its separate stream permit.
#[tokio::test]
async fn explicitly_exempt_stream_outlives_ordinary_timeout() {
    let gate = Arc::new(Notify::new());
    let app = bounded(&gate, None, 1, Duration::from_millis(1));
    let request = tokio::spawn(async move {
        app.oneshot(
            Request::get("/xrpc/com.atproto.sync.subscribeRepos")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
    });
    tokio::time::sleep(Duration::from_millis(10)).await;
    assert!(!request.is_finished());
    gate.notify_waiters();
    assert_eq!(request.await.unwrap().status(), StatusCode::OK);
}
