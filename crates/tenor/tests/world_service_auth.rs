//! World writes cross the real service-auth verifier, XRPC router, current
//! opensocial policy, repo/Space writers and durable shard.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use atproto_identity::key::{KeyData, KeyType, generate_key, to_public};
use atproto_identity::model::Document;
use atproto_identity::traits::IdentityResolver;
use atproto_oauth::jwt::{Claims, Header, JoseClaims, mint};
use atproto_space::SpaceUri;
use atproto_space::credential::verify_delegation_token;
use atproto_space::space_repo::OpAction;
use atproto_space::storage::SpaceRepoStorage;
use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use bytes::Bytes;
use motet_net::{Fetcher, Policy};
use motet_rules::Did;
use motet_tenor::identity::Hosting;
use motet_tenor::keys::{MasterKey, Purpose};
use motet_tenor::repo;
use motet_tenor::shard::{Handle, Op, Options, Shard, Txn, spawn};
use motet_tenor::spaces::{self, ProfileInput, SpaceError, SpaceWrite};
use motet_tenor::store::keys;
use motet_tenor::world::{Status, WorldInfo};
use motet_tenor::{api, pds};
use object_store::ObjectStore;
use object_store::memory::InMemory;
use serde_json::{Value, json};
use tower::ServiceExt as _;

const WORLD: &str = "did:plc:ewvi7nxzyoun6zhxrhs64oiz";
const OTHER_WORLD: &str = "did:plc:aaaaaaaaaaaaaaaaaaaaaaaa";
const OWNER: &str = "did:web:gm.example";
const EDITOR: &str = "did:web:editor.example";
const TARGET: &str = "did:web:member.example";
const OUTSIDER: &str = "did:web:outside.example";
const PROFILE: &str = "at.motet.pilot.profile";
const WORLD_AUDIENCE: &str = "did:plc:ewvi7nxzyoun6zhxrhs64oiz#motet_tenor";
const HANDLE: &str = "world.motet.test";

fn document(did: &str, key: &KeyData) -> Document {
    let public = to_public(key).expect("fixture key has a public component").to_string();
    serde_json::from_value(json!({
        "id": did,
        "alsoKnownAs": if did == WORLD { vec![format!("at://{HANDLE}")] } else { Vec::<String>::new() },
        "verificationMethod": [{"id": format!("{did}#atproto"), "type": "Multikey", "controller": did,
            "publicKeyMultibase": public.strip_prefix("did:key:").expect("public key uses did:key encoding")}],
    }))
    .expect("fixture document has the DID document shape")
}

struct Initialize(&'static str, &'static str);

impl Op for Initialize {
    type Output = ();
    type Error = SpaceError;

    async fn run(self, txn: &mut Txn<'_>) -> Result<(), SpaceError> {
        let world = Did::new(self.0);
        txn.begin_world(world.clone(), 1)
            .expect("fixture World starts in a fresh shard");
        let signing = txn
            .generate_key(&world, Purpose::Signing)
            .expect("fixture signing key generation succeeds");
        txn.generate_key(&world, Purpose::Kem)
            .expect("fixture KEM key generation succeeds");
        let info = WorldInfo {
            did: world.clone(),
            owner: Did::new(OWNER),
            handle: self.1.into(),
            epoch: 1,
            status: Status::Active,
            created: atproto_record::datetime::Datetime::now().into_string(),
            document: document(self.0, &signing),
        };
        txn.put(keys::world_info(&world), serde_json::to_vec(&info)?.into());
        txn.put(keys::handle(self.1), self.0.as_bytes().into());
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

struct Directory(BTreeMap<String, Document>);

#[async_trait::async_trait]
impl IdentityResolver for Directory {
    async fn resolve(&self, subject: &str) -> anyhow::Result<Document> {
        self.0
            .get(subject)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("unknown test identity"))
    }
}

#[derive(Clone)]
struct Client {
    router: axum::Router,
    actor: String,
    key: KeyData,
}

struct Fixture {
    store: Arc<dyn ObjectStore>,
    master: Arc<MasterKey>,
    handle: Handle,
    router: axum::Router,
    keys: BTreeMap<String, KeyData>,
}

impl Fixture {
    async fn new() -> Self {
        let store: Arc<dyn ObjectStore> = Arc::new(InMemory::new());
        let master = Arc::new(MasterKey::from_bytes(&[81; 32]));
        let shard = Shard::open(Arc::clone(&store), "service-auth-writes", Arc::clone(&master))
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
            .run(Initialize(WORLD, HANDLE))
            .await
            .expect("fixture initialization reaches the writer")
            .expect("fixture World initialization succeeds");
        let keys: BTreeMap<String, KeyData> = [OWNER, EDITOR, TARGET, OUTSIDER]
            .into_iter()
            .map(|did| {
                (
                    did.into(),
                    generate_key(KeyType::K256Private).expect("fixture member signing key"),
                )
            })
            .collect();
        let resolver: Arc<dyn IdentityResolver> = Arc::new(Directory(
            keys.iter()
                .map(|(did, key)| (did.clone(), document(did, key)))
                .collect(),
        ));
        let fetcher = Fetcher::new(Policy::default()).expect("default outbound policy is valid");
        let hosting = Arc::new(
            Hosting::new(
                &"https://tenor.motet.test".parse().expect("valid tenor URL"),
                vec![".motet.test".into()],
                "https://plc.example".into(),
                fetcher.clone(),
                Vec::new(),
            )
            .expect("valid hosting configuration"),
        );
        let audience = hosting.audience().to_owned();
        let authenticate = motet_xrpc::service_auth::authenticate(
            Arc::clone(&resolver),
            Arc::new(move |aud| api::accepts_audience(aud, &audience)),
        );
        let state = Arc::new(api::State {
            shard: handle.clone(),
            records: pds::Pds::new(fetcher, resolver),
            hosting,
        });
        let router = api::router(state, authenticate).expect("tenor registry");
        Self {
            store,
            master,
            handle,
            router,
            keys,
        }
    }

    fn router(&self, actor: &str) -> Client {
        Client {
            router: self.router.clone(),
            actor: actor.into(),
            key: self.keys.get(actor).expect("fixture actor has a signing key").clone(),
        }
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

fn token(client: &Client, audience: &str, method: &str) -> String {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).expect("clock").as_secs();
    let mut claims = Claims::new(JoseClaims {
        issuer: Some(client.actor.clone()),
        audience: Some(audience.into()),
        issued_at: Some(now),
        expiration: Some(now + 60),
        json_web_token_id: Some(atproto_record::tid::Tid::new().to_string()),
        ..JoseClaims::default()
    });
    claims.private.insert("lxm".into(), method.into());
    let header = Header::try_from(client.key.clone()).expect("fixture key has a JWT algorithm");
    mint(&client.key, &header, &claims).expect("fixture key signs service auth")
}

async fn request(client: &Client, nsid: &str, body: Value) -> (StatusCode, Value) {
    request_with_token(client, nsid, body, &token(client, WORLD, nsid)).await
}

async fn request_with_token(client: &Client, nsid: &str, body: Value, token: &str) -> (StatusCode, Value) {
    let response = client
        .router
        .clone()
        .oneshot(
            Request::post(format!("/xrpc/{nsid}"))
                .header("authorization", format!("Bearer {token}"))
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

async fn delegation(client: &Client, space: &SpaceUri) -> (StatusCode, Value) {
    let query: String = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("space", &space.to_string())
        .finish();
    let method = "com.atproto.space.getDelegationToken";
    let response = client
        .router
        .clone()
        .oneshot(
            Request::get(format!("/xrpc/{method}?{query}"))
                .header("authorization", format!("Bearer {}", token(client, WORLD, method)))
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

/// Only the verified issuer, the targeted World audience and exact method
/// authorize a write. Catches audience confusion, issuer spoofing and lxm reuse.
#[tokio::test]
async fn signed_service_tokens_bind_actor_audience_method_and_hosted_target() {
    let fixture = Fixture::new().await;
    fixture.editor(&["repo:at.motet.pilot.profile?action=create"]).await;
    let editor = fixture.router(EDITOR);
    let method = "com.atproto.repo.createRecord";
    let body = json!({"repo":HANDLE,"actor":OWNER,"collection":PROFILE,"rkey":"self","record":profile("Member")});
    let before = fixture.visible().await;
    for audience in [
        "did:plc:ewvi7nxzyoun6zhxrhs64oiz#atproto_pds",
        "did:plc:aaaaaaaaaaaaaaaaaaaaaaaa",
        "did:plc:aaaaaaaaaaaaaaaaaaaaaaaa#motet_tenor",
        "did:web:tenor.motet.test",
        "did:web:tenor.motet.test#motet_tenor",
        "did:web:elsewhere.test#motet_tenor",
    ] {
        let (status, reply) =
            request_with_token(&editor, method, body.clone(), &token(&editor, audience, method)).await;
        assert!(!status.is_success(), "{audience}: {reply}");
    }
    let wrong_method = token(&editor, WORLD, "com.atproto.repo.putRecord");
    assert_eq!(
        request_with_token(&editor, method, body.clone(), &wrong_method).await.0,
        StatusCode::UNAUTHORIZED
    );
    let mut forged = editor.clone();
    forged.key = fixture.keys.get(OWNER).unwrap().clone();
    let forged_token = token(&forged, WORLD, method);
    assert_eq!(
        request_with_token(&editor, method, body.clone(), &forged_token).await.0,
        StatusCode::UNAUTHORIZED
    );
    let mut fragment_issuer = editor.clone();
    fragment_issuer.actor = format!("{EDITOR}#atproto");
    let fragment_token = token(&fragment_issuer, WORLD, method);
    assert!(
        !request_with_token(&editor, method, body.clone(), &fragment_token)
            .await
            .0
            .is_success()
    );
    assert!(
        !request(&fixture.router(OUTSIDER), method, body.clone())
            .await
            .0
            .is_success()
    );
    assert_eq!(fixture.visible().await, before);
    let fragment_audience = token(&editor, WORLD_AUDIENCE, method);
    let (status, result) = request_with_token(&editor, method, body, &fragment_audience).await;
    assert_eq!(status, StatusCode::OK, "{result}");
    assert!(result["uri"].as_str().unwrap().starts_with(&format!("at://{WORLD}/")));
    let audit = fixture.audits().await;
    assert_eq!(audit.len(), 1);
    assert_eq!(audit[0]["actor"], EDITOR);
    assert_eq!(audit[0]["action"], "create");
    assert_eq!(audit[0]["subject"], result["uri"]);
}

/// A member of two Worlds cannot use one World's token against the other.
/// Catches binding only the actor, ignoring target mismatches or unaudited lifecycle writes.
#[tokio::test]
async fn world_audience_binds_repo_space_and_group_targets_with_attributed_audits() {
    let fixture = Fixture::new().await;
    fixture
        .handle
        .run(Initialize(OTHER_WORLD, "other.motet.test"))
        .await
        .unwrap()
        .unwrap();
    let owner = fixture.router(OWNER);
    let repo_method = "com.atproto.repo.createRecord";
    let profile_for = |world| json!({"repo":world,"collection":PROFILE,"rkey":"self","record":profile("Owner")});
    let (status, body) = request_with_token(
        &owner,
        repo_method,
        profile_for(OTHER_WORLD),
        &token(&owner, OTHER_WORLD, repo_method),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let space_method = "com.atproto.space.putRecord";
    let private = json!({"space":space(spaces::META).to_string(),"repo":WORLD,"collection":"group.opensocial.profile","rkey":"self","record":{"$type":"group.opensocial.profile","displayName":"Updated","joinPolicy":"invite"}});
    let before = fixture.visible().await;
    for (method, input) in [(repo_method, profile_for(WORLD)), (space_method, private.clone())] {
        let wrong_world = token(&owner, OTHER_WORLD, method);
        let (status, body) = request_with_token(&owner, method, input.clone(), &wrong_world).await;
        assert!(!status.is_success(), "{method}: {body}");
        let wrong_method = token(&owner, WORLD, "com.atproto.repo.deleteRecord");
        let (status, body) = request_with_token(&owner, method, input, &wrong_method).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{method}: {body}");
    }
    let mut mismatch = private.clone();
    mismatch["repo"] = OTHER_WORLD.into();
    assert!(!request(&owner, space_method, mismatch).await.0.is_success());
    let create_method = "group.opensocial.createSpace";
    let create = |key| json!({"type":"at.motet.pilot.space.scene","skey":key,"access":{"public":false,"grants":[]}});
    for audience in [
        "did:web:tenor.motet.test",
        "did:plc:ewvi7nxzyoun6zhxrhs64oiz#atproto_pds",
    ] {
        let (status, body) = request_with_token(
            &owner,
            create_method,
            create("one"),
            &token(&owner, audience, create_method),
        )
        .await;
        assert!(!status.is_success(), "{audience}: {body}");
    }
    let wrong_method = token(&owner, WORLD, repo_method);
    assert_eq!(
        request_with_token(&owner, create_method, create("one"), &wrong_method)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(fixture.visible().await, before);
    let (status, body) = request(&owner, repo_method, profile_for(WORLD)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body) = request(&owner, space_method, private).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    for (key, method) in [
        ("one", "group.opensocial.deleteSpace"),
        ("two", "com.atproto.simplespace.deleteSpace"),
    ] {
        let (status, created) = request(&owner, create_method, create(key)).await;
        assert_eq!(status, StatusCode::OK, "{created}");
        assert!(
            created["space"]
                .as_str()
                .unwrap()
                .starts_with(&format!("at://{WORLD}/"))
        );
        let input = json!({"space":created["space"]});
        let before_delete = fixture.visible().await;
        let (status, body) =
            request_with_token(&owner, method, input.clone(), &token(&owner, OTHER_WORLD, method)).await;
        assert!(!status.is_success(), "{method}: {body}");
        let (status, body) =
            request_with_token(&owner, method, input.clone(), &token(&owner, WORLD, create_method)).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{method}: {body}");
        assert_eq!(fixture.visible().await, before_delete);
        let (status, body) = request(&owner, method, input).await;
        assert_eq!(status, StatusCode::OK, "{body}");
    }
    let audits = fixture.audits().await;
    assert_eq!(audits.len(), 6);
    assert!(audits.iter().all(|audit| audit["actor"] == OWNER));
    assert_eq!(
        audits.iter().filter(|audit| audit["action"] == "createSpace").count(),
        2
    );
    assert_eq!(
        audits.iter().filter(|audit| audit["action"] == "deleteSpace").count(),
        2
    );
}

/// Freshly signed tokens cannot preserve a removed membership, role or grant.
/// Catches trusting the signature alone or caching the authority at authentication.
#[tokio::test]
async fn membership_role_and_permission_revocation_apply_to_the_next_signed_write() {
    for removed in ["membership", "role", "grant"] {
        let fixture = Fixture::new().await;
        let mut permissions = fixture
            .editor(&["repo:at.motet.pilot.profile?action=create&action=update"])
            .await;
        let editor = fixture.router(EDITOR);
        let mut input = json!({"repo":WORLD,"collection":PROFILE,"rkey":"self","record":profile("Before")});
        let (status, body) = request(&editor, "com.atproto.repo.createRecord", input.clone()).await;
        assert_eq!(status, StatusCode::OK, "{removed}: {body}");
        input["record"] = profile("Still authorized");
        let method = "com.atproto.repo.putRecord";
        let previous = token(&editor, WORLD, method);
        let (status, body) = request_with_token(&editor, method, input.clone(), &previous).await;
        assert_eq!(status, StatusCode::OK, "{removed}: {body}");
        let writes = match removed {
            "membership" | "role" => vec![SpaceWrite {
                action: OpAction::Delete,
                collection: format!("group.opensocial.{removed}"),
                rkey: if removed == "role" { "editor" } else { EDITOR }.into(),
                value: None,
                swap: None,
            }],
            "grant" => {
                permissions["grants"]
                    .as_array_mut()
                    .unwrap()
                    .retain(|grant| grant["role"] != "editor");
                vec![update("group.opensocial.permissions", "self", &permissions)]
            }
            _ => unreachable!("fixture enumerates all authority removals"),
        };
        fixture.admin(space(spaces::MEMBERS), writes).await;
        let before = fixture.visible().await;
        input["record"] = profile("Must be refused");
        let fresh = token(&editor, WORLD, method);
        assert_ne!(fresh, previous);
        let (status, body) = request_with_token(&editor, method, input.clone(), &fresh).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{removed}: {body}");
        let (status, body) = request_with_token(&editor, method, input, &previous).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{removed} old token: {body}");
        assert_eq!(fixture.visible().await, before, "{removed}");
        assert_eq!(fixture.audits().await.len(), 2, "{removed}");
    }
}

/// Removing a role takes effect even for freshly minted service tokens.
/// Catches caching roles or missing a repo, Space or lifecycle mutation path.
#[tokio::test]
async fn role_removal_rejects_every_world_mutation_before_state_changes() {
    let fixture = Fixture::new().await;
    let router = fixture.router(OWNER);
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
    let character = json!({"repo":WORLD,"collection":"at.motet.pilot.character","rkey":atproto_record::tid::Tid::new().to_string(),"record":{"$type":"at.motet.pilot.character","name":"New","createdAt":"2026-10-26T00:00:00Z"}});
    assert!(
        !request(&router, "com.atproto.repo.createRecord", character)
            .await
            .0
            .is_success()
    );
    for nsid in ["com.atproto.repo.putRecord", "com.atproto.repo.deleteRecord"] {
        assert!(!request(&router, nsid, public.clone()).await.0.is_success());
    }
    assert!(!request(&router, "com.atproto.repo.applyWrites", json!({"repo":WORLD,"writes":[{"$type":"com.atproto.repo.applyWrites#update","collection":PROFILE,"rkey":"self","value":profile("After")}]})).await.0.is_success());
    let private = json!({"space":space(spaces::META).to_string(),"repo":WORLD,"collection":"group.opensocial.profile","rkey":"self","record":{"$type":"group.opensocial.profile","displayName":"After","joinPolicy":"invite"}});
    let rule = json!({"space":space(spaces::META).to_string(),"repo":WORLD,"collection":"group.opensocial.rule","rkey":atproto_record::tid::Tid::new().to_string(),"record":{"$type":"group.opensocial.rule","title":"New","text":"New","order":1}});
    assert!(
        !request(&router, "com.atproto.space.createRecord", rule)
            .await
            .0
            .is_success()
    );
    for nsid in ["com.atproto.space.putRecord", "com.atproto.space.deleteRecord"] {
        assert!(!request(&router, nsid, private.clone()).await.0.is_success());
    }
    assert!(!request(&router, "com.atproto.space.applyWrites", json!({"space":space(spaces::META).to_string(),"repo":WORLD,"writes":[{"$type":"com.atproto.space.applyWrites#update","collection":"group.opensocial.profile","rkey":"self","value":private["record"]}]})).await.0.is_success());
    let mut new_modality = modality;
    new_modality["skey"] = "two".into();
    assert!(
        !request(&router, "group.opensocial.createSpace", new_modality)
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

/// A member's create-only grant cannot update, delete or mix in an
/// unauthorized collection, even with a valid service token.
#[tokio::test]
async fn narrow_role_grants_limit_actions_and_collections_with_an_audit() {
    let fixture = Fixture::new().await;
    fixture.editor(&["repo:at.motet.pilot.profile?action=create"]).await;
    let router = fixture.router(EDITOR);
    let public = json!({"repo":WORLD,"collection":PROFILE,"rkey":"self","record":profile("Allowed")});
    let before = fixture.visible().await;
    let record = json!({"$type":"at.motet.pilot.character","name":"Not granted","createdAt":"2026-10-26T00:00:00Z"});
    let batch = json!({"repo":WORLD,"writes":[
        {"$type":"com.atproto.repo.applyWrites#create","collection":PROFILE,"rkey":"self","value":profile("Must roll back")},
        {"$type":"com.atproto.repo.applyWrites#create","collection":"at.motet.pilot.character","value":record}
    ]});
    let (status, body) = request(&router, "com.atproto.repo.applyWrites", batch).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert_eq!(fixture.visible().await, before);
    let (status, result) = request(&router, "com.atproto.repo.createRecord", public.clone()).await;
    assert_eq!(status, StatusCode::OK, "{result}");
    let audits = fixture.audits().await;
    assert_eq!(audits.len(), 1);
    assert_eq!(audits[0]["actor"], EDITOR);
    assert_eq!(audits[0]["action"], "create");
    assert_eq!(audits[0]["subject"], result["uri"]);
    assert!(atproto_record::datetime::Datetime::new(audits[0]["at"].as_str().unwrap()).is_ok());
    let before = fixture.visible().await;
    for nsid in ["com.atproto.repo.putRecord", "com.atproto.repo.deleteRecord"] {
        assert!(!request(&router, nsid, public.clone()).await.0.is_success());
    }
    let wide = fixture.router(OWNER);
    assert!(!request(&wide, "com.atproto.repo.createRecord", json!({"repo":WORLD,"collection":"unknown.example.record","rkey":"self","record":{"$type":"unknown.example.record"},"validate":false})).await.0.is_success());
    assert_eq!(fixture.visible().await, before);
}

/// Repo, Space and lifecycle changes must not outlive their audit. Catches
/// appending audit separately or swallowing an audit-destination failure.
#[tokio::test]
async fn audit_failure_rolls_back_public_and_private_records_and_events() {
    let fixture = Fixture::new().await;
    let router = fixture.router(OWNER);
    let create = |key| json!({"type":"at.motet.pilot.space.scene","skey":key,"access":{"public":false,"grants":[]}});
    let (status, existing) = request(&router, "group.opensocial.createSpace", create("existing")).await;
    assert_eq!(status, StatusCode::OK, "{existing}");
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
    assert!(
        !request(&router, "group.opensocial.createSpace", create("new"))
            .await
            .0
            .is_success()
    );
    assert_eq!(fixture.visible().await, before);
    for method in ["group.opensocial.deleteSpace", "com.atproto.simplespace.deleteSpace"] {
        assert!(
            !request(&router, method, json!({"space":existing["space"]}))
                .await
                .0
                .is_success()
        );
        assert_eq!(fixture.visible().await, before);
    }
}

/// An acknowledged member write and its actor audit survive a shard takeover.
/// Catches an audit appended only in memory or omitted from durable replay.
#[tokio::test]
async fn signed_member_write_and_its_audit_survive_replay_together() {
    let fixture = Fixture::new().await;
    fixture.editor(&["repo:at.motet.pilot.profile?action=create"]).await;
    let (status, result) = request(
        &fixture.router(EDITOR),
        "com.atproto.repo.createRecord",
        json!({"repo":WORLD,"collection":PROFILE,"rkey":"self","record":profile("Durable")}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{result}");
    let store = Arc::clone(&fixture.store);
    let master = Arc::clone(&fixture.master);
    drop(fixture);
    let reopened = Shard::open(store, "service-auth-writes", master)
        .await
        .expect("shard replays acknowledged writes");
    let reader = reopened.reader();
    let index = reader
        .get(&keys::repo_record(&Did::new(WORLD), PROFILE, "self"))
        .await
        .unwrap()
        .expect("replay restores the repo record");
    assert_eq!(
        atproto_dasl::Cid::from_bytes(&index).unwrap().to_string(),
        result["cid"].as_str().unwrap()
    );
    let audit = spaces::storage::ReadStorage::new(&reader, &Did::new(WORLD))
        .list_records(&space(spaces::GMS), "at.motet.pilot.audit", None, 100, true)
        .await
        .unwrap()
        .records;
    assert_eq!(audit.len(), 1);
    let audit: Value = atproto_dasl::from_slice_to_json(&audit[0].value).unwrap();
    assert_eq!(audit["actor"], EDITOR);
    assert_eq!(audit["action"], "create");
    assert_eq!(audit["subject"], result["uri"]);
}

/// Local access grants augment role permissions only in their own Space.
/// Catches expanding a permission set globally or ignoring the repo grant.
#[tokio::test]
async fn permission_sets_and_local_grants_remain_confined_to_their_space() {
    let fixture = Fixture::new().await;
    fixture.editor(&["repo:at.motet.pilot.profile?action=create"]).await;
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
    let router = fixture.router(EDITOR);
    let public = json!({"repo":WORLD,"collection":PROFILE,"rkey":"self","record":profile("Allowed")});
    assert_eq!(
        request(&router, "com.atproto.repo.createRecord", public.clone())
            .await
            .0,
        StatusCode::OK
    );
    let meta = json!({"space":space(spaces::META).to_string(),"repo":WORLD,"collection":"group.opensocial.profile","rkey":"self","record":{"$type":"group.opensocial.profile","displayName":"Configured","joinPolicy":"invite"}});
    let (status, body) = request(&router, "com.atproto.space.putRecord", meta.clone()).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let before = fixture.visible().await;
    assert!(
        !request(&router, "com.atproto.repo.putRecord", public)
            .await
            .0
            .is_success()
    );
    let mut other_space = meta;
    other_space["space"] = space(spaces::MEMBERS).to_string().into();
    assert!(
        !request(&router, "com.atproto.space.putRecord", other_space)
            .await
            .0
            .is_success()
    );
    assert_eq!(fixture.visible().await, before);
}

/// Record-write authority does not imply authority to assign every role or
/// mint new permissions. Catches checking only the collection/action grant.
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
    let router = fixture.router(EDITOR);
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

/// Delegation requires current whole-Space read, not `read_self` or write.
/// Catches ignoring the member's grant or signing as the member instead of the World.
#[tokio::test]
async fn delegation_requires_current_whole_space_read_and_is_signed_as_world() {
    let meta = space(spaces::META);
    for action in ["read_self", "update"] {
        let fixture = Fixture::new().await;
        let scope = format!("space:group.opensocial.meta?action={action}");
        fixture.editor(&[&scope]).await;
        assert!(!delegation(&fixture.router(EDITOR), &meta).await.0.is_success());
    }
    let fixture = Fixture::new().await;
    fixture.editor(&["space:group.opensocial.meta?action=read"]).await;
    let (status, result) = delegation(&fixture.router(EDITOR), &meta).await;
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
    assert_ne!(claims.iss, EDITOR);
}

/// A remote Space delegation uses the issuing World, but still requires a
/// grant covering the remote authority. Catches treating local read as global.
#[tokio::test]
async fn remote_delegation_requires_a_covering_grant_and_world_signature() {
    let fixture = Fixture::new().await;
    fixture.editor(&["space:group.opensocial.meta?action=read"]).await;
    let remote = spaces::world_space(&Did::new(OTHER_WORLD), spaces::META).unwrap();
    assert!(!delegation(&fixture.router(EDITOR), &remote).await.0.is_success());
    let (status, result) = delegation(&fixture.router(OWNER), &remote).await;
    assert_eq!(status, StatusCode::OK, "{result}");
    let key = fixture
        .handle
        .reader()
        .key(&Did::new(WORLD), Purpose::Signing)
        .await
        .unwrap()
        .unwrap();
    let token = result["token"].as_str().unwrap();
    let claims = verify_delegation_token(token, OTHER_WORLD, &remote, &key).unwrap();
    assert_eq!(claims.iss, WORLD);
    assert_eq!(claims.aud, format!("{OTHER_WORLD}#atproto_pds"));
    assert_eq!(claims.sub, remote.to_string());
    assert!(verify_delegation_token(token, WORLD, &remote, &key).is_err());
}

/// Space CRUD and batches name each action's record in the same audit stream.
/// Catches an unaudited batch path, a scope checked only by collection, or
/// stale swaps which leave audit evidence for a write that did not happen.
#[tokio::test]
async fn space_crud_and_batches_preserve_scope_swaps_and_attributed_audits() {
    let fixture = Fixture::new().await;
    fixture
        .editor(&["space:group.opensocial.meta?collection=group.opensocial.rule"])
        .await;
    let router = fixture.router(EDITOR);
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
    assert!(audits.iter().all(|audit| audit["actor"] == EDITOR));
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
    let router = fixture.router(OWNER);
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
async fn public_access_widening_requires_actor_whole_space_read() {
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
    assert!(
        !request(&fixture.router(EDITOR), "com.atproto.space.putRecord", input.clone())
            .await
            .0
            .is_success()
    );
    assert!(!spaces::access(fixture.handle.reader(), &meta, None).await.unwrap().read);
    assert_eq!(fixture.visible().await, before);
    let router = fixture.router(OWNER);
    let (status, body) = request(&router, "com.atproto.space.putRecord", input).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(spaces::access(fixture.handle.reader(), &meta, None).await.unwrap().read);
}

/// Creating a Space must not bootstrap read authority from its new public
/// flag. Catches the lifecycle path bypassing access-record grant ceilings.
#[tokio::test]
async fn public_space_creation_requires_preexisting_actor_read() {
    let fixture = Fixture::new().await;
    let create_only = "space:at.motet.pilot.space.scene?manage=create&action=read_self";
    fixture.editor(&[create_only]).await;
    let input = json!({"type":"at.motet.pilot.space.scene","skey":"public","access":{"public":true,"grants":[]}});
    let before = fixture.visible().await;
    assert!(
        !request(&fixture.router(EDITOR), "group.opensocial.createSpace", input.clone())
            .await
            .0
            .is_success()
    );
    assert_eq!(fixture.visible().await, before);
    let (status, body) = request(&fixture.router(OWNER), "group.opensocial.createSpace", input).await;
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
    let router = fixture.router(EDITOR);
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

/// Rules-backed records and audit evidence are not generic writer inputs.
/// Catches bypasses through either CRUD or a mixed-authority batch.
#[tokio::test]
async fn rules_backed_records_refuse_all_generic_repo_and_space_mutations() {
    let fixture = Fixture::new().await;
    let router = fixture.router(OWNER);
    let rkey = atproto_record::tid::Tid::new().to_string();
    let other = atproto_record::tid::Tid::new().to_string();
    let cid = atproto_dasl::cid::compute_cid(b"fixture post").to_string();
    let post = json!({"uri":format!("at://{OWNER}/at.motet.pilot.post/post"),"cid":cid});
    let admission = json!({"uri":format!("at://{WORLD}/at.motet.pilot.admission/{rkey}"),"cid":cid});
    let place = json!({"epoch":1,"seq":1});
    let records = [
        (
            "at.motet.pilot.scene",
            json!({"$type":"at.motet.pilot.scene","title":"Scene","state":"open","cast":[OWNER],"openedBy":OWNER,"createdAt":"2026-10-26T00:00:00Z"}),
        ),
        (
            "at.motet.pilot.admission",
            json!({"$type":"at.motet.pilot.admission","post":post,"author":OWNER,"scene":"scene","place":place,"authorityRev":1,"createdAt":"2026-10-26T00:00:00Z"}),
        ),
        (
            "at.motet.pilot.closing",
            json!({"$type":"at.motet.pilot.closing","admission":admission,"post":post,"scene":"scene","place":place,"createdAt":"2026-10-26T00:00:00Z"}),
        ),
        (
            "at.motet.pilot.audit",
            json!({"$type":"at.motet.pilot.audit","actor":OWNER,"action":"create","subject":format!("at://{WORLD}/{PROFILE}/self"),"at":"2026-10-26T00:00:00Z"}),
        ),
    ];
    let seeded = records
        .iter()
        .map(|(collection, value)| repo::Write::Create {
            collection: repo::Nsid::new(*collection).unwrap(),
            rkey: rkey.clone(),
            record: repo::record_from_json(value).unwrap(),
        })
        .collect();
    fixture
        .handle
        .run(repo::ApplyWrites {
            world: Did::new(WORLD),
            writes: seeded,
            swap_commit: None,
        })
        .await
        .unwrap()
        .expect("trusted rules publication accepts valid records");
    fixture
        .admin(
            space(spaces::META),
            records
                .iter()
                .map(|(collection, value)| create(collection, &rkey, value))
                .collect(),
        )
        .await;
    let before = fixture.visible().await;
    for (collection, value) in records {
        let repo_input =
            |key: &str| json!({"repo":WORLD,"collection":collection,"rkey":key,"record":value,"validate":false});
        for (method, key) in [
            ("com.atproto.repo.createRecord", other.as_str()),
            ("com.atproto.repo.putRecord", rkey.as_str()),
            ("com.atproto.repo.deleteRecord", rkey.as_str()),
            ("com.atproto.repo.deleteRecord", other.as_str()),
        ] {
            let (status, body) = request(&router, method, repo_input(key)).await;
            assert_eq!(status, StatusCode::FORBIDDEN, "{collection} {method}: {body}");
            assert_eq!(body["error"], "InsufficientScope");
        }
        let batch = json!({"repo":WORLD,"validate":false,"writes":[
            {"$type":"com.atproto.repo.applyWrites#create","collection":PROFILE,"rkey":"self","value":profile("Must roll back")},
            {"$type":"com.atproto.repo.applyWrites#update","collection":collection,"rkey":rkey,"value":value}
        ]});
        let (status, body) = request(&router, "com.atproto.repo.applyWrites", batch).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{collection}: {body}");
        assert_eq!(body["error"], "InsufficientScope");
        let space_input = |key: &str| json!({"space":space(spaces::META).to_string(),"repo":WORLD,"collection":collection,"rkey":key,"record":value,"validate":false});
        for (method, key) in [
            ("com.atproto.space.createRecord", other.as_str()),
            ("com.atproto.space.putRecord", rkey.as_str()),
            ("com.atproto.space.deleteRecord", rkey.as_str()),
            ("com.atproto.space.deleteRecord", other.as_str()),
        ] {
            let (status, body) = request(&router, method, space_input(key)).await;
            assert_eq!(status, StatusCode::UNAUTHORIZED, "{collection} {method}: {body}");
        }
        let batch = json!({"space":space(spaces::META).to_string(),"repo":WORLD,"validate":false,"writes":[
            {"$type":"com.atproto.space.applyWrites#create","collection":"group.opensocial.rule","rkey":other,"value":{"$type":"group.opensocial.rule","title":"Rule","text":"Must roll back","order":1}},
            {"$type":"com.atproto.space.applyWrites#update","collection":collection,"rkey":rkey,"value":value}
        ]});
        let (status, body) = request(&router, "com.atproto.space.applyWrites", batch).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{collection}: {body}");
        assert_eq!(fixture.visible().await, before, "{collection}");
    }
}
