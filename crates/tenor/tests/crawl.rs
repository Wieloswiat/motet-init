//! Crawl announcements must reach every configured relay, not only the first
//! healthy one, and carry the public authority rather than the bind address.

use std::sync::{Arc, Mutex};

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::post;
use motet_net::{Fetcher, Policy};
use motet_tenor::crawl::{self, CrawlError};
use serde_json::{Value, json};
use url::Url;

async fn relay(status: StatusCode, seen: Arc<Mutex<Vec<Value>>>) -> (Url, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind fixture relay");
    let address = listener.local_addr().expect("fixture listener has an address");
    let router = axum::Router::new()
        .route(
            "/xrpc/com.atproto.sync.requestCrawl",
            post(
                move |State(seen): State<Arc<Mutex<Vec<Value>>>>, Json(body): Json<Value>| async move {
                    seen.lock()
                        .expect("fixture observations lock is not poisoned")
                        .push(body);
                    status
                },
            ),
        )
        .with_state(seen);
    let task = tokio::spawn(async move {
        axum::serve(listener, router).await.expect("serve fixture relay");
    });
    (
        format!("http://{address}")
            .parse()
            .expect("listener address forms a URL"),
        task,
    )
}

/// One refused announcement must not prevent discovery by the other relays.
/// Catches an early-return-on-error loop and sending the private bind address.
#[tokio::test]
async fn every_relay_receives_the_public_hostname_despite_another_failure() {
    let rejected = Arc::new(Mutex::new(Vec::new()));
    let accepted = Arc::new(Mutex::new(Vec::new()));
    let (first, first_task) = relay(StatusCode::SERVICE_UNAVAILABLE, Arc::clone(&rejected)).await;
    let (second, second_task) = relay(StatusCode::OK, Arc::clone(&accepted)).await;
    let fetcher = Fetcher::new(Policy::default().allow_private()).unwrap();
    let outcome = crawl::announce(&fetcher, &[first.clone(), second], "worlds.example:2584").await;
    assert!(matches!(outcome, Err(CrawlError::Relay { relay, .. }) if relay == first));
    let expected = vec![json!({ "hostname": "worlds.example:2584" })];
    assert_eq!(*rejected.lock().unwrap(), expected);
    assert_eq!(*accepted.lock().unwrap(), expected);
    first_task.abort();
    second_task.abort();
}
