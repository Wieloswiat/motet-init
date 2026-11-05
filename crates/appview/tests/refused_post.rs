//! A current member outside a scene's cast cannot make a public post count.
//!
//! The author's PDS is a small HTTP fixture which stores the actual record and
//! signs service tokens. AppView authentication, tenor authorization, rules,
//! publication, archive and public reads use their production implementations.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use atproto_identity::key::{KeyData, KeyType, generate_key, to_public};
use atproto_identity::model::Document;
use atproto_identity::traits::IdentityResolver;
use atproto_oauth::jwt::{Claims, Header, JoseClaims, mint};
use axum::Json;
use axum::body::{Body, to_bytes};
use axum::extract::{Query, State};
use axum::http::{HeaderMap, HeaderValue, Request, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use chrono::{Duration as ChronoDuration, Utc};
use motet_appview::api;
use motet_appview::archive::Archive;
use motet_appview::auth::pds::Grant;
use motet_appview::auth::store::SessionStore;
use motet_appview::event::{Action, Event, RecordEvent};
use motet_appview::indexer::Indexer;
use motet_appview::{MIGRATOR, auth, writes};
use motet_net::{Fetcher, Policy};
use motet_rules::world::Command;
use motet_rules::{Did, OperationId, SceneId};
use motet_tenor::identity::Hosting;
use motet_tenor::keys::{MasterKey, Purpose};
use motet_tenor::publication::PublicationError;
use motet_tenor::repo::{self, Write};
use motet_tenor::shard::{Op, Options, Shard, Txn, spawn};
use motet_tenor::spaces::{self, ProfileInput, SpaceWrite};
use motet_tenor::store::keys;
use motet_tenor::world::{Status, WorldInfo};
use object_store::memory::InMemory;
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::sync::Mutex;
use tower::ServiceExt as _;
use url::Url;

const WORLD: &str = "did:plc:ewvi7nxzyoun6zhxrhs64oiz";
const OWNER: &str = "did:plc:aaaaaaaaaaaaaaaaaaaaaaaa";
const AUTHOR: &str = "did:plc:bbbbbbbbbbbbbbbbbbbbbbbb";
const ORIGIN: &str = "https://motet.example";
const SCENE: &str = "room";
const SCENE_COLLECTION: &str = "at.motet.pilot.scene";
const POST_COLLECTION: &str = "at.motet.pilot.post";
const SUBMIT: &str = "at.motet.pilot.play.submitAdmission";

fn document(did: &str, key: &KeyData, services: &Value) -> Document {
    let public = to_public(key).expect("fixture key has a public component").to_string();
    serde_json::from_value(json!({
        "id": did,
        "verificationMethod": [{"id": format!("{did}#atproto"), "type": "Multikey", "controller": did,
            "publicKeyMultibase": public.strip_prefix("did:key:").expect("public key is multibase")}],
        "service": services,
    }))
    .expect("fixture DID document")
}

#[derive(Clone)]
struct WrittenPost {
    uri: String,
    rkey: String,
    cid: String,
    value: Value,
}

struct AuthorPds {
    key: KeyData,
    posts: Mutex<BTreeMap<String, WrittenPost>>,
}

#[derive(Deserialize)]
struct CreateRecord {
    repo: String,
    collection: String,
    rkey: String,
    record: Value,
}

fn nonce_response(value: Value) -> Response {
    let mut response = Json(value).into_response();
    response
        .headers_mut()
        .insert("dpop-nonce", HeaderValue::from_static("pds-nonce"));
    response
}

fn assert_dpop(headers: &HeaderMap) {
    assert!(headers.get("dpop").is_some(), "the PDS receives a DPoP proof");
    assert!(
        headers
            .get(header::AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| value.starts_with("DPoP "))
    );
}

async fn create_record(
    State(pds): State<Arc<AuthorPds>>,
    headers: HeaderMap,
    Json(input): Json<CreateRecord>,
) -> Response {
    assert_dpop(&headers);
    assert_eq!(input.repo, AUTHOR);
    assert_eq!(input.collection, POST_COLLECTION);
    let bytes = atproto_dasl::to_vec_from_json(&input.record).expect("post encodes as DRISL");
    let cid = atproto_dasl::cid::compute_cid(&bytes).to_string();
    let uri = format!("at://{AUTHOR}/{POST_COLLECTION}/{}", input.rkey);
    let written = WrittenPost {
        uri: uri.clone(),
        rkey: input.rkey.clone(),
        cid: cid.clone(),
        value: input.record,
    };
    assert!(
        pds.posts.lock().await.insert(input.rkey, written).is_none(),
        "post creates only once"
    );
    nonce_response(json!({"uri": uri, "cid": cid}))
}

async fn get_record(State(pds): State<Arc<AuthorPds>>, Query(query): Query<BTreeMap<String, String>>) -> Response {
    assert_eq!(query.get("repo").map(String::as_str), Some(AUTHOR));
    assert_eq!(query.get("collection").map(String::as_str), Some(POST_COLLECTION));
    let posts = pds.posts.lock().await;
    let Some(record) = query.get("rkey").and_then(|key| posts.get(key)) else {
        return (StatusCode::BAD_REQUEST, Json(json!({"error":"RecordNotFound"}))).into_response();
    };
    Json(json!({"uri": record.uri, "cid": record.cid, "value": record.value})).into_response()
}

async fn service_token(
    State(pds): State<Arc<AuthorPds>>,
    headers: HeaderMap,
    Query(query): Query<BTreeMap<String, String>>,
) -> Response {
    assert_dpop(&headers);
    assert_eq!(
        query.get("aud").map(String::as_str),
        Some(format!("{WORLD}#motet_tenor").as_str())
    );
    assert_eq!(query.get("lxm").map(String::as_str), Some(SUBMIT));
    let now = SystemTime::now().duration_since(UNIX_EPOCH).expect("clock").as_secs();
    let mut claims = Claims::new(JoseClaims {
        issuer: Some(AUTHOR.into()),
        audience: Some(format!("{WORLD}#motet_tenor")),
        json_web_token_id: Some(uuid::Uuid::now_v7().to_string()),
        issued_at: Some(now),
        expiration: Some(now + 60),
        ..JoseClaims::default()
    });
    claims.private.insert("lxm".into(), SUBMIT.into());
    let header = Header::try_from(pds.key.clone()).expect("author key has a JWT algorithm");
    let token = mint(&pds.key, &header, &claims).expect("author PDS signs service token");
    nonce_response(json!({"token": token}))
}

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

struct SceneFact {
    document: Document,
    rev: String,
    cid: String,
    value: Value,
}

struct Initialize {
    tenor_endpoint: String,
}

impl Op for Initialize {
    type Output = SceneFact;
    type Error = PublicationError;

    async fn run(self, txn: &mut Txn<'_>) -> Result<SceneFact, PublicationError> {
        let world = Did::new(WORLD);
        txn.begin_world(world.clone(), 1).expect("fresh World");
        let signing = txn.generate_key(&world, Purpose::Signing).expect("World signing key");
        txn.generate_key(&world, Purpose::Kem).expect("World KEM key");
        let document = document(
            WORLD,
            &signing,
            &json!([
                {"id":"#motet_tenor", "type":"MotetTenor", "serviceEndpoint":self.tenor_endpoint}
            ]),
        );
        let info = WorldInfo {
            did: world.clone(),
            owner: Did::new(OWNER),
            handle: "scene.motet.test".into(),
            epoch: 1,
            status: Status::Active,
            created: atproto_record::datetime::Datetime::now().into_string(),
            document: document.clone(),
        };
        txn.put(keys::world_info(&world), serde_json::to_vec(&info)?.into());
        repo::genesis(txn, &world).await?;
        spaces::bootstrap(txn, &world, &Did::new(OWNER), ProfileInput::default()).await?;
        let members = spaces::world_space(&world, spaces::MEMBERS)?;
        spaces::write_as_world(
            txn,
            &members,
            vec![
                SpaceWrite::create("group.opensocial.role", "player", &json!({"displayName":"Player"}))?,
                SpaceWrite::create(
                    "group.opensocial.membership",
                    AUTHOR,
                    &json!({
                        "member": AUTHOR, "roles":["player"], "createdAt":"2026-10-26T00:00:00Z"
                    }),
                )?,
            ],
        )
        .await?;
        let scene = json!({"$type": SCENE_COLLECTION, "title":"The cast-only scene", "state":"open",
            "cast":[OWNER], "openedBy":OWNER, "createdAt":"2026-10-26T00:00:00Z"});
        let committed = repo::commit(
            txn,
            &world,
            vec![Write::Create {
                collection: SCENE_COLLECTION.parse().expect("valid scene collection"),
                rkey: SCENE.into(),
                record: repo::record_from_json(&scene).expect("scene encodes"),
            }],
            None,
        )
        .await?;
        let cid = committed
            .results
            .first()
            .and_then(|(_, cid)| *cid)
            .expect("scene was written")
            .to_string();
        txn.command(
            world,
            OperationId::new("open-scene"),
            Command::OpenScene {
                scene: SceneId::new(SCENE),
                cast: BTreeSet::from([Did::new(OWNER)]),
            },
        )
        .await?
        .expect("scene opens");
        Ok(SceneFact {
            document,
            rev: committed.rev.to_string(),
            cid,
            value: scene,
        })
    }
}

async fn json_response(response: Response) -> (StatusCode, Value) {
    let status = response.status();
    let body = to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("bounded response");
    (status, serde_json::from_slice(&body).expect("JSON response"))
}

/// A non-cast member's published post is refused and remains absent from the
/// public scene. Only its author can track a pending retry through refusal.
/// Catches bypassed cast checks, leaked outcomes, and stale pending statuses.
#[tokio::test]
#[ignore = "integration: needs PostgreSQL (devenv up)"]
#[expect(
    clippy::too_many_lines,
    reason = "The test follows one post across the real authorization and read boundaries."
)]
async fn non_cast_member_post_is_refused_and_never_appears_in_the_public_scene() {
    let author_key = generate_key(KeyType::K256Private).expect("author signing key");
    let pds = Arc::new(AuthorPds {
        key: author_key.clone(),
        posts: Mutex::new(BTreeMap::new()),
    });
    let pds_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let pds_url = format!("http://{}", pds_listener.local_addr().unwrap());
    let pds_router = axum::Router::new()
        .route("/xrpc/com.atproto.repo.createRecord", post(create_record))
        .route("/xrpc/com.atproto.repo.getRecord", get(get_record))
        .route("/xrpc/com.atproto.server.getServiceAuth", get(service_token))
        .with_state(Arc::clone(&pds));
    let pds_task = tokio::spawn(async move { axum::serve(pds_listener, pds_router).await.unwrap() });

    let tenor_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let tenor_url = format!("http://{}", tenor_listener.local_addr().unwrap());
    let shard = Shard::open(
        Arc::new(InMemory::new()),
        "non-cast",
        Arc::new(MasterKey::from_bytes(&[37; 32])),
    )
    .await
    .unwrap();
    let handle = spawn(
        shard,
        Options {
            max_batch: 8,
            checkpoint_every: Duration::from_secs(3600),
        },
    );
    let scene = handle
        .run(Initialize {
            tenor_endpoint: tenor_url.clone(),
        })
        .await
        .unwrap()
        .unwrap();
    let author_document = document(
        AUTHOR,
        &author_key,
        &json!([
            {"id":"#atproto_pds", "type":"AtprotoPersonalDataServer", "serviceEndpoint":pds_url}
        ]),
    );
    let resolver: Arc<dyn IdentityResolver> = Arc::new(Directory(BTreeMap::from([
        (WORLD.into(), scene.document.clone()),
        (AUTHOR.into(), author_document),
    ])));
    let fetcher = Fetcher::new(Policy::default().allow_private()).expect("test network policy");
    let hosting = Arc::new(
        Hosting::new(
            &"https://tenor.example".parse().unwrap(),
            vec![".motet.test".into()],
            "https://plc.example".into(),
            fetcher.clone(),
            Vec::new(),
        )
        .unwrap(),
    );
    let audience = hosting.audience().to_owned();
    let tenor_auth = motet_xrpc::service_auth::authenticate(
        Arc::clone(&resolver),
        Arc::new(move |aud| motet_tenor::api::accepts_audience(aud, &audience)),
    );
    let tenor = motet_tenor::api::router(
        Arc::new(motet_tenor::api::State {
            shard: handle.clone(),
            records: motet_tenor::pds::Pds::new(fetcher.clone(), Arc::clone(&resolver)),
            hosting,
        }),
        tenor_auth,
    )
    .unwrap();
    let tenor_task = tokio::spawn(async move { axum::serve(tenor_listener, tenor).await.unwrap() });

    let pool = motet_testkit::services::database().await;
    MIGRATOR.run(&pool).await.unwrap();
    let keys_dir = tempfile::tempdir().unwrap();
    let signing = keys_dir.path().join("signing");
    let sealing = keys_dir.path().join("sealing");
    std::fs::write(&signing, [1; 32]).unwrap();
    std::fs::write(&sealing, [2; 32]).unwrap();
    let oauth = auth::Server::new(
        pool.clone(),
        ORIGIN.into(),
        pds_url.clone(),
        &signing,
        &sealing,
        fetcher.clone(),
        Arc::clone(&resolver),
    )
    .unwrap();
    let mut grant = Grant {
        did: AUTHOR.into(),
        handle: "author.motet.test".into(),
        pds: pds_url.clone(),
        issuer: pds_url.clone(),
        token_endpoint: format!("{pds_url}/oauth/token"),
        revocation_endpoint: None,
        access_token: "test-access-token".into(),
        refresh_token: "test-refresh-token".into(),
        expires_at: Utc::now().timestamp() + 600,
        scope: "atproto".into(),
        dpop_private_key: vec![3; 32],
    };
    let cookie = SessionStore::new(pool.clone(), [2; 32])
        .create_session(&grant, Utc::now() + ChronoDuration::hours(1))
        .await
        .unwrap();
    grant.did = OWNER.into();
    grant.handle = "owner.motet.test".into();
    let other_cookie = SessionStore::new(pool.clone(), [2; 32])
        .create_session(&grant, Utc::now() + ChronoDuration::hours(1))
        .await
        .unwrap();
    let client = Arc::new(
        writes::client::Client::new(fetcher, Arc::clone(&resolver), &tenor_url, "did:web:tenor.example").unwrap(),
    );
    let fallback = motet_xrpc::service_auth::authenticate(resolver, Arc::new(|_| false));
    let app = api::router(
        Arc::new(api::State {
            pool: pool.clone(),
            auth: Arc::clone(&oauth),
            client: Arc::clone(&client),
        }),
        oauth.authenticate(fallback),
    )
    .unwrap();
    let archive = Archive::new(Arc::new(InMemory::new()), "non-cast-archive");
    let mut indexer = Indexer::start(pool.clone(), &archive).await.unwrap();
    indexer
        .ingest(&[Event::Record(RecordEvent {
            did: WORLD.into(),
            rev: scene.rev,
            collection: SCENE_COLLECTION.into(),
            rkey: SCENE.into(),
            action: Action::Create,
            cid: Some(scene.cid),
            record: Some(scene.value),
        })])
        .await
        .unwrap();

    let request = Request::post("/xrpc/at.motet.pilot.createPost")
        .header(header::ORIGIN, ORIGIN)
        .header(header::COOKIE, format!("__Host-motet_session={cookie}"))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({"op":atproto_record::tid::Tid::new().to_string(), "world":WORLD, "scene":SCENE,
            "body":"A non-cast member cannot enter this scene", "register":"in-character"})
            .to_string(),
        ))
        .unwrap();
    let (status, answer) = json_response(app.clone().oneshot(request).await.unwrap()).await;
    assert_eq!(status, StatusCode::OK, "{answer}");
    assert_eq!(answer["status"], "refused", "{answer}");
    assert_eq!(answer["reason"], "NotInCast", "{answer}");
    let written = pds
        .posts
        .lock()
        .await
        .values()
        .next()
        .expect("author PDS stored the post")
        .clone();
    assert_eq!(answer["uri"], written.uri);
    assert_eq!(answer["cid"], written.cid);
    // Retry the durable admission after an interrupted response. The tenor
    // must return the original decision for the same operation.
    sqlx::query(
        "UPDATE service.outbox SET state = 'queued', outcome = NULL, next_attempt_at = clock_timestamp(), \
         lease_owner = NULL, lease_until = NULL WHERE account = $1 AND payload->>'uri' = $2",
    )
    .bind(AUTHOR)
    .bind(&written.uri)
    .execute(&pool)
    .await
    .unwrap();
    let mut status_url = Url::parse("https://motet.example/xrpc/at.motet.pilot.getPostStatus").unwrap();
    status_url.query_pairs_mut().append_pair("uri", &written.uri);
    let status_path = format!("{}?{}", status_url.path(), status_url.query().unwrap());
    let request = |cookie: Option<&str>| {
        let mut builder = Request::get(&status_path);
        if let Some(cookie) = cookie {
            builder = builder.header(header::COOKIE, format!("__Host-motet_session={cookie}"));
        }
        builder.body(Body::empty()).unwrap()
    };
    let (status, pending) = json_response(app.clone().oneshot(request(Some(&cookie))).await.unwrap()).await;
    assert_eq!(status, StatusCode::OK, "{pending}");
    assert_eq!(pending["status"], "pending");
    assert_eq!(pending["uri"], written.uri);
    assert_eq!(pending["cid"], written.cid);
    let anonymous = app.clone().oneshot(request(None)).await.unwrap();
    assert_eq!(anonymous.status(), StatusCode::UNAUTHORIZED);
    let other = app.clone().oneshot(request(Some(&other_cookie))).await.unwrap();
    assert_eq!(other.status(), StatusCode::NOT_FOUND);

    let (shutdown, receiver) = tokio::sync::watch::channel(false);
    let worker = tokio::spawn(motet_appview::outbox::run(
        pool.clone(),
        Arc::clone(&oauth),
        Arc::clone(&client),
        receiver,
    ));
    let refused = tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let (status, value) = json_response(app.clone().oneshot(request(Some(&cookie))).await.unwrap()).await;
            assert_eq!(status, StatusCode::OK, "{value}");
            if value["status"] != "pending" {
                break value;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("admission retry finishes");
    assert_eq!(refused["status"], "refused");
    assert_eq!(refused["reason"], "NotInCast");
    assert_eq!(refused["uri"], written.uri);
    assert_eq!(refused["cid"], written.cid);
    assert_eq!(
        app.clone()
            .oneshot(request(Some(&other_cookie)))
            .await
            .unwrap()
            .status(),
        StatusCode::NOT_FOUND
    );
    shutdown.send(true).unwrap();
    worker.await.unwrap();
    sqlx::query("UPDATE service.outbox SET state = 'failed', outcome = $3 WHERE account = $1 AND payload->>'uri' = $2")
        .bind(AUTHOR)
        .bind(&written.uri)
        .bind(json!({"error": "upstream"}))
        .execute(&pool)
        .await
        .unwrap();
    let (status, failed) = json_response(app.clone().oneshot(request(Some(&cookie))).await.unwrap()).await;
    assert_eq!(status, StatusCode::OK, "{failed}");
    assert_eq!(failed["status"], "failed");
    assert_eq!(failed["reason"], "upstream");
    let drained = motet_tenor::publication::drain(&handle).await.unwrap();
    assert_eq!(
        (drained.published, drained.failed),
        (0, 0),
        "a refusal has no admission to publish"
    );
    assert_eq!(
        handle
            .reader()
            .scan(
                &keys::repo_records(&Did::new(WORLD), Some("at.motet.pilot.admission")),
                None,
                100
            )
            .await
            .unwrap(),
        [] as [(bytes::Bytes, bytes::Bytes); 0]
    );

    // Simulate Tap delivering the real PDS record: its existence alone must
    // not let getScene present it as admitted.
    indexer
        .ingest(&[Event::Record(RecordEvent {
            did: AUTHOR.into(),
            rev: atproto_record::tid::Tid::new().to_string(),
            collection: POST_COLLECTION.into(),
            rkey: written.rkey.clone(),
            action: Action::Create,
            cid: Some(written.cid.clone()),
            record: Some(written.value.clone()),
        })])
        .await
        .unwrap();
    let mut version = Url::parse("https://motet.example/xrpc/at.motet.pilot.getRecordVersion").unwrap();
    version
        .query_pairs_mut()
        .append_pair("uri", &written.uri)
        .append_pair("cid", &written.cid);
    let path = format!("{}?{}", version.path(), version.query().unwrap());
    let (status, observed) = json_response(
        app.clone()
            .oneshot(Request::get(path).body(Body::empty()).unwrap())
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{observed}");
    assert_eq!(
        observed["value"], written.value,
        "the refused version really reached the public archive"
    );

    let mut scene_url = Url::parse("https://motet.example/xrpc/at.motet.pilot.getScene").unwrap();
    scene_url
        .query_pairs_mut()
        .append_pair("actor", WORLD)
        .append_pair("scene", SCENE);
    let path = format!("{}?{}", scene_url.path(), scene_url.query().unwrap());
    let (status, page) = json_response(
        app.oneshot(Request::get(path).body(Body::empty()).unwrap())
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{page}");
    assert_eq!(page["scene"]["value"]["cast"], json!([OWNER]));
    assert_eq!(
        page["posts"],
        json!([]),
        "a refused public post must not appear in the scene"
    );
    pds_task.abort();
    tenor_task.abort();
}
