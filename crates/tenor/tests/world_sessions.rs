//! The authenticator is a fixture boundary here; these tests exercise the real
//! XRPC registry, current opensocial policy, repo/Space writers and shard rollback.

use std::sync::Arc;
use std::time::Duration;

use atproto_identity::model::Document;
use atproto_identity::traits::IdentityResolver;
use atproto_oauth::scopes::{RepoAction, ScopesSet, SpaceAction, SpaceTarget};
use atproto_space::SpaceUri;
use atproto_space::credential::verify_delegation_token;
use atproto_space::space_repo::OpAction;
use atproto_space::storage::SpaceRepoStorage;
use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use bytes::Bytes;
use motet_net::{Fetcher, Policy};
use motet_rules::Did;
use motet_tenor::keys::{MasterKey, Purpose};
use motet_tenor::oauth::{grants, writes};
use motet_tenor::repo;
use motet_tenor::shard::{Handle, Op, Options, Shard, Txn, spawn};
use motet_tenor::spaces::{self, ProfileInput, SpaceError, SpaceWrite, http};
use motet_tenor::store::keys;
use motet_xrpc::Caller;
use motet_xrpc::server::{Authenticate, Service};
use object_store::memory::InMemory;
use serde_json::{Value, json};
use tower::ServiceExt as _;

const WORLD: &str = "did:plc:ewvi7nxzyoun6zhxrhs64oiz";
const OWNER: &str = "did:web:gm.example";
const EDITOR: &str = "did:web:editor.example";
const TARGET: &str = "did:web:member.example";
const PROFILE: &str = "at.motet.pilot.profile";
const ALL: &[&str] = &[
    "atproto",
    "repo:*",
    "space:*?authority=*&collection=*&manage=create&manage=delete",
];

struct Initialize;

impl Op for Initialize {
    type Output = ();
    type Error = SpaceError;

    async fn run(self, txn: &mut Txn<'_>) -> Result<(), SpaceError> {
        let world = Did::new(WORLD);
        txn.begin_world(world.clone(), 1)
            .expect("fixture World starts in a fresh shard");
        txn.generate_key(&world, Purpose::Signing)
            .expect("fixture signing key generation succeeds");
        txn.generate_key(&world, Purpose::Kem)
            .expect("fixture KEM key generation succeeds");
        repo::genesis(txn, &world).await?;
        spaces::bootstrap(txn, &world, &Did::new(OWNER), ProfileInput::default()).await
    }
}

struct AdminWrites {
    space: SpaceUri,
    writes: Vec<SpaceWrite>,
}

impl Op for AdminWrites {
    type Output = ();
    type Error = SpaceError;
    async fn run(self, txn: &mut Txn<'_>) -> Result<(), SpaceError> {
        spaces::write_as_world(txn, &self.space, self.writes).await?;
        Ok(())
    }
}

struct RemoveAuditSpace;

impl Op for RemoveAuditSpace {
    type Output = ();
    type Error = SpaceError;
    async fn run(self, txn: &mut Txn<'_>) -> Result<(), SpaceError> {
        spaces::delete(txn, &space(spaces::GMS)).await
    }
}

struct Offline;

#[async_trait::async_trait]
impl IdentityResolver for Offline {
    async fn resolve(&self, _: &str) -> anyhow::Result<Document> {
        anyhow::bail!("session mutations must not resolve network identities")
    }
}

struct Fixture {
    handle: Handle,
}

impl Fixture {
    async fn new() -> Self {
        let shard = Shard::open(
            Arc::new(InMemory::new()),
            "session-writes",
            Arc::new(MasterKey::from_bytes(&[81; 32])),
        )
        .await
        .expect("fresh in-memory shard opens");
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
            .expect("fixture initialization reaches the writer")
            .expect("fixture World initialization succeeds");
        Self { handle }
    }

    fn router(&self, actor: &str, scopes: &[&str]) -> axum::Router {
        let caller = Caller::Session {
            world: WORLD.into(),
            actor: actor.into(),
            client_id: "https://client.example/metadata.json".into(),
            scopes: scopes.iter().map(|scope| (*scope).into()).collect(),
        };
        let auth: Authenticate = Arc::new(move |_| {
            let caller = caller.clone();
            Box::pin(async move { Ok(caller) })
        });
        let state = Arc::new(http::State {
            shard: self.handle.clone(),
            resolver: Arc::new(Offline),
            fetcher: Fetcher::new(Policy::default()).expect("default outbound policy is valid"),
        });
        http::register(writes::register(Service::new(auth), &self.handle), &state)
            .into_router(())
            .expect("session and Space method declarations are unique")
    }

    async fn admin(&self, space: SpaceUri, writes: Vec<SpaceWrite>) {
        self.handle
            .run(AdminWrites { space, writes })
            .await
            .expect("fixture writes reach the writer")
            .expect("trusted fixture writes satisfy their schemas");
    }

    async fn visible(&self) -> Vec<(Bytes, Bytes)> {
        let mut rows = Vec::new();
        for prefix in [b"h/".as_slice(), b"r/", b"b/", b"s/", keys::EVENTS] {
            rows.extend(
                self.handle
                    .reader()
                    .scan(prefix, None, usize::MAX)
                    .await
                    .expect("fixture state is readable"),
            );
        }
        rows
    }

    async fn audits(&self) -> Vec<Value> {
        spaces::storage::ReadStorage::new(self.handle.reader(), &Did::new(WORLD))
            .list_records(&space(spaces::GMS), "at.motet.pilot.audit", None, 100, true)
            .await
            .expect("GM audit records are readable")
            .records
            .into_iter()
            .map(|row| atproto_dasl::from_slice_to_json(&row.value).expect("persisted audit is valid DRISL"))
            .collect()
    }

    async fn editor(&self, scopes: &[&str]) -> Value {
        let members = space(spaces::MEMBERS);
        let row = spaces::record(self.handle.reader(), &members, "group.opensocial.permissions", "self")
            .await
            .expect("fixture permissions are readable")
            .expect("bootstrap created permissions");
        let mut permissions = atproto_dasl::from_slice_to_json(&row.value).expect("permissions are valid DRISL");
        permissions
            .get_mut("grants")
            .expect("permissions have grants")
            .as_array_mut()
            .expect("permission grants are an array")
            .push(json!({"role":"editor", "scopes":scopes, "assignable":["reader"]}));
        self.admin(
            members,
            vec![
                create("group.opensocial.role", "editor", &json!({"displayName":"Editor"})),
                create("group.opensocial.role", "reader", &json!({"displayName":"Reader"})),
                create("group.opensocial.membership", EDITOR, &membership(EDITOR, &["editor"])),
                create("group.opensocial.membership", TARGET, &membership(TARGET, &["reader"])),
                update("group.opensocial.permissions", "self", &permissions),
            ],
        )
        .await;
        permissions
    }
}

fn space(kind: &str) -> SpaceUri {
    spaces::world_space(&Did::new(WORLD), kind).expect("fixture World and Space type form a valid URI")
}

fn create(collection: &str, rkey: &str, value: &Value) -> SpaceWrite {
    SpaceWrite::create(collection, rkey, value).expect("fixture record encodes as DRISL")
}

fn update(collection: &str, rkey: &str, value: &Value) -> SpaceWrite {
    let mut write = create(collection, rkey, value);
    write.action = OpAction::Update;
    write
}

fn membership(member: &str, roles: &[&str]) -> Value {
    json!({"member":member,"roles":roles,"createdAt":"2026-10-26T00:00:00Z"})
}

fn profile(name: &str) -> Value {
    json!({"$type":PROFILE,"displayName":name,"createdAt":"2026-10-26T00:00:00Z"})
}

async fn request(router: &axum::Router, nsid: &str, body: Value) -> (StatusCode, Value) {
    let response = router
        .clone()
        .oneshot(
            Request::post(format!("/xrpc/{nsid}"))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).expect("request body serializes")))
                .expect("request URI and headers are valid"),
        )
        .await
        .expect("router is infallible");
    let status = response.status();
    let body = to_bytes(response.into_body(), 2 * 1024 * 1024)
        .await
        .expect("response fits the fixture limit");
    (
        status,
        if body.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&body).expect("nonempty XRPC response is JSON")
        },
    )
}

async fn delegation(router: &axum::Router, space: &SpaceUri) -> (StatusCode, Value) {
    let query: String = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("space", &space.to_string())
        .finish();
    let response = router
        .clone()
        .oneshot(
            Request::get(format!("/xrpc/com.atproto.space.getDelegationToken?{query}"))
                .body(Body::empty())
                .expect("delegation request URI is valid"),
        )
        .await
        .expect("router is infallible");
    let status = response.status();
    (
        status,
        serde_json::from_slice(
            &to_bytes(response.into_body(), 1024 * 1024)
                .await
                .expect("delegation response is bounded"),
        )
        .expect("delegation response is JSON"),
    )
}

/// A queued request cannot retain yesterday's role. Catches checking only at
/// consent/authentication or leaving one CRUD/lifecycle entry point unchecked.
#[tokio::test]
async fn role_removal_rejects_every_session_mutation_before_state_changes() {
    let fixture = Fixture::new().await;
    let router = fixture.router(OWNER, ALL);
    let public = json!({"repo":WORLD,"collection":PROFILE,"rkey":"self","record":profile("Before")});
    assert_eq!(
        request(&router, "com.atproto.repo.createRecord", public.clone())
            .await
            .0,
        StatusCode::OK
    );
    let modality = json!({"type":"at.motet.pilot.space.scene","skey":"one","access":{"public":false,"grants":[{"role":"owner","scopes":["space:*?collection=*"]}]}});
    let (status, created) = request(&router, "group.opensocial.createSpace", modality.clone()).await;
    assert_eq!(status, StatusCode::OK, "{created}");
    fixture
        .admin(
            space(spaces::MEMBERS),
            vec![SpaceWrite {
                action: OpAction::Delete,
                collection: "group.opensocial.role".into(),
                rkey: "owner".into(),
                value: None,
                swap: None,
            }],
        )
        .await;
    let before = fixture.visible().await;
    for nsid in [
        "com.atproto.repo.createRecord",
        "com.atproto.repo.putRecord",
        "com.atproto.repo.deleteRecord",
    ] {
        assert!(!request(&router, nsid, public.clone()).await.0.is_success());
    }
    assert!(!request(&router, "com.atproto.repo.applyWrites", json!({"repo":WORLD,"writes":[{"$type":"com.atproto.repo.applyWrites#update","collection":PROFILE,"rkey":"self","value":profile("After")}]})).await.0.is_success());
    let private = json!({"space":space(spaces::META).to_string(),"repo":WORLD,"collection":"group.opensocial.profile","rkey":"self","record":{"$type":"group.opensocial.profile","displayName":"After","joinPolicy":"invite"}});
    for nsid in [
        "com.atproto.space.createRecord",
        "com.atproto.space.putRecord",
        "com.atproto.space.deleteRecord",
    ] {
        assert!(!request(&router, nsid, private.clone()).await.0.is_success());
    }
    assert!(!request(&router, "com.atproto.space.applyWrites", json!({"space":space(spaces::META).to_string(),"repo":WORLD,"writes":[{"$type":"com.atproto.space.applyWrites#update","collection":"group.opensocial.profile","rkey":"self","value":private["record"]}]})).await.0.is_success());
    assert!(
        !request(&router, "group.opensocial.createSpace", modality)
            .await
            .0
            .is_success()
    );
    for nsid in ["group.opensocial.deleteSpace", "com.atproto.simplespace.deleteSpace"] {
        assert!(
            !request(&router, nsid, json!({"space":created["space"]}))
                .await
                .0
                .is_success()
        );
    }
    assert!(!delegation(&router, &space(spaces::META)).await.0.is_success());
    assert_eq!(fixture.visible().await, before);
}

/// A create-only grant cannot update via putRecord, delete, cross collections
/// in a batch, or write an unknown schema even with validate=false.
#[tokio::test]
async fn narrow_scopes_allow_only_the_named_action_and_collection_with_an_audit() {
    let fixture = Fixture::new().await;
    let router = fixture.router(OWNER, &["atproto", "repo:at.motet.pilot.profile?action=create"]);
    let public = json!({"repo":WORLD,"collection":PROFILE,"rkey":"self","record":profile("Allowed")});
    let (status, result) = request(&router, "com.atproto.repo.createRecord", public.clone()).await;
    assert_eq!(status, StatusCode::OK, "{result}");
    let audits = fixture.audits().await;
    assert_eq!(audits.len(), 1);
    assert_eq!(audits[0]["actor"], OWNER);
    assert_eq!(audits[0]["action"], "create");
    assert_eq!(audits[0]["subject"], result["uri"]);
    assert!(atproto_record::datetime::Datetime::new(audits[0]["at"].as_str().unwrap()).is_ok());
    let before = fixture.visible().await;
    for nsid in ["com.atproto.repo.putRecord", "com.atproto.repo.deleteRecord"] {
        assert!(!request(&router, nsid, public.clone()).await.0.is_success());
    }
    let record = json!({"$type":"at.motet.pilot.character","name":"Not granted","createdAt":"2026-10-26T00:00:00Z"});
    assert!(!request(&router, "com.atproto.repo.applyWrites", json!({"repo":WORLD,"writes":[{"$type":"com.atproto.repo.applyWrites#create","collection":"at.motet.pilot.character","value":record}]})).await.0.is_success());
    let wide = fixture.router(OWNER, ALL);
    assert!(!request(&wide, "com.atproto.repo.createRecord", json!({"repo":WORLD,"collection":"unknown.example.record","rkey":"self","record":{"$type":"unknown.example.record"},"validate":false})).await.0.is_success());
    assert_eq!(fixture.visible().await, before);
}

/// Repo and Space commits must not outlive their audit. Catches appending audit
/// in a second operation or swallowing an audit-destination storage failure.
#[tokio::test]
async fn audit_failure_rolls_back_public_and_private_records_and_events() {
    let fixture = Fixture::new().await;
    let router = fixture.router(OWNER, ALL);
    fixture.handle.run(RemoveAuditSpace).await.unwrap().unwrap();
    let before = fixture.visible().await;
    assert!(
        !request(
            &router,
            "com.atproto.repo.createRecord",
            json!({"repo":WORLD,"collection":PROFILE,"rkey":"self","record":profile("Must roll back")})
        )
        .await
        .0
        .is_success()
    );
    assert_eq!(fixture.visible().await, before);
    assert!(!request(&router, "com.atproto.space.putRecord", json!({"space":space(spaces::META).to_string(),"repo":WORLD,"collection":"group.opensocial.profile","rkey":"self","record":{"$type":"group.opensocial.profile","displayName":"Must roll back","joinPolicy":"invite"}})).await.0.is_success());
    assert_eq!(fixture.visible().await, before);
}

/// The two grants sources union, but access grants never escape their Space.
/// Catches treating include names literally, dropping RPC members of a set,
/// issuing a wildcard from one collection, or giving atproto to a nonmember.
#[tokio::test]
async fn consent_expands_permission_sets_and_confines_local_grants() {
    let fixture = Fixture::new().await;
    fixture
        .editor(&[
            "include:group.opensocial.manageMembers",
            "repo:at.motet.pilot.profile?action=create",
        ])
        .await;
    fixture
        .admin(
            space(spaces::META),
            vec![update(
                "group.opensocial.access",
                "self",
                &json!({"public":false,"grants":[{"role":"editor","scopes":["include:group.opensocial.configure"]}]}),
            )],
        )
        .await;
    let requested: Vec<String> = [
        "atproto",
        "include:group.opensocial.manageMembers",
        "include:group.opensocial.configure",
        "repo:*",
        "space:*?action=read",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    let granted = grants::granted(fixture.handle.reader(), &Did::new(WORLD), &Did::new(EDITOR), &requested)
        .await
        .unwrap();
    assert!(granted.iter().all(|scope| !scope.starts_with("include:")));
    let scopes = ScopesSet::from_scopes(granted).with_subject(WORLD);
    assert!(scopes.allows_repo(PROFILE, &RepoAction::Create));
    assert!(!scopes.allows_repo(PROFILE, &RepoAction::Update));
    assert!(!scopes.allows_repo("at.motet.pilot.character", &RepoAction::Create));
    assert!(scopes.allows_rpc("group.opensocial.denyJoinRequest", WORLD));
    assert!(scopes.allows_space(&SpaceTarget::with_collection(
        spaces::META,
        WORLD,
        "self",
        SpaceAction::Update,
        "group.opensocial.profile"
    )));
    assert!(!scopes.allows_space(&SpaceTarget::with_collection(
        spaces::MEMBERS,
        WORLD,
        "self",
        SpaceAction::Update,
        "group.opensocial.role"
    )));
    assert!(!scopes.allows_space(&SpaceTarget::new(spaces::GMS, WORLD, "self", SpaceAction::Read)));
    assert!(matches!(
        grants::granted(
            fixture.handle.reader(),
            &Did::new(WORLD),
            &Did::new("did:web:stranger.example"),
            &requested
        )
        .await,
        Err(SpaceError::NotAuthorized)
    ));
}

/// Record-write scopes do not imply authority to assign every role or to mint
/// new permissions. Catches checking just the collection's OAuth scope.
#[tokio::test]
async fn membership_and_grant_changes_cannot_escalate_assignable_or_scopes() {
    let fixture = Fixture::new().await;
    let permissions = fixture
        .editor(&[
            "include:group.opensocial.manageMembers",
            "space:group.opensocial.members?collection=group.opensocial.permissions&action=update",
            "space:*?collection=group.opensocial.access&action=update",
        ])
        .await;
    let router = fixture.router(EDITOR, ALL);
    let before = fixture.visible().await;
    let members = space(spaces::MEMBERS).to_string();
    assert!(!request(&router, "com.atproto.space.putRecord", json!({"space":members,"repo":WORLD,"collection":"group.opensocial.membership","rkey":TARGET,"record":{"$type":"group.opensocial.membership","member":TARGET,"roles":["owner"],"createdAt":"2026-10-26T00:00:00Z"}})).await.0.is_success());
    for grant in [
        json!({"role":"reader","scopes":["repo:*"],"assignable":[]}),
        json!({"role":"reader","scopes":[],"assignable":["owner"]}),
        json!({"role":"editor","scopes":["repo:*"],"assignable":["reader"]}),
    ] {
        let mut changed = permissions.clone();
        if grant["role"] == "editor" {
            changed["grants"][1] = grant;
        } else {
            changed["grants"].as_array_mut().unwrap().push(grant);
        }
        assert!(!request(&router, "com.atproto.space.putRecord", json!({"space":members,"repo":WORLD,"collection":"group.opensocial.permissions","rkey":"self","record":changed})).await.0.is_success());
    }
    let row = spaces::record(
        fixture.handle.reader(),
        &space(spaces::META),
        "group.opensocial.access",
        "self",
    )
    .await
    .unwrap()
    .unwrap();
    let mut access = atproto_dasl::from_slice_to_json(&row.value).unwrap();
    access["grants"]
        .as_array_mut()
        .unwrap()
        .push(json!({"role":"reader","scopes":["space:*?collection=*&action=delete"]}));
    assert!(!request(&router, "com.atproto.space.putRecord", json!({"space":space(spaces::META).to_string(),"repo":WORLD,"collection":"group.opensocial.access","rkey":"self","record":access})).await.0.is_success());
    assert_eq!(fixture.visible().await, before);
    let mut allowed = permissions;
    allowed["grants"]
        .as_array_mut()
        .unwrap()
        .push(json!({"role":"reader","scopes":["include:group.opensocial.manageMembers"],"assignable":["reader"]}));
    let (status, result) = request(&router, "com.atproto.space.putRecord", json!({"space":members,"repo":WORLD,"collection":"group.opensocial.permissions","rkey":"self","record":allowed})).await;
    assert_eq!(status, StatusCode::OK, "{result}");
    let (status, result) = request(
        &router,
        "com.atproto.space.deleteRecord",
        json!({"space":members,"repo":WORLD,"collection":"group.opensocial.membership","rkey":TARGET}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{result}");
    assert_eq!(
        spaces::roles(fixture.handle.reader(), &Did::new(WORLD), &Did::new(TARGET))
            .await
            .unwrap(),
        Vec::<String>::new(),
    );
    assert_eq!(fixture.audits().await[0]["actor"], EDITOR);
}

/// A verified OAuth client can delegate only whole-Space read, not `read_self`
/// or write-only access. The returned token must verify as the World, not GM.
#[tokio::test]
async fn delegation_uses_verified_session_identity_and_covering_read_scope() {
    let fixture = Fixture::new().await;
    let meta = space(spaces::META);
    for action in ["read_self", "update"] {
        let scope = format!("space:group.opensocial.meta?action={action}");
        assert!(
            !delegation(&fixture.router(OWNER, &["atproto", &scope]), &meta)
                .await
                .0
                .is_success()
        );
    }
    let (status, result) = delegation(
        &fixture.router(OWNER, &["atproto", "space:group.opensocial.meta?action=read"]),
        &meta,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{result}");
    let key = fixture
        .handle
        .reader()
        .key(&Did::new(WORLD), Purpose::Signing)
        .await
        .unwrap()
        .unwrap();
    let claims = verify_delegation_token(result["token"].as_str().unwrap(), WORLD, &meta, &key).unwrap();
    assert_eq!(claims.iss, WORLD);
    assert_ne!(claims.iss, OWNER);
}

/// Space CRUD and batches name each action's record in the same audit stream.
/// Catches an unaudited batch path, a scope checked only by collection, or
/// stale swaps which leave audit evidence for a write that did not happen.
#[tokio::test]
async fn space_crud_and_batches_preserve_scope_swaps_and_attributed_audits() {
    let fixture = Fixture::new().await;
    let router = fixture.router(
        OWNER,
        &[
            "atproto",
            "space:group.opensocial.meta?collection=group.opensocial.rule",
        ],
    );
    let first = atproto_record::tid::Tid::new().to_string();
    let second = atproto_record::tid::Tid::new().to_string();
    let rule = |text: &str| json!({"$type":"group.opensocial.rule","title":"Rule","text":text,"order":1});
    let input = |rkey: &str, value: Value| json!({"space":space(spaces::META).to_string(),"repo":WORLD,"collection":"group.opensocial.rule","rkey":rkey,"record":value});
    let (status, created) = request(&router, "com.atproto.space.createRecord", input(&first, rule("First"))).await;
    assert_eq!(status, StatusCode::OK, "{created}");
    let mut change = input(&first, rule("Changed"));
    change["swapRecord"] = created["cid"].clone();
    let (status, changed) = request(&router, "com.atproto.space.putRecord", change.clone()).await;
    assert_eq!(status, StatusCode::OK, "{changed}");
    let before = fixture.visible().await;
    assert_eq!(
        request(&router, "com.atproto.space.putRecord", change).await.0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(fixture.visible().await, before);
    let batch = json!({"space":space(spaces::META).to_string(),"repo":WORLD,"writes":[
        {"$type":"com.atproto.space.applyWrites#create","collection":"group.opensocial.rule","rkey":second,"value":rule("Second")},
        {"$type":"com.atproto.space.applyWrites#delete","collection":"group.opensocial.rule","rkey":first}
    ]});
    let (status, results) = request(&router, "com.atproto.space.applyWrites", batch).await;
    assert_eq!(status, StatusCode::OK, "{results}");
    assert_eq!(
        results["results"][1],
        json!({"$type":"com.atproto.space.applyWrites#deleteResult"})
    );
    assert_eq!(
        request(&router, "com.atproto.space.deleteRecord", input(&second, Value::Null))
            .await
            .0,
        StatusCode::OK
    );
    let audits = fixture.audits().await;
    assert_eq!(audits.len(), 5);
    assert!(audits.iter().all(|audit| audit["actor"] == OWNER));
    assert_eq!(audits.iter().filter(|audit| audit["action"] == "delete").count(), 2);
    assert!(audits.iter().all(|audit| {
        audit["subject"]
            .as_str()
            .unwrap()
            .starts_with(&space(spaces::META).to_string())
    }));
    let before = fixture.visible().await;
    assert!(!request(&router, "com.atproto.space.putRecord", json!({"space":space(spaces::META).to_string(),"repo":WORLD,"collection":"group.opensocial.profile","rkey":"self","record":{"$type":"group.opensocial.profile","displayName":"Not allowed","joinPolicy":"invite"}})).await.0.is_success());
    assert_eq!(fixture.visible().await, before);
}

/// Public repo results follow the standard closed union and commit metadata;
/// a null put guard must not overwrite a record, and deleting twice is safe.
/// Catches dispatching put as unconditional create or returning Space shapes.
#[tokio::test]
async fn repo_crud_and_batch_wire_results_preserve_compare_and_swap() {
    let fixture = Fixture::new().await;
    let router = fixture.router(OWNER, ALL);
    let public = json!({"repo":WORLD,"collection":PROFILE,"rkey":"self","record":profile("First"),"swapRecord":null});
    let (status, created) = request(&router, "com.atproto.repo.putRecord", public.clone()).await;
    assert_eq!(status, StatusCode::OK, "{created}");
    assert_eq!(created["validationStatus"], "valid");
    assert!(created["commit"]["cid"].is_string());
    let before = fixture.visible().await;
    assert_eq!(
        request(&router, "com.atproto.repo.putRecord", public).await.0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(fixture.visible().await, before);
    let (status, batch) = request(&router, "com.atproto.repo.applyWrites", json!({"repo":WORLD,"swapCommit":created["commit"]["cid"],"writes":[
        {"$type":"com.atproto.repo.applyWrites#update","collection":PROFILE,"rkey":"self","value":profile("Changed")},
        {"$type":"com.atproto.repo.applyWrites#create","collection":"at.motet.pilot.character","value":{"$type":"at.motet.pilot.character","name":"Character","createdAt":"2026-10-26T00:00:00Z"}}
    ]})).await;
    assert_eq!(status, StatusCode::OK, "{batch}");
    assert_eq!(
        batch["results"][0]["$type"],
        "com.atproto.repo.applyWrites#updateResult"
    );
    assert_eq!(
        batch["results"][1]["$type"],
        "com.atproto.repo.applyWrites#createResult"
    );
    assert!(batch["commit"]["rev"].is_string());
    let delete = json!({"repo":WORLD,"collection":PROFILE,"rkey":"self","swapRecord":batch["results"][0]["cid"]});
    let (status, deleted) = request(&router, "com.atproto.repo.deleteRecord", delete).await;
    assert_eq!(status, StatusCode::OK, "{deleted}");
    assert_eq!(
        deleted
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        vec!["commit"]
    );
    let (status, absent) = request(
        &router,
        "com.atproto.repo.deleteRecord",
        json!({"repo":WORLD,"collection":PROFILE,"rkey":"self","swapRecord":null}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{absent}");
    assert_eq!(absent, json!({}));
    assert_eq!(fixture.audits().await.len(), 5);
}

/// Changing only public must not reveal private data. Catches checking role
/// entries while ignoring the anonymous whole-Space read grant.
#[tokio::test]
async fn public_access_widening_requires_actor_and_session_whole_space_read() {
    let fixture = Fixture::new().await;
    fixture
        .editor(&["space:*?collection=group.opensocial.access&action=update"])
        .await;
    let meta = space(spaces::META);
    let row = spaces::record(fixture.handle.reader(), &meta, "group.opensocial.access", "self")
        .await
        .unwrap()
        .unwrap();
    let mut access = atproto_dasl::from_slice_to_json(&row.value).unwrap();
    access["public"] = true.into();
    let input = json!({"space":meta.to_string(),"repo":WORLD,"collection":"group.opensocial.access","rkey":"self","record":access});
    let before = fixture.visible().await;
    let write_only = ["atproto", "space:*?collection=group.opensocial.access&action=update"];
    for router in [fixture.router(EDITOR, ALL), fixture.router(OWNER, &write_only)] {
        assert!(
            !request(&router, "com.atproto.space.putRecord", input.clone())
                .await
                .0
                .is_success()
        );
        assert!(!spaces::access(fixture.handle.reader(), &meta, None).await.unwrap().read);
        assert_eq!(fixture.visible().await, before);
    }
    let router = fixture.router(
        OWNER,
        &[
            "atproto",
            "space:*?collection=group.opensocial.access&action=read&action=update",
        ],
    );
    let (status, body) = request(&router, "com.atproto.space.putRecord", input).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(spaces::access(fixture.handle.reader(), &meta, None).await.unwrap().read);
}

/// Creating a Space must not bootstrap read authority from its new public
/// flag. Catches the lifecycle path bypassing access-record grant ceilings.
#[tokio::test]
async fn public_space_creation_requires_preexisting_actor_and_session_read() {
    let fixture = Fixture::new().await;
    let create_only = "space:at.motet.pilot.space.scene?manage=create&action=read_self";
    fixture.editor(&[create_only]).await;
    let input = json!({"type":"at.motet.pilot.space.scene","skey":"public","access":{"public":true,"grants":[]}});
    let before = fixture.visible().await;
    for router in [
        fixture.router(EDITOR, ALL),
        fixture.router(OWNER, &["atproto", create_only]),
    ] {
        assert!(
            !request(&router, "group.opensocial.createSpace", input.clone())
                .await
                .0
                .is_success()
        );
        assert_eq!(fixture.visible().await, before);
    }
    let (status, body) = request(&fixture.router(OWNER, ALL), "group.opensocial.createSpace", input).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let created: SpaceUri = body["space"].as_str().unwrap().parse().unwrap();
    assert!(
        spaces::access(fixture.handle.reader(), &created, None)
            .await
            .unwrap()
            .read
    );
}

/// Deleting and recreating a role changes every old membership which names
/// it. Catches recovering a revoked privilege using only role-create scope.
#[tokio::test]
async fn role_reactivation_and_deletion_require_assignment_authority() {
    let fixture = Fixture::new().await;
    let mut permissions = fixture
        .editor(&["space:group.opensocial.members?collection=group.opensocial.role&action=create&action=delete"])
        .await;
    permissions["grants"]
        .as_array_mut()
        .unwrap()
        .push(json!({"role":"power","scopes":["repo:*"],"assignable":[]}));
    fixture
        .admin(
            space(spaces::MEMBERS),
            vec![
                create("group.opensocial.role", "power", &json!({"displayName":"Power"})),
                update("group.opensocial.permissions", "self", &permissions),
                update(
                    "group.opensocial.membership",
                    EDITOR,
                    &membership(EDITOR, &["editor", "power"]),
                ),
            ],
        )
        .await;
    fixture
        .admin(
            space(spaces::MEMBERS),
            vec![SpaceWrite {
                action: OpAction::Delete,
                collection: "group.opensocial.role".into(),
                rkey: "power".into(),
                value: None,
                swap: None,
            }],
        )
        .await;
    let router = fixture.router(EDITOR, ALL);
    let input = |role: &str| json!({"space":space(spaces::MEMBERS).to_string(),"repo":WORLD,"collection":"group.opensocial.role","rkey":role,"record":{"$type":"group.opensocial.role","displayName":role}});
    let before = fixture.visible().await;
    assert!(
        !request(&router, "com.atproto.space.createRecord", input("power"))
            .await
            .0
            .is_success()
    );
    assert!(
        !request(&router, "com.atproto.space.deleteRecord", input("owner"))
            .await
            .0
            .is_success()
    );
    assert!(
        !request(
            &router,
            "com.atproto.repo.createRecord",
            json!({"repo":WORLD,"collection":PROFILE,"rkey":"self","record":profile("Still revoked")})
        )
        .await
        .0
        .is_success()
    );
    assert_eq!(fixture.visible().await, before);
    // Declaring an unreferenced ID grants nobody authority. Assignment still
    // needs its own authorization; referenced IDs use the stricter branch.
    assert_eq!(
        request(&router, "com.atproto.space.createRecord", input("fresh"))
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(
        request(&router, "com.atproto.space.deleteRecord", input("reader"))
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(
        spaces::roles(fixture.handle.reader(), &Did::new(WORLD), &Did::new(TARGET))
            .await
            .unwrap(),
        Vec::<String>::new(),
    );
    assert_eq!(
        request(&router, "com.atproto.space.createRecord", input("reader"))
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(
        spaces::roles(fixture.handle.reader(), &Did::new(WORLD), &Did::new(TARGET))
            .await
            .unwrap(),
        vec!["reader"]
    );
}

/// Legacy clients get their supported public-repo/blob/non-chat-RPC powers,
/// never Space, identity or chat powers.
///
/// Catches storing `transition:generic` literally or translating it into an
/// unrestricted RPC wildcard.
#[tokio::test]
async fn legacy_generic_consent_expands_only_legacy_authority() {
    let fixture = Fixture::new().await;
    let requested = vec!["atproto".to_owned(), "transition:generic".to_owned()];
    let granted = grants::granted(fixture.handle.reader(), &Did::new(WORLD), &Did::new(OWNER), &requested)
        .await
        .unwrap();
    assert!(!granted.iter().any(|scope| scope.starts_with("transition:")));
    let scopes = ScopesSet::from_scopes(granted.iter().cloned()).with_subject(WORLD);
    assert!(scopes.allows_repo(PROFILE, &RepoAction::Create));
    assert!(scopes.allows_blob("image/png"));
    assert!(scopes.allows_rpc("group.opensocial.denyJoinRequest", WORLD));
    assert!(!scopes.allows_rpc("chat.bsky.convo.sendMessage", WORLD));
    assert!(!scopes.allows_rpc("com.atproto.identity.updateHandle", WORLD));
    assert!(!scopes.allows_identity_handle());
    assert!(!scopes.allows_space(&SpaceTarget::new(spaces::GMS, WORLD, "self", SpaceAction::Read)));
    let scopes: Vec<_> = granted.iter().map(String::as_str).collect();
    let (status, body) = request(
        &fixture.router(OWNER, &scopes),
        "com.atproto.repo.createRecord",
        json!({"repo":WORLD,"collection":PROFILE,"rkey":"self","record":profile("Legacy owner")}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    fixture.editor(&["transition:generic"]).await;
    let router = fixture.router(EDITOR, &["atproto", "transition:generic"]);
    let (status, body) = request(
        &router,
        "com.atproto.repo.putRecord",
        json!({"repo":WORLD,"collection":PROFILE,"rkey":"self","record":profile("Legacy role")}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(!delegation(&router, &space(spaces::META)).await.0.is_success());
}

/// Requesting a legacy blanket cannot amplify a narrow current role. Catches
/// expansion after role intersection or treating it as implicit full access.
#[tokio::test]
async fn legacy_generic_requests_still_intersect_narrow_current_roles() {
    let fixture = Fixture::new().await;
    fixture
        .editor(&[
            "repo:at.motet.pilot.profile?action=create",
            "blob:image/png",
            "rpc:group.opensocial.denyJoinRequest?aud=*",
        ])
        .await;
    let granted = grants::granted(
        fixture.handle.reader(),
        &Did::new(WORLD),
        &Did::new(EDITOR),
        &["atproto".into(), "transition:generic".into()],
    )
    .await
    .unwrap();
    let scopes = ScopesSet::from_scopes(granted).with_subject(WORLD);
    assert!(scopes.allows_repo(PROFILE, &RepoAction::Create));
    assert!(!scopes.allows_repo(PROFILE, &RepoAction::Update));
    assert!(!scopes.allows_repo("at.motet.pilot.character", &RepoAction::Create));
    assert!(scopes.allows_blob("image/png"));
    assert!(!scopes.allows_blob("video/mp4"));
    assert!(scopes.allows_rpc("group.opensocial.denyJoinRequest", WORLD));
    assert!(!scopes.allows_rpc("group.opensocial.resolveSubject", WORLD));
    let router = fixture.router(EDITOR, &["atproto", "transition:generic"]);
    let input = json!({"repo":WORLD,"collection":PROFILE,"rkey":"self","record":profile("Only create")});
    let (status, body) = request(&router, "com.atproto.repo.createRecord", input.clone()).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let before = fixture.visible().await;
    assert!(
        !request(&router, "com.atproto.repo.putRecord", input)
            .await
            .0
            .is_success()
    );
    assert_eq!(fixture.visible().await, before);
}
