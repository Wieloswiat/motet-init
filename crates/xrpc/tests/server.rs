//! The dispatch order, through a real router: nothing reaches a handler, or
//! has its body read, unless the registry let it through.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::response::IntoResponse;
use motet_xrpc::server::{Authenticate, Input, Service};
use motet_xrpc::{Access, Caller, Kind, Method, XrpcError};
use tower::ServiceExt;

/// Callers by header, standing in for real verification.
fn authenticate() -> Authenticate {
    Arc::new(|request| {
        let caller = match request.headers.get("x-test-caller").and_then(|v| v.to_str().ok()) {
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
    let response = router(Arc::clone(&calls))
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

/// Reject the route and verb before credential work or body polling.
/// Moving authentication ahead of resolution would make invalid routes issue
/// nonce challenges and perform signature verification.
#[tokio::test]
async fn undeclared_methods_and_wrong_verbs_never_authenticate_or_read_bodies() {
    let authentications = Arc::new(AtomicUsize::new(0));
    let auth_calls = Arc::clone(&authentications);
    let authenticate: Authenticate = Arc::new(move |_| {
        auth_calls.fetch_add(1, Ordering::SeqCst);
        Box::pin(async { Err(XrpcError::AuthRequired) })
    });
    let calls = Arc::new(AtomicUsize::new(0));
    let router = Service::new(authenticate)
        .method(
            Method {
                nsid: "test.putThing",
                kind: Kind::Procedure,
                access: Access::Session,
                max_body: 8,
            },
            |calls: Arc<AtomicUsize>, _: Caller, _: Input| async move {
                calls.fetch_add(1, Ordering::SeqCst);
                Ok("invoked".into_response())
            },
        )
        .into_router(Arc::clone(&calls))
        .unwrap();
    for (verb, uri, status) in [
        ("GET", "/xrpc/test.nothing", StatusCode::NOT_IMPLEMENTED),
        ("POST", "/xrpc/test.nothing", StatusCode::NOT_IMPLEMENTED),
        ("GET", "/xrpc/test.putThing", StatusCode::BAD_REQUEST),
        ("DELETE", "/xrpc/test.putThing", StatusCode::BAD_REQUEST),
        ("HEAD", "/xrpc/test.putThing", StatusCode::BAD_REQUEST),
        ("OPTIONS", "/xrpc/test.putThing", StatusCode::BAD_REQUEST),
    ] {
        let polls = Arc::new(AtomicUsize::new(0));
        let body_polls = Arc::clone(&polls);
        let body = Body::from_stream(futures::stream::poll_fn(move |_| {
            body_polls.fetch_add(1, Ordering::SeqCst);
            std::task::Poll::Ready(Some(Ok::<_, std::io::Error>(bytes_for_test())))
        }));
        let response = router
            .clone()
            .oneshot(Request::builder().method(verb).uri(uri).body(body).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), status);
        assert_eq!(authentications.load(Ordering::SeqCst), 0);
        assert_eq!(polls.load(Ordering::SeqCst), 0);
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }
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

/// An upgrade must not bypass the registry or poll a refused request's body.
/// Extracting the socket first would turn these into handshake failures.
#[tokio::test]
async fn refused_upgrades_never_invoke_handlers_or_read_bodies() {
    let calls = Arc::new(AtomicUsize::new(0));
    let echo = |calls: Arc<AtomicUsize>, _: Caller, _: Input| async move {
        calls.fetch_add(1, Ordering::SeqCst);
        Ok("invoked".into_response())
    };
    let router = Service::new(authenticate())
        .method(
            Method {
                nsid: "test.subscribe",
                kind: Kind::Query,
                access: Access::Service,
                max_body: 0,
            },
            echo,
        )
        .into_router(Arc::clone(&calls))
        .expect("unique method");
    for (nsid, caller, status) in [
        ("test.undeclared", None, StatusCode::NOT_IMPLEMENTED),
        ("test.subscribe", None, StatusCode::UNAUTHORIZED),
        ("test.subscribe", Some("test.other"), StatusCode::UNAUTHORIZED),
    ] {
        let polls = Arc::new(AtomicUsize::new(0));
        let body_polls = Arc::clone(&polls);
        let body = Body::from_stream(futures::stream::poll_fn(move |_| {
            body_polls.fetch_add(1, Ordering::SeqCst);
            std::task::Poll::Ready(Some(Ok::<_, std::io::Error>(bytes_for_test())))
        }));
        let mut request = Request::builder()
            .uri(format!("/xrpc/{nsid}"))
            .header("connection", "upgrade")
            .header("upgrade", "websocket")
            .header("sec-websocket-version", "13")
            .header("sec-websocket-key", "dGhlIHNhbXBsZSBub25jZQ==");
        if let Some(caller) = caller {
            request = request.header("x-test-caller", caller);
        }
        let response = router
            .clone()
            .oneshot(request.body(body).expect("request"))
            .await
            .expect("router");
        assert_eq!(response.status(), status);
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert_eq!(polls.load(Ordering::SeqCst), 0);
    }
}

fn bytes_for_test() -> axum::body::Bytes {
    axum::body::Bytes::from_static(b"must not be read")
}

fn world_session() -> Caller {
    Caller::Session {
        world: "did:plc:world".into(),
        actor: "did:plc:alice".into(),
        client_id: "https://client.test/metadata.json".into(),
        scopes: vec!["atproto".into()],
    }
}

/// The verifier must see the actual verb and unmodified URI, not headers
/// claiming them. Losing that context would permit proof reuse on another
/// method or resource. This exercises transport binding, not `DPoP` cryptography.
#[tokio::test]
async fn authentication_binds_the_session_to_the_http_context() {
    let authenticate: Authenticate = Arc::new(|request| {
        Box::pin(async move {
            let bound_method = request.headers.get("x-proof-method").unwrap().to_str().unwrap();
            let bound_uri = request.headers.get("x-proof-uri").unwrap().to_str().unwrap();
            if request.method.as_str() != bound_method || request.uri != bound_uri {
                return Err(XrpcError::AuthRequired);
            }
            Ok(world_session())
        })
    });
    let calls = Arc::new(AtomicUsize::new(0));
    let mut service = Service::new(authenticate);
    for (nsid, kind) in [
        ("test.readSession", Kind::Query),
        ("test.otherSession", Kind::Query),
        ("test.writeSession", Kind::Procedure),
    ] {
        service = service.method(
            Method {
                nsid,
                kind,
                access: Access::Session,
                max_body: 0,
            },
            |calls: Arc<AtomicUsize>, caller: Caller, _: Input| async move {
                calls.fetch_add(1, Ordering::SeqCst);
                // Session authority is the World, never its human actor.
                assert_eq!(caller.did(), Some("did:plc:world"));
                Ok("invoked".into_response())
            },
        );
    }
    let router = service.into_router(Arc::clone(&calls)).unwrap();
    for (verb, uri, bound_method, bound_uri, status) in [
        (
            "GET",
            "/xrpc/test.readSession?item=a%2Fb&item=c",
            "GET",
            "/xrpc/test.readSession?item=a%2Fb&item=c",
            StatusCode::OK,
        ),
        (
            "GET",
            "/xrpc/test.otherSession",
            "GET",
            "/xrpc/test.readSession",
            StatusCode::UNAUTHORIZED,
        ),
        (
            "POST",
            "/xrpc/test.writeSession",
            "GET",
            "/xrpc/test.writeSession",
            StatusCode::UNAUTHORIZED,
        ),
    ] {
        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .method(verb)
                    .uri(uri)
                    .header("x-proof-method", bound_method)
                    .header("x-proof-uri", bound_uri)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), status);
    }
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

/// A cross-origin client must read the verifier's challenge and nonce before
/// retrying. Missing CORS exposure would hide the nonce from browser clients.
#[tokio::test]
async fn nonce_challenges_allow_a_fresh_proof_without_reading_the_refused_body() {
    let authenticate: Authenticate = Arc::new(|request| {
        Box::pin(async move {
            if request
                .headers
                .get("x-proof-nonce")
                .and_then(|value| value.to_str().ok())
                != Some("fresh-nonce")
            {
                return Err(XrpcError::DpopNonceRequired {
                    nonce: "fresh-nonce".into(),
                });
            }
            Ok(world_session())
        })
    });
    let calls = Arc::new(AtomicUsize::new(0));
    let router = Service::new(authenticate)
        .method(
            Method {
                nsid: "test.writeSession",
                kind: Kind::Procedure,
                access: Access::Session,
                max_body: 8,
            },
            |calls: Arc<AtomicUsize>, _: Caller, _: Input| async move {
                calls.fetch_add(1, Ordering::SeqCst);
                Ok("invoked".into_response())
            },
        )
        .into_router(Arc::clone(&calls))
        .unwrap();
    let polls = Arc::new(AtomicUsize::new(0));
    let body_polls = Arc::clone(&polls);
    let body = Body::from_stream(futures::stream::poll_fn(move |_| {
        body_polls.fetch_add(1, Ordering::SeqCst);
        std::task::Poll::Ready(Some(Ok::<_, std::io::Error>(bytes_for_test())))
    }));
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/xrpc/test.writeSession")
                .header("origin", "https://independent.test")
                .body(body)
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(response.headers()["www-authenticate"], "DPoP error=\"use_dpop_nonce\"");
    assert_public_cors(&response);
    let nonce = response.headers()["dpop-nonce"].clone();
    let body = axum::body::to_bytes(response.into_body(), 1024).await.unwrap();
    let error: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(error["error"], "use_dpop_nonce");
    assert_eq!(polls.load(Ordering::SeqCst), 0);
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    let response = router
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/xrpc/test.writeSession")
                .header("origin", "https://independent.test")
                .header("x-proof-nonce", nonce)
                .body(Body::from("write"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_public_cors(&response);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

fn assert_public_cors(response: &axum::response::Response) {
    assert_eq!(response.headers()["access-control-allow-origin"], "*");
    assert!(!response.headers().contains_key("access-control-allow-credentials"));
    for required in ["dpop-nonce", "www-authenticate"] {
        assert!(
            response
                .headers()
                .get_all("access-control-expose-headers")
                .iter()
                .any(|value| value
                    .to_str()
                    .expect("CORS exposure header is text")
                    .split(',')
                    .any(|name| name.trim().eq_ignore_ascii_case(required)))
        );
    }
}

fn preflight_router(authentications: &Arc<AtomicUsize>, calls: &Arc<AtomicUsize>) -> axum::Router {
    let auth_calls = Arc::clone(authentications);
    let authenticate: Authenticate = Arc::new(move |_| {
        auth_calls.fetch_add(1, Ordering::SeqCst);
        Box::pin(async { Err(XrpcError::AuthRequired) })
    });
    let mut service = Service::new(authenticate);
    for (nsid, kind) in [
        ("test.readSession", Kind::Query),
        ("test.writeSession", Kind::Procedure),
    ] {
        service = service.method(
            Method {
                nsid,
                kind,
                access: Access::Session,
                max_body: 0,
            },
            |calls: Arc<AtomicUsize>, _: Caller, _: Input| async move {
                calls.fetch_add(1, Ordering::SeqCst);
                Ok("invoked".into_response())
            },
        );
    }
    service
        .into_router(Arc::clone(calls))
        .expect("unique preflight methods")
}

/// Browsers may preflight only declared verbs and allowed credential headers.
/// A blanket OPTIONS route would approve undeclared methods; authenticating
/// preflights would reject legitimate browsers before they can send a token.
#[tokio::test]
async fn browser_preflights_validate_the_declaration_without_authentication_or_body_reads() {
    let authentications = Arc::new(AtomicUsize::new(0));
    let calls = Arc::new(AtomicUsize::new(0));
    let router = preflight_router(&authentications, &calls);
    for (nsid, method, requested_headers, status) in [
        (
            "test.writeSession",
            Some("POST"),
            Some("Authorization, DPoP, Content-Type"),
            StatusCode::NO_CONTENT,
        ),
        ("test.readSession", Some("GET"), None, StatusCode::NO_CONTENT),
        (
            "test.readSession",
            Some("GET"),
            Some("authorization,signature,signature-input,atproto-space-audience,atproto-space-want-mac"),
            StatusCode::NO_CONTENT,
        ),
        (
            "test.undeclared",
            Some("POST"),
            Some("dpop"),
            StatusCode::NOT_IMPLEMENTED,
        ),
        ("test.readSession", Some("POST"), Some("dpop"), StatusCode::BAD_REQUEST),
        ("test.writeSession", Some("GET"), Some("dpop"), StatusCode::BAD_REQUEST),
        ("test.writeSession", Some("PUT"), Some("dpop"), StatusCode::BAD_REQUEST),
        (
            "test.writeSession",
            Some("POST, POST"),
            Some("dpop"),
            StatusCode::BAD_REQUEST,
        ),
        ("test.writeSession", None, Some("dpop"), StatusCode::BAD_REQUEST),
        (
            "test.writeSession",
            Some("POST"),
            Some("cookie"),
            StatusCode::BAD_REQUEST,
        ),
        (
            "test.writeSession",
            Some("POST"),
            Some("dpop, forwarded"),
            StatusCode::BAD_REQUEST,
        ),
        ("test.writeSession", Some("POST"), Some(""), StatusCode::BAD_REQUEST),
    ] {
        let polls = Arc::new(AtomicUsize::new(0));
        let body_polls = Arc::clone(&polls);
        let body = Body::from_stream(futures::stream::poll_fn(move |_| {
            body_polls.fetch_add(1, Ordering::SeqCst);
            std::task::Poll::Ready(Some(Ok::<_, std::io::Error>(bytes_for_test())))
        }));
        let mut request = Request::builder()
            .method("OPTIONS")
            .uri(format!("/xrpc/{nsid}"))
            .header("origin", "https://independent.test");
        if let Some(method) = method {
            request = request.header("access-control-request-method", method);
        }
        if let Some(headers) = requested_headers {
            request = request.header("access-control-request-headers", headers);
        }
        let response = router.clone().oneshot(request.body(body).unwrap()).await.unwrap();
        assert_eq!(response.status(), status, "{nsid} {method:?} {requested_headers:?}");
        assert_public_cors(&response);
        if status.is_success() {
            assert_eq!(response.headers()["access-control-allow-methods"], method.unwrap());
            let allowed = response.headers()["access-control-allow-headers"].to_str().unwrap();
            if let Some(requested) = requested_headers {
                for name in requested.split(',') {
                    assert!(
                        allowed
                            .split(',')
                            .any(|allowed| allowed.eq_ignore_ascii_case(name.trim()))
                    );
                }
            }
        } else {
            assert!(!response.headers().contains_key("access-control-allow-methods"));
            assert!(!response.headers().contains_key("access-control-allow-headers"));
        }
        assert_eq!(authentications.load(Ordering::SeqCst), 0);
        assert_eq!(polls.load(Ordering::SeqCst), 0);
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }
}
