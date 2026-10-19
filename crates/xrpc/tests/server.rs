//! The dispatch order, through a real router: nothing reaches a handler, or
//! has its body read, unless the registry let it through.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::response::IntoResponse;
use motet_xrpc::server::{Authenticate, Input, Service};
use motet_xrpc::{Access, Caller, Kind, Method};
use tower::ServiceExt;

/// Callers by header, standing in for real verification.
fn authenticate() -> Authenticate {
    Arc::new(|headers| {
        let caller = match headers.get("x-test-caller").and_then(|v| v.to_str().ok()) {
            None => Ok(Caller::Anonymous),
            Some("alice") => Ok(Caller::Account {
                did: "did:plc:alice".into(),
            }),
            Some(lxm) => Ok(Caller::Service {
                did: "did:plc:alice".into(),
                lxm: lxm.into(),
                audience: "did:web:motet.test".into(),
            }),
        };
        Box::pin(async move { caller })
    })
}

fn router(calls: Arc<AtomicUsize>) -> axum::Router {
    let echo = |calls: Arc<AtomicUsize>, caller: Caller, input: Input| async move {
        calls.fetch_add(1, Ordering::SeqCst);
        Ok(format!("{:?} {} {:?}", caller.did(), input.body.len(), input.params).into_response())
    };
    Service::new(authenticate())
        .method(
            Method {
                nsid: "test.getThing",
                kind: Kind::Query,
                access: Access::Public,
                max_body: 0,
            },
            echo,
        )
        .method(
            Method {
                nsid: "test.putThing",
                kind: Kind::Procedure,
                access: Access::Account,
                max_body: 8,
            },
            echo,
        )
        .method(
            Method {
                nsid: "test.serviceThing",
                kind: Kind::Procedure,
                access: Access::Service,
                max_body: 8,
            },
            echo,
        )
        .into_router(calls)
        .expect("no duplicate methods")
}

async fn call(verb: &str, uri: &str, caller: Option<&str>, body: &'static str) -> (StatusCode, usize) {
    let calls = Arc::new(AtomicUsize::new(0));
    let mut request = Request::builder().method(verb).uri(uri);
    if let Some(caller) = caller {
        request = request.header("x-test-caller", caller);
    }
    let response = router(calls.clone())
        .oneshot(request.body(Body::from(body)).expect("request"))
        .await
        .expect("infallible");
    (response.status(), calls.load(Ordering::SeqCst))
}

#[tokio::test]
async fn declared_methods_reach_their_handler() {
    assert_eq!(
        call("GET", "/xrpc/test.getThing?a=1&a=2", None, "").await,
        (StatusCode::OK, 1)
    );
    assert_eq!(
        call("POST", "/xrpc/test.putThing", Some("alice"), "1234").await,
        (StatusCode::OK, 1)
    );
    assert_eq!(
        call("POST", "/xrpc/test.serviceThing", Some("test.serviceThing"), "").await,
        (StatusCode::OK, 1)
    );
}

#[tokio::test]
async fn undeclared_methods_and_wrong_verbs_never_reach_a_handler() {
    assert_eq!(
        call("GET", "/xrpc/test.nothing", Some("alice"), "").await,
        (StatusCode::NOT_IMPLEMENTED, 0)
    );
    assert_eq!(
        call("POST", "/xrpc/test.getThing", None, "").await,
        (StatusCode::BAD_REQUEST, 0)
    );
    assert_eq!(
        call("DELETE", "/xrpc/test.putThing", Some("alice"), "").await,
        (StatusCode::BAD_REQUEST, 0)
    );
}

#[tokio::test]
async fn missing_or_misdirected_credentials_never_reach_a_handler() {
    assert_eq!(
        call("POST", "/xrpc/test.putThing", None, "").await,
        (StatusCode::UNAUTHORIZED, 0)
    );
    assert_eq!(
        call("POST", "/xrpc/test.serviceThing", Some("alice"), "").await,
        (StatusCode::UNAUTHORIZED, 0)
    );
    // A token minted for another method opens nothing else.
    assert_eq!(
        call("POST", "/xrpc/test.serviceThing", Some("test.putThing"), "").await,
        (StatusCode::UNAUTHORIZED, 0)
    );
}

#[tokio::test]
async fn bodies_over_the_method_limit_are_refused() {
    assert_eq!(
        call("POST", "/xrpc/test.putThing", Some("alice"), "123456789").await,
        (StatusCode::PAYLOAD_TOO_LARGE, 0)
    );
}
