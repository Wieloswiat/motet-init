//! Tenor destinations and failures at the actual outbound HTTP boundary.

use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use atproto_identity::model::Document;
use atproto_identity::traits::IdentityResolver;
use axum::Json;
use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::IntoResponse as _;
use axum::routing::{get, post};
use motet_appview::writes::client::{Client, ClientError};
use motet_net::identity::DidResolver;
use motet_net::{Fetcher, Policy};
use serde_json::{Value, json};
use tokio::task::JoinHandle;

const WORLD: &str = "did:plc:ewvi7nxzyoun6zhxrhs64oiz";
const METHOD: &str = "at.motet.pilot.play.openScene";

struct Directory(Document);

// IdentityResolver uses async-trait; this is its boxed future signature.
impl IdentityResolver for Directory {
    fn resolve<'life0, 'life1, 'async_trait>(
        &'life0 self,
        _subject: &'life1 str,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<Document>> + Send + 'async_trait>>
    where
        'life0: 'async_trait,
        'life1: 'async_trait,
        Self: 'async_trait,
    {
        Box::pin(async move { Ok(self.0.clone()) })
    }
}

fn document(subject: &str, id: &str, kind: &str, endpoint: &str) -> Document {
    serde_json::from_value(json!({
        "id": subject,
        "service": [{"id": id, "type": kind, "serviceEndpoint": endpoint}]
    }))
    .expect("fixture is a DID document")
}

fn client(policy: Policy, document: Document) -> Client {
    Client::new(
        Fetcher::new(policy).expect("network client"),
        Arc::new(Directory(document)),
        "https://tenor.example",
        "did:web:tenor.example",
    )
    .expect("valid configured tenor")
}

async fn serve(router: axum::Router) -> (String, JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("fixture listener");
    let address = listener.local_addr().expect("listener address");
    let server = tokio::spawn(async move {
        axum::serve(listener, router).await.expect("fixture server");
    });
    (format!("http://{address}"), server)
}

/// A DID controller must not make the AppView send a service token to an
/// internal host, a different DID's service, or an ambiguous service entry.
#[tokio::test]
async fn world_discovery_refuses_internal_foreign_and_ambiguous_services() {
    let private = client(
        Policy::default(),
        document(WORLD, "#motet_tenor", "MotetTenor", "https://127.0.0.1"),
    );
    assert!(matches!(private.world_target(WORLD).await, Err(ClientError::Upstream)));

    for doc in [
        document(
            "did:plc:abcdefghijklmnopqrstuvwx",
            "#motet_tenor",
            "MotetTenor",
            "https://tenor.example",
        ),
        document(
            WORLD,
            "did:plc:abcdefghijklmnopqrstuvwx#motet_tenor",
            "MotetTenor",
            "https://tenor.example",
        ),
        document(WORLD, "#motet_tenor", "OtherService", "https://tenor.example"),
        serde_json::from_value(json!({
            "id": WORLD,
            "service": [
                {"id": "#motet_tenor", "type": "MotetTenor", "serviceEndpoint": "https://one.example"},
                {"id": format!("{WORLD}#motet_tenor"), "type": "MotetTenor", "serviceEndpoint": "https://two.example"}
            ]
        }))
        .expect("ambiguous document"),
    ] {
        let target = client(Policy::default(), doc);
        assert!(matches!(target.world_target(WORLD).await, Err(ClientError::NotFound)));
    }
}

/// A cast DID is not verified if the identity service returns another
/// subject; otherwise that response could admit an unintended account.
#[tokio::test]
async fn cast_did_must_match_its_resolved_document() {
    let correct = client(
        Policy::default(),
        document(WORLD, "#motet_tenor", "MotetTenor", "https://tenor.example"),
    );
    assert_eq!(correct.member_did(WORLD).await.expect("verified identity"), WORLD);
    let other = client(
        Policy::default(),
        document(
            "did:plc:abcdefghijklmnopqrstuvwx",
            "#motet_tenor",
            "MotetTenor",
            "https://tenor.example",
        ),
    );
    assert!(matches!(other.member_did(WORLD).await, Err(ClientError::NotFound)));
}

async fn directory_status(State(status): State<StatusCode>) -> StatusCode {
    status
}

/// A directory outage must not permanently discard a pending admission as
/// an unknown World, while an actual missing DID is not retried forever.
#[tokio::test]
async fn directory_outage_is_distinct_from_a_missing_identity() {
    for status in [StatusCode::NOT_FOUND, StatusCode::SERVICE_UNAVAILABLE] {
        let (directory, server) = serve(
            axum::Router::new()
                .route("/{did}", get(directory_status))
                .with_state(status),
        )
        .await;
        let fetcher = Fetcher::new(Policy::default().allow_private()).expect("network client");
        let resolver = Arc::new(DidResolver::new(fetcher.clone(), directory));
        let client =
            Client::new(fetcher, resolver, "https://tenor.example", "did:web:tenor.example").expect("configured tenor");
        let result = client.world_target(WORLD).await;
        if status == StatusCode::NOT_FOUND {
            assert!(matches!(result, Err(ClientError::NotFound)));
        } else {
            assert!(matches!(result, Err(ClientError::Upstream)));
        }
        server.abort();
    }
}

#[derive(Default)]
struct Captured {
    calls: Mutex<Vec<(String, String, Value)>>,
}

async fn observe(
    State(captured): State<Arc<Captured>>,
    Path(method): Path<String>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> impl axum::response::IntoResponse {
    let authorization = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_owned();
    captured
        .calls
        .lock()
        .expect("capture mutex")
        .push((method, authorization, body));
    (StatusCode::FORBIDDEN, Json(json!({"error": "Forbidden"})))
}

/// A World write must use the resolved World's service and exact audience,
/// not the configured creation tenor; a forbidden reply remains a refusal.
#[tokio::test]
async fn world_write_uses_its_resolved_service_and_keeps_forbidden_distinct() {
    let captured = Arc::new(Captured::default());
    let (world_url, server) = serve(
        axum::Router::new()
            .route("/xrpc/{method}", post(observe))
            .with_state(Arc::clone(&captured)),
    )
    .await;
    let client = client(
        Policy::default().allow_private(),
        document(WORLD, "#motet_tenor", "MotetTenor", &world_url),
    );
    let target = client.world_target(WORLD).await.expect("World service");
    assert_eq!(target.audience(), format!("{WORLD}#motet_tenor"));
    assert!(matches!(
        client
            .post(&target, "member-service-token", METHOD, &json!({"world": WORLD}))
            .await,
        Err(ClientError::Forbidden)
    ));
    let calls = captured.calls.lock().expect("capture mutex");
    assert_eq!(calls.len(), 1);
    let (actual_method, authorization, body) = calls.first().expect("one request");
    assert_eq!(actual_method, METHOD);
    assert_eq!(authorization, "Bearer member-service-token");
    assert_eq!(body["world"], WORLD);
    drop(calls);
    server.abort();
}

async fn error_response(Json(body): Json<Value>) -> impl axum::response::IntoResponse {
    let (status, value) = match body.get("case").and_then(Value::as_str) {
        Some("missing") => (StatusCode::NOT_FOUND, json!({"error": "NotFound"})),
        Some("invalid") => (
            StatusCode::BAD_REQUEST,
            json!({"error": "InvalidRequest", "message": "invalid scene"}),
        ),
        Some("forbidden") => (StatusCode::BAD_REQUEST, json!({"error": "Forbidden"})),
        Some("unauthorized") => (StatusCode::UNAUTHORIZED, json!({"error": "AuthRequired"})),
        _ => (StatusCode::SERVICE_UNAVAILABLE, json!({"error": "Unavailable"})),
    };
    (status, Json(value))
}

/// A durable refusal, invalid request, rejected credential and transient
/// failure must not all become the same retryable outcome.
#[tokio::test]
async fn tenor_error_classification_keeps_refusals_distinct_from_failures() {
    let (source, server) = serve(axum::Router::new().route("/xrpc/{method}", post(error_response))).await;
    let client = client(
        Policy::default().allow_private(),
        document(WORLD, "#motet_tenor", "MotetTenor", &source),
    );
    let target = client.world_target(WORLD).await.expect("World service");
    assert!(matches!(
        client.post(&target, "token", METHOD, &json!({"case": "missing"})).await,
        Err(ClientError::NotFound)
    ));
    assert!(matches!(
        client.post(&target, "token", METHOD, &json!({"case": "invalid"})).await,
        Err(ClientError::InvalidRequest(message)) if message == "invalid scene"
    ));
    assert!(matches!(
        client
            .post(&target, "token", METHOD, &json!({"case": "forbidden"}))
            .await,
        Err(ClientError::Forbidden)
    ));
    assert!(matches!(
        client
            .post(&target, "token", METHOD, &json!({"case": "unauthorized"}))
            .await,
        Err(ClientError::Unauthorized)
    ));
    assert!(matches!(
        client
            .post(&target, "token", METHOD, &json!({"case": "unavailable"}))
            .await,
        Err(ClientError::Upstream)
    ));
    server.abort();
}

async fn redirect(State(destination): State<String>) -> impl axum::response::IntoResponse {
    (StatusCode::TEMPORARY_REDIRECT, [(header::LOCATION, destination)])
}

async fn received(State(count): State<Arc<AtomicUsize>>) -> impl axum::response::IntoResponse {
    count.fetch_add(1, Ordering::SeqCst);
    StatusCode::NO_CONTENT
}

/// A tenor response cannot redirect the actor's bearer token to another host.
#[tokio::test]
async fn tenor_redirect_never_forwards_the_service_token() {
    let count = Arc::new(AtomicUsize::new(0));
    let (destination, receiver) = serve(
        axum::Router::new()
            .route("/stolen", post(received))
            .with_state(Arc::clone(&count)),
    )
    .await;
    let (source, server) = serve(
        axum::Router::new()
            .route("/xrpc/{method}", post(redirect))
            .with_state(format!("{destination}/stolen")),
    )
    .await;
    let client = client(
        Policy::default().allow_private(),
        document(WORLD, "#motet_tenor", "MotetTenor", &source),
    );
    let target = client.world_target(WORLD).await.expect("World service");
    assert!(matches!(
        client.post(&target, "member-service-token", METHOD, &json!({})).await,
        Err(ClientError::Upstream)
    ));
    assert_eq!(count.load(Ordering::SeqCst), 0);
    server.abort();
    receiver.abort();
}

async fn oversized() -> axum::response::Response {
    let chunks = futures::stream::iter(vec![
        Ok::<_, std::convert::Infallible>(bytes::Bytes::from(vec![b'a'; 40])),
        Ok(bytes::Bytes::from(vec![b'b'; 40])),
    ]);
    (StatusCode::OK, Body::from_stream(chunks)).into_response()
}

/// The response cap also applies without a Content-Length; otherwise a
/// malicious tenor could exhaust memory using a streaming response.
#[tokio::test]
async fn tenor_stream_cannot_bypass_response_size_limit() {
    let (source, server) = serve(axum::Router::new().route("/xrpc/{method}", post(oversized))).await;
    let mut policy = Policy::default().allow_private();
    policy.max_response_bytes = 64;
    let client = client(policy, document(WORLD, "#motet_tenor", "MotetTenor", &source));
    let target = client.world_target(WORLD).await.expect("World service");
    assert!(matches!(
        client.post(&target, "member-service-token", METHOD, &json!({})).await,
        Err(ClientError::Upstream)
    ));
    server.abort();
}
