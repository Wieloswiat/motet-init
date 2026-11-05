//! Public scene authority, signed records and replay through the XRPC boundary.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use atproto_identity::key::{KeyData, KeyType, generate_key, sign, to_public};
use atproto_identity::model::{Document, VerificationMethod};
use atproto_space::space_repo::OpAction;
use atproto_space::storage::SpaceRepoStorage;
use atproto_testkit::highport::FakePlcDirectory;
use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use motet_net::identity::DidResolver;
use motet_net::{Fetcher, Policy};
use motet_rules::world::{ReadBack, Refusal};
use motet_rules::{Cid, Did, RecordUri, SceneId, StrongRef};
use motet_tenor::admission::{FetchError, Fetched, Records};
use motet_tenor::identity::Hosting;
use motet_tenor::keys::{MasterKey, Purpose};
use motet_tenor::shard::{self, Handle, Op, Options, Shard, Txn};
use motet_tenor::spaces::{self, ProfileInput, SpaceError, SpaceWrite};
use motet_tenor::store::keys;
use motet_tenor::world::{Status, WorldInfo};
use motet_tenor::{api, repo};
use object_store::ObjectStore;
use object_store::memory::InMemory;
use proptest::prelude::*;
use serde_json::{Value, json};
use tower::ServiceExt as _;

const WORLD: &str = "did:plc:ewvi7nxzyoun6zhxrhs64oiz";
const OWNER: &str = "did:plc:owner";
const GM: &str = "did:plc:gm";
const PLAYER: &str = "did:plc:player";
const MEMBER: &str = "did:plc:member";
const STRANGER: &str = "did:plc:stranger";
const OPEN: &str = "at.motet.pilot.play.openScene";
const SET_CAST: &str = "at.motet.pilot.play.setSceneCast";
const CLOSE: &str = "at.motet.pilot.play.closeScene";
const CONTROLS: &str = "at.motet.pilot.play.getSceneControls";
const ADMIT: &str = "at.motet.pilot.play.submitAdmission";
const SCENE: &str = "at.motet.pilot.scene";

#[derive(Clone, Default)]
struct AuthorRepos {
    scene: Arc<Mutex<String>>,
}

impl Records for AuthorRepos {
    fn fetch(&self, post: &StrongRef) -> impl Future<Output = Result<Option<Fetched>, FetchError>> + Send {
        let author = post.uri.as_str().split('/').nth(2).expect("post URI has an author");
        let scene = self.scene.lock().expect("fixture scene mutex is not poisoned").clone();
        std::future::ready(Ok(Some(Fetched {
            cid: post.cid.clone(),
            author: Did::new(author),
            world: Did::new(WORLD),
            scene: SceneId::new(scene),
        })))
    }
}

fn write(collection: &str, rkey: &str, value: &Value) -> SpaceWrite {
    SpaceWrite::create(collection, rkey, value).expect("fixture record encodes")
}

fn membership(actor: &str, roles: &[&str]) -> Value {
    json!({"member": actor, "roles": roles, "createdAt": "2026-10-26T00:00:00Z"})
}

struct Setup;

impl Op for Setup {
    type Output = ();
    type Error = SpaceError;

    async fn run(self, txn: &mut Txn<'_>) -> Result<(), SpaceError> {
        let world = Did::new(WORLD);
        txn.begin_world(world.clone(), 1).expect("fixture World begins once");
        txn.generate_key(&world, Purpose::Signing).expect("fixture signing key");
        txn.generate_key(&world, Purpose::Kem).expect("fixture KEM key");
        repo::genesis(txn, &world).await?;
        let info = WorldInfo {
            did: world.clone(),
            handle: "fixture.motet.test".into(),
            owner: Did::new(OWNER),
            epoch: 1,
            status: Status::Active,
            created: "2026-10-26T00:00:00Z".into(),
            document: Document {
                context: Vec::new(),
                id: WORLD.into(),
                also_known_as: Vec::new(),
                service: Vec::new(),
                verification_method: Vec::new(),
                extra: HashMap::new(),
            },
        };
        txn.put(keys::world_info(&world), serde_json::to_vec(&info)?.into());
        spaces::bootstrap(txn, &world, &Did::new(OWNER), ProfileInput::default()).await?;
        spaces::write_as_world(
            txn,
            &spaces::world_space(&world, spaces::MEMBERS)?,
            vec![
                write("group.opensocial.role", "player", &json!({"displayName": "Player"})),
                write("group.opensocial.role", "member", &json!({"displayName": "Member"})),
                write("group.opensocial.membership", GM, &membership(GM, &["gm"])),
                write("group.opensocial.membership", PLAYER, &membership(PLAYER, &["player"])),
                write("group.opensocial.membership", MEMBER, &membership(MEMBER, &["member"])),
            ],
        )
        .await?;
        Ok(())
    }
}

struct UpdateMembers(Vec<SpaceWrite>);

impl Op for UpdateMembers {
    type Output = ();
    type Error = SpaceError;

    async fn run(self, txn: &mut Txn<'_>) -> Result<(), SpaceError> {
        let members = spaces::world_space(&Did::new(WORLD), spaces::MEMBERS)?;
        spaces::write_as_world(txn, &members, self.0).await?;
        Ok(())
    }
}

struct DeleteAuditSpace;

impl Op for DeleteAuditSpace {
    type Output = ();
    type Error = SpaceError;

    async fn run(self, txn: &mut Txn<'_>) -> Result<(), SpaceError> {
        spaces::delete(txn, &spaces::world_space(&Did::new(WORLD), spaces::GMS)?).await
    }
}

struct CheckCast {
    scene: String,
    author: &'static str,
}

impl Op for CheckCast {
    type Output = Result<(), Refusal>;
    type Error = SpaceError;

    fn run(self, txn: &mut Txn<'_>) -> impl Future<Output = Result<Self::Output, SpaceError>> + Send {
        let post = ReadBack {
            post: StrongRef::new(
                RecordUri::new(format!("at://{}/at.motet.pilot.post/check", self.author)),
                Cid::new("fixture"),
            ),
            author: Did::new(self.author),
            scene: SceneId::new(self.scene),
        };
        std::future::ready(Ok(txn
            .world(&Did::new(WORLD))
            .expect("fixture World exists")
            .check(&post, None)))
    }
}

fn running(shard: Shard) -> Handle {
    shard::spawn(
        shard,
        Options {
            max_batch: 8,
            checkpoint_every: Duration::from_secs(3600),
        },
    )
}

struct Fixture {
    store: Arc<dyn ObjectStore>,
    handle: Option<Handle>,
    plc: FakePlcDirectory,
    hosting: Arc<Hosting>,
    fetcher: Fetcher,
    signing: KeyData,
    repos: AuthorRepos,
}

impl Fixture {
    async fn new() -> Self {
        let store: Arc<dyn ObjectStore> = Arc::new(InMemory::new());
        let handle = running(
            Shard::open(Arc::clone(&store), "scenes", Arc::new(MasterKey::from_bytes(&[64; 32])))
                .await
                .expect("fresh scene shard opens"),
        );
        handle
            .run(Setup)
            .await
            .expect("setup reaches writer")
            .expect("World bootstrap succeeds");
        let plc = FakePlcDirectory::start().await;
        let mut policy = Policy::default().allow_private();
        policy.timeout = Duration::from_secs(1);
        let fetcher = Fetcher::new(policy).expect("fixture network policy");
        let hosting = Arc::new(
            Hosting::new(
                &"https://tenor.motet.test".parse().expect("tenor origin"),
                vec![".motet.test".into()],
                plc.origin().into(),
                fetcher.clone(),
                Vec::new(),
            )
            .expect("hosting configuration"),
        );
        let signing = generate_key(KeyType::K256Private).expect("PDS signing key");
        for did in [OWNER, GM, PLAYER, MEMBER, STRANGER] {
            let public = to_public(&signing).expect("public PDS key").to_string();
            let document = Document {
                context: Vec::new(),
                id: did.into(),
                also_known_as: Vec::new(),
                service: Vec::new(),
                verification_method: vec![VerificationMethod::Multikey {
                    id: format!("{did}#atproto"),
                    controller: did.into(),
                    public_key_multibase: public.strip_prefix("did:key:").expect("did:key").into(),
                    extra: HashMap::new(),
                }],
                extra: HashMap::new(),
            };
            plc.set_document(did, serde_json::to_value(document).expect("DID document"));
        }
        Self {
            store,
            handle: Some(handle),
            plc,
            hosting,
            fetcher,
            signing,
            repos: AuthorRepos::default(),
        }
    }

    fn handle(&self) -> &Handle {
        self.handle.as_ref().expect("fixture shard is open")
    }

    async fn reopen(&mut self) {
        drop(self.handle.take());
        self.handle = Some(running(
            Shard::open(
                Arc::clone(&self.store),
                "scenes",
                Arc::new(MasterKey::from_bytes(&[64; 32])),
            )
            .await
            .expect("scene shard reopens from its log"),
        ));
    }

    fn router(&self) -> axum::Router {
        let resolver = Arc::new(DidResolver::new(self.fetcher.clone(), self.plc.origin()));
        let state = Arc::new(api::State {
            shard: self.handle().clone(),
            records: self.repos.clone(),
            hosting: Arc::clone(&self.hosting),
        });
        let audience = self.hosting.audience().to_owned();
        api::router(
            state,
            motet_xrpc::service_auth::authenticate(
                resolver,
                Arc::new(move |aud| api::accepts_audience(aud, &audience)),
            ),
        )
        .expect("the scene registry builds")
    }

    fn token(&self, issuer: &str, audience: &str, method: &str) -> String {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).expect("clock").as_secs();
        let header = URL_SAFE_NO_PAD.encode(br#"{"alg":"ES256K","typ":"JWT"}"#);
        let claims = URL_SAFE_NO_PAD.encode(
            serde_json::to_vec(&json!({
                "iss": issuer, "aud": audience, "lxm": method, "iat": now, "exp": now + 60,
                "jti": atproto_record::tid::Tid::new().to_string(),
            }))
            .expect("claims"),
        );
        let input = format!("{header}.{claims}");
        let signature = sign(&self.signing, input.as_bytes()).expect("PDS signs service token");
        format!("{input}.{}", URL_SAFE_NO_PAD.encode(signature))
    }

    async fn post_token(&self, method: &str, body: Value, token: &str) -> (StatusCode, Value) {
        let request = Request::post(format!("/xrpc/{method}"))
            .header("authorization", format!("Bearer {token}"))
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(&body).expect("request JSON")))
            .expect("request");
        response(self.router().oneshot(request).await.expect("router response")).await
    }

    async fn post(&self, method: &str, body: Value, actor: &str) -> (StatusCode, Value) {
        self.post_token(method, body, &self.token(actor, WORLD, method)).await
    }

    async fn controls(&self, world: &str, token: &str) -> (StatusCode, Value) {
        let request = Request::get(format!("/xrpc/{CONTROLS}?world={world}"))
            .header("authorization", format!("Bearer {token}"))
            .body(Body::empty())
            .expect("controls query");
        response(self.router().oneshot(request).await.expect("controls response")).await
    }

    async fn get_scene(&self, scene: &str) -> Value {
        let path = format!("/xrpc/com.atproto.repo.getRecord?repo={WORLD}&collection={SCENE}&rkey={scene}");
        let request = Request::get(path).body(Body::empty()).expect("record query");
        let (status, body) = response(self.router().oneshot(request).await.expect("record response")).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        body
    }

    async fn audit(&self) -> Vec<Value> {
        let world = Did::new(WORLD);
        let gms = spaces::world_space(&world, spaces::GMS).expect("GM Space URI");
        spaces::storage::ReadStorage::new(self.handle().reader(), &world)
            .list_records(&gms, "at.motet.pilot.audit", None, 100, true)
            .await
            .expect("audit records")
            .records
            .into_iter()
            .map(|row| atproto_dasl::from_slice_to_json(&row.value).expect("audit DRISL"))
            .collect()
    }

    async fn admin(&self, writes: Vec<SpaceWrite>) {
        self.handle()
            .run(UpdateMembers(writes))
            .await
            .expect("authority update reaches shard")
            .expect("authority records validate");
    }

    async fn grant_player_open(&self) {
        let members = spaces::world_space(&Did::new(WORLD), spaces::MEMBERS).expect("members Space URI");
        let row = spaces::record(self.handle().reader(), &members, "group.opensocial.permissions", "self")
            .await
            .expect("permissions read")
            .expect("bootstrap permissions");
        let mut permissions = atproto_dasl::from_slice_to_json(&row.value).expect("permissions DRISL");
        permissions
            .get_mut("grants")
            .and_then(Value::as_array_mut)
            .expect("grants array")
            .push(json!({
                "role":"player", "assignable":[], "scopes":["rpc:at.motet.pilot.play.openScene"]
            }));
        let mut updated = write("group.opensocial.permissions", "self", &permissions);
        updated.action = OpAction::Update;
        self.admin(vec![updated]).await;
    }

    async fn admit(&self, scene: &str, actor: &str, op: &str) -> Value {
        scene.clone_into(&mut self.repos.scene.lock().expect("fixture scene mutex"));
        let cid = atproto_dasl::cid::compute_cid(op.as_bytes()).to_string();
        let (status, body) = self
            .post(
                ADMIT,
                json!({
                    "world": WORLD, "op": op,
                    "post": {"uri":format!("at://{actor}/at.motet.pilot.post/{op}"), "cid":cid}
                }),
                actor,
            )
            .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        body
    }
}

async fn response(response: axum::response::Response) -> (StatusCode, Value) {
    let status = response.status();
    let body = to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("response body");
    (status, serde_json::from_slice(&body).expect("JSON response"))
}

/// A scene receipt is retryable with a fresh JWT, but a spent JWT never
/// authorizes another body or a receipt after restart. Catches replay before
/// the receipt check and a ledger omitted from durable shard effects.
#[tokio::test]
async fn scene_retry_requires_fresh_signed_token_even_after_shard_replay() {
    let mut f = Fixture::new().await;
    let body = json!({"op":"one-use","world":WORLD,"title":"Original","cast":[MEMBER]});
    let signed = f.token(GM, WORLD, OPEN);
    let (status, opened) = f.post_token(OPEN, body.clone(), &signed).await;
    assert_eq!(status, StatusCode::OK, "{opened}");
    let changed = json!({"op":"another-op","world":WORLD,"title":"Forged"});
    assert_eq!(
        f.post_token(OPEN, changed.clone(), &signed).await.0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(f.post_token(OPEN, body.clone(), &signed).await.0, StatusCode::FORBIDDEN);
    f.reopen().await;
    assert_eq!(f.post_token(OPEN, body.clone(), &signed).await.0, StatusCode::FORBIDDEN);
    let (status, receipt) = f.post(OPEN, body, GM).await;
    assert_eq!(status, StatusCode::OK, "{receipt}");
    assert_eq!(receipt, opened);
    let (status, another) = f.post(OPEN, changed, GM).await;
    assert_eq!(status, StatusCode::OK, "{another}");
    assert_ne!(another["scene"], opened["scene"]);
    assert_eq!(f.audit().await.len(), 2);
}

/// The signed issuer and its live role decide permission, not a body field or
/// a previously issued token. Catches broad grants and stale membership caches.
#[tokio::test]
async fn scene_open_uses_verified_issuer_current_exact_grant_and_adds_the_opener() {
    let f = Fixture::new().await;
    let body = json!({"op":"player-open","world":WORLD,"title":"A public scene","cast":[MEMBER],"actor":OWNER});
    let member = f.token(MEMBER, WORLD, OPEN);
    assert_eq!(f.post_token(OPEN, body.clone(), &member).await.0, StatusCode::FORBIDDEN);
    assert_eq!(f.post(OPEN, body.clone(), STRANGER).await.0, StatusCode::FORBIDDEN);
    let wrong_method = f.token(OWNER, WORLD, CLOSE);
    assert_eq!(
        f.post_token(OPEN, body.clone(), &wrong_method).await.0,
        StatusCode::UNAUTHORIZED
    );
    let wrong_audience = f.token(OWNER, "did:web:elsewhere.example#motet_tenor", OPEN);
    assert_eq!(
        f.post_token(OPEN, body.clone(), &wrong_audience).await.0,
        StatusCode::UNAUTHORIZED
    );
    let wrong_world = f.token(OWNER, "did:plc:another", OPEN);
    assert_eq!(
        f.post_token(OPEN, body.clone(), &wrong_world).await.0,
        StatusCode::FORBIDDEN,
        "a signed token for one World cannot target another"
    );
    assert_eq!(
        f.post_token(
            OPEN,
            json!({"op":"unknown","world":"did:plc:another","title":"Unknown"}),
            &wrong_world
        )
        .await
        .0,
        StatusCode::FORBIDDEN,
        "an unhosted World does not expose its existence"
    );
    assert_eq!(f.post(OPEN, body.clone(), PLAYER).await.0, StatusCode::FORBIDDEN);
    assert_eq!(f.audit().await, [] as [Value; 0]);

    f.grant_player_open().await;
    let token = f.token(PLAYER, &format!("{WORLD}#motet_tenor"), OPEN);
    let (status, opened) = f.post_token(OPEN, body.clone(), &token).await;
    assert_eq!(status, StatusCode::OK, "{opened}");
    let scene = opened["scene"].as_str().expect("scene key");
    let record = f.get_scene(scene).await;
    assert_eq!(record["uri"], opened["uri"]);
    assert_eq!(record["cid"], opened["cid"]);
    assert_eq!(record["value"]["cast"], json!([MEMBER, PLAYER]));
    assert_eq!(record["value"]["state"], "open");
    assert_eq!(
        f.post(
            SET_CAST,
            json!({"op":"player-set","world":WORLD,"scene":scene,"cast":[PLAYER],"swapRecord":opened["cid"]}),
            PLAYER
        )
        .await
        .0,
        StatusCode::FORBIDDEN,
        "an openScene grant must not permit cast changes"
    );
    let audits = f.audit().await;
    assert!(
        audits
            .iter()
            .any(|a| a["actor"] == PLAYER && a["action"] == OPEN && a["subject"] == opened["uri"])
    );

    let mut revoked = write("group.opensocial.membership", PLAYER, &membership(PLAYER, &["member"]));
    revoked.action = OpAction::Update;
    f.admin(vec![revoked]).await;
    assert_eq!(
        f.post_token(OPEN, json!({"op":"stale","world":WORLD,"title":"stale"}), &token)
            .await
            .0,
        StatusCode::FORBIDDEN,
        "the same valid token loses the revoked role immediately"
    );
    assert_eq!(
        f.post_token(OPEN, body, &token).await.0,
        StatusCode::FORBIDDEN,
        "a recorded result cannot bypass a revoked role"
    );
}

async fn assert_open_retry_contract(f: &Fixture, open_body: &Value, opened: &Value, scene: &str) {
    let (retry_status, retry) = f.post(OPEN, open_body.clone(), GM).await;
    assert_eq!(retry_status, StatusCode::OK);
    assert_eq!(retry, *opened, "an uncertain opening returns its first scene and CID");
    assert_eq!(
        f.post(OPEN, json!({"op":"gm-open","world":WORLD,"title":"different"}), GM)
            .await
            .0,
        StatusCode::BAD_REQUEST,
        "an operation ID cannot name a different body"
    );
    assert_eq!(
        f.post(
            SET_CAST,
            json!({
                "op":"gm-open", "world":WORLD, "scene":scene, "cast":[PLAYER], "swapRecord":opened["cid"]
            }),
            GM
        )
        .await
        .0,
        StatusCode::BAD_REQUEST,
        "operation IDs cannot be reused across scene methods"
    );
}

/// Published cast and rules must move together and survive a restart. Catches a
/// repo-only cast change, a rules-only closure, and unreplayed rule commands.
#[tokio::test]
async fn public_cast_and_closure_govern_admission_before_and_after_replay() {
    let mut f = Fixture::new().await;
    let open_body = json!({"op":"gm-open","world":WORLD,"title":"The Reach","cast":[MEMBER]});
    let (status, opened) = f.post(OPEN, open_body.clone(), GM).await;
    assert_eq!(status, StatusCode::OK, "{opened}");
    let scene = opened["scene"].as_str().expect("scene key").to_owned();
    let initial = f.get_scene(&scene).await;
    assert_eq!(initial["value"]["cast"], json!([GM, MEMBER]));
    assert_eq!(initial["value"]["openedBy"], GM);
    assert_open_retry_contract(&f, &open_body, &opened, &scene).await;
    assert_eq!(f.admit(&scene, GM, "before-gm").await["status"], "admitted");
    assert_eq!(f.admit(&scene, MEMBER, "before-member").await["status"], "admitted");
    let refused = f.admit(&scene, PLAYER, "before-player").await;
    assert_eq!(
        (refused["status"].as_str(), refused["reason"].as_str()),
        (Some("refused"), Some("NotInCast"))
    );

    let cast_body = json!({"op":"gm-cast","world":WORLD,"scene":scene,"cast":[PLAYER],"swapRecord":opened["cid"]});
    let (status, changed) = f.post(SET_CAST, cast_body.clone(), GM).await;
    assert_eq!(status, StatusCode::OK, "{changed}");
    let record = f.get_scene(&scene).await;
    assert_eq!(record["cid"], changed["cid"]);
    assert_eq!(
        record["value"]["cast"],
        json!([GM, PLAYER]),
        "the opener remains in the replacement cast"
    );
    assert_eq!(
        f.post(SET_CAST, cast_body.clone(), GM).await.1,
        changed,
        "a retry cannot fail its own swap"
    );
    let stale = f
        .post(
            SET_CAST,
            json!({"op":"stale-cast","world":WORLD,"scene":scene,"cast":[MEMBER],"swapRecord":opened["cid"]}),
            GM,
        )
        .await;
    assert_eq!(stale.1["error"], "InvalidSwap");
    assert_eq!(
        f.get_scene(&scene).await,
        record,
        "a stale cast change must not overwrite the current record"
    );
    assert_eq!(f.admit(&scene, PLAYER, "after-player").await["status"], "admitted");
    assert_eq!(f.admit(&scene, GM, "after-gm").await["status"], "admitted");
    assert_eq!(f.admit(&scene, MEMBER, "after-member").await["reason"], "NotInCast");

    f.reopen().await;
    assert_eq!(f.get_scene(&scene).await, record);
    assert_eq!(
        f.post(OPEN, open_body, GM).await.1,
        opened,
        "the opening receipt survives replay"
    );
    assert_eq!(
        f.post(SET_CAST, cast_body, GM).await.1,
        changed,
        "the cast receipt survives replay"
    );
    assert_eq!(f.admit(&scene, MEMBER, "replay-member").await["reason"], "NotInCast");
    assert_eq!(f.admit(&scene, PLAYER, "replay-player").await["status"], "admitted");
    let close_body = json!({"op":"gm-close","world":WORLD,"scene":scene,"swapRecord":record["cid"]});
    let (status, closed) = f.post(CLOSE, close_body.clone(), GM).await;
    assert_eq!(status, StatusCode::OK, "{closed}");
    let final_record = f.get_scene(&scene).await;
    assert_eq!(final_record["cid"], closed["cid"]);
    assert_eq!(final_record["value"]["state"], "closed");
    assert_eq!(final_record["value"]["cast"], json!([GM, PLAYER]));
    assert_eq!(f.admit(&scene, PLAYER, "closed-player").await["reason"], "SceneClosed");
    assert_eq!(
        f.post(
            SET_CAST,
            json!({"op":"closed-cast","world":WORLD,"scene":scene,"cast":[MEMBER],"swapRecord":closed["cid"]}),
            GM
        )
        .await
        .1["error"],
        "SceneClosed"
    );
    f.reopen().await;
    assert_eq!(f.get_scene(&scene).await, final_record);
    assert_eq!(
        f.post(CLOSE, close_body, GM).await.1,
        closed,
        "the closure receipt survives replay"
    );
    assert_eq!(f.admit(&scene, PLAYER, "replay-closed").await["reason"], "SceneClosed");
    let audits = f.audit().await;
    for action in [OPEN, SET_CAST, CLOSE] {
        assert_eq!(
            audits
                .iter()
                .filter(|a| a["actor"] == GM && a["action"] == action && a["subject"] == opened["uri"])
                .count(),
            1,
            "successful retries do not duplicate scene audits"
        );
    }
}

/// If attribution cannot be written, neither the public record nor the rules
/// may change. Catches committing the scene before a failing audit append.
#[tokio::test]
async fn audit_failure_rolls_back_the_public_record_and_cast_decision() {
    let f = Fixture::new().await;
    let (status, opened) = f
        .post(
            OPEN,
            json!({
                "op":"audit-open", "world":WORLD, "title":"Audited", "cast":[MEMBER]
            }),
            GM,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{opened}");
    let scene = opened["scene"].as_str().expect("scene key").to_owned();
    let before = f.get_scene(&scene).await;
    f.handle()
        .run(DeleteAuditSpace)
        .await
        .expect("delete reaches writer")
        .expect("audit Space deleted");
    let (status, _) = f
        .post(
            SET_CAST,
            json!({
                "op":"audit-cast", "world":WORLD, "scene":scene, "cast":[PLAYER], "swapRecord":opened["cid"]
            }),
            GM,
        )
        .await;
    assert_ne!(status, StatusCode::OK);
    assert_eq!(f.get_scene(&scene).await, before);
    let member = f
        .handle()
        .run(CheckCast {
            scene: scene.clone(),
            author: MEMBER,
        })
        .await
        .expect("check reaches writer")
        .expect("World is readable");
    let player = f
        .handle()
        .run(CheckCast { scene, author: PLAYER })
        .await
        .expect("check reaches writer")
        .expect("World is readable");
    assert_eq!(member, Ok(()));
    assert_eq!(player, Err(Refusal::NotInCast));
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(16))]

    /// Discovery agrees with writes across role combinations and independent grants.
    /// Catches hard-coded GM checks, merging another role's grants, or granting both controls together.
    #[test]
    fn scene_controls_match_mutations_for_generated_roles_and_grants(
        roles in 0_u8..4,
        player_grants in 0_u8..4,
        member_grants in 0_u8..4,
    ) {
        tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(async {
            let f = Fixture::new().await;
            let mut opened = Vec::with_capacity(2);
            for op in ["cast-target", "close-target"] {
                let (status, scene) = f.post(OPEN, json!({"op":op,"world":WORLD,"title":op}), OWNER).await;
                assert_eq!(status, StatusCode::OK, "{scene}");
                opened.push(scene);
            }
            let assigned: Vec<_> = [(1, "player"), (2, "member")].into_iter()
                .filter_map(|(bit, role)| (roles & bit != 0).then_some(role)).collect();
            let mut member = write("group.opensocial.membership", PLAYER, &membership(PLAYER, &assigned));
            member.action = OpAction::Update;
            let grants: Vec<_> = [("player", player_grants), ("member", member_grants)].into_iter()
                .map(|(role, mask)| {
                    let scopes: Vec<_> = [(1, SET_CAST), (2, CLOSE)].into_iter()
                        .filter(|(bit, _)| mask & bit != 0).map(|(_, method)| format!("rpc:{method}")).collect();
                    json!({"role":role,"assignable":[],"scopes":scopes})
                }).collect();
            let mut permissions = write("group.opensocial.permissions", "self", &json!({"grants":grants}));
            permissions.action = OpAction::Update;
            f.admin(vec![member, permissions]).await;
            let signed = f.token(PLAYER, WORLD, CONTROLS);
            let (status, controls) = f.controls(WORLD, &signed).await;
            if roles == 0 {
                assert_eq!(status, StatusCode::FORBIDDEN, "{controls}");
            } else {
                assert_eq!(status, StatusCode::OK, "{controls}");
            }
            let granted = if roles & 1 != 0 { player_grants } else { 0 }
                | if roles & 2 != 0 { member_grants } else { 0 };
            for (index, (method, field, bit)) in [(SET_CAST, "setCast", 1), (CLOSE, "close", 2)].into_iter().enumerate() {
                let scene = &opened[index];
                let (status, body) = f.post(method, json!({
                    "op":format!("mutate-{index}"), "world":WORLD, "scene":scene["scene"],
                    "swapRecord":scene["cid"], "cast":[MEMBER],
                }), PLAYER).await;
                let allowed = granted & bit != 0;
                assert_eq!(status, if allowed { StatusCode::OK } else { StatusCode::FORBIDDEN }, "{body}");
                if roles != 0 {
                    assert_eq!(controls[field], allowed);
                }
            }
        });
    }
}

/// A discovery credential is reusable but cannot retain authority after role deletion.
/// Catches consuming query tokens, cached policy, and trusting discovery instead of checking a write.
#[tokio::test]
async fn scene_controls_are_read_only_and_lose_removed_roles_before_mutation() {
    let f = Fixture::new().await;
    let (status, opened) = f
        .post(OPEN, json!({"op":"target","world":WORLD,"title":"Target"}), GM)
        .await;
    assert_eq!(status, StatusCode::OK, "{opened}");
    let signed = f.token(GM, &format!("{WORLD}#motet_tenor"), CONTROLS);
    let before = f.audit().await;
    for _ in 0..2 {
        let (status, controls) = f.controls(WORLD, &signed).await;
        assert_eq!(status, StatusCode::OK, "{controls}");
        assert_eq!(controls, json!({"setCast":true,"close":true}));
    }
    let claims = signed.split('.').nth(1).expect("JWT claims");
    let claims: Value = serde_json::from_slice(&URL_SAFE_NO_PAD.decode(claims).expect("base64 claims")).unwrap();
    assert!(
        f.handle()
            .reader()
            .get(&keys::service_token(&Did::new(GM), claims["jti"].as_str().unwrap()))
            .await
            .unwrap()
            .is_none(),
        "read-only query must not spend a token"
    );
    assert_eq!(f.audit().await, before);
    let mut remove = write("group.opensocial.role", "gm", &json!({"displayName":"GM"}));
    remove.action = OpAction::Delete;
    remove.value = None;
    f.admin(vec![remove]).await;
    assert_eq!(f.controls(WORLD, &signed).await.0, StatusCode::FORBIDDEN);
    for method in [SET_CAST, CLOSE] {
        assert_eq!(
            f.post(
                method,
                json!({
                    "op":"removed-role", "world":WORLD, "scene":opened["scene"],
                    "swapRecord":opened["cid"], "cast":[MEMBER],
                }),
                GM
            )
            .await
            .0,
            StatusCode::FORBIDDEN
        );
    }
}

/// Discovery only exposes a verified member's own target World's controls.
/// Catches skipping exact lxm validation or using the query World independently of the audience.
#[tokio::test]
async fn scene_controls_refuse_nonmembers_wrong_methods_and_cross_world_tokens() {
    let f = Fixture::new().await;
    let owner = f.token(OWNER, WORLD, CONTROLS);
    for query in [
        format!("{WORLD}&world={WORLD}"),
        format!("{WORLD}&actor={OWNER}"),
        "not-a-did".into(),
    ] {
        assert_eq!(f.controls(&query, &owner).await.0, StatusCode::BAD_REQUEST);
    }
    assert_eq!(
        f.controls(WORLD, &f.token(STRANGER, WORLD, CONTROLS)).await.0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        f.controls(WORLD, &f.token(OWNER, WORLD, CLOSE)).await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        f.controls(WORLD, &f.token(OWNER, "did:plc:another", CONTROLS)).await.0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        f.controls(
            WORLD,
            &f.token(OWNER, "did:web:elsewhere.example#motet_tenor", CONTROLS)
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
}
