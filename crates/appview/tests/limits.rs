//! The deployed AppView shares one admission and deadline boundary across all routes.

mod support;

use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;

use axum::body::{Body, Bytes, to_bytes};
use axum::http::{Request, StatusCode, header};
use futures::FutureExt as _;
use motet_appview::api;
use motet_xrpc::Caller;
use motet_xrpc::limits::Limits;
use motet_xrpc::server::Authenticate;
use serde_json::Value;
use sqlx::postgres::PgPoolOptions;
use tokio::sync::Notify;
use tower::ServiceExt as _;

const SESSION: &str = "/xrpc/at.motet.pilot.auth.getSession";

fn router(authenticate: Authenticate, timeout: Duration) -> axum::Router {
    // None of these requests touches storage; an accidental database access must not pass silently.
    let pool = PgPoolOptions::new()
        .connect_lazy("postgres://unused:unused@127.0.0.1:1/unused")
        .expect("lazy test database configuration");
    let auth = support::oauth(pool.clone());
    api::served_router(
        Arc::new(api::State {
            pool,
            auth,
            client: support::client(),
        }),
        authenticate,
        Limits::new(1, 1, timeout, None),
    )
    .expect("served AppView router")
}

fn authenticated() -> Authenticate {
    Arc::new(|_| {
        Box::pin(async {
            Ok(Caller::AppviewSession {
                did: "did:plc:reader".into(),
                handle: "reader.example".into(),
            })
        })
    })
}

fn request(path: &str) -> Request<Body> {
    Request::get(path).body(Body::empty()).expect("fixture GET request")
}

fn pending_form(entered: Arc<Notify>, released: Arc<Notify>) -> Request<Body> {
    let body = futures::stream::once(async move {
        entered.notify_one();
        released.notified().await;
        Ok::<_, Infallible>(Bytes::new())
    });
    Request::post("/oauth/sign-in")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from_stream(body))
        .expect("fixture OAuth form request")
}

fn assert_shed(router: &axum::Router) {
    // No origin is supplied: absent overload, this OAuth POST must be refused as cross-origin, not shed.
    let form = Request::post("/oauth/sign-in")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::empty())
        .expect("fixture OAuth form request");
    for request in [
        request("/_health"),
        request("/oauth/client-metadata.json"),
        form,
        request(SESSION),
    ] {
        let response = router
            .clone()
            .oneshot(request)
            .now_or_never()
            .expect("overload must respond without awaiting the held request")
            .expect("router is infallible");
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    }
}

/// A slow authenticator must occupy the outer budget, not just a handler's budget.
/// Catches applying admission after authentication or exempting health and OAuth accidentally.
#[tokio::test]
async fn pending_authentication_sheds_all_routes_without_waiting_for_a_free_slot() {
    let entered = Arc::new(Notify::new());
    let released = Arc::new(Notify::new());
    let authenticate: Authenticate = {
        let entered = Arc::clone(&entered);
        let released = Arc::clone(&released);
        Arc::new(move |_| {
            let entered = Arc::clone(&entered);
            let released = Arc::clone(&released);
            Box::pin(async move {
                entered.notify_one();
                released.notified().await;
                Ok(Caller::AppviewSession {
                    did: "did:plc:reader".into(),
                    handle: "reader.example".into(),
                })
            })
        })
    };
    let router = router(authenticate, Duration::from_secs(60));
    let pending = tokio::spawn(router.clone().oneshot(request(SESSION)));
    entered.notified().await;
    assert_shed(&router);
    released.notify_one();
    assert_eq!(
        pending.await.expect("request task").expect("router response").status(),
        StatusCode::OK
    );
    assert_eq!(
        router
            .oneshot(request("/_health"))
            .await
            .expect("health response")
            .status(),
        StatusCode::OK
    );
}

/// Pending form extraction must share admission with XRPC, and completion must return its permit.
/// Catches limiting only the XRPC subrouter or dropping the permit before reading the OAuth body.
#[tokio::test]
async fn pending_oauth_form_sheds_all_routes_and_body_completion_restores_capacity() {
    let entered = Arc::new(Notify::new());
    let released = Arc::new(Notify::new());
    let router = router(authenticated(), Duration::from_secs(60));
    let pending = tokio::spawn(
        router
            .clone()
            .oneshot(pending_form(Arc::clone(&entered), Arc::clone(&released))),
    );
    entered.notified().await;
    assert_shed(&router);
    released.notify_one();
    assert_eq!(
        pending.await.expect("request task").expect("router response").status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        router
            .oneshot(request("/_health"))
            .await
            .expect("health response")
            .status(),
        StatusCode::OK
    );
}

/// Slow authentication and OAuth body extraction must time out and release shared admission.
/// Catches a deadline placed only around XRPC handlers, or retaining permits after cancellation.
#[tokio::test]
async fn deadline_covers_authentication_and_oauth_body_and_returns_capacity() {
    for oauth_body in [false, true] {
        let entered = Arc::new(Notify::new());
        let released = Arc::new(Notify::new());
        let authenticate: Authenticate = if oauth_body {
            authenticated()
        } else {
            let entered = Arc::clone(&entered);
            Arc::new(move |_| {
                let entered = Arc::clone(&entered);
                Box::pin(async move {
                    entered.notify_one();
                    std::future::pending().await
                })
            })
        };
        let router = router(authenticate, Duration::from_millis(25));
        let request = if oauth_body {
            pending_form(Arc::clone(&entered), released)
        } else {
            request(SESSION)
        };
        let pending = tokio::spawn(router.clone().oneshot(request));
        entered.notified().await;
        let response = pending.await.expect("deadline request task").expect("router response");
        assert_eq!(
            response.status(),
            StatusCode::GATEWAY_TIMEOUT,
            "OAuth body: {oauth_body}"
        );
        let response = router
            .oneshot(self::request("/_health"))
            .await
            .expect("health after timeout");
        assert_eq!(
            response.status(),
            StatusCode::OK,
            "timeout must release the occupied permit"
        );
    }
}

/// Admission below capacity must preserve real health, OAuth metadata and authenticated API behavior.
/// Catches an outer limiter that rejects healthy requests or alters successful responses.
#[tokio::test]
async fn requests_under_capacity_keep_health_oauth_and_session_responses() {
    let router = router(authenticated(), Duration::from_secs(60));
    let health = router
        .clone()
        .oneshot(request("/_health"))
        .await
        .expect("health response");
    assert_eq!(health.status(), StatusCode::OK);
    assert_eq!(to_bytes(health.into_body(), 1024).await.expect("health body"), "ok");
    let metadata = router
        .clone()
        .oneshot(request("/oauth/client-metadata.json"))
        .await
        .expect("metadata response");
    assert_eq!(metadata.status(), StatusCode::OK);
    let metadata: Value = serde_json::from_slice(&to_bytes(metadata.into_body(), 16_384).await.expect("metadata body"))
        .expect("OAuth metadata is JSON");
    assert_eq!(
        metadata.get("client_id").and_then(Value::as_str),
        Some("https://motet.example/oauth/client-metadata.json")
    );
    let session = router.oneshot(request(SESSION)).await.expect("session response");
    assert_eq!(session.status(), StatusCode::OK);
    let session: Value = serde_json::from_slice(&to_bytes(session.into_body(), 4096).await.expect("session body"))
        .expect("session is JSON");
    assert_eq!(session.get("did").and_then(Value::as_str), Some("did:plc:reader"));
}
