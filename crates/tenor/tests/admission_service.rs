//! Admission crosses real service-auth verification, the shard writer and the
//! publication queue; only the author's PDS read-back is a test double.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use atproto_identity::key::{KeyData, KeyType, generate_key, to_public};
use atproto_identity::model::Document;
use atproto_identity::traits::IdentityResolver;
use atproto_oauth::jwt::{Claims, Header, JoseClaims, mint};
use atproto_space::SpaceUri;
use atproto_space::space_repo::OpAction;
use atproto_space::storage::SpaceRepoStorage;
use axum::body::{Body, to_bytes};
use axum::http::{Request as HttpRequest, StatusCode};
use motet_log::Log;
use motet_net::{Fetcher, Policy};
use motet_rules::world::{Command, Outcome, Refusal};
use motet_rules::{Cid, Did, OperationId, RecordUri, SceneId, StrongRef};
use motet_tenor::admission::{self, Decision, FetchError, Fetched, Records, Request};
use motet_tenor::api;
use motet_tenor::identity::Hosting;
use motet_tenor::keys::{MasterKey, Purpose};
use motet_tenor::publication::{self, PublicationError};
use motet_tenor::repo::{self, Write};
use motet_tenor::shard::{Entry, Handle, Op, Options, Shard, Txn, spawn};
use motet_tenor::spaces::{self, ProfileInput, SpaceError, SpaceWrite};
use motet_tenor::store::keys;
use motet_tenor::world::{Status, WorldInfo};
use object_store::ObjectStore;
use object_store::memory::InMemory;
use serde_json::{Value, json};
use tokio::sync::Notify;
use tower::ServiceExt as _;

const SHARD: &str = "authenticated-admission";
const WORLD: &str = "did:plc:ewvi7nxzyoun6zhxrhs64oiz";
const OTHER_WORLD: &str = "did:plc:aaaaaaaaaaaaaaaaaaaaaaaa";
const OWNER: &str = "did:web:owner.example";
const ALICE: &str = "did:web:alice.example";
const BOB: &str = "did:web:bob.example";
const TENOR: &str = "did:web:tenor.motet.test#motet_tenor";
const HANDLE: &str = "world.motet.test";
const METHOD: &str = "at.motet.pilot.play.submitAdmission";
const SCENE: &str = "at.motet.pilot.scene";
const MEMBERSHIP: &str = "group.opensocial.membership";

fn document(did: &str, key: &KeyData) -> Document {
    let public = to_public(key).expect("fixture key has a public component").to_string();
    serde_json::from_value(json!({
        "id": did,
        "verificationMethod": [{"id": format!("{did}#atproto"), "type": "Multikey", "controller": did,
            "publicKeyMultibase": public.strip_prefix("did:key:").expect("public key uses did:key encoding")}],
    }))
    .expect("fixture document has the DID document shape")
}

fn membership(member: &str) -> Value {
    json!({"member":member,"roles":["player"],"createdAt":"2026-10-26T00:00:00Z"})
}

fn space(kind: &str) -> SpaceUri {
    spaces::world_space(&Did::new(WORLD), kind).expect("fixture Space URI is valid")
}

struct Initialize;

impl Op for Initialize {
    type Output = ();
    type Error = PublicationError;

    async fn run(self, txn: &mut Txn<'_>) -> Result<(), Self::Error> {
        let world = Did::new(WORLD);
        txn.begin_world(world.clone(), 1)
            .expect("World starts in a fresh shard");
        let signing = txn.generate_key(&world, Purpose::Signing).expect("signing key");
        txn.generate_key(&world, Purpose::Kem).expect("KEM key");
        let info = WorldInfo {
            did: world.clone(),
            owner: Did::new(OWNER),
            handle: HANDLE.into(),
            epoch: 1,
            status: Status::Active,
            created: atproto_record::datetime::Datetime::now().into_string(),
            document: document(WORLD, &signing),
        };
        txn.put(keys::world_info(&world), serde_json::to_vec(&info)?.into());
        txn.put(keys::handle(HANDLE), WORLD.as_bytes().into());
        repo::genesis(txn, &world).await?;
        spaces::bootstrap(txn, &world, &Did::new(OWNER), ProfileInput::default()).await?;
        spaces::write_as_world(
            txn,
            &space(spaces::MEMBERS),
            vec![
                SpaceWrite::create("group.opensocial.role", "player", &json!({"displayName":"Player"}))?,
                SpaceWrite::create(MEMBERSHIP, ALICE, &membership(ALICE))?,
                SpaceWrite::create(MEMBERSHIP, BOB, &membership(BOB))?,
            ],
        )
        .await?;
        let record = json!({"$type":SCENE,"title":"The Reach","state":"open","cast":[OWNER, ALICE],"openedBy":OWNER,"createdAt":"2026-10-26T00:00:00Z"});
        repo::commit(
            txn,
            &world,
            vec![Write::Create {
                collection: SCENE.parse().expect("scene collection is a valid NSID"),
                rkey: "scene".into(),
                record: repo::record_from_json(&record).expect("scene record encodes"),
            }],
            None,
        )
        .await?;
        txn.command(
            world,
            OperationId::new("setup-scene"),
            Command::OpenScene {
                scene: SceneId::new("scene"),
                cast: BTreeSet::from([Did::new(OWNER), Did::new(ALICE)]),
            },
        )
        .await?
        .expect("scene opens");
        Ok(())
    }
}

struct AdminWrites(Vec<SpaceWrite>);

impl Op for AdminWrites {
    type Output = ();
    type Error = SpaceError;

    async fn run(self, txn: &mut Txn<'_>) -> Result<(), SpaceError> {
        spaces::write_as_world(txn, &space(spaces::MEMBERS), self.0).await?;
        Ok(())
    }
}

struct DeleteAuditSpace;

impl Op for DeleteAuditSpace {
    type Output = ();
    type Error = SpaceError;

    async fn run(self, txn: &mut Txn<'_>) -> Result<(), SpaceError> {
        spaces::delete(txn, &space(spaces::GMS)).await
    }
}

#[derive(Clone, Debug)]
struct Gate {
    first: Arc<AtomicBool>,
    entered: Arc<Notify>,
    release: Arc<Notify>,
}

impl Gate {
    fn new() -> Self {
        Self {
            first: Arc::new(AtomicBool::new(true)),
            entered: Arc::new(Notify::new()),
            release: Arc::new(Notify::new()),
        }
    }
}

#[derive(Clone, Debug)]
struct AuthorRepos {
    posts: Arc<BTreeMap<String, Fetched>>,
    gate: Option<Gate>,
}

impl Records for AuthorRepos {
    async fn fetch(&self, post: &StrongRef) -> Result<Option<Fetched>, FetchError> {
        if let Some(gate) = &self.gate
            && gate.first.swap(false, Ordering::SeqCst)
        {
            gate.entered.notify_one();
            gate.release.notified().await;
        }
        if post.uri.as_str().ends_with("/unreachable") {
            return Err(FetchError::Unavailable("author's PDS unavailable".into()));
        }
        Ok(self.posts.get(post.uri.as_str()).cloned())
    }
}

fn post(author: &str, rkey: &str) -> StrongRef {
    StrongRef::new(
        RecordUri::new(format!("at://{author}/at.motet.pilot.post/{rkey}")),
        Cid::new(atproto_dasl::cid::compute_cid(b"the author's version").to_string()),
    )
}

fn body(author: &str, rkey: &str, op: &str) -> Value {
    let post = post(author, rkey);
    json!({"world":WORLD,"op":op,"post":{"uri":post.uri.as_str(),"cid":post.cid.as_str()}})
}

struct Directory(BTreeMap<String, Document>);

#[async_trait::async_trait]
impl IdentityResolver for Directory {
    async fn resolve(&self, subject: &str) -> anyhow::Result<Document> {
        self.0
            .get(subject)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("unknown identity"))
    }
}

#[derive(Clone)]
struct Client {
    router: axum::Router,
    did: String,
    key: KeyData,
}

struct Fixture {
    store: Arc<dyn ObjectStore>,
    handle: Handle,
    router: axum::Router,
    records: AuthorRepos,
    keys: BTreeMap<String, KeyData>,
}

impl Fixture {
    async fn new(gate: Option<Gate>) -> Self {
        let store: Arc<dyn ObjectStore> = Arc::new(InMemory::new());
        let shard = Shard::open(Arc::clone(&store), SHARD, Arc::new(MasterKey::from_bytes(&[37; 32])))
            .await
            .expect("fresh shard opens");
        let handle = spawn(
            shard,
            Options {
                max_batch: 8,
                checkpoint_every: Duration::from_secs(3600),
            },
        );
        handle
            .run(Initialize)
            .await
            .expect("writer accepts setup")
            .expect("World initializes");
        let keys: BTreeMap<String, KeyData> = [OWNER, ALICE, BOB]
            .into_iter()
            .map(|did| {
                (
                    did.into(),
                    generate_key(KeyType::K256Private).expect("member signing key"),
                )
            })
            .collect();
        let resolver: Arc<dyn IdentityResolver> = Arc::new(Directory(
            keys.iter()
                .map(|(did, key)| (did.clone(), document(did, key)))
                .collect(),
        ));
        let fetcher = Fetcher::new(Policy::default()).expect("outbound policy");
        let hosting = Arc::new(
            Hosting::new(
                &"https://tenor.motet.test".parse().expect("valid tenor URL"),
                vec![".motet.test".into()],
                "https://plc.example".into(),
                fetcher,
                Vec::new(),
            )
            .expect("hosting configuration"),
        );
        let audience = hosting.audience().to_owned();
        let authenticate = motet_xrpc::service_auth::authenticate(
            resolver,
            Arc::new(move |aud| api::accepts_audience(aud, &audience)),
        );
        let mut posts = BTreeMap::new();
        for (uri_author, rkey, fetched_author, fetched_world) in [
            (ALICE, "one", ALICE, WORLD),
            (ALICE, "revoked", ALICE, WORLD),
            (ALICE, "rollback", ALICE, WORLD),
            (ALICE, "forged", BOB, WORLD),
            (ALICE, "elsewhere", ALICE, OTHER_WORLD),
            (BOB, "one", BOB, WORLD),
            (BOB, "two", BOB, WORLD),
        ] {
            let post = post(uri_author, rkey);
            posts.insert(
                post.uri.to_string(),
                Fetched {
                    cid: post.cid,
                    author: Did::new(fetched_author),
                    world: Did::new(fetched_world),
                    scene: SceneId::new("scene"),
                },
            );
        }
        let records = AuthorRepos {
            posts: Arc::new(posts),
            gate,
        };
        let state = Arc::new(api::State {
            shard: handle.clone(),
            records: records.clone(),
            hosting,
        });
        let router = api::router(state, authenticate).expect("tenor router");
        Self {
            store,
            handle,
            router,
            records,
            keys,
        }
    }

    fn client(&self, did: &str) -> Client {
        Client {
            router: self.router.clone(),
            did: did.into(),
            key: self.keys.get(did).expect("member signing key").clone(),
        }
    }

    async fn admin(&self, writes: Vec<SpaceWrite>) {
        self.handle
            .run(AdminWrites(writes))
            .await
            .expect("writer accepts admin change")
            .expect("admin change is valid");
    }

    async fn cast(&self, op: &str, members: &[&str]) {
        self.handle
            .submit(Entry::Apply {
                world: Did::new(WORLD),
                op: OperationId::new(op),
                command: Command::SetSceneCast {
                    scene: SceneId::new("scene"),
                    cast: members.iter().map(|member| Did::new(*member)).collect(),
                },
            })
            .await
            .expect("writer accepts cast change")
            .expect("cast change applies");
    }

    async fn audits(&self) -> Vec<Value> {
        spaces::storage::ReadStorage::new(self.handle.reader(), &Did::new(WORLD))
            .list_records(&space(spaces::GMS), "at.motet.pilot.audit", None, 100, true)
            .await
            .expect("GM audits are readable")
            .records
            .into_iter()
            .map(|row| atproto_dasl::from_slice_to_json(&row.value).expect("audit is valid DRISL"))
            .collect()
    }

    async fn segments(&self) -> usize {
        Log::new(Arc::clone(&self.store), SHARD)
            .read_from(0)
            .await
            .expect("shard log is readable")
            .len()
    }

    async fn drain(&self) -> (usize, usize) {
        let result = publication::drain(&self.handle)
            .await
            .expect("publication drain completes");
        (result.published, result.failed)
    }

    async fn published_admissions(&self) -> usize {
        self.handle
            .reader()
            .scan(
                &keys::repo_records(&Did::new(WORLD), Some("at.motet.pilot.admission")),
                None,
                usize::MAX,
            )
            .await
            .expect("World repo index is readable")
            .len()
    }
}

fn token(client: &Client, audience: &str, method: &str) -> String {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).expect("clock").as_secs();
    let mut claims = Claims::new(JoseClaims {
        issuer: Some(client.did.clone()),
        audience: Some(audience.into()),
        issued_at: Some(now),
        expiration: Some(now + 60),
        ..JoseClaims::default()
    });
    claims.private.insert("lxm".into(), method.into());
    let header = Header::try_from(client.key.clone()).expect("key has JWT algorithm");
    mint(&client.key, &header, &claims).expect("member signs service auth")
}

async fn send(client: &Client, body: Value, token: &str) -> (StatusCode, Value) {
    let response = client
        .router
        .clone()
        .oneshot(
            HttpRequest::post(format!("/xrpc/{METHOD}"))
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).expect("request serializes")))
                .expect("request URI and headers are valid"),
        )
        .await
        .expect("router is infallible");
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("response is bounded");
    (status, serde_json::from_slice(&bytes).expect("XRPC response is JSON"))
}

/// A player in the cast can publish without an RPC grant. A retry must not
/// append a second audit or a second public admission.
#[tokio::test]
async fn signed_player_admission_is_attributed_and_retries_publish_once() {
    let fixture = Fixture::new(None).await;
    let alice = fixture.client(ALICE);
    let token = token(&alice, WORLD, METHOD);
    let request = body(ALICE, "one", "first");
    let (status, first) = send(&alice, request.clone(), &token).await;
    assert_eq!(status, StatusCode::OK, "{first}");
    assert_eq!(first["status"], "admitted");
    let audits = fixture.audits().await;
    assert_eq!(audits.len(), 1);
    assert_eq!(audits[0]["actor"], ALICE);
    assert_eq!(audits[0]["action"], "admit");
    assert_eq!(audits[0]["subject"], post(ALICE, "one").uri.as_str());
    assert_eq!(send(&alice, request.clone(), &token).await, (status, first));
    assert_eq!(fixture.audits().await, audits);
    fixture
        .admin(vec![SpaceWrite {
            action: OpAction::Delete,
            collection: MEMBERSHIP.into(),
            rkey: ALICE.into(),
            value: None,
            swap: None,
        }])
        .await;
    let (status, denied) = send(&alice, request, &token).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{denied}");
    assert_eq!(fixture.audits().await, audits);
    assert_eq!(fixture.drain().await, (1, 0));
    assert_eq!(fixture.published_admissions().await, 1);
    assert_eq!(fixture.drain().await, (0, 0));
}

/// A member outside the cast receives a durable refusal, not a publication.
/// Replaying the same ID after a cast change cannot turn it into an admission
/// or append a second audit; a new operation may then succeed.
#[tokio::test]
async fn cast_and_recorded_refusals_survive_retries_and_cast_changes() {
    let fixture = Fixture::new(None).await;
    let bob = fixture.client(BOB);
    let token = token(&bob, WORLD, METHOD);
    let request = body(BOB, "one", "outside");
    let (status, first) = send(&bob, request.clone(), &token).await;
    assert_eq!(status, StatusCode::OK, "{first}");
    assert_eq!(first["status"], "refused");
    assert_eq!(first["reason"], "NotInCast");
    let audits = fixture.audits().await;
    assert_eq!(audits.len(), 1);
    assert_eq!(audits[0]["actor"], BOB);
    assert_eq!(audits[0]["action"], "refuseAdmission");
    assert_eq!(audits[0]["subject"], post(BOB, "one").uri.as_str());
    assert_eq!(fixture.drain().await, (0, 0));
    fixture.cast("add-bob", &[OWNER, ALICE, BOB]).await;
    assert_eq!(send(&bob, request, &token).await, (status, first));
    assert_eq!(fixture.audits().await, audits);
    let (status, collision) = send(&bob, body(BOB, "two", "outside"), &token).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{collision}");
    assert_eq!(fixture.audits().await, audits);
    let (status, admitted) = send(&bob, body(BOB, "one", "inside"), &token).await;
    assert_eq!(status, StatusCode::OK, "{admitted}");
    assert_eq!(admitted["status"], "admitted");
    assert_eq!(fixture.audits().await.len(), 2);
    assert_eq!(fixture.drain().await, (1, 0));
    assert_eq!(fixture.published_admissions().await, 1);
}

/// If automatic ingestion decided first, a signed retry must return that
/// decision without inventing an actor for the earlier write.
#[tokio::test]
async fn automatic_decisions_are_not_reattributed_by_later_member_retries() {
    let fixture = Fixture::new(None).await;
    for (author, expected) in [(ALICE, "admitted"), (BOB, "refused")] {
        let request = Request {
            op: OperationId::new(format!("{author}/automatic")),
            world: Did::new(WORLD),
            post: post(author, "one"),
            replaces: None,
        };
        let first = admission::admit(&fixture.handle, &fixture.records, request)
            .await
            .expect("automatic ingestion decides");
        match (author, first) {
            (ALICE, Decision::Decided(Ok(Outcome::Admitted(_))))
            | (BOB, Decision::Decided(Ok(Outcome::Refused(Refusal::NotInCast)))) => {}
            (_, other) => panic!("unexpected automatic decision: {other:?}"),
        }
        let member = fixture.client(author);
        let token = token(&member, &format!("{WORLD}#motet_tenor"), METHOD);
        let (status, result) = send(&member, body(author, "one", "automatic"), &token).await;
        assert_eq!(status, StatusCode::OK, "{result}");
        assert_eq!(result["status"], expected);
        assert!(fixture.audits().await.is_empty());
    }
    assert_eq!(fixture.drain().await, (1, 0));
    assert_eq!(fixture.published_admissions().await, 1);
}

/// A role removed while read-back is in flight must take effect at the writer.
/// Otherwise a valid but stale JWT could publish after membership was removed.
#[tokio::test]
async fn role_revoked_during_readback_prevents_admission_and_does_not_consume_the_operation() {
    let gate = Gate::new();
    let fixture = Fixture::new(Some(gate.clone())).await;
    let alice = fixture.client(ALICE);
    let token = token(&alice, WORLD, METHOD);
    let request = body(ALICE, "revoked", "revoked");
    let mut waiting = {
        let alice = alice.clone();
        let token = token.clone();
        let request = request.clone();
        tokio::spawn(async move { send(&alice, request, &token).await })
    };
    tokio::select! {
        () = gate.entered.notified() => {},
        result = &mut waiting => panic!("request returned before read-back: {result:?}"),
    }
    fixture
        .admin(vec![SpaceWrite {
            action: OpAction::Delete,
            collection: MEMBERSHIP.into(),
            rkey: ALICE.into(),
            value: None,
            swap: None,
        }])
        .await;
    gate.release.notify_one();
    let (status, refused) = waiting.await.expect("request task completes");
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{refused}");
    assert!(fixture.audits().await.is_empty());
    assert_eq!(fixture.drain().await, (0, 0));
    assert_eq!(fixture.published_admissions().await, 0);
    fixture
        .admin(vec![
            SpaceWrite::create(MEMBERSHIP, ALICE, &membership(ALICE)).expect("membership encodes"),
        ])
        .await;
    let (status, admitted) = send(&alice, request, &token).await;
    assert_eq!(status, StatusCode::OK, "{admitted}");
    assert_eq!(admitted["status"], "admitted");
    assert_eq!(fixture.audits().await.len(), 1);
}

/// Removing a player from the cast during PDS read-back prevents publication.
/// This catches deciding eligibility before the request reaches the writer.
#[tokio::test]
async fn cast_removal_during_readback_refuses_and_records_the_member_decision() {
    let gate = Gate::new();
    let fixture = Fixture::new(Some(gate.clone())).await;
    let alice = fixture.client(ALICE);
    let token = token(&alice, WORLD, METHOD);
    let mut waiting = tokio::spawn(async move { send(&alice, body(ALICE, "one", "removed-cast"), &token).await });
    tokio::select! {
        () = gate.entered.notified() => {},
        result = &mut waiting => panic!("request returned before read-back: {result:?}"),
    }
    fixture.cast("remove-cast", &[OWNER]).await;
    gate.release.notify_one();
    let (status, refused) = waiting.await.expect("request task completes");
    assert_eq!(status, StatusCode::OK, "{refused}");
    assert_eq!(refused["status"], "refused");
    assert_eq!(refused["reason"], "NotInCast");
    let audits = fixture.audits().await;
    assert_eq!(audits.len(), 1);
    assert_eq!(audits[0]["actor"], ALICE);
    assert_eq!(audits[0]["action"], "refuseAdmission");
    assert_eq!(fixture.drain().await, (0, 0));
    assert_eq!(fixture.published_admissions().await, 0);
}

/// Neither a foreign URI nor a PDS response naming a different author may use
/// the signer's cast membership. Missing and unreachable records stay pending.
#[tokio::test]
async fn verified_issuer_and_readback_are_both_required_before_any_decision() {
    let fixture = Fixture::new(None).await;
    let before = fixture.segments().await;
    let alice = fixture.client(ALICE);
    let token = token(&alice, WORLD, METHOD);
    for request in [body(BOB, "one", "foreign"), body(ALICE, "forged", "forged")] {
        let (status, result) = send(&alice, request, &token).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{result}");
    }
    for rkey in ["missing", "unreachable"] {
        let (status, result) = send(&alice, body(ALICE, rkey, rkey), &token).await;
        assert_eq!(status, StatusCode::OK, "{result}");
        assert_eq!(result["status"], "pending");
    }
    let (status, _) = send(&alice, body(ALICE, "elsewhere", "other-world"), &token).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let mut mismatched_world = body(ALICE, "one", "mismatched-world");
    mismatched_world["world"] = OTHER_WORLD.into();
    let (status, result) = send(&alice, mismatched_world, &token).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{result}");
    let wrong_method = self::token(&alice, WORLD, "com.atproto.repo.createRecord");
    let wrong_audience = self::token(&alice, TENOR, METHOD);
    for invalid in [wrong_method, wrong_audience] {
        let (status, result) = send(&alice, body(ALICE, "one", "invalid-token"), &invalid).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{result}");
    }
    assert!(fixture.audits().await.is_empty());
    assert_eq!(fixture.drain().await, (0, 0));
    assert_eq!(fixture.published_admissions().await, 0);
    assert_eq!(fixture.segments().await, before, "nothing reached the World's log");
}

/// A failed audit must roll back the command and publication intent together.
/// Automatic ingestion remains usable without inventing a member actor.
#[tokio::test]
async fn audit_failure_rolls_back_the_decision_while_automatic_ingestion_still_works() {
    let fixture = Fixture::new(None).await;
    fixture
        .handle
        .run(DeleteAuditSpace)
        .await
        .expect("writer accepts deletion")
        .expect("audit Space deleted");
    let alice = fixture.client(ALICE);
    let token = token(&alice, WORLD, METHOD);
    let (status, result) = send(&alice, body(ALICE, "rollback", "rollback"), &token).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR, "{result}");
    assert_eq!(fixture.drain().await, (0, 0));
    assert_eq!(fixture.published_admissions().await, 0);

    fixture.cast("remove-alice", &[OWNER]).await;
    let request = Request {
        op: OperationId::new(format!("{ALICE}/rollback")),
        world: Did::new(WORLD),
        post: post(ALICE, "rollback"),
        replaces: None,
    };
    let decision = admission::admit(&fixture.handle, &fixture.records, request)
        .await
        .expect("automatic ingestion decides");
    assert_eq!(decision, Decision::Decided(Ok(Outcome::Refused(Refusal::NotInCast))));
    assert_eq!(fixture.drain().await, (0, 0));

    fixture.cast("restore-alice", &[OWNER, ALICE]).await;
    let request = Request {
        op: OperationId::new("automatic-after-failure"),
        world: Did::new(WORLD),
        post: post(ALICE, "rollback"),
        replaces: None,
    };
    let decision = admission::admit(&fixture.handle, &fixture.records, request)
        .await
        .expect("automatic ingestion decides");
    assert!(matches!(decision, Decision::Decided(Ok(Outcome::Admitted(_)))));
    assert_eq!(fixture.drain().await, (1, 0));
    assert_eq!(fixture.published_admissions().await, 1);
}
