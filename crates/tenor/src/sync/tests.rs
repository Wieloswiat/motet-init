//! Consumer contracts through the registered transport and a real shard.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::sync::Arc;
use std::time::Duration;

use atproto_dasl::{CarBlock, CarReader, CarWriter, Cid};
use atproto_firehose::wire::{Event as WireEvent, split_frame};
use atproto_identity::key::{KeyData, to_public};
use atproto_identity::model::DocumentBuilder;
use atproto_record::tid::Tid;
use atproto_repo::{MemoryRepository, RecordPath, RepoConfig};
use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use bytes::Bytes;
use motet_net::identity::DidResolver;
use motet_net::{Fetcher, Policy};
use motet_rules::Did;
use motet_xrpc::Caller;
use motet_xrpc::server::Service;
use object_store::memory::InMemory;
use serde_json::{Value, json};
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
use tokio::net::{TcpListener, TcpStream};
use tower::ServiceExt as _;

use crate::api::State;
use crate::event::{Event, Sequenced};
use crate::identity::Hosting;
use crate::keys::{MasterKey, Purpose};
use crate::pds::Pds;
use crate::repo::{self, ApplyWrites, Committed, Nsid, Write};
use crate::shard::{self, Entry, Handle, Op, Options, Shard, Txn};
use crate::store::keys;
use crate::world::{Status, WorldInfo};

use super::feed::{Feed, StartError, Subscription};
use super::frame;

const WORLD: &str = "did:plc:abcdefghijklmnopqrstuvwx";
const OTHER: &str = "did:plc:bcdefghijklmnopqrstuvwxy";
const HANDLE: &str = "world.motet.test";
const COLLECTION: &str = "group.opensocial.rule";
const TIME: &str = "2026-10-26T00:00:00Z";

struct Begin {
    did: Did,
    handle: String,
}

impl Op for Begin {
    type Output = Committed;
    type Error = anyhow::Error;

    async fn run(self, txn: &mut Txn<'_>) -> Result<Committed, anyhow::Error> {
        txn.begin_world(self.did.clone(), 1)?;
        let private = txn.generate_key(&self.did, Purpose::Signing)?;
        let public = to_public(&private)?;
        let document = DocumentBuilder::new()
            .id(self.did.to_string())
            .add_also_known_as(format!("at://{}", self.handle))
            .add_pds_service("https://tenor.motet.test")
            .add_multikey(
                format!("{}#atproto", self.did),
                self.did.to_string(),
                public.to_string(),
            )
            .build()
            .expect("valid test DID document");
        let info = WorldInfo {
            did: self.did.clone(),
            handle: self.handle,
            owner: Did::new("did:plc:owner"),
            epoch: 1,
            status: Status::Active,
            created: TIME.into(),
            document,
        };
        txn.put(keys::world_info(&self.did), serde_json::to_vec(&info)?.into());
        txn.put(
            keys::handle(&info.handle),
            Bytes::copy_from_slice(self.did.as_str().as_bytes()),
        );
        Ok(repo::genesis(txn, &self.did).await?)
    }
}

async fn running() -> Handle {
    let shard = Shard::open(
        Arc::new(InMemory::new()),
        "sync",
        Arc::new(MasterKey::from_bytes(&[31; 32])),
    )
    .await
    .expect("in-memory shard");
    shard::spawn(
        shard,
        Options {
            max_batch: 16,
            checkpoint_every: Duration::from_secs(3600),
        },
    )
}

struct Fixture {
    handle: Handle,
    router: axum::Router,
    public: KeyData,
}

impl Fixture {
    async fn new() -> Self {
        let handle = running().await;
        handle
            .run(Begin {
                did: Did::new(WORLD),
                handle: HANDLE.into(),
            })
            .await
            .expect("writer")
            .expect("World");
        let private = handle
            .reader()
            .key(&Did::new(WORLD), Purpose::Signing)
            .await
            .expect("key read")
            .expect("key");
        let fetcher = Fetcher::new(Policy::default()).expect("HTTP client");
        let resolver = Arc::new(DidResolver::new(fetcher.clone(), "https://plc.motet.test"));
        let hosting = Arc::new(
            Hosting::new(
                &"https://tenor.motet.test".parse().expect("origin"),
                vec![".motet.test".into()],
                "https://plc.motet.test".into(),
                fetcher.clone(),
                Vec::new(),
            )
            .expect("hosting configuration"),
        );
        let state = Arc::new(State {
            shard: handle.clone(),
            records: Pds::new(fetcher, resolver),
            hosting,
        });
        let service = Service::new(Arc::new(|_| Box::pin(async { Ok(Caller::Anonymous) })));
        let router = super::methods(service)
            .into_router(state)
            .expect("registered sync transport")
            .layer(axum::middleware::from_fn_with_state(
                motet_xrpc::limits::Limits::new(
                    128,
                    64,
                    Duration::from_secs(30),
                    Some("/xrpc/com.atproto.sync.subscribeRepos"),
                ),
                motet_xrpc::limits::enforce,
            ));
        Self {
            handle,
            router,
            public: to_public(&private).expect("public key"),
        }
    }

    async fn write(&self, writes: Vec<Write>) -> Committed {
        self.handle
            .run(ApplyWrites {
                world: Did::new(WORLD),
                writes,
                swap_commit: None,
            })
            .await
            .expect("writer")
            .expect("commit")
    }

    async fn get(&self, method: &str, query: &str) -> (StatusCode, Bytes, String) {
        let response = self
            .router
            .clone()
            .oneshot(
                Request::get(format!("/xrpc/{method}?{query}"))
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("router");
        let status = response.status();
        let content_type = response
            .headers()
            .get("content-type")
            .expect("content type")
            .to_str()
            .expect("ASCII")
            .to_owned();
        let bytes = to_bytes(response.into_body(), 8 * 1024 * 1024).await.expect("body");
        (status, bytes, content_type)
    }

    async fn json(&self, method: &str, query: &str) -> Value {
        let (status, bytes, _) = self.get(method, query).await;
        assert_eq!(status, StatusCode::OK, "{}", std::str::from_utf8(&bytes).expect("JSON"));
        serde_json::from_slice(&bytes).expect("JSON response")
    }

    async fn car(&self, method: &str, query: &str) -> Bytes {
        let (status, bytes, content_type) = self.get(method, query).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(content_type, "application/vnd.ipld.car");
        bytes
    }
}

fn rkey(slot: u8) -> String {
    Tid::from_parts(1_700_000_000_000_000 + u64::from(slot), 0).to_string()
}

fn value(number: u8) -> Value {
    json!({ "$type": COLLECTION, "title": format!("Rule {number}"), "text": "Be kind", "order": number,
        "bytes": { "$bytes": "AQID" },
        "link": { "$link": atproto_dasl::cid::compute_cid(b"an external reference").to_string() } })
}

fn create(slot: u8) -> Write {
    Write::Create {
        collection: Nsid::new(COLLECTION).expect("NSID"),
        rkey: rkey(slot),
        record: repo::record_from_json(&value(slot)).expect("record"),
    }
}

async fn blocks(bytes: &[u8]) -> (Cid, BTreeMap<Cid, Vec<u8>>) {
    let mut car = CarReader::new(bytes).await.expect("CAR");
    let root = *car.root().expect("root");
    let mut blocks = BTreeMap::new();
    while let Some(block) = car.next_block().await.expect("verified block") {
        assert!(blocks.insert(block.cid, block.data).is_none(), "CAR repeats a block");
    }
    (root, blocks)
}

async fn repack(root: Cid, blocks: BTreeMap<Cid, Vec<u8>>) -> Vec<u8> {
    let mut writer = CarWriter::new(Vec::new(), vec![root]).await.expect("CAR writer");
    for (cid, data) in blocks {
        writer
            .write_block(&CarBlock::new(cid, data))
            .await
            .expect("write block");
    }
    writer.finish().await.expect("CAR complete")
}

async fn verified(bytes: &[u8], public: &KeyData) -> MemoryRepository {
    MemoryRepository::from_car_verified(bytes, RepoConfig::default(), public, WORLD)
        .await
        .expect("signed repository")
}

/// A caller reconstructs the current signed repository from a baseline plus
/// since, but never receives obsolete blocks in a full export. Catches storage
/// enumeration masquerading as reachability, omitted roots and broken proofs.
#[tokio::test]
async fn exports_and_proofs_verify_and_deltas_reconstruct_the_current_tree() {
    let fixture = Fixture::new().await;
    fixture.write((0..8).map(create).collect()).await;
    let baseline = fixture.car("com.atproto.sync.getRepo", &format!("did={HANDLE}")).await;
    let old_repo = verified(&baseline, &fixture.public).await;
    let since = old_repo.rev().to_owned();
    let old_cid = old_repo
        .get_record_cid(&RecordPath::new(COLLECTION, rkey(2)))
        .await
        .expect("old lookup")
        .expect("record");
    let current = fixture
        .write(vec![
            Write::Update {
                collection: Nsid::new(COLLECTION).expect("NSID"),
                rkey: rkey(2),
                record: repo::record_from_json(&value(42)).expect("record"),
                swap: None,
            },
            Write::Delete {
                collection: Nsid::new(COLLECTION).expect("NSID"),
                rkey: rkey(3),
                swap: None,
            },
            create(8),
        ])
        .await;
    let full = fixture.car("com.atproto.sync.getRepo", &format!("did={WORLD}")).await;
    let (root, full_blocks) = blocks(&full).await;
    let (old_root, baseline_blocks) = blocks(&baseline).await;
    assert!(!full_blocks.contains_key(&old_root));
    assert!(!full_blocks.contains_key(&old_cid));
    let delta = fixture
        .car("com.atproto.sync.getRepo", &format!("did={WORLD}&since={since}"))
        .await;
    let (delta_root, delta_blocks) = blocks(&delta).await;
    assert_eq!(delta_root, root);
    let expected: BTreeSet<_> = full_blocks
        .keys()
        .filter(|cid| **cid == root || !baseline_blocks.contains_key(cid))
        .copied()
        .collect();
    assert_eq!(delta_blocks.keys().copied().collect::<BTreeSet<_>>(), expected);
    let mut combined = baseline_blocks;
    combined.extend(delta_blocks);
    let replay = repack(root, combined).await;
    let consumer = verified(&replay, &fixture.public).await;
    assert_eq!(consumer.record_count().await.expect("records"), 8);
    assert_eq!(
        consumer
            .get_record(&RecordPath::new(COLLECTION, rkey(2)))
            .await
            .expect("updated record"),
        Some(value(42))
    );
    assert!(
        consumer
            .get_record(&RecordPath::new(COLLECTION, rkey(3)))
            .await
            .expect("deleted record")
            .is_none()
    );
    let proof = fixture
        .car(
            "com.atproto.sync.getRecord",
            &format!("did={WORLD}&collection={COLLECTION}&rkey={}", rkey(2)),
        )
        .await;
    let proof_repo = verified(&proof, &fixture.public).await;
    assert_eq!(
        proof_repo
            .get_record(&RecordPath::new(COLLECTION, rkey(2)))
            .await
            .expect("covered MST path"),
        Some(value(42))
    );
    let unchanged = fixture
        .car(
            "com.atproto.sync.getRepo",
            &format!("did={WORLD}&since={}", current.rev),
        )
        .await;
    let (unchanged_root, unchanged_blocks) = blocks(&unchanged).await;
    assert_eq!(unchanged_root, root);
    assert_eq!(unchanged_blocks.len(), 1);
    let (status, body, _) = fixture
        .get(
            "com.atproto.sync.getRepo",
            &format!("did={WORLD}&since={}", Tid::from_parts(1, 0)),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let error: Value = serde_json::from_slice(&body).expect("error");
    assert_eq!(error.get("error"), Some(&json!("InvalidRequest")));
    assert_block_reads(&fixture, current.commit, &consumer).await;
    assert_commit_frames(&fixture).await;
}

/// A deleted record has a signed sync absence proof, but remains missing to
/// repo readers. Catches returning `RecordNotFound` before exporting the proof.
#[tokio::test]
async fn deleted_record_sync_proof_verifies_absence_while_repo_read_stays_not_found() {
    let fixture = Fixture::new().await;
    fixture.write((0..8).map(create).collect()).await;
    let current = fixture
        .write(vec![Write::Delete {
            collection: Nsid::new(COLLECTION).expect("NSID"),
            rkey: rkey(3),
            swap: None,
        }])
        .await;
    let proof = fixture
        .car(
            "com.atproto.sync.getRecord",
            &format!("did={WORLD}&collection={COLLECTION}&rkey={}", rkey(3)),
        )
        .await;
    let consumer = verified(&proof, &fixture.public).await;
    assert_eq!(consumer.rev(), current.rev.to_string());
    assert!(
        consumer
            .get_record(&RecordPath::new(COLLECTION, rkey(3)))
            .await
            .expect("covered absent MST path")
            .is_none()
    );
    let (status, body, _) = fixture
        .get(
            "com.atproto.repo.getRecord",
            &format!("repo={WORLD}&collection={COLLECTION}&rkey={}", rkey(3)),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let error: Value = serde_json::from_slice(&body).expect("error");
    assert_eq!(error.get("error"), Some(&json!("RecordNotFound")));
}

async fn assert_block_reads(fixture: &Fixture, head: Cid, consumer: &MemoryRepository) {
    let cid = consumer
        .get_record_cid(&RecordPath::new(COLLECTION, rkey(2)))
        .await
        .expect("lookup")
        .expect("record");
    let car = fixture
        .car(
            "com.atproto.sync.getBlocks",
            &format!("did={WORLD}&cids={head}&cids={cid}&cids={cid}"),
        )
        .await;
    let (_, requested) = blocks(&car).await;
    assert_eq!(
        requested.keys().copied().collect::<BTreeSet<_>>(),
        BTreeSet::from([head, cid])
    );
    let absent = atproto_dasl::cid::compute_cid(b"not in this repository");
    let (_, body, _) = fixture
        .get("com.atproto.sync.getBlocks", &format!("did={WORLD}&cids={absent}"))
        .await;
    let error: Value = serde_json::from_slice(&body).expect("error");
    assert_eq!(error.get("error"), Some(&json!("BlockNotFound")));
}

async fn assert_commit_frames(fixture: &Fixture) {
    let events = fixture
        .handle
        .reader()
        .scan(keys::EVENTS, None, 32)
        .await
        .expect("events");
    for (_, stored) in events {
        let event: Sequenced = serde_json::from_slice(&stored).expect("stored event");
        let bytes = frame::event(&event).expect("frame");
        let frame = split_frame(&bytes).expect("consumer frame");
        match frame.event().expect("consumer event") {
            WireEvent::Sync(sync) => {
                assert_eq!(sync.did, WORLD);
                let (root, blocks) = blocks(&frame.sync_blocks().expect("CBOR byte string")).await;
                assert_eq!(blocks.len(), 1);
                assert!(blocks.contains_key(&root));
            }
            WireEvent::Commit(commit) => {
                let (root, _) = blocks(&frame.commit_blocks().expect("CBOR byte string")).await;
                assert_eq!(root, commit.commit);
                assert_eq!(commit.repo, WORLD);
                assert!(commit.prev_data.is_some());
                assert!(commit.since.is_some());
            }
            other => panic!("unexpected event {other:?}"),
        }
    }
}

async fn page_keys(fixture: &Fixture, reverse: bool) -> Vec<String> {
    let mut cursor = None;
    let mut out = Vec::new();
    loop {
        let mut query = format!("repo={HANDLE}&collection={COLLECTION}&limit=2&reverse={reverse}");
        if let Some(cursor) = cursor {
            write!(query, "&cursor={cursor}").expect("writing to a String cannot fail");
        }
        let page = fixture.json("com.atproto.repo.listRecords", &query).await;
        for record in page.get("records").and_then(Value::as_array).expect("records") {
            let uri = record.get("uri").and_then(Value::as_str).expect("URI");
            let key = uri.rsplit('/').next().expect("key");
            assert!(!out.iter().any(|previous| previous == key), "page repeated {key}");
            out.push(key.to_owned());
        }
        cursor = page.get("cursor").and_then(Value::as_str).map(str::to_owned);
        if cursor.is_none() {
            return out;
        }
    }
}

/// Pagination returns each current record once in both directions, and handle
/// lookups describe the same signed account. Catches inclusive cursors, leaked
/// revision-index rows, JSON link/byte corruption and stale-CID substitution.
#[tokio::test]
async fn record_and_repository_pages_describe_current_hosted_state() {
    let fixture = Fixture::new().await;
    let committed = fixture.write((0..7).map(create).collect()).await;
    assert_eq!(
        page_keys(&fixture, false).await,
        (0..7).rev().map(rkey).collect::<Vec<_>>()
    );
    assert_eq!(page_keys(&fixture, true).await, (0..7).map(rkey).collect::<Vec<_>>());
    let query = format!("repo={HANDLE}&collection={COLLECTION}&rkey={}", rkey(2));
    let record = fixture.json("com.atproto.repo.getRecord", &query).await;
    assert_eq!(record.get("value"), Some(&value(2)));
    let cid = record.get("cid").and_then(Value::as_str).expect("CID");
    let pinned = fixture
        .json("com.atproto.repo.getRecord", &format!("{query}&cid={cid}"))
        .await;
    assert_eq!(pinned, record);
    let absent = atproto_dasl::cid::compute_cid(b"a different record version");
    let (_, bytes, _) = fixture
        .get("com.atproto.repo.getRecord", &format!("{query}&cid={absent}"))
        .await;
    let error: Value = serde_json::from_slice(&bytes).expect("error");
    assert_eq!(error.get("error"), Some(&json!("RecordNotFound")));
    let described = fixture
        .json("com.atproto.repo.describeRepo", &format!("repo={HANDLE}"))
        .await;
    assert_eq!(described.get("did"), Some(&json!(WORLD)));
    assert_eq!(described.get("collections"), Some(&json!([COLLECTION])));
    assert_eq!(described.get("handleIsCorrect"), Some(&json!(true)));
    let document: atproto_identity::model::Document =
        serde_json::from_value(described.get("didDoc").expect("DID document").clone()).expect("DID document consumer");
    assert_eq!(document.id, WORLD);
    assert!(document.confirms_handle(HANDLE));
    let latest = fixture
        .json("com.atproto.sync.getLatestCommit", &format!("did={WORLD}"))
        .await;
    assert_eq!(latest.get("cid"), Some(&json!(committed.commit.to_string())));
    let status = fixture
        .json("com.atproto.sync.getRepoStatus", &format!("did={WORLD}"))
        .await;
    assert_eq!(status.get("rev"), latest.get("rev"));
    assert_eq!(status.get("active"), Some(&json!(true)));
    fixture
        .handle
        .run(Begin {
            did: Did::new(OTHER),
            handle: "other.motet.test".into(),
        })
        .await
        .expect("writer")
        .expect("other World");
    let first = fixture.json("com.atproto.sync.listRepos", "limit=1").await;
    assert_eq!(first.pointer("/repos/0/did"), Some(&json!(WORLD)));
    let cursor = first.get("cursor").and_then(Value::as_str).expect("next page");
    let second = fixture
        .json("com.atproto.sync.listRepos", &format!("limit=1&cursor={cursor}"))
        .await;
    assert_eq!(second.pointer("/repos/0/did"), Some(&json!(OTHER)));
    assert!(second.get("cursor").is_none());
}

async fn emit(handle: &Handle, start: u64, count: u64) {
    let events = (start..start + count)
        .map(|seq| {
            if seq % 2 == 0 {
                Event::Identity {
                    did: Did::new(WORLD),
                    handle: Some(format!("world-{seq}.motet.test")),
                    time: TIME.into(),
                }
            } else {
                Event::Account {
                    did: Did::new(WORLD),
                    active: true,
                    status: None,
                    time: TIME.into(),
                }
            }
        })
        .collect();
    handle
        .submit(Entry::Write(crate::shard::Write {
            mutations: Vec::new(),
            events,
        }))
        .await
        .expect("writer")
        .expect("events committed");
}

async fn next_sequence(feed: &mut Feed) -> u64 {
    let event = tokio::time::timeout(Duration::from_secs(5), feed.next())
        .await
        .expect("stream must not stall")
        .expect("history read")
        .expect("live stream");
    let frame = frame::event(&event).expect("frame");
    let sequence = match split_frame(&frame)
        .expect("frame consumer")
        .event()
        .expect("event consumer")
    {
        WireEvent::Identity(event) => event.seq,
        WireEvent::Account(event) => event.seq,
        other => panic!("unexpected event {other:?}"),
    };
    u64::try_from(sequence).expect("nonnegative sequence")
}

/// A commit between subscribing and reading the watermark appears in both
/// storage and broadcast. Consumers must see it once, followed by live events.
/// Catches subscribe-after-scan gaps and missing transition deduplication.
#[tokio::test]
async fn cursor_backfill_and_live_delivery_have_no_gaps_or_duplicates() {
    let handle = running().await;
    emit(&handle, 0, 300).await;
    let subscription = Subscription::new(&handle);
    emit(&handle, 300, 50).await;
    let mut feed = subscription.start(Some(42)).await.expect("valid cursor");
    emit(&handle, 350, 10).await;
    for expected in 43..360 {
        assert_eq!(next_sequence(&mut feed).await, expected);
    }
    emit(&handle, 360, 1).await;
    assert_eq!(next_sequence(&mut feed).await, 360);
}

/// A relay may subscribe at cursor zero before the first event exists and at
/// the current head later. An off-by-one future-cursor check would disconnect
/// it permanently or skip the first World on a fresh tenor.
#[tokio::test]
async fn empty_and_current_head_cursors_deliver_new_events() {
    let handle = running().await;
    assert!(matches!(
        Subscription::new(&handle).start(Some(1)).await,
        Err(StartError::FutureCursor)
    ));
    let mut from_empty = Subscription::new(&handle).start(Some(0)).await.expect("empty cursor");
    emit(&handle, 0, 1).await;
    assert_eq!(next_sequence(&mut from_empty).await, 0);

    let mut from_head = Subscription::new(&handle).start(Some(0)).await.expect("head cursor");
    emit(&handle, 1, 1).await;
    assert_eq!(next_sequence(&mut from_head).await, 1);
}

/// Overflow the real shard's broadcast buffer, then resume through more than
/// one stored page. Dropping Lagged or advancing to the newest notification
/// would lose committed events; failing to deduplicate repeats them afterwards.
#[tokio::test]
async fn lagged_consumers_resume_from_durable_pages() {
    let handle = running().await;
    emit(&handle, 0, 1).await;
    let mut feed = Subscription::new(&handle).start(Some(0)).await.expect("cursor");
    emit(&handle, 1, 5000).await;
    for expected in 1..=5000 {
        assert_eq!(next_sequence(&mut feed).await, expected);
    }
    emit(&handle, 5001, 1).await;
    assert_eq!(next_sequence(&mut feed).await, 5001);
}

async fn connect(router: axum::Router, query: &str) -> (TcpStream, tokio::task::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("listener");
    let address = listener.local_addr().expect("address");
    let server = tokio::spawn(async move {
        axum::serve(listener, router).await.expect("server");
    });
    let mut socket = TcpStream::connect(address).await.expect("connect");
    socket.write_all(format!(
        "GET /xrpc/com.atproto.sync.subscribeRepos?{query} HTTP/1.1\r\nHost: {address}\r\nConnection: Upgrade\r\nUpgrade: websocket\r\nSec-WebSocket-Version: 13\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Protocol: xrpc.v0.cbor\r\n\r\n"
    ).as_bytes()).await.expect("upgrade request");
    let mut headers = Vec::new();
    while !headers.ends_with(b"\r\n\r\n") {
        headers.push(socket.read_u8().await.expect("upgrade response"));
        assert!(headers.len() < 4096);
    }
    let headers = std::str::from_utf8(&headers).expect("HTTP response");
    assert!(headers.starts_with("HTTP/1.1 101 "), "{headers}");
    assert!(
        headers
            .to_ascii_lowercase()
            .contains("sec-websocket-protocol: xrpc.v0.cbor")
    );
    (socket, server)
}

async fn websocket_message(socket: &mut TcpStream) -> (u8, Vec<u8>) {
    let first = socket.read_u8().await.expect("frame opcode");
    let length = socket.read_u8().await.expect("frame size");
    assert_eq!(length & 0x80, 0, "server frames are unmasked");
    let length = match length & 0x7f {
        126 => u64::from(socket.read_u16().await.expect("16-bit size")),
        127 => socket.read_u64().await.expect("64-bit size"),
        length => u64::from(length),
    };
    let mut bytes = vec![0; usize::try_from(length).expect("test frame size")];
    socket.read_exact(&mut bytes).await.expect("frame payload");
    (first & 0x0f, bytes)
}

/// A future cursor is a terminal XRPC CBOR error after a successful upgrade,
/// not an HTTP error, an info notice or a malformed event body.
#[tokio::test]
async fn future_cursor_emits_a_consumer_readable_terminal_error_frame() {
    let fixture = Fixture::new().await;
    let (mut socket, server) = connect(fixture.router, "cursor=100").await;
    let (opcode, bytes) = tokio::time::timeout(Duration::from_secs(5), websocket_message(&mut socket))
        .await
        .expect("error frame");
    assert_eq!(opcode, 2);
    let frame = split_frame(&bytes).expect("consumer frame");
    assert!(frame.header.is_error());
    assert!(frame.header.t.is_none());
    let WireEvent::Error(error) = frame.event().expect("consumer error") else {
        panic!("terminal error expected")
    };
    assert_eq!(error.error, "FutureCursor");
    let (opcode, _) = websocket_message(&mut socket).await;
    assert_eq!(opcode, 8);
    server.abort();
}
