//! Admission bounds through the same router composition as the tenor binary.

use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use atproto_identity::key::{KeyData, KeyType, generate_key, sign, to_public};
use atproto_identity::model::DocumentBuilder;
use atproto_testkit::highport::FakePlcDirectory;
use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use motet_net::identity::DidResolver;
use motet_net::{Fetcher, Policy};
use motet_rules::Did;
use motet_tenor::identity::Hosting;
use motet_tenor::keys::{MasterKey, Purpose};
use motet_tenor::shard::{self, Handle, Op, Options, Shard, Txn};
use motet_tenor::spaces::{self, ProfileInput};
use motet_tenor::store::keys;
use motet_tenor::world::{Status, WorldInfo};
use motet_tenor::{api, pds, repo};
use motet_xrpc::limits::Limits;
use object_store::memory::InMemory;
use serde_json::{Value, json};
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::oneshot;
use tower::ServiceExt as _;

const WORLD: &str = "did:plc:ewvi7nxzyoun6zhxrhs64oiz";
const OWNER: &str = "did:plc:owner";
const OPEN: &str = "at.motet.pilot.play.openScene";
const DEADLINE: Duration = Duration::from_millis(100);

struct Setup;

impl Op for Setup {
    type Output = ();
    type Error = anyhow::Error;

    async fn run(self, txn: &mut Txn<'_>) -> anyhow::Result<()> {
        let world = Did::new(WORLD);
        txn.begin_world(world.clone(), 1)?;
        txn.generate_key(&world, Purpose::Signing)?;
        txn.generate_key(&world, Purpose::Kem)?;
        repo::genesis(txn, &world).await?;
        let info = WorldInfo {
            did: world.clone(),
            handle: "fixture.motet.test".into(),
            owner: Did::new(OWNER),
            epoch: 1,
            status: Status::Active,
            created: "2026-10-26T00:00:00Z".into(),
            document: DocumentBuilder::new().id(WORLD).build()?,
        };
        txn.put(keys::world_info(&world), serde_json::to_vec(&info)?.into());
        spaces::bootstrap(txn, &world, &Did::new(OWNER), ProfileInput::default()).await?;
        Ok(())
    }
}

// Hold the real writer before the HTTP operation is queued; releasing this
// barrier, not an elapsed sleep, lets the independent writer finish the call.
struct Hold {
    entered: oneshot::Sender<()>,
    release: oneshot::Receiver<()>,
}

impl Op for Hold {
    type Output = ();
    type Error = anyhow::Error;

    async fn run(self, _txn: &mut Txn<'_>) -> anyhow::Result<()> {
        self.entered
            .send(())
            .expect("the test waits for the writer before releasing it");
        self.release.await?;
        Ok(())
    }
}

struct Fence;

impl Op for Fence {
    type Output = ();
    type Error = std::convert::Infallible;

    fn run(self, _txn: &mut Txn<'_>) -> impl Future<Output = Result<(), Self::Error>> + Send {
        std::future::ready(Ok(()))
    }
}

struct Fixture {
    handle: Handle,
    router: axum::Router,
    signing: KeyData,
    _plc: FakePlcDirectory,
}

impl Fixture {
    async fn new() -> Self {
        let shard = Shard::open(
            Arc::new(InMemory::new()),
            "limits",
            Arc::new(MasterKey::from_bytes(&[64; 32])),
        )
        .await
        .expect("an in-memory shard opens");
        let handle = shard::spawn(
            shard,
            Options {
                max_batch: 8,
                checkpoint_every: Duration::from_secs(3600),
            },
        );
        handle
            .run(Setup)
            .await
            .expect("the shard accepts the setup operation")
            .expect("setup writes the fixture World");
        let plc = FakePlcDirectory::start().await;
        let signing = generate_key(KeyType::K256Private).expect("a fixture signing key is generated");
        let public = to_public(&signing).expect("a generated key has a public half");
        let document = DocumentBuilder::new()
            .id(OWNER)
            .add_multikey(format!("{OWNER}#atproto"), OWNER, public.to_string())
            .build()
            .expect("the fixture DID document is valid");
        plc.set_document(
            OWNER,
            serde_json::to_value(document).expect("a DID document serializes"),
        );
        let fetcher = Fetcher::new(Policy::default().allow_private()).expect("the local test client builds");
        let resolver = Arc::new(DidResolver::new(fetcher.clone(), plc.origin()));
        let hosting = Arc::new(
            Hosting::new(
                &"https://tenor.motet.test".parse().expect("the fixture origin is a URL"),
                vec![".motet.test".into()],
                plc.origin().into(),
                fetcher.clone(),
                Vec::new(),
            )
            .expect("the fixture hosting configuration is valid"),
        );
        let audience = hosting.audience().to_owned();
        let router = api::served_router(
            Arc::new(api::State {
                shard: handle.clone(),
                records: pds::Pds::new(fetcher, Arc::<DidResolver>::clone(&resolver)),
                hosting,
            }),
            motet_xrpc::service_auth::authenticate(
                resolver,
                Arc::new(move |aud| api::accepts_audience(aud, &audience)),
            ),
            Limits::new(1, 1, DEADLINE, Some("/xrpc/com.atproto.sync.subscribeRepos")),
        )
        .expect("the router composes with the real authenticator");
        Self {
            handle,
            router,
            signing,
            _plc: plc,
        }
    }

    fn request(&self, body: &Value) -> Request<Body> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("the clock is after the epoch")
            .as_secs();
        let header = URL_SAFE_NO_PAD.encode(br#"{"alg":"ES256K","typ":"JWT"}"#);
        let claims = URL_SAFE_NO_PAD.encode(
            serde_json::to_vec(&json!({
                "iss": OWNER, "aud": WORLD, "lxm": OPEN, "iat": now, "exp": now + 60,
                "jti": atproto_record::tid::Tid::new().to_string(),
            }))
            .expect("the claims serialize"),
        );
        let input = format!("{header}.{claims}");
        let signature = sign(&self.signing, input.as_bytes()).expect("the fixture key signs");
        let token = format!("{input}.{}", URL_SAFE_NO_PAD.encode(signature));
        Request::post(format!("/xrpc/{OPEN}"))
            .header("authorization", format!("Bearer {token}"))
            .header("content-type", "application/json")
            .body(Body::from(
                serde_json::to_vec(body).expect("the request body serializes"),
            ))
            .expect("the fixture request is well formed")
    }

    async fn scenes(&self) -> Value {
        let response = self
            .router
            .clone()
            .oneshot(
                Request::get(format!(
                    "/xrpc/com.atproto.repo.listRecords?repo={WORLD}&collection=at.motet.pilot.scene"
                ))
                .body(Body::empty())
                .expect("the fixture request is well formed"),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), 1024 * 1024)
            .await
            .expect("the response body is readable");
        serde_json::from_slice(&body).expect("the response is JSON")
    }
}

/// A lost HTTP response cannot cancel an accepted World operation or apply it twice.
/// Catches cancellation propagation into the writer and retrying without its durable receipt.
#[tokio::test]
async fn timed_out_world_mutation_commits_and_a_fresh_token_recovers_its_original_outcome() {
    let fixture = Fixture::new().await;
    // Warm the real DID resolver without changing the World before holding the writer.
    let response = fixture
        .router
        .clone()
        .oneshot(fixture.request(&json!({})))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let (entered, started) = oneshot::channel();
    let (release, held) = oneshot::channel();
    let handle = fixture.handle.clone();
    let writer = tokio::spawn(async move { handle.run(Hold { entered, release: held }).await.unwrap().unwrap() });
    started.await.unwrap();
    let body = json!({ "world": WORLD, "op": "lost-response", "title": "Committed after timeout" });
    let response = fixture.router.clone().oneshot(fixture.request(&body)).await.unwrap();
    assert_eq!(response.status(), StatusCode::GATEWAY_TIMEOUT);
    release.send(()).unwrap();
    writer.await.unwrap();
    // FIFO acknowledgement establishes that the abandoned call committed before any retry.
    fixture.handle.run(Fence).await.unwrap().unwrap();
    let before = fixture.scenes().await;
    let records = before["records"].as_array().unwrap();
    assert_eq!(records.len(), 1, "the timed-out call still creates its scene");
    let response = fixture.router.clone().oneshot(fixture.request(&body)).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let outcome: Value = serde_json::from_slice(&to_bytes(response.into_body(), 4096).await.unwrap()).unwrap();
    assert_eq!(outcome["uri"], records[0]["uri"]);
    assert_eq!(outcome["cid"], records[0]["cid"]);
    assert_eq!(
        fixture.scenes().await,
        before,
        "retry must not create or rewrite another scene"
    );
}

/// Ordinary overload must be refused immediately rather than queue behind a stalled caller.
/// Catches a semaphore created per request or admission applied only to mutation routes.
#[tokio::test]
async fn ordinary_requests_share_one_budget_across_routes() {
    let fixture = Fixture::new().await;
    let request = fixture.request(&json!({}));
    let (entered, started) = oneshot::channel();
    let body = Body::from_stream(futures::stream::once(async move {
        entered.send(()).unwrap();
        std::future::pending::<Result<bytes::Bytes, std::io::Error>>().await
    }));
    let (parts, _) = request.into_parts();
    let router = fixture.router.clone();
    let held = tokio::spawn(async move { router.oneshot(Request::from_parts(parts, body)).await.unwrap() });
    started.await.unwrap();
    let response = fixture
        .router
        .clone()
        .oneshot(
            Request::get("/xrpc/com.atproto.server.describeServer")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(held.await.unwrap().status(), StatusCode::GATEWAY_TIMEOUT);
    let response = fixture
        .router
        .oneshot(Request::get("/.well-known/did.json").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "deadline releases the ordinary permit"
    );
}

async fn handshake(address: std::net::SocketAddr) -> (TcpStream, String) {
    let mut socket = TcpStream::connect(address)
        .await
        .expect("the fixture server accepts the connection");
    socket.write_all(format!(
        "GET /xrpc/com.atproto.sync.subscribeRepos HTTP/1.1\r\nHost: {address}\r\nConnection: Upgrade\r\nUpgrade: websocket\r\nSec-WebSocket-Version: 13\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Protocol: xrpc.v0.cbor\r\n\r\n"
    ).as_bytes()).await.expect("the handshake request is written");
    let mut headers = Vec::new();
    while !headers.ends_with(b"\r\n\r\n") {
        headers.push(socket.read_u8().await.expect("the server answers the handshake"));
        assert!(headers.len() < 4096);
    }
    (
        socket,
        String::from_utf8(headers).expect("the handshake response is HTTP text"),
    )
}

async fn websocket_message(socket: &mut TcpStream) -> Vec<u8> {
    assert_eq!(
        socket.read_u8().await.expect("the frame has an opcode byte") & 0x0f,
        2,
        "binary firehose frame"
    );
    let length = socket.read_u8().await.expect("the frame has a length byte");
    assert_eq!(length & 0x80, 0, "server frames are unmasked");
    let length = match length & 0x7f {
        126 => u64::from(socket.read_u16().await.expect("the frame has a 16-bit length")),
        127 => socket.read_u64().await.expect("the frame has a 64-bit length"),
        length => u64::from(length),
    };
    assert!(length < 1024 * 1024, "bounded fixture frame");
    let length = usize::try_from(length).expect("a bounded frame length fits a usize");
    let mut bytes = vec![0; length];
    socket.read_exact(&mut bytes).await.expect("the frame body arrives");
    bytes
}

/// Relays retain a separate connection slot for the upgraded socket's lifetime.
/// Catches releasing the permit at the handshake or ending subscriptions at the HTTP deadline.
#[tokio::test]
async fn firehose_survives_the_ordinary_deadline_and_refuses_excess_live_connections() {
    let fixture = Fixture::new().await;
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let router = fixture.router.clone();
    let server = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    let (mut socket, headers) = handshake(address).await;
    assert!(headers.starts_with("HTTP/1.1 101 "), "{headers}");
    let (_refused, headers) = handshake(address).await;
    assert!(headers.starts_with("HTTP/1.1 503 "), "{headers}");

    // An ordinary call reaches its actual deadline while the stream remains live.
    // This tests elapsed deadline behavior without a sleep as synchronization.
    let request = fixture.request(&json!({}));
    let (parts, _) = request.into_parts();
    let body = Body::from_stream(futures::stream::pending::<Result<bytes::Bytes, std::io::Error>>());
    let since = tokio::time::Instant::now();
    let response = fixture
        .router
        .clone()
        .oneshot(Request::from_parts(parts, body))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::GATEWAY_TIMEOUT);
    assert!(since.elapsed() >= DEADLINE);
    fixture
        .handle
        .submit(shard::Entry::Write(shard::Write {
            mutations: Vec::new(),
            events: vec![motet_tenor::event::Event::Identity {
                did: Did::new(WORLD),
                handle: Some("after-deadline.motet.test".into()),
                time: "2026-10-26T00:00:00Z".into(),
            }],
        }))
        .await
        .unwrap()
        .unwrap();
    let bytes = tokio::time::timeout(Duration::from_secs(5), websocket_message(&mut socket))
        .await
        .unwrap();
    let frame = atproto_firehose::wire::split_frame(&bytes).unwrap();
    let atproto_firehose::wire::Event::Identity(event) = frame.event().unwrap() else {
        panic!("expected post-deadline identity event")
    };
    assert_eq!(event.did, WORLD);
    assert_eq!(event.handle.as_deref(), Some("after-deadline.motet.test"));
    server.abort();
}
