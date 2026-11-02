//! Browser session boundaries use real XRPC dispatch and PostgreSQL state.

mod support;

use std::sync::Arc;

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode, header};
use chrono::{Duration, Utc};
use motet_appview::MIGRATOR;
use motet_appview::api;
use motet_appview::auth::pds::Grant;
use motet_appview::auth::store::SessionStore;
use motet_net::identity::DidResolver;
use motet_net::{Fetcher, Policy};
use motet_xrpc::service_auth;
use serde_json::{Value, json};
use tower::ServiceExt as _;
use url::Url;

const ORIGIN: &str = support::ORIGIN;
const SESSION_COOKIE: &str = "__Host-motet_session";

async fn setup() -> (SessionStore, axum::Router) {
    let pool = motet_testkit::services::database().await;
    MIGRATOR.run(&pool).await.expect("migrations");
    let store = SessionStore::new(pool.clone(), [2; 32]);
    let oauth = support::oauth(pool.clone());
    let fetcher = Fetcher::new(Policy::default()).expect("fetcher");
    let resolver = Arc::new(DidResolver::new(fetcher, "https://plc.directory"));
    let service_auth = service_auth::authenticate(resolver, Arc::new(|_| true));
    let api = api::router(
        Arc::new(api::State {
            pool,
            auth: Arc::clone(&oauth),
        }),
        oauth.authenticate(service_auth),
    )
    .expect("API");
    (store, api.merge(oauth.router()))
}

fn grant() -> Grant {
    Grant {
        did: "did:plc:alice".into(),
        handle: "alice.example".into(),
        pds: "https://pds.example".into(),
        issuer: "https://pds.example".into(),
        token_endpoint: "https://pds.example/oauth/token".into(),
        revocation_endpoint: None,
        access_token: "private-access-token".into(),
        refresh_token: "private-refresh-token".into(),
        expires_at: Utc::now().timestamp() + 600,
        scope: "atproto".into(),
        dpop_private_key: vec![3; 32],
    }
}

fn request(uri: &str, cookie: Option<&str>) -> Request<Body> {
    let mut request = Request::builder().uri(uri);
    if let Some(cookie) = cookie {
        request = request.header(header::COOKIE, cookie);
    }
    request.body(Body::empty()).expect("request")
}

fn query(path: &str, values: &[(&str, &str)]) -> String {
    let mut url = Url::parse(&format!("{ORIGIN}{path}")).expect("URL");
    url.query_pairs_mut().extend_pairs(values.iter().copied());
    format!("{}?{}", url.path(), url.query().expect("query"))
}

/// An attacker must not turn the post-login redirect into a foreign origin,
/// including with authority syntax or encoded path separators.
#[tokio::test]
#[ignore = "integration: PostgreSQL"]
async fn login_rejects_foreign_or_ambiguous_return_paths_before_contacting_pds() {
    let (_store, router) = setup().await;
    for path in [
        "https://attacker.example/",
        "//attacker.example/",
        "/\\attacker.example/",
        "/%2fattacker.example/",
        "/%5cattacker.example/",
        "/%0d%0aLocation: https://attacker.example/",
    ] {
        let uri = query("/oauth/sign-up", &[("returnTo", path)]);
        let response = router.clone().oneshot(request(&uri, None)).await.expect("response");
        assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{path}");
        assert!(response.headers().get(header::LOCATION).is_none());
    }
}

/// The published confidential-client key must verify the configured signer
/// without disclosing its private scalar, or the PDS cannot authenticate it.
#[tokio::test]
#[ignore = "integration: PostgreSQL"]
async fn metadata_publishes_the_matching_public_key_without_the_private_scalar() {
    use atproto_identity::key::{KeyData, KeyType};
    use atproto_oauth::jwk::{WrappedJsonWebKey, to_key_data};
    use atproto_oauth::jwt::{Claims, Header, JoseClaims, mint, verify};

    let (_store, router) = setup().await;
    let response = router
        .clone()
        .oneshot(request("/oauth/client-metadata.json", None))
        .await
        .expect("metadata");
    assert_eq!(response.status(), StatusCode::OK);
    let metadata: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 16_384).await.expect("body")).expect("metadata JSON");
    assert_eq!(metadata["client_id"], format!("{ORIGIN}/oauth/client-metadata.json"));
    assert_eq!(metadata["token_endpoint_auth_method"], "private_key_jwt");
    assert_eq!(metadata["jwks_uri"], format!("{ORIGIN}/oauth/jwks.json"));

    let response = router.oneshot(request("/oauth/jwks.json", None)).await.expect("JWKS");
    assert_eq!(response.status(), StatusCode::OK);
    let jwks: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 16_384).await.expect("body")).expect("JWKS JSON");
    let keys = jwks["keys"].as_array().expect("JWKS keys");
    assert_eq!(keys.len(), 1);
    let key = keys.first().expect("one key");
    assert!(
        key.get("d").is_none(),
        "a public JWKS must not contain the private scalar"
    );
    let public = to_key_data(&serde_json::from_value::<WrappedJsonWebKey>(key.clone()).expect("public JWK"))
        .expect("public key");
    let private = KeyData::new(KeyType::P256Private, vec![1; 32]);
    let header: Header = private.clone().try_into().expect("JWT header");
    let claims = Claims::new(JoseClaims {
        issuer: Some(format!("{ORIGIN}/oauth/client-metadata.json")),
        ..Default::default()
    });
    let assertion = mint(&private, &header, &claims).expect("client assertion");
    assert!(
        verify(&assertion, &public).is_ok(),
        "the advertised key must verify the signer"
    );
}

/// A code from another OAuth server or a different flow cannot complete the
/// browser-bound login; otherwise an issuer mix-up could sign in an attacker.
#[tokio::test]
#[ignore = "integration: PostgreSQL"]
async fn callback_refuses_mismatched_state_and_issuer_and_consumes_the_attempt() {
    let (store, router) = setup().await;
    let state = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQ";
    let other_state = "bcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQR";
    let binding = "unpredictable-browser-binding-for-test";
    let pending = json!({
        "pending": {
            "expected_did": "did:plc:alice",
            "handle": "alice.example",
            "pds": "https://pds.example",
            "issuer": "https://issuer.example",
            "authorization_endpoint": "https://issuer.example/oauth/authorize",
            "token_endpoint": "https://issuer.example/oauth/token",
            "pushed_authorization_request_endpoint": "https://issuer.example/oauth/par",
            "revocation_endpoint": null,
            "state": state,
            "pkce_verifier": "private-verifier",
            "dpop_private_key": [3, 4, 5],
            "created_at": Utc::now().timestamp()
        },
        "return_to": "/worlds"
    });
    store
        .store_pending(state, binding, &pending, Utc::now() + Duration::minutes(10))
        .await
        .expect("pending login");
    let cookie = format!("__Host-motet_login_{state}={binding}");
    let wrong_state = query(
        "/oauth/callback",
        &[
            ("state", other_state),
            ("code", "code"),
            ("iss", "https://issuer.example"),
        ],
    );
    let response = router
        .clone()
        .oneshot(request(&wrong_state, Some(&cookie)))
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let wrong_issuer = query(
        "/oauth/callback",
        &[("state", state), ("code", "code"), ("iss", "https://attacker.example")],
    );
    let response = router
        .clone()
        .oneshot(request(&wrong_issuer, Some(&cookie)))
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert!(response.headers().get(header::SET_COOKIE).is_none());
    let replay = query(
        "/oauth/callback",
        &[("state", state), ("code", "code"), ("iss", "https://issuer.example")],
    );
    let response = router.oneshot(request(&replay, Some(&cookie))).await.expect("response");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

/// A cookie must cease authorizing XRPC calls at expiry or logout, even if a
/// browser continues to send its former value.
#[tokio::test]
#[ignore = "integration: PostgreSQL"]
async fn expired_and_signed_out_sessions_cannot_use_session_methods() {
    let (store, router) = setup().await;
    let expired = store
        .create_session(&grant(), Utc::now() - Duration::seconds(1))
        .await
        .expect("expired session");
    let expired = format!("{SESSION_COOKIE}={expired}");
    let endpoint = "/xrpc/at.motet.pilot.auth.getSession";
    let response = router
        .clone()
        .oneshot(request(endpoint, Some(&expired)))
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    let live = store
        .create_session(&grant(), Utc::now() + Duration::hours(1))
        .await
        .expect("live session");
    let cookie = format!("{SESSION_COOKIE}={live}");
    let response = router
        .clone()
        .oneshot(request(endpoint, Some(&cookie)))
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 4096).await.expect("body")).expect("session JSON");
    assert_eq!(body, json!({"did": "did:plc:alice", "handle": "alice.example"}));

    let cross_origin = Request::builder()
        .method("POST")
        .uri("/oauth/sign-out")
        .header(header::ORIGIN, "https://attacker.example")
        .header(header::COOKIE, &cookie)
        .body(Body::empty())
        .expect("request");
    let response = router.clone().oneshot(cross_origin).await.expect("response");
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let sign_out = Request::builder()
        .method("POST")
        .uri("/oauth/sign-out")
        .header(header::ORIGIN, ORIGIN)
        .header(header::COOKIE, &cookie)
        .body(Body::empty())
        .expect("request");
    let response = router.clone().oneshot(sign_out).await.expect("response");
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    let cleared = response
        .headers()
        .get(header::SET_COOKIE)
        .expect("cleared cookie")
        .to_str()
        .expect("header");
    assert!(cleared.contains("Max-Age=0") && cleared.contains("HttpOnly") && cleared.contains("Secure"));
    let response = router
        .oneshot(request(endpoint, Some(&cookie)))
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

/// A legacy World `DPoP` token is not an AppView cookie session, even when a
/// caller presents it directly to a session-only method.
#[tokio::test]
#[ignore = "integration: PostgreSQL"]
async fn legacy_dpop_credentials_do_not_authenticate_as_browser_sessions() {
    let (_store, router) = setup().await;
    let request = Request::builder()
        .uri("/xrpc/at.motet.pilot.auth.getSession")
        .header(header::AUTHORIZATION, "DPoP world-session-token")
        .body(Body::empty())
        .expect("request");
    let response = router.oneshot(request).await.expect("response");
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

/// Same-site sibling origins cannot use a browser's cookie to submit an
/// unsafe XRPC procedure; otherwise a cross-origin page could write as it.
#[tokio::test]
#[ignore = "integration: PostgreSQL"]
async fn cookie_authenticated_procedures_require_the_exact_appview_origin() {
    use axum::response::IntoResponse as _;
    use motet_xrpc::server::{Authenticate, Service};
    use motet_xrpc::{Access, Caller, Kind, Method};

    let pool = motet_testkit::services::database().await;
    MIGRATOR.run(&pool).await.expect("migrations");
    let store = SessionStore::new(pool.clone(), [2; 32]);
    let oauth = support::oauth(pool);
    let cookie = store
        .create_session(&grant(), Utc::now() + Duration::hours(1))
        .await
        .expect("session");
    let anonymous: Authenticate = Arc::new(|_| Box::pin(async { Ok(Caller::Anonymous) }));
    let router = Service::new(oauth.authenticate(anonymous))
        .method(
            Method {
                nsid: "test.browser.write",
                kind: Kind::Procedure,
                access: Access::AppviewSession,
                max_body: 0,
            },
            |(), _caller, _input| async { Ok(StatusCode::NO_CONTENT.into_response()) },
        )
        .into_router(())
        .expect("router");
    let cookie = format!("{SESSION_COOKIE}={cookie}");
    for (origin, expected) in [
        ("https://sibling.motet.example", StatusCode::UNAUTHORIZED),
        (ORIGIN, StatusCode::NO_CONTENT),
    ] {
        let request = Request::builder()
            .method("POST")
            .uri("/xrpc/test.browser.write")
            .header(header::ORIGIN, origin)
            .header(header::COOKIE, &cookie)
            .body(Body::empty())
            .expect("request");
        let response = router.clone().oneshot(request).await.expect("response");
        assert_eq!(response.status(), expected);
    }
}
