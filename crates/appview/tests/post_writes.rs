//! Posts cross the real AppView router, PostgreSQL outbox and tenor shard.
//! The PDS is a controllable HTTP boundary so failures and read-back lag can
//! be induced without replacing either service's admission decisions.

use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use atproto_identity::key::{KeyData, KeyType, generate_key, to_public};
use atproto_identity::model::Document;
use atproto_identity::traits::IdentityResolver;
use atproto_oauth::jwt::{Claims, Header, JoseClaims, mint};
use atproto_space::space_repo::OpAction;
use axum::Json;
use axum::body::{Body, to_bytes};
use axum::extract::{Query, Request, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use chrono::{Duration as ChronoDuration, Utc};
use motet_appview::MIGRATOR;
use motet_appview::api;
use motet_appview::auth::{
    self,
    pds::Grant,
    store::{SessionRef, SessionStore},
};
use motet_appview::outbox;
use motet_appview::writes::client::Client;
use motet_net::{Fetcher, Policy};
use motet_rules::world::Command;
use motet_rules::{Cid, Did, OperationId, SceneId, StrongRef};
use motet_tenor::admission::{FetchError, Fetched, Records};
use motet_tenor::identity::Hosting;
use motet_tenor::keys::{MasterKey, Purpose};
use motet_tenor::publication::{self, PublicationError};
use motet_tenor::repo::{self, Write};
use motet_tenor::shard::{Handle, Op, Options, Shard, Txn, spawn};
use motet_tenor::spaces::{self, ProfileInput, SpaceError, SpaceWrite};
use motet_tenor::store::keys;
use motet_tenor::world::{Status, WorldInfo};
use motet_xrpc::Caller;
use motet_xrpc::server::Authenticate;
use object_store::ObjectStore;
use object_store::memory::InMemory;
use serde_json::{Value, json};
use sqlx::PgPool;
use tokio::sync::watch;
use tokio::task::JoinHandle;
use tower::ServiceExt as _;

const WORLD: &str = "did:plc:ewvi7nxzyoun6zhxrhs64oiz";
const OWNER: &str = "did:web:owner.example";
const AUTHOR: &str = "did:web:alice.example";
const OTHER_AUTHOR: &str = "did:web:bob.example";
const SCENE: &str = "scene";
const ORIGIN: &str = "https://motet.example";
const SUBMIT: &str = "at.motet.pilot.play.submitAdmission";

fn document(did: &str, key: &KeyData) -> Document {
    let public = to_public(key).expect("public key").to_string();
    serde_json::from_value(json!({
        "id": did,
        "verificationMethod": [{"id": format!("{did}#atproto"), "type": "Multikey", "controller": did,
            "publicKeyMultibase": public.strip_prefix("did:key:").expect("did:key encoding")}]
    }))
    .expect("identity document")
}

#[derive(Clone)]
struct Directory(BTreeMap<String, Document>);

#[async_trait]
impl IdentityResolver for Directory {
    async fn resolve(&self, subject: &str) -> anyhow::Result<Document> {
        self.0
            .get(subject)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("unknown DID"))
    }
}

struct Initialize;

impl Op for Initialize {
    type Output = ();
    type Error = PublicationError;

    async fn run(self, txn: &mut Txn<'_>) -> Result<(), Self::Error> {
        let world = Did::new(WORLD);
        txn.begin_world(world.clone(), 1).expect("fresh World");
        let signing = txn.generate_key(&world, Purpose::Signing).expect("signing key");
        txn.generate_key(&world, Purpose::Kem).expect("KEM key");
        let info = WorldInfo {
            did: world.clone(),
            owner: Did::new(OWNER),
            handle: "world.motet.test".into(),
            epoch: 1,
            status: Status::Active,
            created: atproto_record::datetime::Datetime::now().into_string(),
            document: document(WORLD, &signing),
        };
        txn.put(keys::world_info(&world), serde_json::to_vec(&info)?.into());
        txn.put(keys::handle("world.motet.test"), WORLD.as_bytes().into());
        repo::genesis(txn, &world).await?;
        spaces::bootstrap(txn, &world, &Did::new(OWNER), ProfileInput::default()).await?;
        let members = spaces::world_space(&world, spaces::MEMBERS).expect("members Space");
        spaces::write_as_world(
            txn,
            &members,
            vec![
                SpaceWrite::create("group.opensocial.role", "player", &json!({"displayName":"Player"}))?,
                SpaceWrite::create(
                    "group.opensocial.membership",
                    AUTHOR,
                    &json!({"member":AUTHOR,"roles":["player"],"createdAt":"2026-10-26T00:00:00Z"}),
                )?,
                SpaceWrite::create(
                    "group.opensocial.membership",
                    OTHER_AUTHOR,
                    &json!({"member":OTHER_AUTHOR,"roles":["player"],"createdAt":"2026-10-26T00:00:00Z"}),
                )?,
            ],
        )
        .await?;
        let record = json!({"$type":"at.motet.pilot.scene","title":"A scene","state":"open",
            "cast":[OWNER,AUTHOR,OTHER_AUTHOR],"openedBy":OWNER,"createdAt":"2026-10-26T00:00:00Z"});
        repo::commit(
            txn,
            &world,
            vec![Write::Create {
                collection: "at.motet.pilot.scene".parse().expect("scene NSID"),
                rkey: SCENE.into(),
                record: repo::record_from_json(&record).expect("scene record"),
            }],
            None,
        )
        .await?;
        txn.command(
            world,
            OperationId::new("setup-scene"),
            Command::OpenScene {
                scene: SceneId::new(SCENE),
                cast: BTreeSet::from([Did::new(OWNER), Did::new(AUTHOR), Did::new(OTHER_AUTHOR)]),
            },
        )
        .await?
        .expect("scene opens");
        Ok(())
    }
}

struct RemoveAuthor;

impl Op for RemoveAuthor {
    type Output = ();
    type Error = SpaceError;

    async fn run(self, txn: &mut Txn<'_>) -> Result<(), Self::Error> {
        let members = spaces::world_space(&Did::new(WORLD), spaces::MEMBERS).expect("members Space");
        spaces::write_as_world(
            txn,
            &members,
            vec![SpaceWrite {
                action: OpAction::Delete,
                collection: "group.opensocial.membership".into(),
                rkey: AUTHOR.into(),
                value: None,
                swap: None,
            }],
        )
        .await?;
        Ok(())
    }
}

struct Network {
    key: KeyData,
    fail_pds: AtomicBool,
    lose_pds_response: AtomicBool,
    conflict_pds: AtomicBool,
    reads: AtomicUsize,
    fail_tenor: AtomicBool,
    tenor_faults: Mutex<VecDeque<StatusCode>>,
    failing_op: Mutex<Option<String>>,
    visible: AtomicBool,
    writes: AtomicUsize,
    tokens: AtomicUsize,
    token_ids: Mutex<Vec<String>>,
    records: Mutex<BTreeMap<String, Fetched>>,
    values: Mutex<BTreeMap<String, Value>>,
    submissions: Mutex<Vec<Value>>,
}

#[derive(Clone)]
struct AuthorRepo(Arc<Network>);

impl Records for AuthorRepo {
    fn fetch(&self, post: &StrongRef) -> impl Future<Output = Result<Option<Fetched>, FetchError>> + Send {
        let record = if self.0.visible.load(Ordering::SeqCst) {
            self.0.records.lock().expect("records").get(post.uri.as_str()).cloned()
        } else {
            None
        };
        std::future::ready(Ok(record))
    }
}

fn nonce(mut response: Response) -> Response {
    response
        .headers_mut()
        .insert("dpop-nonce", HeaderValue::from_static("nonce"));
    response
}

async fn create_record(State(network): State<Arc<Network>>, Json(input): Json<Value>) -> Response {
    network.writes.fetch_add(1, Ordering::SeqCst);
    if network.fail_pds.load(Ordering::SeqCst) {
        return nonce((StatusCode::SERVICE_UNAVAILABLE, Json(json!({"error":"Unavailable"}))).into_response());
    }
    let author = input.get("repo").and_then(Value::as_str).expect("repo");
    assert!(author == AUTHOR || author == OTHER_AUTHOR);
    assert_eq!(
        input.get("collection").and_then(Value::as_str),
        Some("at.motet.pilot.post")
    );
    let record = input.get("record").expect("record");
    assert_eq!(record.get("world").and_then(Value::as_str), Some(WORLD));
    assert_eq!(record.get("scene").and_then(Value::as_str), Some(SCENE));
    let rkey = input.get("rkey").and_then(Value::as_str).expect("create-only key");
    assert!(
        rkey.parse::<atproto_record::tid::Tid>().is_ok(),
        "post key must be a TID"
    );
    let uri = format!("at://{author}/at.motet.pilot.post/{rkey}");
    let cid =
        atproto_dasl::cid::compute_cid(&atproto_dasl::to_vec_from_json(record).expect("record DRISL")).to_string();
    if network.conflict_pds.swap(false, Ordering::SeqCst) {
        let mut existing = record.clone();
        existing["body"] = json!("Already here");
        let cid = atproto_dasl::cid::compute_cid(&atproto_dasl::to_vec_from_json(&existing).expect("record DRISL"))
            .to_string();
        network.records.lock().expect("records").insert(
            uri.clone(),
            Fetched {
                cid: Cid::new(cid),
                author: Did::new(author),
                world: Did::new(WORLD),
                scene: SceneId::new(SCENE),
            },
        );
        network.values.lock().expect("record values").insert(uri, existing);
        return nonce((StatusCode::BAD_REQUEST, Json(json!({"error":"RecordAlreadyExists"}))).into_response());
    }
    let mut records = network.records.lock().expect("records");
    if records.contains_key(&uri) {
        return nonce((StatusCode::BAD_REQUEST, Json(json!({"error":"RecordAlreadyExists"}))).into_response());
    }
    records.insert(
        uri.clone(),
        Fetched {
            cid: Cid::new(cid.clone()),
            author: Did::new(author),
            world: Did::new(WORLD),
            scene: SceneId::new(SCENE),
        },
    );
    network
        .values
        .lock()
        .expect("record values")
        .insert(uri.clone(), record.clone());
    if network.lose_pds_response.swap(false, Ordering::SeqCst) {
        let dropped = futures::stream::once(async {
            Err::<bytes::Bytes, _>(std::io::Error::new(
                std::io::ErrorKind::ConnectionReset,
                "lost response",
            ))
        });
        return nonce((StatusCode::OK, Body::from_stream(dropped)).into_response());
    }
    nonce(Json(json!({"uri":uri,"cid":cid})).into_response())
}

async fn get_record(State(network): State<Arc<Network>>, Query(query): Query<HashMap<String, String>>) -> Response {
    network.reads.fetch_add(1, Ordering::SeqCst);
    let author = query.get("repo").map(String::as_str).expect("repo");
    assert!(author == AUTHOR || author == OTHER_AUTHOR);
    assert_eq!(query.get("collection").map(String::as_str), Some("at.motet.pilot.post"));
    let rkey = query.get("rkey").expect("record key");
    let uri = format!("at://{author}/at.motet.pilot.post/{rkey}");
    let records = network.records.lock().expect("records");
    let values = network.values.lock().expect("record values");
    match (records.get(&uri), values.get(&uri)) {
        (Some(record), Some(value)) => {
            nonce(Json(json!({"uri":uri,"cid":record.cid.as_str(),"value":value})).into_response())
        }
        _ => nonce((StatusCode::BAD_REQUEST, Json(json!({"error":"RecordNotFound"}))).into_response()),
    }
}

fn token(key: &KeyData, author: &str, audience: &str, method: &str) -> (String, String) {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).expect("clock").as_secs();
    let jti = uuid::Uuid::now_v7().to_string();
    let mut claims = Claims::new(JoseClaims {
        issuer: Some(author.into()),
        audience: Some(audience.into()),
        issued_at: Some(now),
        json_web_token_id: Some(jti.clone()),
        expiration: Some(now + 60),
        ..JoseClaims::default()
    });
    claims.private.insert("lxm".into(), method.into());
    let signed = mint(key, &Header::try_from(key.clone()).expect("JWT header"), &claims).expect("service token");
    (signed, jti)
}

async fn service_token(
    State(network): State<Arc<Network>>,
    headers: axum::http::HeaderMap,
    Query(query): Query<HashMap<String, String>>,
) -> Response {
    network.tokens.fetch_add(1, Ordering::SeqCst);
    let audience = query.get("aud").expect("audience");
    let method = query.get("lxm").expect("method");
    assert_eq!(audience, &format!("{WORLD}#motet_tenor"));
    assert_eq!(method, SUBMIT);
    let author = match headers.get(header::AUTHORIZATION).and_then(|value| value.to_str().ok()) {
        Some("DPoP other-access-private") => OTHER_AUTHOR,
        Some("DPoP access-private" | "DPoP fresh") => AUTHOR,
        other => panic!("unexpected PDS access token: {other:?}"),
    };
    let (signed, jti) = token(&network.key, author, audience, method);
    network.token_ids.lock().expect("token IDs").push(jti);
    nonce(Json(json!({"token":signed})).into_response())
}

fn assert_distinct_tokens(network: &Network) {
    let ids = network.token_ids.lock().expect("token IDs");
    assert!(ids.len() >= 2, "a retry must ask the PDS for a fresh token");
    assert_eq!(ids.iter().collect::<BTreeSet<_>>().len(), ids.len());
}

#[derive(Clone)]
struct TenorProxy {
    router: axum::Router,
    network: Arc<Network>,
}

async fn forward(State(proxy): State<TenorProxy>, request: Request) -> Response {
    let (parts, body) = request.into_parts();
    let bytes = to_bytes(body, 16_384).await.expect("admission request");
    let value: Value = serde_json::from_slice(&bytes).expect("admission JSON");
    proxy
        .network
        .submissions
        .lock()
        .expect("submissions")
        .push(value.clone());
    if let Some(status) = proxy.network.tenor_faults.lock().expect("tenor faults").pop_front() {
        return (status, Json(json!({"error":"Unavailable"}))).into_response();
    }
    if let Some(op) = proxy.network.failing_op.lock().expect("failing op").as_deref()
        && value.get("op").and_then(Value::as_str) == Some(op)
    {
        return (StatusCode::SERVICE_UNAVAILABLE, Json(json!({"error":"Unavailable"}))).into_response();
    }
    if proxy.network.fail_tenor.load(Ordering::SeqCst) {
        return (StatusCode::SERVICE_UNAVAILABLE, Json(json!({"error": "Unavailable"}))).into_response();
    }
    proxy
        .router
        .clone()
        .oneshot(Request::from_parts(parts, Body::from(bytes)))
        .await
        .expect("tenor response")
}

async fn serve(router: axum::Router) -> (String, JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("listener");
    let address = listener.local_addr().expect("address");
    let task = tokio::spawn(async move {
        axum::serve(listener, router).await.expect("server");
    });
    (format!("http://{address}"), task)
}

struct Fixture {
    pool: PgPool,
    store: SessionStore,
    auth: Arc<auth::Server>,
    client: Arc<Client>,
    router: axum::Router,
    network: Arc<Network>,
    tenor: Handle,
    pds_url: String,
    tenor_url: String,
    cookie: String,
    pds_task: JoinHandle<()>,
    tenor_task: JoinHandle<()>,
}

impl Fixture {
    #[expect(
        clippy::too_many_lines,
        reason = "This fixture wires one real AppView, authenticated tenor shard, and controllable PDS boundary"
    )]
    async fn new(fail_pds: bool) -> Self {
        let pool = motet_testkit::services::database().await;
        MIGRATOR.run(&pool).await.expect("migrations");
        let key = generate_key(KeyType::K256Private).expect("author signing key");
        let network = Arc::new(Network {
            key,
            fail_pds: AtomicBool::new(fail_pds),
            lose_pds_response: AtomicBool::new(false),
            conflict_pds: AtomicBool::new(false),
            reads: AtomicUsize::new(0),
            fail_tenor: AtomicBool::new(false),
            tenor_faults: Mutex::new(VecDeque::new()),
            failing_op: Mutex::new(None),
            visible: AtomicBool::new(false),
            writes: AtomicUsize::new(0),
            tokens: AtomicUsize::new(0),
            token_ids: Mutex::new(Vec::new()),
            records: Mutex::new(BTreeMap::new()),
            values: Mutex::new(BTreeMap::new()),
            submissions: Mutex::new(Vec::new()),
        });
        let (pds_url, pds_task) = serve(
            axum::Router::new()
                .route("/xrpc/com.atproto.repo.createRecord", post(create_record))
                .route("/xrpc/com.atproto.repo.getRecord", get(get_record))
                .route("/xrpc/com.atproto.server.getServiceAuth", get(service_token))
                .with_state(Arc::clone(&network)),
        )
        .await;
        let bucket: Arc<dyn ObjectStore> = Arc::new(InMemory::new());
        let shard = Shard::open(bucket, "post-boundary", Arc::new(MasterKey::from_bytes(&[37; 32])))
            .await
            .expect("shard");
        let tenor = spawn(
            shard,
            Options {
                max_batch: 8,
                checkpoint_every: Duration::from_secs(3600),
            },
        );
        tenor.run(Initialize).await.expect("writer").expect("World setup");
        let identities: Arc<dyn IdentityResolver> = Arc::new(Directory(BTreeMap::from([
            (AUTHOR.into(), document(AUTHOR, &network.key)),
            (OTHER_AUTHOR.into(), document(OTHER_AUTHOR, &network.key)),
        ])));
        let fetcher = Fetcher::new(Policy::default().allow_private()).expect("network client");
        let hosting = Arc::new(
            Hosting::new(
                &"https://tenor.motet.test".parse().expect("tenor URL"),
                vec![".motet.test".into()],
                "https://plc.example".into(),
                fetcher.clone(),
                Vec::new(),
            )
            .expect("hosting"),
        );
        let audience = hosting.audience().to_owned();
        let authenticate = motet_xrpc::service_auth::authenticate(
            identities,
            Arc::new(move |aud| motet_tenor::api::accepts_audience(aud, &audience)),
        );
        let tenor_router = motet_tenor::api::router(
            Arc::new(motet_tenor::api::State {
                shard: tenor.clone(),
                records: AuthorRepo(Arc::clone(&network)),
                hosting,
            }),
            authenticate,
        )
        .expect("tenor router");
        let proxy = TenorProxy {
            router: tenor_router,
            network: Arc::clone(&network),
        };
        let (tenor_url, tenor_task) = serve(
            axum::Router::new()
                .route("/xrpc/at.motet.pilot.play.submitAdmission", post(forward))
                .with_state(proxy),
        )
        .await;
        let world_doc: Document = serde_json::from_value(json!({"id": WORLD, "service": [{
            "id":"#motet_tenor", "type":"MotetTenor", "serviceEndpoint":tenor_url,
        }]}))
        .expect("World DID document");
        let directory: Arc<dyn IdentityResolver> = Arc::new(Directory(BTreeMap::from([(WORLD.into(), world_doc)])));
        let client = Arc::new(
            Client::new(
                fetcher.clone(),
                Arc::clone(&directory),
                "https://tenor.example",
                "did:web:tenor.example",
            )
            .expect("tenor client"),
        );
        let files = tempfile::tempdir().expect("keys");
        let signing = files.path().join("signing");
        let sealing = files.path().join("sealing");
        std::fs::write(&signing, [1; 32]).expect("signing key");
        std::fs::write(&sealing, [2; 32]).expect("sealing key");
        let auth = auth::Server::new(
            pool.clone(),
            ORIGIN.into(),
            pds_url.clone(),
            &signing,
            &sealing,
            fetcher,
            directory,
        )
        .expect("OAuth server");
        let store = SessionStore::new(pool.clone(), [2; 32]);
        let grant = Grant {
            did: AUTHOR.into(),
            handle: "alice.example".into(),
            pds: pds_url.clone(),
            issuer: pds_url.clone(),
            token_endpoint: format!("{pds_url}/oauth/token"),
            revocation_endpoint: None,
            access_token: "access-private".into(),
            refresh_token: "refresh-private".into(),
            expires_at: Utc::now().timestamp() + 600,
            scope: "atproto".into(),
            dpop_private_key: vec![3; 32],
        };
        let cookie = store
            .create_session(&grant, Utc::now() + ChronoDuration::hours(1))
            .await
            .expect("session");
        let anonymous: Authenticate = Arc::new(|_| Box::pin(async { Ok(Caller::Anonymous) }));
        let router = api::router(
            Arc::new(api::State {
                pool: pool.clone(),
                auth: Arc::clone(&auth),
                client: Arc::clone(&client),
            }),
            auth.authenticate(anonymous),
        )
        .expect("AppView router");
        Self {
            pool,
            store,
            auth,
            client,
            router,
            network,
            tenor,
            pds_url,
            tenor_url,
            cookie,
            pds_task,
            tenor_task,
        }
    }

    fn post_request_as(&self, cookie: &str, op: &str, body: &str) -> Request {
        Request::builder()
            .method("POST")
            .uri("/xrpc/at.motet.pilot.createPost")
            .header(header::ORIGIN, ORIGIN)
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::COOKIE, format!("__Host-motet_session={cookie}"))
            .body(Body::from(
                json!({"op":op,"world":WORLD,"scene":SCENE,"body":body, "register":"in-character"}).to_string(),
            ))
            .expect("request")
    }

    fn post_request_with(&self, op: &str, body: &str) -> Request {
        self.post_request_as(&self.cookie, op, body)
    }

    fn post_request(&self) -> Request {
        self.post_request_with(&atproto_record::tid::Tid::new().to_string(), "Hello")
    }

    async fn post_with(&self, op: &str, body: &str) -> (StatusCode, Value) {
        let response = self
            .router
            .clone()
            .oneshot(self.post_request_with(op, body))
            .await
            .expect("AppView response");
        let status = response.status();
        let body = to_bytes(response.into_body(), 64 * 1024).await.expect("response body");
        (status, serde_json::from_slice(&body).expect("response JSON"))
    }

    async fn status(&self, uri: &str) -> (StatusCode, Value) {
        let request = Request::builder()
            .uri(format!("/xrpc/at.motet.pilot.getPostStatus?uri={uri}"))
            .header(header::COOKIE, format!("__Host-motet_session={}", self.cookie))
            .body(Body::empty())
            .expect("status request");
        let response = self.router.clone().oneshot(request).await.expect("status response");
        let status = response.status();
        let body = to_bytes(response.into_body(), 64 * 1024).await.expect("response body");
        (status, serde_json::from_slice(&body).expect("status JSON"))
    }

    async fn post(&self) -> (StatusCode, Value) {
        let response = self
            .router
            .clone()
            .oneshot(self.post_request())
            .await
            .expect("AppView response");
        let status = response.status();
        let body = to_bytes(response.into_body(), 64 * 1024).await.expect("response body");
        (status, serde_json::from_slice(&body).expect("response JSON"))
    }

    fn worker(&self) -> (watch::Sender<bool>, JoinHandle<()>) {
        let (sender, receiver) = watch::channel(false);
        let task = tokio::spawn(outbox::run(
            self.pool.clone(),
            Arc::clone(&self.auth),
            Arc::clone(&self.client),
            receiver,
        ));
        (sender, task)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        self.pds_task.abort();
        self.tenor_task.abort();
    }
}

async fn wait_for(pool: &PgPool, state: &str, require_paused: bool) -> (String, Value, bool) {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if let Some(row) = sqlx::query_as::<_, (String, Value, bool)>(
                "SELECT state, outcome, paused FROM service.outbox WHERE state = $1 AND (NOT $2 OR paused)",
            )
            .bind(state)
            .bind(require_paused)
            .fetch_optional(pool)
            .await
            .expect("outbox")
            {
                break row;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("outbox reached its expected state")
}

/// Missing or non-TID operations must be rejected before contacting the PDS.
/// Accepting a generic record key would defer failure to the real PDS.
#[tokio::test]
#[ignore = "integration: PostgreSQL"]
async fn invalid_post_operations_are_rejected_before_publication() {
    let fixture = Fixture::new(false).await;
    for input in [
        json!({"world":WORLD,"scene":SCENE,"body":"Hello","register":"in-character"}),
        json!({"op":uuid::Uuid::now_v7().to_string(),"world":WORLD,"scene":SCENE,"body":"Hello","register":"in-character"}),
    ] {
        let request = Request::builder()
            .method("POST")
            .uri("/xrpc/at.motet.pilot.createPost")
            .header(header::ORIGIN, ORIGIN)
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::COOKIE, format!("__Host-motet_session={}", fixture.cookie))
            .body(Body::from(input.to_string()))
            .expect("request");
        let response = fixture.router.clone().oneshot(request).await.expect("AppView response");
        let status = response.status();
        let body = to_bytes(response.into_body(), 64 * 1024).await.expect("response body");
        let answer: Value = serde_json::from_slice(&body).expect("response JSON");
        assert_eq!(status, StatusCode::BAD_REQUEST, "{answer}");
        assert_eq!(answer["error"], "InvalidRequest");
    }
    assert_eq!(fixture.network.writes.load(Ordering::SeqCst), 0);
    assert_eq!(fixture.network.tokens.load(Ordering::SeqCst), 0);
    assert!(fixture.network.submissions.lock().expect("submissions").is_empty());
    let jobs: i64 = sqlx::query_scalar("SELECT count(*) FROM service.outbox")
        .fetch_one(&fixture.pool)
        .await
        .expect("outbox");
    assert_eq!(jobs, 0);
}

/// A lost browser response must not create a second record or admission when
/// the client retries; accepting changed content would silently misattribute it.
#[tokio::test]
#[ignore = "integration: PostgreSQL"]
async fn fault_lost_appview_response_reuses_post_and_rejects_changed_content() {
    let fixture = Fixture::new(false).await;
    fixture.network.visible.store(true, Ordering::SeqCst);
    let op = atproto_record::tid::Tid::new().to_string();
    let response = fixture
        .router
        .clone()
        .oneshot(fixture.post_request_with(&op, "Hello"))
        .await
        .expect("AppView response");
    assert_eq!(response.status(), StatusCode::OK);
    drop(response);
    let uri = fixture
        .network
        .records
        .lock()
        .expect("records")
        .keys()
        .next()
        .expect("published record")
        .clone();
    assert_eq!(uri, format!("at://{AUTHOR}/at.motet.pilot.post/{op}"));
    let (status, original) = fixture.status(&uri).await;
    assert_eq!(status, StatusCode::OK, "{original}");
    assert_eq!(original["status"], "admitted");
    let (status, retried) = fixture.post_with(&op, "Hello").await;
    assert_eq!(status, StatusCode::OK, "{retried}");
    assert_eq!(retried, original);
    let (status, conflict) = fixture.post_with(&op, "Changed").await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{conflict}");
    assert_eq!(conflict["error"], "InvalidRequest");
    assert_eq!(fixture.network.writes.load(Ordering::SeqCst), 1);
    assert_eq!(fixture.network.records.lock().expect("records").len(), 1);
    assert_eq!(fixture.network.submissions.lock().expect("submissions").len(), 1);
    assert_eq!(
        publication::drain(&fixture.tenor).await.expect("publication").published,
        1
    );
}

/// Different authors may independently choose the same client operation.
/// Global deduplication would reject the second post or return the first author's URI.
#[tokio::test]
#[ignore = "integration: PostgreSQL"]
async fn same_client_operation_is_isolated_by_author_through_admission() {
    let fixture = Fixture::new(false).await;
    fixture.network.visible.store(true, Ordering::SeqCst);
    let mut other_grant = fixture
        .store
        .get_session::<Grant>(&fixture.cookie)
        .await
        .expect("read session")
        .expect("live session")
        .grant;
    other_grant.did = OTHER_AUTHOR.into();
    other_grant.handle = "bob.example".into();
    other_grant.access_token = "other-access-private".into();
    let other_cookie = fixture
        .store
        .create_session(&other_grant, Utc::now() + ChronoDuration::hours(1))
        .await
        .expect("other author session");
    let op = atproto_record::tid::Tid::new().to_string();
    let (status, first) = fixture.post_with(&op, "Alice's post").await;
    assert_eq!(status, StatusCode::OK, "{first}");
    assert_eq!(first["status"], "admitted");
    let response = fixture
        .router
        .clone()
        .oneshot(fixture.post_request_as(&other_cookie, &op, "Bob's post"))
        .await
        .expect("other author's response");
    let status = response.status();
    let body = to_bytes(response.into_body(), 64 * 1024).await.expect("response body");
    let second: Value = serde_json::from_slice(&body).expect("post response JSON");
    assert_eq!(status, StatusCode::OK, "{second}");
    assert_eq!(second["status"], "admitted");
    let first_uri = first["uri"].as_str().expect("first URI");
    let second_uri = second["uri"].as_str().expect("second URI");
    assert_eq!(first_uri, format!("at://{AUTHOR}/at.motet.pilot.post/{op}"));
    assert_eq!(second_uri, format!("at://{OTHER_AUTHOR}/at.motet.pilot.post/{op}"));
    assert_ne!(first["cid"], second["cid"]);
    assert_eq!(fixture.network.records.lock().expect("records").len(), 2);
    assert_eq!(
        publication::drain(&fixture.tenor).await.expect("publication").published,
        2
    );
}

/// A create-only key can already belong to a different record. Blindly
/// accepting its read-back would admit content the author did not submit.
#[tokio::test]
#[ignore = "integration: PostgreSQL"]
async fn fault_existing_pds_key_with_different_content_is_never_admitted() {
    let fixture = Fixture::new(false).await;
    fixture.network.visible.store(true, Ordering::SeqCst);
    let op = atproto_record::tid::Tid::new().to_string();
    fixture.network.conflict_pds.store(true, Ordering::SeqCst);
    let (status, answer) = fixture.post_with(&op, "Different content").await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{answer}");
    assert_eq!(answer["error"], "InvalidRequest");
    assert_eq!(fixture.network.writes.load(Ordering::SeqCst), 1);
    assert!(fixture.network.reads.load(Ordering::SeqCst) >= 1);
    assert_eq!(fixture.network.records.lock().expect("records").len(), 1);
    let values = fixture.network.values.lock().expect("record values");
    assert_eq!(values.values().next().expect("existing value")["body"], "Already here");
    drop(values);
    assert!(fixture.network.submissions.lock().expect("submissions").is_empty());
    assert_eq!(
        publication::drain(&fixture.tenor).await.expect("publication").published,
        0
    );
}

/// A PDS may commit before dropping its response. Without deterministic
/// read-back, a retry can lose the post or publish a second record.
#[tokio::test]
#[ignore = "integration: PostgreSQL"]
async fn fault_lost_pds_response_recovers_committed_record_by_key() {
    let fixture = Fixture::new(false).await;
    fixture.network.visible.store(true, Ordering::SeqCst);
    fixture.network.lose_pds_response.store(true, Ordering::SeqCst);
    let op = atproto_record::tid::Tid::new().to_string();
    let (status, answer) = fixture.post_with(&op, "Hello").await;
    assert_eq!(status, StatusCode::OK, "{answer}");
    assert_eq!(answer["status"], "admitted");
    let uri = answer["uri"].as_str().expect("post URI");
    assert_eq!(uri, format!("at://{AUTHOR}/at.motet.pilot.post/{op}"));
    assert!(fixture.network.records.lock().expect("records").contains_key(uri));
    assert!(fixture.network.reads.load(Ordering::SeqCst) >= 1);
    assert_eq!(fixture.network.records.lock().expect("records").len(), 1);
    assert_eq!(fixture.network.writes.load(Ordering::SeqCst), 1);
    let (_, retried) = fixture.post_with(&op, "Hello").await;
    assert_eq!(retried, answer);
    assert_eq!(
        publication::drain(&fixture.tenor).await.expect("publication").published,
        1
    );
}

/// The tenor can temporarily reject a valid token or be unavailable.
/// Treating its 401 as a revoked PDS session would pause the post forever.
#[tokio::test]
#[ignore = "integration: PostgreSQL"]
async fn fault_tenor_503_then_401_retries_with_fresh_tokens_without_pausing() {
    let fixture = Fixture::new(false).await;
    fixture.network.visible.store(true, Ordering::SeqCst);
    *fixture.network.tenor_faults.lock().expect("faults") =
        VecDeque::from([StatusCode::SERVICE_UNAVAILABLE, StatusCode::UNAUTHORIZED]);
    let (_, initial) = fixture.post().await;
    assert_eq!(initial["status"], "pending");
    let (stop, worker) = fixture.worker();
    tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            let (state, paused): (String, bool) = sqlx::query_as("SELECT state, paused FROM service.outbox")
                .fetch_one(&fixture.pool).await.expect("outbox");
            assert!(!paused, "tenor errors must not pause a usable PDS session");
            if state == "done" { break; }
            assert_eq!(state, "queued");
            sqlx::query("UPDATE service.outbox SET next_attempt_at = clock_timestamp() WHERE state = 'queued' AND lease_owner IS NULL")
                .execute(&fixture.pool).await.expect("make retry due");
            tokio::task::yield_now().await;
        }
    }).await.expect("transient failures recover");
    stop.send(true).expect("stop worker");
    worker.await.expect("worker exits");
    let (status, outcome) = fixture.status(initial["uri"].as_str().expect("post URI")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(outcome["status"], "admitted");
    assert_eq!(outcome["cid"], initial["cid"]);
    assert_distinct_tokens(&fixture.network);
    let calls = fixture.network.submissions.lock().expect("submissions");
    assert_eq!(calls.len(), 3);
    assert!(calls.iter().all(|call| call == &calls[0]));
    drop(calls);
    assert_eq!(
        publication::drain(&fixture.tenor).await.expect("publication").published,
        1
    );
}

/// A permanently unavailable admission cannot block subsequent posts forever.
/// The author must see a failure reason, not an endless pending status.
#[tokio::test]
#[ignore = "integration: PostgreSQL"]
async fn fault_exhausted_admission_exposes_reason_and_unblocks_next_post() {
    let fixture = Fixture::new(false).await;
    fixture.network.visible.store(true, Ordering::SeqCst);
    let op = atproto_record::tid::Tid::new().to_string();
    *fixture.network.failing_op.lock().expect("failing op") = Some(op.clone());
    let (_, first) = fixture.post_with(&op, "First").await;
    assert_eq!(first["status"], "pending");
    let (_, second) = fixture
        .post_with(&atproto_record::tid::Tid::new().to_string(), "Second")
        .await;
    assert_eq!(second["status"], "pending");
    let (stop, worker) = fixture.worker();
    tokio::time::timeout(Duration::from_secs(20), async {
        loop {
            let rows: Vec<(String, bool)> = sqlx::query_as("SELECT state, paused FROM service.outbox ORDER BY sequence")
                .fetch_all(&fixture.pool).await.expect("outbox");
            assert!(rows.iter().all(|row| !row.1), "tenor failures must not pause the session");
            if rows.len() == 2 && rows[0].0 == "failed" && rows[1].0 == "done" { break; }
            sqlx::query("UPDATE service.outbox SET next_attempt_at = clock_timestamp() WHERE state = 'queued' AND lease_owner IS NULL")
                .execute(&fixture.pool).await.expect("make retries due");
            tokio::task::yield_now().await;
        }
    }).await.expect("failed post releases the queue");
    stop.send(true).expect("stop worker");
    worker.await.expect("worker exits");
    let (status, failed) = fixture.status(first["uri"].as_str().expect("first URI")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(failed["status"], "failed");
    assert_eq!(failed["cid"], first["cid"]);
    assert!(
        failed["reason"].as_str().is_some_and(|reason| !reason.is_empty()),
        "{failed}"
    );
    let (_, admitted) = fixture.status(second["uri"].as_str().expect("second URI")).await;
    assert_eq!(admitted["status"], "admitted");
    assert_eq!(admitted["cid"], second["cid"]);
    assert_distinct_tokens(&fixture.network);
    {
        let calls = fixture.network.submissions.lock().expect("submissions");
        let failed_calls: Vec<_> = calls.iter().filter(|call| call["op"] == op).collect();
        assert_eq!(failed_calls.len(), 6, "the retry budget must be finite");
        assert!(failed_calls.iter().all(|call| *call == failed_calls[0]));
    }
    assert_eq!(
        publication::drain(&fixture.tenor).await.expect("publication").published,
        1
    );
}

/// An uncertain PDS failure cannot cause a phantom admission or discard the
/// operation. It must recover after the PDS is available again.
#[tokio::test]
#[ignore = "integration: PostgreSQL"]
async fn fault_uncertain_pds_failure_waits_for_recovery_without_phantom_admission() {
    let fixture = Fixture::new(true).await;
    let (status, answer) = fixture.post().await;
    assert_eq!(status, StatusCode::BAD_GATEWAY, "{answer}");
    assert_eq!(answer["error"], "UpstreamFailure");
    let (state, paused): (String, bool) = sqlx::query_as("SELECT state, paused FROM service.outbox")
        .fetch_one(&fixture.pool)
        .await
        .expect("job state");
    assert_eq!(state, "queued");
    assert!(!paused);
    assert!(fixture.network.writes.load(Ordering::SeqCst) >= 1);
    assert!(fixture.network.records.lock().expect("records").is_empty());
    assert_eq!(fixture.network.tokens.load(Ordering::SeqCst), 0);
    assert!(fixture.network.submissions.lock().expect("submissions").is_empty());
    let row: (Value, Option<Value>, Vec<u8>) =
        sqlx::query_as("SELECT payload, outcome, session_id FROM service.outbox")
            .fetch_one(&fixture.pool)
            .await
            .expect("job");
    assert!(!row.0.to_string().contains(&fixture.cookie));
    assert!(!format!("{:?}", row.1).contains(&fixture.cookie));
    assert_eq!(
        row.2.as_slice(),
        SessionRef::from_cookie(&fixture.cookie).expect("reference").as_bytes()
    );
    assert_eq!(
        publication::drain(&fixture.tenor).await.expect("publication").published,
        0
    );
    fixture.network.fail_pds.store(false, Ordering::SeqCst);
    fixture.network.visible.store(true, Ordering::SeqCst);
    sqlx::query("UPDATE service.outbox SET next_attempt_at = clock_timestamp()")
        .execute(&fixture.pool)
        .await
        .expect("make recovery due");
    let (stop, worker) = fixture.worker();
    let (_, outcome, paused) = wait_for(&fixture.pool, "done", false).await;
    assert!(!paused);
    assert_eq!(outcome["status"], "admitted");
    stop.send(true).expect("stop worker");
    worker.await.expect("worker exits");
    assert_eq!(fixture.network.records.lock().expect("records").len(), 1);
    assert_eq!(
        publication::drain(&fixture.tenor).await.expect("publication").published,
        1
    );
}

/// A busy cross-instance refresh lock is not a failed PDS write. The queued
/// post must wait without sending anything, then publish once the grant is
/// refreshed; otherwise ordinary session contention loses an author's post.
#[tokio::test]
#[ignore = "integration: PostgreSQL"]
async fn fault_busy_session_retries_the_unsubmitted_pds_write() {
    let mut fixture = Fixture::new(false).await;
    let mut grant = fixture
        .store
        .get_session::<Grant>(&fixture.cookie)
        .await
        .expect("read grant")
        .expect("live grant")
        .grant;
    grant.expires_at = Utc::now().timestamp() - 1;
    fixture.cookie = fixture
        .store
        .create_session(&grant, Utc::now() + ChronoDuration::hours(1))
        .await
        .expect("expired access token session");
    let reference = SessionRef::from_cookie(&fixture.cookie).expect("session reference");
    let advisory_key: i64 = sqlx::query_scalar("SELECT advisory_key FROM service.oauth_session_locks WHERE id = $1")
        .bind(reference.as_bytes())
        .fetch_one(&fixture.pool)
        .await
        .expect("session lock key");
    let [first, second, third, fourth, fifth, sixth, seventh, eighth] = advisory_key.to_be_bytes();
    let mut lock = fixture.pool.begin().await.expect("lock transaction");
    sqlx::query("SELECT pg_advisory_xact_lock($1, $2)")
        .bind(i32::from_be_bytes([first, second, third, fourth]))
        .bind(i32::from_be_bytes([fifth, sixth, seventh, eighth]))
        .execute(&mut *lock)
        .await
        .expect("hold another instance's session lock");
    let request = fixture.post_request();
    let router = fixture.router.clone();
    let request_task = tokio::spawn(async move { router.oneshot(request).await.expect("AppView response") });
    tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            // Park the retry until this test's other instance refreshes the grant.
            let parked: Option<uuid::Uuid> = sqlx::query_scalar(
                "UPDATE service.outbox SET next_attempt_at = clock_timestamp() + interval '1 hour' \
                 WHERE state = 'queued' AND payload->>'stage' = 'publish' AND attempts > 0 \
                   AND lease_owner IS NULL RETURNING id",
            )
            .fetch_optional(&fixture.pool)
            .await
            .expect("park unpublished retry");
            if parked.is_some() {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("busy session rescheduled the post");
    assert_eq!(fixture.network.writes.load(Ordering::SeqCst), 0);
    assert!(fixture.network.submissions.lock().expect("submissions").is_empty());
    assert!(
        !request_task.is_finished(),
        "contention must not return a false PDS failure"
    );
    lock.commit().await.expect("release session lock");
    fixture
        .store
        .refresh_by_ref(&reference, 0, |mut grant: Grant| async move {
            grant.expires_at = Utc::now().timestamp() + 600;
            Ok::<_, ()>(grant)
        })
        .await
        .expect("other instance refreshes grant")
        .expect("session remains live");
    fixture.network.visible.store(true, Ordering::SeqCst);
    sqlx::query("UPDATE service.outbox SET next_attempt_at = clock_timestamp()")
        .execute(&fixture.pool)
        .await
        .expect("make retry due");
    let response = tokio::time::timeout(Duration::from_secs(10), request_task)
        .await
        .expect("post completes")
        .expect("request task");
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 64 * 1024).await.expect("body")).expect("post response");
    assert_eq!(body["status"], "admitted");
    assert_eq!(fixture.network.writes.load(Ordering::SeqCst), 1);
    assert_eq!(
        publication::drain(&fixture.tenor).await.expect("publication").published,
        1
    );
}

/// A verified nonmember receives a final refusal, not a pending retry. A 403
/// mistaken for a transient outage would otherwise block this account forever.
#[tokio::test]
#[ignore = "integration: PostgreSQL"]
async fn removed_member_gets_forbidden_refusal_without_a_publication() {
    let fixture = Fixture::new(false).await;
    fixture
        .tenor
        .run(RemoveAuthor)
        .await
        .expect("writer")
        .expect("remove member");
    fixture.network.visible.store(true, Ordering::SeqCst);
    let (status, answer) = fixture.post().await;
    assert_eq!(status, StatusCode::OK, "{answer}");
    assert_eq!(answer["status"], "refused");
    assert_eq!(answer["reason"], "Forbidden");
    let (_, outcome, paused) = wait_for(&fixture.pool, "done", false).await;
    assert_eq!(outcome, answer);
    assert!(!paused);
    assert_eq!(
        publication::drain(&fixture.tenor).await.expect("publication").published,
        0
    );
}

/// A tenor outage must not prevent later PDS writes, and recovery must submit
/// admissions in posting order. Otherwise one pending post can stall or reorder
/// everything the author writes during the outage.
#[tokio::test]
#[ignore = "integration: PostgreSQL"]
async fn fault_tenor_outage_preserves_later_posts_and_admits_them_in_order() {
    let fixture = Fixture::new(false).await;
    fixture.network.visible.store(true, Ordering::SeqCst);
    fixture.network.fail_tenor.store(true, Ordering::SeqCst);
    let (status, first) = fixture.post().await;
    assert_eq!(status, StatusCode::OK, "{first}");
    assert_eq!(first["status"], "pending");
    let (status, second) = fixture.post().await;
    assert_eq!(status, StatusCode::OK, "{second}");
    assert_eq!(second["status"], "pending");
    assert_ne!(first["uri"], second["uri"]);
    assert_eq!(fixture.network.writes.load(Ordering::SeqCst), 2);
    {
        let calls = fixture.network.submissions.lock().expect("submissions");
        assert_eq!(calls.len(), 1, "the second admission cannot overtake the first");
        assert_eq!(calls[0]["post"]["uri"], first["uri"]);
    }
    let queued: Vec<(String, Value)> =
        sqlx::query_as("SELECT op_id, payload FROM service.outbox WHERE state = 'queued' ORDER BY sequence")
            .fetch_all(&fixture.pool)
            .await
            .expect("queued posts");
    assert_eq!(queued.len(), 2);
    assert_eq!(queued[0].1["uri"], first["uri"]);
    assert_eq!(queued[1].1["uri"], second["uri"]);
    fixture.network.fail_tenor.store(false, Ordering::SeqCst);
    sqlx::query("UPDATE service.outbox SET next_attempt_at = clock_timestamp()")
        .execute(&fixture.pool)
        .await
        .expect("make retries due");
    let (stop, worker) = fixture.worker();
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let count: i64 = sqlx::query_scalar("SELECT count(*) FROM service.outbox WHERE state = 'done'")
                .fetch_one(&fixture.pool)
                .await
                .expect("finished jobs");
            if count == 2 {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("both admissions complete");
    stop.send(true).expect("stop worker");
    worker.await.expect("worker exits");
    assert_distinct_tokens(&fixture.network);
    let outcomes: Vec<Value> = sqlx::query_scalar("SELECT outcome FROM service.outbox ORDER BY sequence")
        .fetch_all(&fixture.pool)
        .await
        .expect("outcomes");
    assert_eq!(outcomes.len(), 2);
    assert_eq!(outcomes[0]["status"], "admitted");
    assert_eq!(outcomes[1]["status"], "admitted");
    assert_eq!(outcomes[0]["uri"], first["uri"]);
    assert_eq!(outcomes[1]["uri"], second["uri"]);
    assert_eq!(outcomes[0]["epoch"], outcomes[1]["epoch"]);
    assert!(outcomes[0]["seq"].as_u64().expect("first place") < outcomes[1]["seq"].as_u64().expect("second place"));
    {
        let calls = fixture.network.submissions.lock().expect("submissions");
        assert_eq!(calls.len(), 3);
        assert_eq!(calls[0], calls[1], "the first retry keeps its exact operation");
        assert_eq!(calls[1]["op"], queued[0].0);
        assert_eq!(calls[2]["op"], queued[1].0);
        assert_eq!(calls[2]["post"]["uri"], second["uri"]);
        assert_eq!(calls[2]["post"]["cid"], second["cid"]);
    }
    assert_eq!(
        publication::drain(&fixture.tenor).await.expect("publication").published,
        2
    );
}

/// A lost admission response followed by lease takeover and restart must use
/// the same operation and exact post version; otherwise two World places can
/// be published for one author's post.
#[tokio::test]
#[ignore = "integration: PostgreSQL"]
async fn fault_pending_then_lost_response_survives_takeover_and_admits_once() {
    let fixture = Fixture::new(false).await;
    let (status, first) = fixture.post().await;
    assert_eq!(status, StatusCode::OK, "{first}");
    assert_eq!(first["status"], "pending");
    let original = fixture.network.submissions.lock().expect("submissions")[0].clone();
    assert_eq!(original["post"]["uri"], first["uri"]);
    assert_eq!(original["post"]["cid"], first["cid"]);
    sqlx::query("UPDATE service.outbox SET next_attempt_at = clock_timestamp()")
        .execute(&fixture.pool)
        .await
        .expect("due");
    let stale = outbox::claim(&fixture.pool, "crashed-worker", 1, Duration::from_secs(60))
        .await
        .expect("claim");
    assert_eq!(stale.len(), 1);
    fixture.network.visible.store(true, Ordering::SeqCst);
    let lost = reqwest::Client::new()
        .post(format!("{}/xrpc/{SUBMIT}", fixture.tenor_url))
        .bearer_auth(token(&fixture.network.key, AUTHOR, &format!("{WORLD}#motet_tenor"), SUBMIT).0)
        .json(&original)
        .send()
        .await
        .expect("admission reached tenor");
    assert_eq!(lost.status(), StatusCode::OK);
    let lost: Value = lost.json().await.expect("response");
    assert_eq!(lost["status"], "admitted");
    sqlx::query("UPDATE service.outbox SET lease_until = clock_timestamp() - interval '1 second'")
        .execute(&fixture.pool)
        .await
        .expect("expired lease");
    let (stop, worker) = fixture.worker();
    let (_, outcome, _) = wait_for(&fixture.pool, "done", false).await;
    assert_eq!(outcome["status"], "admitted");
    assert_eq!(outcome["uri"], first["uri"]);
    assert_eq!(outcome["cid"], first["cid"]);
    assert!(
        !outbox::finish(
            &fixture.pool,
            &stale[0],
            "crashed-worker",
            false,
            &json!({"status":"wrong"})
        )
        .await
        .expect("fencing")
    );
    stop.send(true).expect("stop worker");
    worker.await.expect("worker exits");
    assert_distinct_tokens(&fixture.network);
    {
        let submissions = fixture.network.submissions.lock().expect("submissions");
        assert_eq!(submissions.len(), 3);
        assert!(submissions.iter().all(|body| body == &original));
        drop(submissions);
    }
    let published = publication::drain(&fixture.tenor).await.expect("publication");
    assert_eq!(published.published, 1);
    assert_eq!(
        fixture
            .tenor
            .reader()
            .scan(
                &keys::repo_records(&Did::new(WORLD), Some("at.motet.pilot.admission")),
                None,
                usize::MAX
            )
            .await
            .expect("World admissions")
            .len(),
        1
    );
}

/// Sign-out must stop service-token use without losing an uncertain admission;
/// only a fresh session for the same DID can resume the queued operation.
#[tokio::test]
#[ignore = "integration: PostgreSQL"]
async fn revoked_session_pauses_until_same_account_signs_in_again() {
    let fixture = Fixture::new(false).await;
    let (_, initial) = fixture.post().await;
    assert_eq!(initial["status"], "pending");
    let first_request = fixture.network.submissions.lock().expect("submissions")[0].clone();
    fixture.store.revoke(&fixture.cookie).await.expect("sign out");
    sqlx::query("UPDATE service.outbox SET next_attempt_at = clock_timestamp()")
        .execute(&fixture.pool)
        .await
        .expect("due");
    let tokens_before = fixture.network.tokens.load(Ordering::SeqCst);
    let (stop, worker) = fixture.worker();
    let (state, _, paused) = wait_for(&fixture.pool, "queued", true).await;
    assert_eq!(state, "queued");
    assert!(paused);
    assert_eq!(fixture.network.tokens.load(Ordering::SeqCst), tokens_before);
    assert_eq!(
        outbox::claim(&fixture.pool, "other", 1, Duration::from_secs(60))
            .await
            .expect("claim"),
        [] as [outbox::Job; 0]
    );
    let old_reference = SessionRef::from_cookie(&fixture.cookie).expect("reference");
    assert!(
        fixture
            .store
            .get_session_by_ref::<Grant>(&old_reference)
            .await
            .expect("revoked lookup")
            .is_none()
    );
    outbox::resume_account(&fixture.pool, "did:web:other.example", &old_reference)
        .await
        .expect("other author");
    let paused: bool = sqlx::query_scalar("SELECT paused FROM service.outbox")
        .fetch_one(&fixture.pool)
        .await
        .expect("paused");
    assert!(paused);
    let existing = Grant {
        did: AUTHOR.into(),
        handle: "alice.example".into(),
        pds: fixture.pds_url.clone(),
        issuer: fixture.pds_url.clone(),
        token_endpoint: format!("{}/oauth/token", fixture.pds_url),
        revocation_endpoint: None,
        access_token: "fresh".into(),
        refresh_token: "fresh-refresh".into(),
        expires_at: Utc::now().timestamp() + 600,
        scope: "atproto".into(),
        dpop_private_key: vec![3; 32],
    };
    let fresh_cookie = fixture
        .store
        .create_session(&existing, Utc::now() + ChronoDuration::hours(1))
        .await
        .expect("new session");
    let fresh = SessionRef::from_cookie(&fresh_cookie).expect("reference");
    fixture.network.visible.store(true, Ordering::SeqCst);
    outbox::resume_account(&fixture.pool, AUTHOR, &fresh)
        .await
        .expect("same author");
    let (_, outcome, _) = wait_for(&fixture.pool, "done", false).await;
    assert_eq!(outcome["status"], "admitted");
    {
        let submissions = fixture.network.submissions.lock().expect("submissions");
        assert!(submissions.iter().all(|request| request == &first_request));
        drop(submissions);
    }
    stop.send(true).expect("stop");
    worker.await.expect("worker exits");
}

/// A lease owner may be reused after a restart. Its old attempt must not
/// complete the new lease or unblock a later write of the same account.
#[tokio::test]
#[ignore = "integration: PostgreSQL"]
async fn expired_generation_cannot_finish_the_same_owners_new_lease() {
    let pool = motet_testkit::services::database().await;
    MIGRATOR.run(&pool).await.expect("migrations");
    outbox::enqueue(&pool, AUTHOR, "first", "test", &json!({}))
        .await
        .expect("first");
    outbox::enqueue(&pool, AUTHOR, "second", "test", &json!({}))
        .await
        .expect("second");
    let first = outbox::claim(&pool, "worker", 1, Duration::from_secs(60))
        .await
        .expect("claim");
    assert_eq!(first.len(), 1);
    sqlx::query(
        "UPDATE service.outbox SET lease_until = clock_timestamp() - interval '1 second' WHERE op_id = 'first'",
    )
    .execute(&pool)
    .await
    .expect("expire lease");
    let replacement = outbox::claim(&pool, "worker", 1, Duration::from_secs(60))
        .await
        .expect("takeover");
    assert_eq!(replacement.len(), 1);
    assert_eq!(replacement[0].op_id, "first");
    assert!(
        !outbox::finish(&pool, &first[0], "worker", false, &json!({"status":"stale"}))
            .await
            .expect("stale finish")
    );
    assert_eq!(
        outbox::claim(&pool, "another-worker", 1, Duration::from_secs(60))
            .await
            .expect("blocked"),
        [] as [outbox::Job; 0]
    );
    assert!(
        outbox::finish(&pool, &replacement[0], "worker", false, &json!({"status":"done"}))
            .await
            .expect("current finish")
    );
    let next = outbox::claim(&pool, "another-worker", 1, Duration::from_secs(60))
        .await
        .expect("next");
    assert_eq!(next.len(), 1);
    assert_eq!(next[0].op_id, "second");
}
