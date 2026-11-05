//! Scene controls preserve authorization, admission and archive-derived public reads.
//!
//! The author's PDS is a small HTTP fixture which stores the actual record and
//! signs service tokens. AppView authentication, tenor authorization, rules,
//! publication, archive and public reads use their production implementations.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use atproto_identity::key::{KeyData, KeyType, generate_key, to_public};
use atproto_identity::model::Document;
use atproto_identity::traits::IdentityResolver;
use atproto_oauth::jwt::{Claims, Header, JoseClaims, mint, verify};
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
use motet_appview::{MIGRATOR, auth, projection, writes};
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

const WORLD: &str = "did:plc:ewvi7nxzyoun6zhxrhs64oiz";
const OWNER: &str = "did:plc:aaaaaaaaaaaaaaaaaaaaaaaa";
const AUTHOR: &str = "did:plc:bbbbbbbbbbbbbbbbbbbbbbbb";
const ORIGIN: &str = "https://motet.example";
const SCENE: &str = "room";
const SCENE_COLLECTION: &str = "at.motet.pilot.scene";
const POST_COLLECTION: &str = "at.motet.pilot.post";
const SUBMIT: &str = "at.motet.pilot.play.submitAdmission";
const LEGACY_SCOPE: &str = concat!(
    "atproto repo:at.motet.pilot.post repo:at.motet.pilot.profile repo:at.motet.pilot.character ",
    "rpc:at.motet.pilot.tenor.reserveWorld?aud=* rpc:at.motet.pilot.tenor.createWorld?aud=* ",
    "rpc:at.motet.pilot.play.openScene?aud=* rpc:at.motet.pilot.play.setSceneCast?aud=* ",
    "rpc:at.motet.pilot.play.closeScene?aud=* rpc:at.motet.pilot.play.submitAdmission?aud=*"
);

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
    cid: String,
    value: Value,
}

struct AuthorPds {
    key: KeyData,
    posts: Mutex<BTreeMap<String, WrittenPost>>,
    refreshes: AtomicUsize,
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
    let method = query.get("lxm").expect("a service token names its method");
    assert!(
        [
            SUBMIT,
            "at.motet.pilot.play.setSceneCast",
            "at.motet.pilot.play.closeScene",
            "at.motet.pilot.play.getSceneControls"
        ]
        .contains(&method.as_str())
    );
    let authorization = headers
        .get(header::AUTHORIZATION)
        .expect("PDS request carries authorization")
        .to_str()
        .expect("authorization is text");
    let access = authorization.strip_prefix("DPoP ").expect("authenticated PDS request");
    if access == "legacy-author" && method == "at.motet.pilot.play.getSceneControls" {
        let mut response = (StatusCode::FORBIDDEN, Json(json!({"error":"InsufficientScope"}))).into_response();
        response
            .headers_mut()
            .insert("dpop-nonce", HeaderValue::from_static("pds-nonce"));
        return response;
    }
    let actor = if access == "legacy-author" { AUTHOR } else { access };
    assert!(
        [OWNER, AUTHOR].contains(&actor),
        "the access token identifies a fixture account"
    );
    let now = SystemTime::now().duration_since(UNIX_EPOCH).expect("clock").as_secs();
    let mut claims = Claims::new(JoseClaims {
        issuer: Some(actor.into()),
        audience: Some(format!("{WORLD}#motet_tenor")),
        json_web_token_id: Some(uuid::Uuid::now_v7().to_string()),
        issued_at: Some(now),
        expiration: Some(now + 60),
        ..JoseClaims::default()
    });
    claims.private.insert("lxm".into(), method.clone().into());
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
            "cast":[OWNER, AUTHOR], "openedBy":OWNER, "createdAt":"2026-10-26T00:00:00Z"});
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
                cast: BTreeSet::from([Did::new(OWNER), Did::new(AUTHOR)]),
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

/// Current authority governs scene changes and admission, even when retries carry an old swap.
/// Catches trusted body actors, repo-only cast/close writes, and retry checks after CAS checks.
/// Legacy grants can refresh and post without optional discovery; requiring its scope globally would revoke them.
#[tokio::test]
#[ignore = "integration: needs PostgreSQL (devenv up)"]
#[expect(
    clippy::too_many_lines,
    reason = "one ordered journey shares the authenticated tenor and isolated archive"
)]
async fn scene_controls_refuse_unauthorized_actors_and_removed_or_closed_admissions() {
    let author_key = generate_key(KeyType::K256Private).expect("author signing key");
    let pds = Arc::new(AuthorPds {
        key: author_key.clone(),
        posts: Mutex::new(BTreeMap::new()),
        refreshes: AtomicUsize::new(0),
    });
    let pds_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let pds_url = format!("http://{}", pds_listener.local_addr().unwrap());
    let pds_router = axum::Router::new()
        .route("/oauth/token", post(refresh))
        .route("/xrpc/com.atproto.repo.createRecord", post(create_record))
        .route("/xrpc/com.atproto.repo.getRecord", get(get_record))
        .route("/xrpc/com.atproto.server.getServiceAuth", get(service_token))
        .with_state(Arc::clone(&pds));
    let pds_task = tokio::spawn(async move { axum::serve(pds_listener, pds_router).await.unwrap() });

    let tenor_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let tenor_url = format!("http://{}", tenor_listener.local_addr().unwrap());
    let shard = Shard::open(
        Arc::new(InMemory::new()),
        "scene-controls",
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
        (
            OWNER.into(),
            document(
                OWNER,
                &author_key,
                &json!([
                    {"id":"#atproto_pds", "type":"AtprotoPersonalDataServer", "serviceEndpoint":pds_url}
                ]),
            ),
        ),
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
        access_token: AUTHOR.into(),
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
    grant.access_token = OWNER.into();
    grant.handle = "owner.motet.test".into();
    let other_cookie = SessionStore::new(pool.clone(), [2; 32])
        .create_session(&grant, Utc::now() + ChronoDuration::hours(1))
        .await
        .unwrap();
    grant.did = AUTHOR.into();
    grant.handle = "author.motet.test".into();
    grant.access_token = "expired-legacy".into();
    grant.expires_at = Utc::now().timestamp() - 60;
    grant.scope = LEGACY_SCOPE.into();
    let legacy_cookie = SessionStore::new(pool.clone(), [2; 32])
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
    let archive = Archive::new(Arc::new(InMemory::new()), "scene-controls-archive");
    let mut indexer = Indexer::start(pool.clone(), &archive).await.unwrap();
    indexer
        .ingest(&[Event::Record(RecordEvent {
            did: WORLD.into(),
            rev: scene.rev.clone(),
            collection: SCENE_COLLECTION.into(),
            rkey: SCENE.into(),
            action: Action::Create,
            cid: Some(scene.cid.clone()),
            record: Some(scene.value.clone()),
        })])
        .await
        .unwrap();

    check_controls(&app, Some(&other_cookie), Some(true)).await;
    check_controls(&app, Some(&cookie), Some(false)).await;
    check_controls(&app, None, None).await;
    let initial = snapshot(&handle, &tenor_url).await;
    let cast_input = json!({"world":WORLD, "scene":SCENE, "op":"remove-member",
        "swapRecord":scene.cid, "cast":[]});
    for method in ["setSceneCast", "closeScene"] {
        let mut input = cast_input.clone();
        if method == "closeScene" {
            input.as_object_mut().unwrap().remove("cast");
        }
        let (status, refused) = procedure(&app, &cookie, method, &input).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{method}: {refused}");
        assert_eq!(
            snapshot(&handle, &tenor_url).await,
            initial,
            "a player cannot change the scene with the owner's authority"
        );
    }

    // Discovery is optional for old grants; refusing it must not revoke their established posting authority.
    let request = Request::get(format!("/xrpc/at.motet.pilot.getSceneControls?world={WORLD}"))
        .header(header::COOKIE, format!("__Host-motet_session={legacy_cookie}"))
        .body(Body::empty())
        .unwrap();
    let (status, unavailable) = json_response(app.clone().oneshot(request).await.unwrap()).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "missing optional scope: {unavailable}");
    assert_eq!(unavailable["error"], "Forbidden");
    assert_eq!(
        pds.refreshes.load(Ordering::SeqCst),
        1,
        "the expired legacy grant really refreshed"
    );
    let legacy_post = create_post(&app, &legacy_cookie).await;
    assert_eq!(
        legacy_post["status"], "admitted",
        "optional discovery cannot invalidate posting: {legacy_post}"
    );
    let before = create_post(&app, &cookie).await;
    assert_eq!(
        before["status"], "admitted",
        "the fixture member starts in the cast: {before}"
    );
    let removed = successful_procedure(&app, &other_cookie, "setSceneCast", &cast_input).await;
    assert_eq!(
        successful_procedure(&app, &other_cookie, "setSceneCast", &cast_input).await,
        removed,
        "a retry returns the original receipt despite its now-stale swapRecord"
    );
    let mut conflicting = cast_input.clone();
    conflicting["cast"] = json!([AUTHOR]);
    let (status, refused) = procedure(&app, &other_cookie, "setSceneCast", &conflicting).await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "a reused op cannot change its body: {refused}"
    );
    let mut stale = cast_input.clone();
    stale["op"] = json!("new-op-stale-cid");
    let (status, refused) = procedure(&app, &other_cookie, "setSceneCast", &stale).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{refused}");
    assert_eq!(refused["error"], "InvalidSwap");
    let removed_event = snapshot(&handle, &tenor_url).await;
    let removed_record = event_record(&removed_event);
    assert_eq!(removed_record["cast"], json!([OWNER]));
    let after = create_post(&app, &cookie).await;
    assert_eq!(after["status"], "refused", "{after}");
    assert_eq!(
        after["reason"], "NotInCast",
        "a repo-only cast update would admit the removed member"
    );
    check_projection(&pool, &archive, &mut indexer, &app, &removed_event, &initial).await;

    let restore_input = json!({"world":WORLD, "scene":SCENE, "op":"restore-member",
        "swapRecord":removed["cid"], "cast":[AUTHOR]});
    let restored = successful_procedure(&app, &other_cookie, "setSceneCast", &restore_input).await;
    let restored_event = snapshot(&handle, &tenor_url).await;
    assert_eq!(create_post(&app, &cookie).await["status"], "admitted");
    let close_input = json!({"world":WORLD, "scene":SCENE, "op":"close-scene", "swapRecord":restored["cid"]});
    let closed = successful_procedure(&app, &other_cookie, "closeScene", &close_input).await;
    assert_eq!(
        successful_procedure(&app, &other_cookie, "closeScene", &close_input).await,
        closed,
        "closing retries return the first receipt rather than rechecking the obsolete swap"
    );
    assert_eq!(closed["scene"], SCENE);
    assert_eq!(closed["uri"], removed["uri"], "controls update the same public record");
    let mut after_close_cast = restore_input.clone();
    after_close_cast["op"] = json!("new-cast-after-close");
    after_close_cast["swapRecord"] = closed["cid"].clone();
    let (status, refused) = procedure(&app, &other_cookie, "setSceneCast", &after_close_cast).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{refused}");
    assert_eq!(refused["error"], "SceneClosed");
    let closed_event = snapshot(&handle, &tenor_url).await;
    assert_eq!(event_record(&closed_event)["state"], "closed");
    let after_close = create_post(&app, &cookie).await;
    assert_eq!(after_close["status"], "refused", "{after_close}");
    assert_eq!(
        after_close["reason"], "SceneClosed",
        "a repo-only close would still admit a cast member"
    );
    check_projection(&pool, &archive, &mut indexer, &app, &closed_event, &restored_event).await;
    pds_task.abort();
    tenor_task.abort();
}

async fn procedure(app: &axum::Router, cookie: &str, method: &str, input: &Value) -> (StatusCode, Value) {
    let request = Request::post(format!("/xrpc/at.motet.pilot.{method}"))
        .header(header::ORIGIN, ORIGIN)
        .header(header::COOKIE, format!("__Host-motet_session={cookie}"))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(input.to_string()))
        .expect("fixture procedure request is valid");
    json_response(
        app.clone()
            .oneshot(request)
            .await
            .expect("fixture router is infallible"),
    )
    .await
}

async fn successful_procedure(app: &axum::Router, cookie: &str, method: &str, input: &Value) -> Value {
    let (status, body) = procedure(app, cookie, method, input).await;
    assert_eq!(status, StatusCode::OK, "{method}: {body}");
    body
}

async fn create_post(app: &axum::Router, cookie: &str) -> Value {
    successful_procedure(
        app,
        cookie,
        "createPost",
        &json!({
            "op":atproto_record::tid::Tid::new().to_string(), "world":WORLD, "scene":SCENE,
            "body":"A real published post tests current scene authority", "register":"in-character"
        }),
    )
    .await
}

async fn snapshot(handle: &motet_tenor::shard::Handle, tenor: &str) -> Event {
    let record: Value = reqwest::Client::new()
        .get(format!("{tenor}/xrpc/com.atproto.repo.getRecord"))
        .query(&[("repo", WORLD), ("collection", SCENE_COLLECTION), ("rkey", SCENE)])
        .send()
        .await
        .expect("tenor serves the published scene")
        .error_for_status()
        .expect("published scene exists")
        .json()
        .await
        .expect("published scene response is JSON");
    let head = repo::head(handle.reader(), &Did::new(WORLD))
        .await
        .expect("repository head is readable")
        .expect("fixture World has a repository");
    Event::Record(RecordEvent {
        did: WORLD.into(),
        rev: head.rev.to_string(),
        collection: SCENE_COLLECTION.into(),
        rkey: SCENE.into(),
        action: Action::Update,
        cid: Some(
            record
                .get("cid")
                .and_then(Value::as_str)
                .expect("published scene has a CID")
                .into(),
        ),
        record: Some(record.get("value").expect("published scene has a value").clone()),
    })
}

fn event_record(event: &Event) -> &Value {
    let Event::Record(record) = event else {
        panic!("scene snapshot is a record event")
    };
    record.record.as_ref().expect("scene snapshot includes its record")
}

// Real published records supply the oracle; an old delivery must not restore an obsolete cast or reopen a scene.
async fn check_projection(
    pool: &sqlx::PgPool,
    archive: &Archive,
    indexer: &mut Indexer,
    app: &axum::Router,
    current: &Event,
    stale: &Event,
) {
    indexer
        .ingest(&[current.clone(), current.clone(), stale.clone()])
        .await
        .expect("archive accepts scene redelivery");
    let Event::Record(record) = current else {
        panic!("scene snapshot is a record event")
    };
    let expected = json!({"uri":record.uri(), "cid":record.cid, "value":record.record});
    for rebuild in [false, true] {
        if rebuild {
            projection::rebuild(pool, archive)
                .await
                .expect("scene projections rebuild from the archive");
        }
        let request = Request::get(format!("/xrpc/at.motet.pilot.getScene?actor={WORLD}&scene={SCENE}"))
            .body(Body::empty())
            .expect("fixture public read request is valid");
        let (status, page) = json_response(
            app.clone()
                .oneshot(request)
                .await
                .expect("fixture router is infallible"),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{page}");
        assert_eq!(
            page.get("scene"),
            Some(&expected),
            "latest published cast/state survives redelivery and rebuild"
        );
        let request = Request::get(format!("/xrpc/at.motet.pilot.listScenes?actor={WORLD}"))
            .body(Body::empty())
            .expect("fixture public read request is valid");
        let (status, listing) = json_response(
            app.clone()
                .oneshot(request)
                .await
                .expect("fixture router is infallible"),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{listing}");
        assert_eq!(
            listing.get("scenes"),
            Some(&json!([expected])),
            "listing agrees with the individual scene"
        );
    }
}

// Permission responses are personal; public caching could expose the owner's controls to a player.
async fn check_controls(app: &axum::Router, cookie: Option<&str>, permitted: Option<bool>) {
    let mut request = Request::get(format!("/xrpc/at.motet.pilot.getSceneControls?world={WORLD}"));
    if let Some(cookie) = cookie {
        request = request.header(header::COOKIE, format!("__Host-motet_session={cookie}"));
    }
    let response = app
        .clone()
        .oneshot(request.body(Body::empty()).expect("controls query request is valid"))
        .await
        .expect("fixture router is infallible");
    if let Some(permitted) = permitted {
        assert_eq!(
            response
                .headers()
                .get(header::CACHE_CONTROL)
                .expect("personalized response has a cache policy"),
            "private, no-store"
        );
        let (status, controls) = json_response(response).await;
        assert_eq!(status, StatusCode::OK, "{controls}");
        assert_eq!(controls, json!({"setCast":permitted, "close":permitted}));
    } else {
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
}

async fn refresh(State(pds): State<Arc<AuthorPds>>, headers: HeaderMap, body: String) -> Response {
    let form: BTreeMap<_, _> = url::form_urlencoded::parse(body.as_bytes()).into_owned().collect();
    assert_eq!(form.get("grant_type").map(String::as_str), Some("refresh_token"));
    assert_eq!(
        form.get("refresh_token").map(String::as_str),
        Some("test-refresh-token")
    );
    let key =
        to_public(&KeyData::new(KeyType::P256Private, vec![3; 32])).expect("fixture DPoP key has a public component");
    let proof = headers
        .get("dpop")
        .expect("refresh includes a DPoP proof")
        .to_str()
        .expect("DPoP proof is text");
    let claims = verify(proof, &key).expect("refresh carries a signed DPoP proof");
    assert_eq!(claims.jose.http_method.as_deref(), Some("POST"));
    if claims.jose.nonce.as_deref() != Some("refresh-nonce") {
        let mut response = (StatusCode::UNAUTHORIZED, Json(json!({"error":"use_dpop_nonce"}))).into_response();
        response
            .headers_mut()
            .insert("dpop-nonce", HeaderValue::from_static("refresh-nonce"));
        return response;
    }
    pds.refreshes.fetch_add(1, Ordering::SeqCst);
    let mut response = Json(json!({
        "access_token":"legacy-author", "refresh_token":"refreshed-legacy", "token_type":"DPoP",
        "expires_in":600, "scope":LEGACY_SCOPE, "sub":AUTHOR
    }))
    .into_response();
    response
        .headers_mut()
        .insert("dpop-nonce", HeaderValue::from_static("refresh-nonce"));
    response
}
