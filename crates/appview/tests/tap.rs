//! Tap acknowledgements promise durable, queryable events, including after a crash.

use std::sync::Arc;
use std::time::Duration;

use atproto_tap::TapConfig;
use axum::extract::State;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::response::Response;
use axum::routing::get;
use futures::StreamExt as _;
use motet_appview::archive::Archive;
use motet_appview::event::{Action, Event, RecordEvent};
use motet_appview::indexer::Indexer;
use motet_appview::{MIGRATOR, tap};
use serde_json::{Value, json};
use sqlx::PgPool;
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;

const DID: &str = "did:plc:alice";
const COLLECTION: &str = "at.motet.pilot.post";

struct Observation {
    id: u64,
    archived: Vec<(u64, Vec<Event>)>,
    applied: i64,
    records: Vec<Value>,
    versions: Vec<Value>,
    accounts: Vec<Value>,
}

struct FakeState {
    messages: Vec<Value>,
    pool: PgPool,
    archive: Archive,
    acks: mpsc::UnboundedSender<Observation>,
}

struct FakeTap {
    config: TapConfig,
    acks: mpsc::UnboundedReceiver<Observation>,
    server: JoinHandle<()>,
}

impl Drop for FakeTap {
    fn drop(&mut self) {
        self.server.abort();
    }
}

impl FakeTap {
    async fn start(messages: Vec<Value>, pool: PgPool, archive: Archive) -> Self {
        let (acks, receiver) = mpsc::unbounded_channel();
        let state = Arc::new(FakeState {
            messages,
            pool,
            archive,
            acks,
        });
        let app = axum::Router::new().route("/channel", get(channel)).with_state(state);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind fake Tap");
        let config = TapConfig::builder()
            .hostname(format!("http://{}", listener.local_addr().expect("address")))
            .build();
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.expect("fake Tap server");
        });
        Self {
            config,
            acks: receiver,
            server,
        }
    }

    async fn ack(&mut self) -> Observation {
        tokio::time::timeout(Duration::from_secs(10), self.acks.recv())
            .await
            .expect("indexer acknowledges promptly")
            .expect("fake Tap remains open")
    }
}

async fn channel(State(state): State<Arc<FakeState>>, upgrade: WebSocketUpgrade) -> Response {
    upgrade.on_upgrade(move |socket| exchange(socket, state))
}

async fn exchange(mut socket: WebSocket, state: Arc<FakeState>) {
    for message in &state.messages {
        if socket.send(Message::Text(message.to_string().into())).await.is_err() {
            return;
        }
    }
    while let Some(Ok(message)) = socket.recv().await {
        if let Message::Text(text) = message {
            let ack: Value = serde_json::from_str(&text).expect("ack JSON");
            assert_eq!(ack.get("type").and_then(Value::as_str), Some("ack"));
            let id = ack.get("id").and_then(Value::as_u64).expect("ack ID");
            let observation = observe(&state.pool, &state.archive, id).await;
            if state.acks.send(observation).is_err() {
                return;
            }
        }
    }
}

async fn observe(pool: &PgPool, archive: &Archive, id: u64) -> Observation {
    // Read the committed watermark before waiting on any record-table locks.
    // An ack sent while projection application is blocked must fail the test.
    let applied = sqlx::query_scalar("SELECT archive_seq FROM projection.meta")
        .fetch_one(pool)
        .await
        .expect("projection watermark");
    let archived = archive.read_from(0).await.expect("durable archive");
    let records = sqlx::query_scalar("SELECT to_jsonb(r) FROM projection.records r ORDER BY uri")
        .fetch_all(pool)
        .await
        .expect("records");
    let versions = sqlx::query_scalar("SELECT to_jsonb(r) FROM projection.record_versions r ORDER BY uri, cid")
        .fetch_all(pool)
        .await
        .expect("versions");
    let accounts = sqlx::query_scalar("SELECT to_jsonb(a) FROM projection.accounts a ORDER BY did")
        .fetch_all(pool)
        .await
        .expect("accounts");
    Observation {
        id,
        archived,
        applied,
        records,
        versions,
        accounts,
    }
}

async fn setup() -> (PgPool, Archive, Indexer) {
    let pool = motet_testkit::services::database().await;
    MIGRATOR.run(&pool).await.expect("migrations");
    let archive = Archive::new(motet_testkit::services::bucket(), "archive");
    let indexer = Indexer::start(pool.clone(), &archive).await.expect("indexer");
    (pool, archive, indexer)
}

fn record(id: u64, live: bool) -> Value {
    json!({
        "id": id, "type": "record",
        "record": {
            "live": live, "rev": "3l00000000001", "did": DID,
            "collection": COLLECTION, "rkey": format!("k{id}"), "action": "create",
            "cid": format!("bafy-{id}"), "record": {"text": format!("post {id}")}
        }
    })
}

fn start(indexer: Indexer, config: TapConfig) -> (oneshot::Sender<()>, JoinHandle<Result<(), tap::Error>>) {
    let (stop, shutdown) = oneshot::channel();
    let task = tokio::spawn(tap::run(indexer, config, async {
        let _ = shutdown.await;
    }));
    (stop, task)
}

async fn stop(sender: oneshot::Sender<()>, task: JoinHandle<Result<(), tap::Error>>) {
    sender.send(()).expect("input running");
    tokio::time::timeout(Duration::from_secs(10), task)
        .await
        .expect("graceful input shutdown")
        .expect("input task")
        .expect("input succeeds");
}

/// Acks cover both durable storage and committed projections, not just receipt.
/// Acking before ingest, or after append but before apply, violates this property.
#[tokio::test]
#[ignore = "integration: needs PostgreSQL and S3 (devenv up)"]
async fn acks_follow_archive_and_projection_commit() {
    let (pool, archive, indexer) = setup().await;
    let mut fake = FakeTap::start(vec![record(1, true), record(2, false)], pool, archive).await;
    let (shutdown, task) = start(indexer, fake.config.clone());
    for expected in 1..=2 {
        let ack = fake.ack().await;
        assert_eq!(ack.id, expected);
        let (seq, batch) = ack
            .archived
            .iter()
            .find(|(_, events)| {
                events
                    .iter()
                    .any(|event| matches!(event, Event::Record(record) if record.rkey == format!("k{expected}")))
            })
            .expect("acknowledged event is archived");
        assert!(ack.applied >= i64::try_from(*seq).expect("sequence fits"));
        for event in batch {
            if let Event::Record(record) = event {
                assert!(ack.records.iter().any(|row| row["uri"] == record.uri()
                    && row["record"] == json!({"text": format!("post {}", record.rkey.trim_start_matches('k'))})));
                assert!(ack.versions.iter().any(|row| row["uri"] == record.uri()));
            }
        }
    }
    stop(shutdown, task).await;
}

/// A crash after ingest and before ack causes redelivery, not another record or version.
/// Applying a retry as a new version or duplicate record breaks this property.
#[tokio::test]
#[ignore = "integration: needs PostgreSQL and S3 (devenv up)"]
async fn redelivery_after_ingest_before_ack_preserves_projections() {
    let (pool, archive, mut indexer) = setup().await;
    let mut fake = FakeTap::start(vec![record(1, true)], pool.clone(), archive.clone()).await;
    let mut connection = atproto_tap::connect(fake.config.clone());
    let delivered = tokio::time::timeout(Duration::from_secs(10), connection.next())
        .await
        .expect("first delivery")
        .expect("stream open")
        .expect("event");
    assert_eq!(delivered.id(), 1);
    // Reproduce the exact crash state: this received operation is committed,
    // but the process disappears without ever calling ack on its Tap stream.
    indexer
        .ingest(&[Event::Record(RecordEvent {
            did: DID.into(),
            rev: "3l00000000001".into(),
            collection: COLLECTION.into(),
            rkey: "k1".into(),
            action: Action::Create,
            cid: Some("bafy-1".into()),
            record: Some(json!({"text": "post 1"})),
        })])
        .await
        .expect("commit before crash");
    let before = observe(&pool, &archive, 1).await;
    drop(connection);
    drop(indexer);
    assert!(fake.acks.try_recv().is_err());

    let restarted = Indexer::start(pool, &archive).await.expect("restart");
    let (shutdown, task) = start(restarted, fake.config.clone());
    let after = fake.ack().await;
    assert_eq!(after.id, 1);
    assert_eq!(after.archived.len(), before.archived.len() + 1);
    assert_eq!(after.records, before.records);
    assert_eq!(after.versions, before.versions);
    assert_eq!(after.accounts, before.accounts);
    stop(shutdown, task).await;
}

/// Identity deliveries atomically update handles and account status before ack.
/// Ignoring Tap's `is_active`/`status` fields leaves deactivated accounts active.
#[tokio::test]
#[ignore = "integration: needs PostgreSQL and S3 (devenv up)"]
async fn identity_updates_handle_and_account_status() {
    let (pool, archive, indexer) = setup().await;
    let mut fake = FakeTap::start(
        vec![json!({
            "id": 7, "type": "identity",
            "identity": {"did": DID, "handle": "alice.test", "is_active": false, "status": "deactivated"}
        })],
        pool,
        archive,
    )
    .await;
    let (shutdown, task) = start(indexer, fake.config.clone());
    let ack = fake.ack().await;
    assert_eq!(ack.id, 7);
    assert_eq!(
        ack.accounts,
        vec![json!({"did": DID, "handle": "alice.test", "active": false, "status": "deactivated"})]
    );
    let (seq, events) = ack.archived.last().expect("identity archived");
    assert_eq!(
        events,
        &vec![
            Event::Identity {
                did: DID.into(),
                handle: Some("alice.test".into())
            },
            Event::Account {
                did: DID.into(),
                active: false,
                status: Some("deactivated".into())
            },
        ]
    );
    assert_eq!(ack.applied, i64::try_from(*seq).expect("sequence fits"));
    stop(shutdown, task).await;
}
