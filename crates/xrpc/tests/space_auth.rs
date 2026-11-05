//! Space credentials authorize private Space methods, never account actions.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::response::IntoResponse;
use motet_xrpc::server::{Authenticate, Input, Service};
use motet_xrpc::{Access, Caller, Kind, Method, XrpcError};
use tower::ServiceExt;

fn authentication() -> Authenticate {
    Arc::new(|request| {
        // Cryptography belongs to the host verifier. This boundary test checks
        // that its outcomes cannot acquire a different credential's privilege.
        let result = match request
            .headers
            .get("x-verifier-result")
            .and_then(|value| value.to_str().ok())
        {
            None => Ok(Caller::Anonymous),
            Some("invalid") => Err(XrpcError::AuthRequired),
            Some("space") => Ok(Caller::SpaceCredential {
                space: "at://did:plc:world/space/group.opensocial.meta/self".into(),
                audience: "did:plc:world".into(),
            }),
            Some("account") => Ok(Caller::Account {
                did: "did:plc:world".into(),
            }),
            Some("appview") => Ok(Caller::AppviewSession {
                did: "did:plc:alice".into(),
                handle: "alice.test".into(),
            }),
            Some(method) => Ok(Caller::Service {
                did: "did:plc:world".into(),
                lxm: method.into(),
                audience: "did:plc:world#atproto_pds".into(),
                jti: None,
                expires_at: u64::MAX,
            }),
        };
        Box::pin(async move { result })
    })
}

fn router(calls: Arc<AtomicUsize>) -> axum::Router {
    let mut service = Service::new(authentication());
    for (nsid, access) in [
        ("test.readSpace", Access::SpaceCredential),
        ("test.writeAccount", Access::Account),
        ("test.notify", Access::Service),
        ("test.writeSpace", Access::Service),
    ] {
        service = service.method(
            Method {
                nsid,
                kind: Kind::Procedure,
                access,
                max_body: 8,
            },
            |calls: Arc<AtomicUsize>, _: Caller, input: Input| async move {
                calls.fetch_add(1, Ordering::SeqCst);
                Ok(input.body.into_response())
            },
        );
    }
    service.into_router(calls).expect("unique methods")
}

/// Without a dedicated rule, a private read declared Public would admit
/// anonymous requests, and treating a credential as an account would open writes.
#[tokio::test]
async fn credential_kinds_cannot_substitute_for_each_other() {
    let calls = Arc::new(AtomicUsize::new(0));
    let router = router(Arc::clone(&calls));
    for (method, proof, expected) in [
        ("test.readSpace", None, StatusCode::UNAUTHORIZED),
        ("test.readSpace", Some("invalid"), StatusCode::UNAUTHORIZED),
        ("test.readSpace", Some("account"), StatusCode::UNAUTHORIZED),
        ("test.readSpace", Some("test.readSpace"), StatusCode::UNAUTHORIZED),
        ("test.writeAccount", Some("space"), StatusCode::UNAUTHORIZED),
        ("test.notify", Some("space"), StatusCode::UNAUTHORIZED),
        ("test.writeAccount", Some("appview"), StatusCode::UNAUTHORIZED),
        ("test.notify", Some("appview"), StatusCode::UNAUTHORIZED),
        ("test.readSpace", Some("appview"), StatusCode::UNAUTHORIZED),
        ("test.writeSpace", Some("appview"), StatusCode::UNAUTHORIZED),
        ("test.writeSpace", Some("account"), StatusCode::UNAUTHORIZED),
        ("test.writeSpace", Some("space"), StatusCode::UNAUTHORIZED),
        ("test.writeSpace", Some("test.notify"), StatusCode::UNAUTHORIZED),
        ("test.writeAccount", Some("account"), StatusCode::OK),
        ("test.notify", Some("test.notify"), StatusCode::OK),
        ("test.writeSpace", Some("test.writeSpace"), StatusCode::OK),
        ("test.readSpace", Some("space"), StatusCode::OK),
    ] {
        let mut request = Request::builder().method("POST").uri(format!("/xrpc/{method}"));
        if let Some(proof) = proof {
            request = request.header("x-verifier-result", proof);
        }
        let response = router
            .clone()
            .oneshot(request.body(Body::from("private")).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), expected);
    }
    assert_eq!(calls.load(Ordering::SeqCst), 4);
}

/// A failed proof must fail before body polling; authenticating in a handler
/// instead would let an unauthenticated sender consume the body budget.
#[tokio::test]
async fn invalid_space_proofs_never_poll_request_bodies() {
    let calls = Arc::new(AtomicUsize::new(0));
    let polls = Arc::new(AtomicUsize::new(0));
    let body_polls = Arc::clone(&polls);
    let body = Body::from_stream(futures::stream::poll_fn(move |_| {
        body_polls.fetch_add(1, Ordering::SeqCst);
        std::task::Poll::Ready(Some(Ok::<_, std::io::Error>("untrusted")))
    }));
    let response = router(Arc::clone(&calls))
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/xrpc/test.readSpace")
                .header("x-verifier-result", "invalid")
                .body(body)
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert_eq!(polls.load(Ordering::SeqCst), 0);
}
