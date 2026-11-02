//! Private Space reads cross the real registry, authority ledger and bound
//! HTTP signatures. Independent protocol consumers verify the returned STAR/MAC.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use atproto_identity::key::{KeyData, KeyType, generate_key, to_public};
use atproto_identity::model::Document;
use atproto_identity::traits::IdentityResolver;
use atproto_oauth::jwt::{Claims, Header, JoseClaims, mint};
use atproto_record::datetime::Datetime;
use atproto_record::tid::Tid;
use atproto_space::credential::{DELEGATION_TOKEN_TTL_SECS, create_delegation_token, sign_compact_jws};
use atproto_space::http_signature::sign_credential_use;
use atproto_space::kem::{Encapsulation, encapsulate};
use atproto_space::mac::{
    MAC_HEADER, MacType, WANT_MAC_HEADER, format_want_mac_header, parse_mac_header, verify_record_mac,
};
use atproto_space::space_repo::{Op as RecordOp, OpAction};
use atproto_space::sync::{VerifyRepoParams, verify_repo_star_full};
use atproto_space::{LtHash, SpaceRepo, SpaceUri};
use axum::body::{Body, to_bytes};
use axum::http::{HeaderMap, HeaderName, HeaderValue, Request, StatusCode};
use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use motet_lexicons::Record;
use motet_lexicons::group_opensocial_access::Access as AccessRecord;
use motet_lexicons::group_opensocial_defs::SpaceGrant;
use motet_lexicons::group_opensocial_membership::Membership;
use motet_lexicons::group_opensocial_role::Role;
use motet_net::{Fetcher, Policy};
use motet_rules::Did;
use motet_tenor::keys::{MasterKey, Purpose};
use motet_tenor::shard::{Handle, Op, Options, Shard, Txn, spawn};
use motet_tenor::spaces::storage::TxnStorage;
use motet_tenor::spaces::{self, ProfileInput, SpaceError, SpaceWrite, http};
use motet_tenor::store::keys;
use motet_tenor::world::{Status, WorldInfo};
use motet_xrpc::server::Service;
use object_store::memory::InMemory;
use serde_json::{Value, json};
use tower::ServiceExt as _;

const WORLD: &str = "did:plc:aaaaaaaaaaaaaaaaaaaaaaaa";
const OWNER: &str = "did:plc:oooooooooooooooooooooooo";
const MEMBER: &str = "did:plc:bbbbbbbbbbbbbbbbbbbbbbbb";
const RECORD_REPO: &str = "did:plc:cccccccccccccccccccccccc";
const RECORD_COLLECTIONS: [&str; 2] = ["com.example.first", "com.example.second"];

fn varies_on_mac(headers: &HeaderMap) -> bool {
    headers
        .get_all("vary")
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(','))
        .map(str::trim)
        .any(|value| value == "*" || value.eq_ignore_ascii_case(WANT_MAC_HEADER))
}

fn space() -> SpaceUri {
    spaces::world_space(&Did::new(WORLD), spaces::META).expect("fixture World has a valid meta Space URI")
}

fn document(did: &str, key: &KeyData) -> Document {
    let public = to_public(key).expect("fixture key has a public component").to_string();
    serde_json::from_value(json!({
        "id": did,
        "verificationMethod": [{"id": format!("{did}#atproto"), "type": "Multikey", "controller": did,
            "publicKeyMultibase": public.strip_prefix("did:key:").expect("public key uses did:key encoding")}],
    }))
    .expect("fixture document has the DID document shape")
}

struct Initialize;

impl Op for Initialize {
    type Output = ();
    type Error = SpaceError;
    async fn run(self, txn: &mut Txn<'_>) -> Result<(), SpaceError> {
        let world = Did::new(WORLD);
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
            handle: "world.test".into(),
            epoch: 1,
            status: Status::Active,
            created: Datetime::now().into_string(),
            document: document(WORLD, &signing),
        };
        txn.put(keys::world_info(&world), serde_json::to_vec(&info)?.into());
        motet_tenor::repo::genesis(txn, &world).await?;
        spaces::bootstrap(txn, &world, &Did::new(OWNER), ProfileInput::default()).await?;
        let members = spaces::world_space(&world, spaces::MEMBERS)?;
        spaces::write_as_world(
            txn,
            &members,
            vec![
                SpaceWrite::create(
                    Role::NSID,
                    "reader",
                    &Role {
                        default: Some(false),
                        description: None,
                        display_name: "Reader".into(),
                        extra: serde_json::Map::new(),
                    },
                )?,
                SpaceWrite::create(
                    Membership::NSID,
                    MEMBER,
                    &Membership {
                        member: MEMBER.into(),
                        roles: vec!["reader".into()],
                        created_at: Datetime::now().into_string(),
                        extra: serde_json::Map::new(),
                    },
                )?,
            ],
        )
        .await?;
        let mut access = SpaceWrite::create(
            AccessRecord::NSID,
            "self",
            &AccessRecord {
                public: Some(false),
                grants: vec![SpaceGrant {
                    role: "reader".into(),
                    scopes: vec!["space:*?collection=*&action=read".into()],
                    extra: serde_json::Map::new(),
                }],
                extra: serde_json::Map::new(),
            },
        )?;
        access.action = OpAction::Update;
        spaces::write_as_world(txn, &space(), vec![access]).await?;
        Ok(())
    }
}

struct PopulateRecords;

impl Op for PopulateRecords {
    type Output = ();
    type Error = SpaceError;

    async fn run(self, txn: &mut Txn<'_>) -> Result<(), SpaceError> {
        // A second hosted repo has no authority access/profile records in this Space.
        let owner = Did::new(RECORD_REPO);
        txn.begin_world(owner.clone(), 1).expect("new fixture record repo");
        txn.generate_key(&owner, Purpose::Signing)
            .expect("fixture record repo signing key");
        let mut writes = Vec::with_capacity(120);
        for collection in RECORD_COLLECTIONS {
            for index in 0..60 {
                let value = atproto_dasl::to_vec(&json!({"$type": collection, "index": index}))
                    .expect("fixture record is DRISL");
                writes.push(RecordOp {
                    action: OpAction::Create,
                    collection: collection.into(),
                    rkey: format!("{index:03}"),
                    cid: Some(atproto_dasl::cid::compute_cid(&value).to_string()),
                    value: Some(value),
                });
            }
        }
        let repo = SpaceRepo::<_, LtHash>::new(space(), TxnStorage::new(txn, &owner));
        let commit = repo.format_commit(&writes).await?;
        repo.apply_commit(commit).await?;
        Ok(())
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

struct Fixture {
    handle: Handle,
    router: axum::Router,
    owner: KeyData,
    member: KeyData,
    world: KeyData,
    holder: KeyData,
    kem: Encapsulation,
}

impl Fixture {
    async fn new() -> Self {
        let shard = Shard::open(
            Arc::new(InMemory::new()),
            "space-http",
            Arc::new(MasterKey::from_bytes(&[71; 32])),
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
            .expect("fixture initialization reaches the shard")
            .expect("fixture World initialization succeeds");
        let owner = generate_key(KeyType::K256Private).expect("owner signing key generation succeeds");
        let member = generate_key(KeyType::K256Private).expect("member signing key generation succeeds");
        let holder = generate_key(KeyType::P256Private).expect("credential holder key generation succeeds");
        let world = handle
            .reader()
            .key(&Did::new(WORLD), Purpose::Signing)
            .await
            .expect("fixture signing key is readable")
            .expect("initialization installed the signing key");
        let kem = handle
            .reader()
            .key(&Did::new(WORLD), Purpose::Kem)
            .await
            .expect("fixture KEM key is readable")
            .expect("initialization installed the KEM key");
        let kem = encapsulate(&to_public(&kem).expect("fixture KEM key has a public component"))
            .expect("fixture KEM key supports encapsulation");
        let resolver: Arc<dyn IdentityResolver> = Arc::new(Directory(BTreeMap::from([
            (WORLD.into(), document(WORLD, &world)),
            (OWNER.into(), document(OWNER, &owner)),
            (MEMBER.into(), document(MEMBER, &member)),
        ])));
        let state = Arc::new(http::State {
            shard: handle.clone(),
            resolver: Arc::clone(&resolver),
            fetcher: Fetcher::new(Policy::default()).expect("default outbound HTTP policy is valid"),
        });
        let auth = motet_xrpc::service_auth::authenticate(
            resolver,
            Arc::new(|aud| aud == WORLD || aud == format!("{WORLD}#motet_tenor")),
        );
        let router = http::register(Service::new(http::authenticate(auth, Arc::clone(&state))), &state)
            .into_router(())
            .expect("Space method registry is valid")
            .layer(axum::middleware::from_fn(http::vary_mac));
        Self {
            handle,
            router,
            owner,
            member,
            world,
            holder,
            kem,
        }
    }

    async fn credential(&self) -> String {
        let response = self.mint_for(MEMBER, &self.member, &space()).await;
        assert_eq!(response.status(), StatusCode::OK);
        json_body(response)
            .await
            .get("credential")
            .and_then(Value::as_str)
            .expect("successful mint returns a credential string")
            .into()
    }

    async fn mint_for(&self, did: &str, key: &KeyData, space: &SpaceUri) -> axum::response::Response {
        let delegation = create_delegation_token(did, space, key, DELEGATION_TOKEN_TTL_SECS)
            .expect("fixture key signs a delegation token");
        let request = Request::post("/xrpc/com.atproto.space.getSpaceCredential")
            .header("content-type", "application/json")
            .body(Body::from(
                serde_json::to_vec(&json!({"space": space.to_string(), "delegationToken": delegation,
                "credentialKid": to_public(&self.holder).expect("holder key has a public component").to_string()}))
                .expect("mint request JSON serializes"),
            ))
            .expect("mint request has valid HTTP fields");
        self.router
            .clone()
            .oneshot(request)
            .await
            .expect("router is infallible")
    }

    fn headers(&self, credential: &str, kind: MacType) -> HeaderMap {
        self.headers_for(credential, WORLD, kind)
    }

    fn headers_for(&self, credential: &str, audience: &str, kind: MacType) -> HeaderMap {
        let signed = sign_credential_use(credential, audience, &self.holder).expect("holder key signs credential use");
        let mut headers = HeaderMap::new();
        for (name, value) in signed.iter() {
            headers.insert(
                HeaderName::from_static(name),
                HeaderValue::from_str(value).expect("credential signature produces valid HTTP headers"),
            );
        }
        headers.insert(
            WANT_MAC_HEADER,
            HeaderValue::from_str(
                &format_want_mac_header(kind, &self.kem.key_share).expect("fixture key share encodes as a MAC request"),
            )
            .expect("MAC request is a valid HTTP header"),
        );
        headers
    }

    async fn get(&self, method: &str, query: &[(&str, &str)], headers: HeaderMap) -> axum::response::Response {
        let query = url::form_urlencoded::Serializer::new(String::new())
            .extend_pairs(query.iter().copied())
            .finish();
        let mut request = Request::get(format!("/xrpc/{method}?{query}"))
            .body(Body::empty())
            .expect("encoded query forms a valid HTTP request");
        *request.headers_mut() = headers;
        self.router
            .clone()
            .oneshot(request)
            .await
            .expect("router is infallible")
    }

    async fn record_page(
        &self,
        credential: &str,
        collection: Option<&str>,
        cursor: Option<&str>,
        reverse: bool,
    ) -> Value {
        let uri = space().to_string();
        let mut query = vec![
            ("space", uri.as_str()),
            ("repo", RECORD_REPO),
            ("limit", "50"),
            ("reverse", if reverse { "true" } else { "false" }),
        ];
        if let Some(collection) = collection {
            query.push(("collection", collection));
        }
        if let Some(cursor) = cursor {
            query.push(("cursor", cursor));
        }
        let response = self
            .get(
                "com.atproto.space.listRecords",
                &query,
                self.headers_for(credential, RECORD_REPO, MacType::Record),
            )
            .await;
        assert_eq!(response.status(), StatusCode::OK);
        json_body(response).await
    }

    async fn write(&self, issuer: &str, key: &KeyData, method: &str, value: Value) -> axum::response::Response {
        self.write_to(issuer, key, WORLD, method, value).await
    }

    async fn write_to(
        &self,
        issuer: &str,
        key: &KeyData,
        audience: &str,
        method: &str,
        value: Value,
    ) -> axum::response::Response {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("test clock is after the Unix epoch")
            .as_secs();
        let mut claims = Claims::new(JoseClaims {
            issuer: Some(issuer.into()),
            audience: Some(audience.into()),
            issued_at: Some(now),
            expiration: Some(now + 60),
            ..JoseClaims::default()
        });
        claims.private.insert("lxm".into(), method.into());
        let token = mint(
            key,
            &Header::try_from(key.clone()).expect("fixture key has a supported JWT algorithm"),
            &claims,
        )
        .expect("fixture key signs service auth claims");
        self.router
            .clone()
            .oneshot(
                Request::post(format!("/xrpc/{method}"))
                    .header("authorization", format!("Bearer {token}"))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&value).expect("write request JSON serializes"),
                    ))
                    .expect("write request has valid HTTP fields"),
            )
            .await
            .expect("router is infallible")
    }
}

async fn json_body(response: axum::response::Response) -> Value {
    serde_json::from_slice(
        &to_bytes(response.into_body(), 4 * 1024 * 1024)
            .await
            .expect("response body fits the fixture limit"),
    )
    .expect("response contains valid JSON")
}

/// A signature for an invented credential must not be enough. Catches
/// admitting the proof-of-possession without checking the authority JWS/ledger.
#[tokio::test]
async fn private_bytes_require_an_issued_bound_unrevoked_credential() {
    let f = Fixture::new().await;
    let credential = f.credential().await;
    let uri = space().to_string();
    let query = [
        ("space", uri.as_str()),
        ("repo", WORLD),
        ("collection", "group.opensocial.profile"),
        ("rkey", "self"),
    ];
    let response = f.get("com.atproto.space.getRecord", &query, HeaderMap::new()).await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert!(varies_on_mac(response.headers()));

    let mut missing = f.headers(&credential, MacType::Record);
    missing.remove("signature");
    assert_eq!(
        f.get("com.atproto.space.getRecord", &query, missing).await.status(),
        StatusCode::UNAUTHORIZED
    );
    let mut malformed = f.headers(&credential, MacType::Record);
    malformed.insert(
        "authorization",
        HeaderValue::from_static("Atproto-Space-Credential not-a-jwt"),
    );
    assert_eq!(
        f.get("com.atproto.space.getRecord", &query, malformed).await.status(),
        StatusCode::UNAUTHORIZED
    );
    let mut wrong_audience = f.headers(&credential, MacType::Record);
    wrong_audience.insert("atproto-space-audience", HeaderValue::from_static(MEMBER));
    assert_eq!(
        f.get("com.atproto.space.getRecord", &query, wrong_audience)
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    let payload: Value =
        serde_json::from_slice(&URL_SAFE_NO_PAD.decode(credential.split('.').nth(1).unwrap()).unwrap()).unwrap();
    let forged = sign_compact_jws(
        &json!({"alg":"ES256K", "typ":"atproto-space-credential+jwt", "kid":"#atproto"}),
        &payload,
        &generate_key(KeyType::K256Private).unwrap(),
    )
    .unwrap();
    assert_eq!(
        f.get(
            "com.atproto.space.getRecord",
            &query,
            f.headers(&forged, MacType::Record)
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );

    let other_space = spaces::world_space(&Did::new(WORLD), spaces::MEMBERS)
        .unwrap()
        .to_string();
    assert_eq!(
        f.get(
            "com.atproto.space.getLatestCommit",
            &[("space", &other_space), ("repo", WORLD)],
            f.headers(&credential, MacType::Commit)
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
    let response = f
        .get(
            "com.atproto.space.getRecord",
            &query,
            f.headers(&credential, MacType::Record),
        )
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    let mac = parse_mac_header(
        response.headers().get(MAC_HEADER).and_then(|value| value.to_str().ok()),
        MacType::Record,
    )
    .unwrap();
    let record = json_body(response).await;
    verify_record_mac(
        &f.kem.shared_secret,
        record["uri"].as_str().unwrap(),
        &record["cid"].as_str().unwrap().parse().unwrap(),
        &mac,
    )
    .unwrap();
    assert_eq!(record["value"]["displayName"], WORLD);

    f.handle.run(RemoveMember).await.unwrap().unwrap();
    assert_eq!(
        f.get(
            "com.atproto.space.getRecord",
            &query,
            f.headers(&credential, MacType::Record)
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
}

/// The producer's `credentials` payload must invalidate every named credential,
/// not reject its field name or acknowledge the notification without revoking.
#[tokio::test]
async fn revocation_notification_wire_body_revokes_only_the_named_credentials() {
    let f = Fixture::new().await;
    let revoked = [f.credential().await, f.credential().await];
    let retained = f.credential().await;
    let uri = space().to_string();
    let query = [("space", uri.as_str()), ("repo", WORLD)];
    for credential in revoked.iter().chain(std::iter::once(&retained)) {
        assert_eq!(
            f.get(
                "com.atproto.space.getLatestCommit",
                &query,
                f.headers(credential, MacType::Commit),
            )
            .await
            .status(),
            StatusCode::OK
        );
    }
    let credentials: Vec<_> = revoked
        .iter()
        .map(|credential| {
            let payload: Value =
                serde_json::from_slice(&URL_SAFE_NO_PAD.decode(credential.split('.').nth(1).unwrap()).unwrap())
                    .unwrap();
            payload["jti"].as_str().unwrap().to_owned()
        })
        .collect();
    let method = "com.atproto.space.notifyCredentialRevoked";
    // This is the wire shape emitted by spaces::notify::Notification::body.
    let body = json!({"space": uri, "credentials": credentials});
    assert_eq!(
        f.write_to(WORLD, &f.world, WORLD, method, body).await.status(),
        StatusCode::OK
    );
    for credential in &revoked {
        let response = f
            .get(
                "com.atproto.space.getLatestCommit",
                &query,
                f.headers(credential, MacType::Commit),
            )
            .await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(json_body(response).await["error"], "CredentialRevoked");
    }
    assert_eq!(
        f.get(
            "com.atproto.space.getLatestCommit",
            &query,
            f.headers(&retained, MacType::Commit),
        )
        .await
        .status(),
        StatusCode::OK
    );
    assert_eq!(
        f.write_to(
            WORLD,
            &f.world,
            WORLD,
            method,
            json!({"space": uri, "jtis": credentials})
        )
        .await
        .status(),
        StatusCode::BAD_REQUEST
    );
}

/// Whole-Space pages share one limit and resume inside the last collection.
/// Catches per-collection limits, missing cursors, dropped tails and repeated keys.
#[tokio::test]
async fn record_pages_are_bounded_complete_and_ordered_across_collections() {
    let f = Fixture::new().await;
    f.handle.run(PopulateRecords).await.unwrap().unwrap();
    let credential = f.credential().await;
    for collection in [None, Some(RECORD_COLLECTIONS[0]), Some(RECORD_COLLECTIONS[1])] {
        for reverse in [false, true] {
            let expected: Vec<_> = RECORD_COLLECTIONS
                .into_iter()
                .filter(|name| collection.is_none_or(|collection| *name == collection))
                .flat_map(|collection| {
                    (0..60).map(move |index| {
                        let index = if reverse { index } else { 59 - index };
                        (collection.to_owned(), format!("{index:03}"))
                    })
                })
                .collect();
            let mut actual = Vec::with_capacity(expected.len());
            let mut cursor = None;
            for _ in 0..=expected.len() / 50 {
                let page = f.record_page(&credential, collection, cursor.as_deref(), reverse).await;
                let records = page["records"].as_array().unwrap();
                assert!(records.len() <= 50, "the limit covers all collections together");
                actual.extend(records.iter().map(|record| {
                    (
                        record["collection"].as_str().unwrap().to_owned(),
                        record["rkey"].as_str().unwrap().to_owned(),
                    )
                }));
                let next = page.get("cursor").and_then(Value::as_str).map(str::to_owned);
                if next.is_some() {
                    assert_ne!(next, cursor, "a nonterminal page must advance");
                }
                cursor = next;
                if cursor.is_none() {
                    break;
                }
            }
            assert!(cursor.is_none(), "paging must reach the end");
            assert_eq!(actual, expected, "every record appears once in the requested order");
        }
    }
}

/// Separatorless cursors name collection boundaries, including absent collections.
/// Catches ignoring that position or incorrectly rejecting it as an opaque cursor.
#[tokio::test]
async fn collection_only_cursors_skip_earlier_collections_and_resume_the_remainder() {
    let f = Fixture::new().await;
    f.handle.run(PopulateRecords).await.unwrap().unwrap();
    let credential = f.credential().await;
    for cursor in [RECORD_COLLECTIONS[1], "com.example.middle"] {
        let first = f.record_page(&credential, None, Some(cursor), false).await;
        let records = first["records"].as_array().unwrap();
        assert_eq!(records.len(), 50);
        assert!(
            records
                .iter()
                .all(|record| record["collection"] == RECORD_COLLECTIONS[1])
        );
        assert_eq!(records[0]["rkey"], "059");
        assert_eq!(records[49]["rkey"], "010");
        let cursor = first["cursor"].as_str().unwrap();
        let last = f.record_page(&credential, None, Some(cursor), false).await;
        let records = last["records"].as_array().unwrap();
        assert_eq!(records.len(), 10);
        assert!(
            records
                .iter()
                .all(|record| record["collection"] == RECORD_COLLECTIONS[1])
        );
        assert_eq!(records[0]["rkey"], "009");
        assert_eq!(records[9]["rkey"], "000");
        assert!(last.get("cursor").is_none());
    }
}

struct RemoveMember;
impl Op for RemoveMember {
    type Output = ();
    type Error = SpaceError;
    async fn run(self, txn: &mut Txn<'_>) -> Result<(), SpaceError> {
        spaces::write_as_world(
            txn,
            &spaces::world_space(&Did::new(WORLD), spaces::MEMBERS)?,
            vec![SpaceWrite {
                action: OpAction::Delete,
                collection: Membership::NSID.into(),
                rkey: MEMBER.into(),
                value: None,
                swap: None,
            }],
        )
        .await?;
        Ok(())
    }
}

/// A raw JSON dump or a MAC over a different commit would not pass the
/// independent STAR consumer's framing, identity, digest and MAC checks.
#[tokio::test]
async fn star_export_and_record_macs_interoperate_with_the_protocol_consumer() {
    let f = Fixture::new().await;
    let credential = f.credential().await;
    let uri = space().to_string();
    let response = f
        .get(
            "com.atproto.space.getRepo",
            &[("space", &uri), ("repo", WORLD)],
            f.headers(&credential, MacType::Commit),
        )
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["content-type"], "application/octet-stream");
    let mac = parse_mac_header(
        response.headers().get(MAC_HEADER).and_then(|value| value.to_str().ok()),
        MacType::Commit,
    )
    .unwrap();
    let bytes = to_bytes(response.into_body(), 4 * 1024 * 1024).await.unwrap();
    let params = VerifyRepoParams::new(uri, WORLD, f.kem.shared_secret.clone(), mac);
    let verified = verify_repo_star_full(bytes.as_ref(), &params).await.unwrap();
    let records: BTreeMap<_, Value> = verified
        .records
        .into_iter()
        .map(|record| (record.path(), atproto_dasl::from_slice_to_json(&record.value).unwrap()))
        .collect();
    assert_eq!(
        records,
        BTreeMap::from([
            (
                "group.opensocial.access/self".into(),
                json!({
                    "$type": "group.opensocial.access", "public": false,
                    "grants": [{"role": "reader", "scopes": ["space:*?collection=*&action=read"]}],
                })
            ),
            (
                "group.opensocial.profile/self".into(),
                json!({
                    "$type": "group.opensocial.profile", "displayName": WORLD, "joinPolicy": "invite",
                })
            ),
        ])
    );
}

/// A valid signature from the World or a read-only member does not bypass
/// current member policy. Conditional writers must not overwrite a stale CID.
#[tokio::test]
async fn service_auth_requires_member_write_authority_and_conditional_updates_serialize() {
    let f = Fixture::new().await;
    let method = "com.atproto.space.putRecord";
    let rkey = Tid::new().to_string();
    let mut request = json!({"space":space().to_string(), "repo": WORLD, "collection":"group.opensocial.rule", "rkey":rkey,
        "record":{"$type":"group.opensocial.rule", "title":"Rule", "text":"First", "order":1}, "swapRecord":null});
    assert_eq!(
        f.write(MEMBER, &f.member, method, request.clone()).await.status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        f.write(WORLD, &f.world, method, request.clone()).await.status(),
        StatusCode::UNAUTHORIZED
    );
    let created = f.write(OWNER, &f.owner, method, request.clone()).await;
    assert_eq!(created.status(), StatusCode::OK);
    let cid = json_body(created).await["cid"].clone();
    assert_eq!(
        f.write(OWNER, &f.owner, method, request.clone()).await.status(),
        StatusCode::BAD_REQUEST
    );
    request["swapRecord"] = cid;
    request["record"]["text"] = "Second".into();
    assert_eq!(
        f.write(OWNER, &f.owner, method, request.clone()).await.status(),
        StatusCode::OK
    );
    request["record"]["text"] = "Lost update".into();
    let stale = f.write(OWNER, &f.owner, method, request).await;
    assert_eq!(stale.status(), StatusCode::BAD_REQUEST);
    assert_eq!(json_body(stale).await["error"], "InvalidSwap");
}

struct RemoveKem;
impl Op for RemoveKem {
    type Output = ();
    type Error = std::convert::Infallible;
    fn run(self, txn: &mut Txn<'_>) -> impl Future<Output = Result<(), Self::Error>> + Send {
        txn.delete(keys::sealed_key(&Did::new(WORLD), Purpose::Kem));
        std::future::ready(Ok(()))
    }
}

/// KEM availability is checked before parsing a required key share. Catches
/// misclassifying a missing server key as the client's `BadSpaceMac` error.
#[tokio::test]
async fn missing_kem_precedes_bad_mac_and_refusals_still_vary() {
    let f = Fixture::new().await;
    let credential = f.credential().await;
    let uri = space().to_string();
    let mut headers = f.headers(&credential, MacType::Commit);
    headers.remove(WANT_MAC_HEADER);
    let refused = f
        .get(
            "com.atproto.space.getLatestCommit",
            &[("space", &uri), ("repo", WORLD)],
            headers.clone(),
        )
        .await;
    assert_eq!(refused.status(), StatusCode::BAD_REQUEST);
    assert!(varies_on_mac(refused.headers()));
    assert_eq!(json_body(refused).await["error"], "BadSpaceMac");
    f.handle.run(RemoveKem).await.unwrap().unwrap();
    let unavailable = f
        .get(
            "com.atproto.space.getLatestCommit",
            &[("space", &uri), ("repo", WORLD)],
            headers,
        )
        .await;
    assert_eq!(unavailable.status(), StatusCode::INTERNAL_SERVER_ERROR);
    assert!(varies_on_mac(unavailable.headers()));
    assert_eq!(json_body(unavailable).await["error"], "KemKeyUnavailable");
}

/// The owner's bootstrap role reaches every reserved Space, while a valid
/// delegation from a removed member grants none. Catches treating a resolved
/// identity or a prior membership as current read authority.
#[tokio::test]
async fn bootstrap_spaces_admit_the_owner_and_refuse_nonmembers() {
    let f = Fixture::new().await;
    f.handle.run(RemoveMember).await.unwrap().unwrap();
    for kind in [spaces::META, spaces::MEMBERS, spaces::GMS] {
        let space = spaces::world_space(&Did::new(WORLD), kind).unwrap();
        let response = f.mint_for(OWNER, &f.owner, &space).await;
        assert_eq!(response.status(), StatusCode::OK, "{kind}");
        let credential = json_body(response).await["credential"].as_str().unwrap().to_owned();
        let uri = space.to_string();
        let response = f
            .get(
                "com.atproto.space.getLatestCommit",
                &[("space", &uri), ("repo", WORLD)],
                f.headers(&credential, MacType::Commit),
            )
            .await;
        assert_eq!(response.status(), StatusCode::OK, "{kind}");
        let refused = f.mint_for(MEMBER, &f.member, &space).await;
        assert_eq!(refused.status(), StatusCode::FORBIDDEN, "{kind}");
        assert_eq!(json_body(refused).await["error"], "UserNotAuthorized");
    }
}
