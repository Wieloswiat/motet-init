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
use motet_tenor::spaces::credentials::{self, CredentialError, IssuedCredential, VerifiedCredential};
use motet_tenor::spaces::storage::TxnStorage;
use motet_tenor::spaces::{self, ProfileInput, SpaceError, SpaceWrite, http};
use motet_tenor::store::keys;
use motet_tenor::world::{Status, WorldInfo};
use motet_xrpc::server::Service;
use object_store::ObjectStore;
use object_store::memory::InMemory;
use serde_json::{Value, json};
use tower::ServiceExt as _;

const WORLD: &str = "did:plc:aaaaaaaaaaaaaaaaaaaaaaaa";
const OWNER: &str = "did:plc:oooooooooooooooooooooooo";
const MEMBER: &str = "did:plc:bbbbbbbbbbbbbbbbbbbbbbbb";
const SECOND_WORLD: &str = "did:plc:dddddddddddddddddddddddd";
const PUBLIC_ONLY: &str = "did:plc:eeeeeeeeeeeeeeeeeeeeeeee";
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
            vec![SpaceWrite::create(
                Membership::NSID,
                MEMBER,
                &Membership {
                    member: MEMBER.into(),
                    roles: vec![spaces::GM_ROLE.into()],
                    created_at: Datetime::now().into_string(),
                    extra: serde_json::Map::new(),
                },
            )?],
        )
        .await?;
        let mut access = SpaceWrite::create(
            AccessRecord::NSID,
            "self",
            &AccessRecord {
                public: Some(false),
                grants: vec![SpaceGrant {
                    role: spaces::GM_ROLE.into(),
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

struct InitializeSecondWorld;

impl Op for InitializeSecondWorld {
    type Output = ();
    type Error = SpaceError;

    async fn run(self, txn: &mut Txn<'_>) -> Result<(), SpaceError> {
        let world = Did::new(SECOND_WORLD);
        txn.begin_world(world.clone(), 1)
            .expect("second fixture World starts in a fresh shard");
        let signing = txn
            .generate_key(&world, Purpose::Signing)
            .expect("second fixture World signing key is generated");
        txn.generate_key(&world, Purpose::Kem)
            .expect("second fixture World KEM key is generated");
        let info = WorldInfo {
            did: world.clone(),
            owner: Did::new(OWNER),
            handle: "second.test".into(),
            epoch: 1,
            status: Status::Active,
            created: Datetime::now().into_string(),
            document: document(SECOND_WORLD, &signing),
        };
        txn.put(keys::world_info(&world), serde_json::to_vec(&info)?.into());
        motet_tenor::repo::genesis(txn, &world).await?;
        spaces::bootstrap(txn, &world, &Did::new(OWNER), ProfileInput::default()).await?;
        let membership = |member: &str, role: &str| Membership {
            member: member.into(),
            roles: vec![role.into()],
            created_at: Datetime::now().into_string(),
            extra: serde_json::Map::new(),
        };
        spaces::write_as_world(
            txn,
            &spaces::world_space(&world, spaces::MEMBERS)?,
            vec![SpaceWrite::create(
                Membership::NSID,
                MEMBER,
                &membership(MEMBER, spaces::OWNER_ROLE),
            )?],
        )
        .await?;
        spaces::write_as_world(
            txn,
            &spaces::world_space(&Did::new(WORLD), spaces::MEMBERS)?,
            vec![SpaceWrite::create(
                Membership::NSID,
                SECOND_WORLD,
                &membership(SECOND_WORLD, spaces::GM_ROLE),
            )?],
        )
        .await?;
        Ok(())
    }
}

struct PublishMeta;

impl Op for PublishMeta {
    type Output = ();
    type Error = SpaceError;

    async fn run(self, txn: &mut Txn<'_>) -> Result<(), SpaceError> {
        let members = spaces::world_space(&Did::new(WORLD), spaces::MEMBERS)?;
        spaces::write_as_world(
            txn,
            &members,
            vec![
                SpaceWrite::create(
                    Role::NSID,
                    "public-only",
                    &Role {
                        default: Some(false),
                        description: None,
                        display_name: "Public reader".into(),
                        extra: serde_json::Map::new(),
                    },
                )?,
                SpaceWrite::create(
                    Membership::NSID,
                    PUBLIC_ONLY,
                    &Membership {
                        member: PUBLIC_ONLY.into(),
                        roles: vec!["public-only".into()],
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
                public: Some(true),
                grants: vec![SpaceGrant {
                    role: spaces::GM_ROLE.into(),
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
    store: Arc<dyn ObjectStore>,
    master: Arc<MasterKey>,
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
        let store: Arc<dyn ObjectStore> = Arc::new(InMemory::new());
        let master = Arc::new(MasterKey::from_bytes(&[71; 32]));
        let shard = Shard::open(Arc::clone(&store), "space-http", Arc::clone(&master))
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
            Arc::new(|aud| {
                [WORLD, SECOND_WORLD]
                    .iter()
                    .any(|world| aud == *world || aud == format!("{world}#motet_tenor"))
            }),
        );
        let router = http::register(Service::new(http::authenticate(auth, Arc::clone(&state))), &state)
            .into_router(())
            .expect("Space method registry is valid")
            .layer(axum::middleware::from_fn(http::vary_mac));
        Self {
            store,
            master,
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
        self.mint_token(&delegation, space).await
    }

    async fn mint_token(&self, delegation: &str, space: &SpaceUri) -> axum::response::Response {
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

    async fn mint_public(&self, space: &SpaceUri) -> axum::response::Response {
        self.router
            .clone()
            .oneshot(
                Request::post("/xrpc/com.atproto.space.getSpaceCredential")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({"space": space.to_string(), "credentialKid": to_public(&self.holder).expect("holder key has a public component").to_string()})
                            .to_string(),
                    ))
                    .expect("public mint request has valid HTTP fields"),
            )
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

    fn service_token(issuer: &str, key: &KeyData, audience: &str, method: &str) -> String {
        Self::service_token_with_jti(issuer, key, audience, method, Some(Tid::new().to_string()))
    }

    fn service_token_with_jti(
        issuer: &str,
        key: &KeyData,
        audience: &str,
        method: &str,
        jti: Option<String>,
    ) -> String {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("test clock is after the Unix epoch")
            .as_secs();
        let mut claims = Claims::new(JoseClaims {
            issuer: Some(issuer.into()),
            audience: Some(audience.into()),
            issued_at: Some(now),
            expiration: Some(now + 60),
            json_web_token_id: jti,
            ..JoseClaims::default()
        });
        claims.private.insert("lxm".into(), method.into());
        mint(
            key,
            &Header::try_from(key.clone()).expect("fixture key has a supported JWT algorithm"),
            &claims,
        )
        .expect("fixture key signs service auth claims")
    }

    async fn service_get(
        &self,
        issuer: &str,
        key: &KeyData,
        method: &str,
        query: &[(&str, &str)],
    ) -> axum::response::Response {
        self.service_get_to(issuer, key, WORLD, method, query).await
    }

    async fn service_get_to(
        &self,
        issuer: &str,
        key: &KeyData,
        audience: &str,
        method: &str,
        query: &[(&str, &str)],
    ) -> axum::response::Response {
        let token = Self::service_token(issuer, key, audience, method);
        let mut headers = HeaderMap::new();
        headers.insert(
            "authorization",
            HeaderValue::from_str(&format!("Bearer {token}")).expect("service token is a valid HTTP header"),
        );
        self.get(method, query, headers).await
    }

    async fn write_to(
        &self,
        issuer: &str,
        key: &KeyData,
        audience: &str,
        method: &str,
        value: Value,
    ) -> axum::response::Response {
        let token = Self::service_token(issuer, key, audience, method);
        self.write_token(method, value, &token).await
    }

    async fn write_token(&self, method: &str, value: Value, token: &str) -> axum::response::Response {
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
                    "grants": [{"role": "gm", "scopes": ["space:*?collection=*&action=read"]}],
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

async fn world_delegation(f: &Fixture, issuer: &str, key: &KeyData, space: &SpaceUri) -> String {
    world_delegation_from(f, issuer, key, WORLD, space).await
}

async fn world_delegation_from(f: &Fixture, issuer: &str, key: &KeyData, audience: &str, space: &SpaceUri) -> String {
    let uri = space.to_string();
    let response = f
        .service_get_to(
            issuer,
            key,
            audience,
            "com.atproto.space.getDelegationToken",
            &[("space", &uri)],
        )
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    json_body(response)
        .await
        .get("token")
        .and_then(Value::as_str)
        .expect("successful delegation returns a token")
        .to_owned()
}

struct RecheckCredential(VerifiedCredential);

impl Op for RecheckCredential {
    type Output = ();
    type Error = CredentialError;

    async fn run(self, txn: &mut Txn<'_>) -> Result<(), CredentialError> {
        credentials::recheck(txn, &self.0).await
    }
}

async fn prepare_local_delegation(
    f: &Fixture,
    handle: &Handle,
    target: &SpaceUri,
    token: String,
) -> credentials::PreparedMint {
    credentials::prepare_mint(
        handle.reader(),
        &Directory(BTreeMap::new()),
        &Fetcher::new(Policy::default()).expect("default outbound HTTP policy is valid"),
        credentials::MintRequest {
            space: target.clone(),
            credential_kid: to_public(&f.holder)
                .expect("holder key has a public component")
                .to_string(),
            delegation_token: Some(token),
            client_attestation: None,
        },
    )
    .await
    .expect("local delegation has valid claims and signature")
}

async fn assert_private_reads(f: &Fixture, credential: &str, expected: StatusCode) {
    let uri = space().to_string();
    let common = [("space", uri.as_str()), ("repo", WORLD)];
    let record = [
        ("space", uri.as_str()),
        ("repo", WORLD),
        ("collection", "group.opensocial.profile"),
        ("rkey", "self"),
    ];
    for (method, query, mac) in [
        ("com.atproto.space.getRecord", record.as_slice(), MacType::Record),
        ("com.atproto.space.listRecords", common.as_slice(), MacType::Record),
        ("com.atproto.space.getRepo", common.as_slice(), MacType::Commit),
        ("com.atproto.space.listRepoOps", common.as_slice(), MacType::Commit),
    ] {
        assert_eq!(
            f.get(method, query, f.headers(credential, mac)).await.status(),
            expected,
            "{method}"
        );
    }
}

async fn assert_delegation_revocation_survives_recovery(f: &Fixture, credential: &str, pending: String) {
    let recovered = Shard::open(Arc::clone(&f.store), "space-http", Arc::clone(&f.master))
        .await
        .expect("the committed Space state is recoverable");
    let target = space();
    assert!(matches!(
        credentials::verify_http(
            &recovered.reader(),
            &f.headers(credential, MacType::Commit),
            &target,
            &Did::new(WORLD)
        )
        .await,
        Err(CredentialError::CredentialRevoked)
    ));
    let recovered = spawn(
        recovered,
        Options {
            max_batch: 8,
            checkpoint_every: Duration::from_secs(3600),
        },
    );
    let prepared = prepare_local_delegation(f, &recovered, &target, pending).await;
    assert!(matches!(
        recovered
            .run(prepared)
            .await
            .expect("recovered shard decides the pending mint"),
        Err(CredentialError::UserNotAuthorized)
    ));
}

/// A World-signed delegation retains its actual requester through mint and revocation.
/// Catches treating the World's permanent FULL access as the removed member's authority.
#[tokio::test]
async fn removing_member_revokes_world_delegations_and_prepared_reads() {
    let f = Fixture::new().await;
    let target = space();
    let uri = target.to_string();
    let delegation = world_delegation(&f, MEMBER, &f.member, &target).await;
    let pending = world_delegation(&f, MEMBER, &f.member, &target).await;
    let prepared = prepare_local_delegation(&f, &f.handle, &target, pending.clone()).await;
    let minted = f.mint_token(&delegation, &target).await;
    assert_eq!(minted.status(), StatusCode::OK);
    let credential = json_body(minted).await["credential"].as_str().unwrap().to_owned();
    let claims: Value =
        serde_json::from_slice(&URL_SAFE_NO_PAD.decode(credential.split('.').nth(1).unwrap()).unwrap()).unwrap();
    let jti = claims["jti"].as_str().unwrap();
    let issued = credentials::issued(f.handle.reader(), &target, jti)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(issued.member.as_deref(), Some(MEMBER));

    let owner = f.mint_for(OWNER, &f.owner, &target).await;
    assert_eq!(owner.status(), StatusCode::OK);
    let owner = json_body(owner).await["credential"].as_str().unwrap().to_owned();
    let verified = credentials::verify_http(
        f.handle.reader(),
        &f.headers(&credential, MacType::Commit),
        &target,
        &Did::new(WORLD),
    )
    .await
    .unwrap();
    let common = [("space", uri.as_str()), ("repo", WORLD)];
    assert_private_reads(&f, &credential, StatusCode::OK).await;

    let members = spaces::world_space(&Did::new(WORLD), spaces::MEMBERS).unwrap();
    let removal = json!({"space": members.to_string(), "repo": WORLD,
        "collection": Membership::NSID, "rkey": MEMBER});
    assert_eq!(
        f.write(OWNER, &f.owner, "com.atproto.space.deleteRecord", removal)
            .await
            .status(),
        StatusCode::OK
    );
    assert!(matches!(
        f.handle.run(RecheckCredential(verified)).await.unwrap(),
        Err(CredentialError::CredentialRevoked)
    ));
    assert!(matches!(
        f.handle.run(prepared).await.unwrap(),
        Err(CredentialError::UserNotAuthorized)
    ));
    assert!(
        credentials::issued(f.handle.reader(), &target, jti)
            .await
            .unwrap()
            .unwrap()
            .revoked
    );
    let pending_mint = f.mint_token(&pending, &target).await;
    assert_eq!(pending_mint.status(), StatusCode::FORBIDDEN);
    assert_eq!(json_body(pending_mint).await["error"], "UserNotAuthorized");
    assert_private_reads(&f, &credential, StatusCode::UNAUTHORIZED).await;
    assert_eq!(
        f.get(
            "com.atproto.space.getLatestCommit",
            &common,
            f.headers(&owner, MacType::Commit)
        )
        .await
        .status(),
        StatusCode::OK
    );
    assert_eq!(
        f.service_get(
            MEMBER,
            &f.member,
            "com.atproto.space.getDelegationToken",
            &[("space", &uri)]
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_delegation_revocation_survives_recovery(&f, &credential, pending).await;
}

struct MarkLegacyWorldCredential(String);

impl Op for MarkLegacyWorldCredential {
    type Output = ();
    type Error = SpaceError;

    async fn run(self, txn: &mut Txn<'_>) -> Result<(), SpaceError> {
        let key = keys::space_credential(&space().to_string(), &Did::new(WORLD), &self.0);
        let value = txn.get(&key).await?.ok_or(SpaceError::NotFound)?;
        let mut issued: IssuedCredential = serde_json::from_slice(&value)?;
        issued.member = Some(WORLD.into());
        txn.put(key, serde_json::to_vec(&issued)?.into());
        Ok(())
    }
}

/// Persisted credentials without the original member must not inherit World access.
/// Catches retaining pre-upgrade credentials after the new origin ledger is installed.
#[tokio::test]
async fn legacy_unattributed_world_credentials_fail_authentication_and_recheck() {
    let f = Fixture::new().await;
    let credential = f.credential().await;
    let payload: Value =
        serde_json::from_slice(&URL_SAFE_NO_PAD.decode(credential.split('.').nth(1).unwrap()).unwrap()).unwrap();
    let jti = payload["jti"].as_str().unwrap().to_owned();
    let headers = f.headers(&credential, MacType::Commit);
    let verified = credentials::verify_http(f.handle.reader(), &headers, &space(), &Did::new(WORLD))
        .await
        .unwrap();
    f.handle
        .run(MarkLegacyWorldCredential(jti.clone()))
        .await
        .unwrap()
        .unwrap();
    let uri = space().to_string();
    assert_eq!(
        f.get(
            "com.atproto.space.getLatestCommit",
            &[("space", &uri), ("repo", WORLD)],
            headers
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
    assert!(matches!(
        f.handle.run(RecheckCredential(verified)).await.unwrap(),
        Err(CredentialError::CredentialRevoked)
    ));
    f.handle.run(RemoveMember).await.unwrap().unwrap();
    assert!(
        credentials::issued(f.handle.reader(), &space(), &jti)
            .await
            .unwrap()
            .unwrap()
            .revoked
    );
}

struct SetListingPolicy;

impl Op for SetListingPolicy {
    type Output = ();
    type Error = SpaceError;

    async fn run(self, txn: &mut Txn<'_>) -> Result<(), SpaceError> {
        let world = Did::new(WORLD);
        for (kind, public, scopes) in [
            (spaces::MEMBERS, true, None),
            (
                spaces::GMS,
                false,
                Some("space:*?collection=*&action=read_self&manage=update"),
            ),
        ] {
            let mut grants = vec![SpaceGrant {
                role: spaces::OWNER_ROLE.into(),
                scopes: vec!["space:*?collection=*".into()],
                extra: serde_json::Map::new(),
            }];
            if let Some(scope) = scopes {
                grants.push(SpaceGrant {
                    role: spaces::GM_ROLE.into(),
                    scopes: vec![scope.into()],
                    extra: serde_json::Map::new(),
                });
            }
            let access = AccessRecord {
                public: Some(public),
                grants,
                extra: serde_json::Map::new(),
            };
            let mut write = SpaceWrite::create(AccessRecord::NSID, "self", &access)?;
            write.action = OpAction::Update;
            spaces::write_as_world(txn, &spaces::world_space(&world, kind)?, vec![write]).await?;
        }
        Ok(())
    }
}

async fn discovered_spaces(f: &Fixture, issuer: &str, key: &KeyData) -> std::collections::BTreeSet<String> {
    let mut discovered = std::collections::BTreeSet::new();
    let mut cursor: Option<String> = None;
    loop {
        let mut query = vec![("limit", "1")];
        if let Some(cursor) = &cursor {
            query.push(("cursor", cursor));
        }
        let response = f.service_get(issuer, key, "com.atproto.space.listSpaces", &query).await;
        assert_eq!(response.status(), StatusCode::OK);
        let page = json_body(response).await;
        let entries = page
            .get("spaces")
            .and_then(Value::as_array)
            .expect("Space list response contains an array");
        assert!(entries.len() <= 1);
        for entry in entries {
            let uri = entry
                .get("uri")
                .and_then(Value::as_str)
                .expect("each listed Space has a URI");
            assert!(discovered.insert(uri.to_owned()));
        }
        cursor = page.get("cursor").and_then(Value::as_str).map(str::to_owned);
        if let Some(cursor) = &cursor {
            assert!(discovered.contains(cursor));
        } else {
            return discovered;
        }
    }
}

async fn assert_hidden_roster_is_concealed(f: &Fixture, uri: &str) {
    let absent = format!("at://{WORLD}/space/{}/absent", spaces::GMS);
    let hidden = f
        .service_get(
            MEMBER,
            &f.member,
            "com.atproto.simplespace.listMembers",
            &[("space", uri)],
        )
        .await;
    let missing = f
        .service_get(
            MEMBER,
            &f.member,
            "com.atproto.simplespace.listMembers",
            &[("space", &absent)],
        )
        .await;
    assert_eq!(missing.status(), hidden.status());
    assert_eq!(json_body(missing).await, json_body(hidden).await);
}

/// Inventory uses Space access; rosters also require access to the members Space.
/// Catches hidden-entry pagination leaks, public-roster leaks and denying managers.
#[tokio::test]
async fn space_inventory_and_rosters_require_their_respective_current_grants() {
    let f = Fixture::new().await;
    let meta = space().to_string();
    let members = spaces::world_space(&Did::new(WORLD), spaces::MEMBERS)
        .unwrap()
        .to_string();
    let gms = spaces::world_space(&Did::new(WORLD), spaces::GMS).unwrap().to_string();
    let one = f
        .service_get(MEMBER, &f.member, "com.atproto.space.listSpaces", &[("limit", "1")])
        .await;
    assert_eq!(one.status(), StatusCode::OK);
    let one = json_body(one).await;
    assert_eq!(one["spaces"].as_array().unwrap().len(), 1);
    assert_eq!(one["spaces"][0]["uri"], meta);
    assert!(one.get("cursor").is_none());
    assert_eq!(
        discovered_spaces(&f, OWNER, &f.owner).await,
        [meta.clone(), members.clone(), gms.clone()].into_iter().collect()
    );
    assert_eq!(
        f.service_get(
            MEMBER,
            &f.member,
            "com.atproto.simplespace.listMembers",
            &[("space", &meta)]
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        f.service_get(
            MEMBER,
            &f.member,
            "com.atproto.simplespace.listMembers",
            &[("space", &gms)]
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_hidden_roster_is_concealed(&f, &gms).await;
    assert_eq!(
        f.service_get(
            OWNER,
            &f.owner,
            "com.atproto.simplespace.listMembers",
            &[("space", &gms)]
        )
        .await
        .status(),
        StatusCode::OK
    );

    f.handle.run(SetListingPolicy).await.unwrap().unwrap();
    assert_eq!(
        discovered_spaces(&f, MEMBER, &f.member).await,
        [meta, members.clone(), gms.clone()].into_iter().collect()
    );
    for uri in [&members, &gms] {
        assert_eq!(
            f.service_get(
                MEMBER,
                &f.member,
                "com.atproto.simplespace.listMembers",
                &[("space", uri)]
            )
            .await
            .status(),
            StatusCode::OK
        );
    }
    let gm_space = spaces::world_space(&Did::new(WORLD), spaces::GMS).unwrap();
    assert_eq!(
        f.mint_for(MEMBER, &f.member, &gm_space).await.status(),
        StatusCode::FORBIDDEN
    );

    f.handle.run(RemoveMember).await.unwrap().unwrap();
    for (method, query) in [
        ("com.atproto.space.listSpaces", Vec::new()),
        ("com.atproto.simplespace.listMembers", vec![("space", gms.as_str())]),
    ] {
        assert_eq!(
            f.service_get(MEMBER, &f.member, method, &query).await.status(),
            StatusCode::UNAUTHORIZED
        );
    }
}

fn create_scene_space(skey: &str) -> Value {
    json!({
        "type": "at.motet.pilot.space.scene",
        "skey": skey,
        "index": false,
        "access": {"public": false, "grants": [
            {"role": spaces::GM_ROLE, "scopes": ["space:*?action=read"]}
        ]}
    })
}

/// A member token authorizes one committed group mutation, including idempotent deletes.
/// Catches changed-body replay and spending a token when its transaction aborts.
#[tokio::test]
async fn group_mutation_tokens_are_single_use_and_failed_creates_do_not_spend_them() {
    let f = Fixture::new().await;
    let create = "group.opensocial.createSpace";
    let token = Fixture::service_token(OWNER, &f.owner, WORLD, create);
    let first = f.write_token(create, create_scene_space("first"), &token).await;
    assert_eq!(first.status(), StatusCode::OK);
    let first_uri = json_body(first).await["space"].as_str().unwrap().to_owned();
    assert_eq!(
        f.write_token(create, create_scene_space("replayed"), &token)
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    let unused = Fixture::service_token(OWNER, &f.owner, WORLD, create);
    assert_eq!(
        f.write_token(create, create_scene_space("first"), &unused)
            .await
            .status(),
        StatusCode::BAD_REQUEST
    );
    let second = f.write_token(create, create_scene_space("second"), &unused).await;
    assert_eq!(second.status(), StatusCode::OK);
    let second_uri = json_body(second).await["space"].as_str().unwrap().to_owned();
    let third = f.write(OWNER, &f.owner, create, create_scene_space("third")).await;
    assert_eq!(third.status(), StatusCode::OK);
    let third_uri = json_body(third).await["space"].as_str().unwrap().to_owned();

    let simple = "com.atproto.simplespace.deleteSpace";
    let token = Fixture::service_token(OWNER, &f.owner, WORLD, simple);
    let deletion = json!({"space": first_uri});
    assert_eq!(
        f.write_token(simple, deletion.clone(), &token).await.status(),
        StatusCode::OK
    );
    assert_eq!(
        f.write_token(simple, deletion.clone(), &token).await.status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        f.write(OWNER, &f.owner, simple, deletion).await.status(),
        StatusCode::OK
    );

    let group = "group.opensocial.deleteSpace";
    let token = Fixture::service_token(OWNER, &f.owner, WORLD, group);
    assert_eq!(
        f.write_token(group, json!({"space": second_uri}), &token)
            .await
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        f.write_token(group, json!({"space": &third_uri}), &token)
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    assert!(
        !spaces::info(f.handle.reader(), &third_uri.parse().unwrap())
            .await
            .unwrap()
            .deleted
    );
}

fn rule_write(rkey: &str, text: &str) -> Value {
    json!({"space": space().to_string(), "repo": WORLD, "collection": "group.opensocial.rule", "rkey": rkey,
        "record": {"$type": "group.opensocial.rule", "title": "Rule", "text": text, "order": 1}})
}

/// Every Space record mutation binds the token to one committed request.
/// Catches reusing a valid signature to create, replace, delete or batch different records.
#[tokio::test]
async fn space_record_and_batch_tokens_cannot_be_replayed_with_changed_bodies() {
    let f = Fixture::new().await;
    for method in ["com.atproto.space.createRecord", "com.atproto.space.putRecord"] {
        let first = Tid::new().to_string();
        let second = Tid::new().to_string();
        let token = Fixture::service_token(OWNER, &f.owner, WORLD, method);
        assert_eq!(
            f.write_token(method, rule_write(&first, "first"), &token)
                .await
                .status(),
            StatusCode::OK
        );
        assert_eq!(
            f.write_token(method, rule_write(&second, "replayed"), &token)
                .await
                .status(),
            StatusCode::UNAUTHORIZED
        );
        assert!(
            spaces::record(f.handle.reader(), &space(), "group.opensocial.rule", &second)
                .await
                .unwrap()
                .is_none()
        );
    }
    let first = Tid::new().to_string();
    let second = Tid::new().to_string();
    for rkey in [&first, &second] {
        assert_eq!(
            f.write(
                OWNER,
                &f.owner,
                "com.atproto.space.putRecord",
                rule_write(rkey, "retained")
            )
            .await
            .status(),
            StatusCode::OK
        );
    }
    let method = "com.atproto.space.deleteRecord";
    let token = Fixture::service_token(OWNER, &f.owner, WORLD, method);
    let body = |rkey: &str| {
        json!({"space": space().to_string(), "repo": WORLD,
        "collection": "group.opensocial.rule", "rkey": rkey})
    };
    assert_eq!(
        f.write_token(method, body(&first), &token).await.status(),
        StatusCode::OK
    );
    assert_eq!(
        f.write_token(method, body(&second), &token).await.status(),
        StatusCode::UNAUTHORIZED
    );
    assert!(
        spaces::record(f.handle.reader(), &space(), "group.opensocial.rule", &second)
            .await
            .unwrap()
            .is_some()
    );
    let method = "com.atproto.space.applyWrites";
    let token = Fixture::service_token(OWNER, &f.owner, WORLD, method);
    let first = Tid::new().to_string();
    let second = Tid::new().to_string();
    let batch = |rkey: &str| {
        json!({"space": space().to_string(), "repo": WORLD, "writes": [{
            "$type": "com.atproto.space.applyWrites#create", "collection": "group.opensocial.rule", "rkey": rkey,
            "value": {"$type": "group.opensocial.rule", "title": "Rule", "text": "batch", "order": 1}
        }]})
    };
    assert_eq!(
        f.write_token(method, batch(&first), &token).await.status(),
        StatusCode::OK
    );
    assert_eq!(
        f.write_token(method, batch(&second), &token).await.status(),
        StatusCode::UNAUTHORIZED
    );
    assert!(
        spaces::record(f.handle.reader(), &space(), "group.opensocial.rule", &second)
            .await
            .unwrap()
            .is_none()
    );
}

/// Missing JTIs are refused for writes, but read-only calls can reuse their service token.
/// Catches applying the replay guard to reads or bypassing it for record writes.
#[tokio::test]
async fn service_token_jti_is_required_for_mutations_not_queries() {
    let f = Fixture::new().await;
    let method = "com.atproto.space.putRecord";
    let token = Fixture::service_token_with_jti(OWNER, &f.owner, WORLD, method, None);
    let rkey = Tid::new().to_string();
    assert_eq!(
        f.write_token(method, rule_write(&rkey, "must not be written"), &token)
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    assert!(
        spaces::record(f.handle.reader(), &space(), "group.opensocial.rule", &rkey)
            .await
            .unwrap()
            .is_none()
    );
    let method = "com.atproto.space.listSpaces";
    let token = Fixture::service_token_with_jti(OWNER, &f.owner, WORLD, method, None);
    let mut headers = HeaderMap::new();
    headers.insert(
        "authorization",
        HeaderValue::from_str(&format!("Bearer {token}")).unwrap(),
    );
    for _ in 0..2 {
        assert_eq!(f.get(method, &[], headers.clone()).await.status(), StatusCode::OK);
    }
}

/// A delegation belongs to one Space generation and to its exact signed claims.
/// Catches reusing a signed JTI for another Space or after deleting and recreating its URI.
#[tokio::test]
async fn world_delegation_binding_cannot_cross_space_or_generation() {
    let f = Fixture::new().await;
    let target = space();
    let token = world_delegation(&f, MEMBER, &f.member, &target).await;
    let mut parts = token.split('.');
    let header: Value = serde_json::from_slice(&URL_SAFE_NO_PAD.decode(parts.next().unwrap()).unwrap()).unwrap();
    let mut claims: Value = serde_json::from_slice(&URL_SAFE_NO_PAD.decode(parts.next().unwrap()).unwrap()).unwrap();
    let other = spaces::world_space(&Did::new(WORLD), spaces::GMS).unwrap();
    claims["sub"] = other.to_string().into();
    let forged = sign_compact_jws(&header, &claims, &f.world).unwrap();
    let response = f.mint_token(&forged, &other).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(json_body(response).await["error"], "InvalidDelegationToken");
    assert_eq!(f.mint_token(&token, &target).await.status(), StatusCode::OK);

    let create = "group.opensocial.createSpace";
    let first = f.write(OWNER, &f.owner, create, create_scene_space("recreated")).await;
    assert_eq!(first.status(), StatusCode::OK);
    let uri = json_body(first).await["space"].as_str().unwrap().to_owned();
    let scene: SpaceUri = uri.parse().unwrap();
    let old = world_delegation(&f, MEMBER, &f.member, &scene).await;
    assert_eq!(
        f.write(OWNER, &f.owner, "group.opensocial.deleteSpace", json!({"space": &uri}))
            .await
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        f.write(OWNER, &f.owner, create, create_scene_space("recreated"))
            .await
            .status(),
        StatusCode::OK
    );
    let response = f.mint_token(&old, &scene).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(json_body(response).await["error"], "InvalidDelegationToken");
    let fresh = world_delegation(&f, MEMBER, &f.member, &scene).await;
    assert_eq!(f.mint_token(&fresh, &scene).await.status(), StatusCode::OK);
}

/// Abandoned delegation origins are reclaimed after expiry without deleting live origins.
/// Catches an unbounded durable ledger or an expiry sweep that destroys usable delegations.
#[tokio::test]
async fn delegation_origin_sweep_is_bounded_and_preserves_live_tokens() {
    let f = Fixture::new().await;
    let target = space();
    let live = world_delegation(&f, MEMBER, &f.member, &target).await;
    let abandoned = [
        world_delegation(&f, MEMBER, &f.member, &target).await,
        world_delegation(&f, MEMBER, &f.member, &target).await,
    ];
    let claims: Vec<_> = std::iter::once(&live)
        .chain(abandoned.iter())
        .map(|token| atproto_space::credential::peek_delegation_token(token).unwrap())
        .collect();
    let skew = atproto_space::credential::CLOCK_SKEW_SECS;
    let first_expiry = claims.iter().map(|claim| claim.exp + skew).min().unwrap();
    let last_expiry = claims.iter().map(|claim| claim.exp + skew).max().unwrap();
    assert_eq!(
        f.handle
            .run(credentials::SweepDelegationOrigins {
                now: UNIX_EPOCH + Duration::from_secs(first_expiry - 1),
                limit: 1,
            })
            .await
            .unwrap()
            .unwrap(),
        0
    );
    let response = f.mint_token(&live, &target).await;
    assert_eq!(response.status(), StatusCode::OK);
    let credential = json_body(response).await["credential"].as_str().unwrap().to_owned();
    let payload: Value =
        serde_json::from_slice(&URL_SAFE_NO_PAD.decode(credential.split('.').nth(1).unwrap()).unwrap()).unwrap();
    let ledger = keys::space_credential(&target.to_string(), &Did::new(WORLD), payload["jti"].as_str().unwrap());
    let reverse = keys::space_delegated_credential(&Did::new(WORLD), &ledger);
    for expected in [1, 1, 0] {
        assert_eq!(
            f.handle
                .run(credentials::SweepDelegationOrigins {
                    now: UNIX_EPOCH + Duration::from_secs(last_expiry),
                    limit: 1,
                })
                .await
                .unwrap()
                .unwrap(),
            expected
        );
    }
    for claim in &claims[1..] {
        assert!(
            f.handle
                .reader()
                .get(&keys::space_delegation_origin(&Did::new(WORLD), &claim.jti))
                .await
                .unwrap()
                .is_none()
        );
    }
    let response = f.mint_token(&abandoned[0], &target).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(json_body(response).await["error"], "InvalidDelegationToken");
    assert!(f.handle.reader().get(&reverse).await.unwrap().is_some());
    assert_eq!(
        f.handle
            .run(credentials::SweepDelegationOrigins {
                now: UNIX_EPOCH + Duration::from_secs(payload["exp"].as_u64().unwrap() + skew),
                limit: 1,
            })
            .await
            .unwrap()
            .unwrap(),
        1
    );
    assert!(f.handle.reader().get(&reverse).await.unwrap().is_none());
}

/// World-signed credentials track their member even when anonymous reads are public.
/// Catches treating public access as proof that a removed originator is still authorized.
#[tokio::test]
async fn public_space_does_not_keep_removed_members_world_delegation_alive() {
    let f = Fixture::new().await;
    f.handle.run(PublishMeta).await.unwrap().unwrap();
    let target = space();
    let token = world_delegation(&f, MEMBER, &f.member, &target).await;
    let pending = world_delegation(&f, MEMBER, &f.member, &target).await;
    let response = f.mint_token(&token, &target).await;
    assert_eq!(response.status(), StatusCode::OK);
    let credential = json_body(response).await["credential"].as_str().unwrap().to_owned();
    assert_private_reads(&f, &credential, StatusCode::OK).await;
    let uri = spaces::world_space(&Did::new(WORLD), spaces::MEMBERS).unwrap();
    let removal = json!({"space": uri.to_string(), "repo": WORLD,
        "collection": Membership::NSID, "rkey": MEMBER});
    assert_eq!(
        f.write(OWNER, &f.owner, "com.atproto.space.deleteRecord", removal)
            .await
            .status(),
        StatusCode::OK
    );
    assert_private_reads(&f, &credential, StatusCode::UNAUTHORIZED).await;
    let refused = f.mint_token(&pending, &target).await;
    assert_eq!(refused.status(), StatusCode::FORBIDDEN);
    assert_eq!(json_body(refused).await["error"], "UserNotAuthorized");
    let public = f.mint_public(&target).await;
    assert_eq!(public.status(), StatusCode::OK);
    let public = json_body(public).await["credential"].as_str().unwrap().to_owned();
    assert_private_reads(&f, &public, StatusCode::OK).await;
}

/// Public content does not make a private membership roster public.
/// Catches exposing all World members merely because anonymous Space reads are allowed.
#[tokio::test]
async fn public_space_roster_requires_separate_roster_access_and_explicit_grants() {
    let f = Fixture::new().await;
    f.handle.run(PublishMeta).await.unwrap().unwrap();
    let uri = space().to_string();
    let query = [("space", uri.as_str())];
    assert_eq!(
        f.service_get(MEMBER, &f.member, "com.atproto.simplespace.listMembers", &query)
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    let response = f
        .service_get(OWNER, &f.owner, "com.atproto.simplespace.listMembers", &query)
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    let roster = json_body(response).await;
    let members = roster["members"].as_array().unwrap();
    assert!(members.iter().any(|member| member["did"] == MEMBER));
    assert!(!members.iter().any(|member| member["did"] == PUBLIC_ONLY));
}

/// Permission to create a Space does not disclose existing private Spaces of that type.
/// Catches treating `manage=create` as management of an existing Space.
#[tokio::test]
async fn create_only_space_permission_does_not_disclose_existing_uris_or_rosters() {
    let f = Fixture::new().await;
    f.handle.run(SetListingPolicy).await.unwrap().unwrap();
    let mut create = create_scene_space("secret");
    create["access"]["grants"][0]["scopes"] = json!(["space:*?action=read_self&manage=create"]);
    let response = f.write(OWNER, &f.owner, "group.opensocial.createSpace", create).await;
    assert_eq!(response.status(), StatusCode::OK);
    let secret = json_body(response).await["space"].as_str().unwrap().to_owned();
    assert!(!discovered_spaces(&f, MEMBER, &f.member).await.contains(&secret));
    assert_eq!(
        f.service_get(
            MEMBER,
            &f.member,
            "com.atproto.simplespace.listMembers",
            &[("space", &secret)]
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
}

struct AdvanceTokenClock;

impl Op for AdvanceTokenClock {
    type Output = ();
    type Error = std::convert::Infallible;

    fn run(self, txn: &mut Txn<'_>) -> impl Future<Output = Result<(), Self::Error>> + Send {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("test clock is after the Unix epoch")
            .as_secs();
        txn.put(
            keys::SERVICE_TOKEN_CLOCK.to_vec(),
            (now + 120).to_be_bytes().to_vec().into(),
        );
        std::future::ready(Ok(()))
    }
}

/// A service token that expires while queued cannot produce a delegation or private metadata.
/// Catches validating its expiry only before the request reaches the shard.
#[tokio::test]
async fn queued_space_queries_recheck_service_token_expiry_without_spending_jti() {
    let f = Fixture::new().await;
    f.handle.run(AdvanceTokenClock).await.unwrap().unwrap();
    let target = space().to_string();
    for (method, query) in [
        ("com.atproto.space.getDelegationToken", vec![("space", target.as_str())]),
        ("com.atproto.space.listSpaces", Vec::new()),
        ("com.atproto.simplespace.listMembers", vec![("space", target.as_str())]),
    ] {
        assert_eq!(
            f.service_get(OWNER, &f.owner, method, &query).await.status(),
            StatusCode::UNAUTHORIZED,
            "{method}"
        );
    }
}

async fn remove_world_member(f: &Fixture, world: &str, member: &str) {
    let members =
        spaces::world_space(&Did::new(world), spaces::MEMBERS).expect("fixture World has a valid members Space URI");
    let removal = json!({"space": members.to_string(), "repo": world,
        "collection": Membership::NSID, "rkey": member});
    assert_eq!(
        f.write_to(OWNER, &f.owner, world, "com.atproto.space.deleteRecord", removal)
            .await
            .status(),
        StatusCode::OK
    );
}

async fn cross_world_credential(f: &Fixture) -> (String, String) {
    let target = space();
    let token = world_delegation_from(f, MEMBER, &f.member, SECOND_WORLD, &target).await;
    let pending = world_delegation_from(f, MEMBER, &f.member, SECOND_WORLD, &target).await;
    let minted = f.mint_token(&token, &target).await;
    assert_eq!(minted.status(), StatusCode::OK);
    let credential = json_body(minted)
        .await
        .get("credential")
        .and_then(Value::as_str)
        .expect("successful mint returns a credential")
        .to_owned();
    (credential, pending)
}

/// A locally hosted issuing World cannot hide which member requested its delegation.
/// Catches attributing a cross-World credential only to the still-authorized issuing World.
#[tokio::test]
async fn cross_world_delegation_revokes_when_issuing_world_removes_requester() {
    let f = Fixture::new().await;
    f.handle.run(InitializeSecondWorld).await.unwrap().unwrap();
    let target = space();
    let signing = f
        .handle
        .reader()
        .key(&Did::new(SECOND_WORLD), Purpose::Signing)
        .await
        .unwrap()
        .unwrap();
    let unbound = f.mint_for(SECOND_WORLD, &signing, &target).await;
    assert_eq!(unbound.status(), StatusCode::BAD_REQUEST);
    assert_eq!(json_body(unbound).await["error"], "InvalidDelegationToken");
    let (credential, pending) = cross_world_credential(&f).await;
    let claims: Value =
        serde_json::from_slice(&URL_SAFE_NO_PAD.decode(credential.split('.').nth(1).unwrap()).unwrap()).unwrap();
    let jti = claims["jti"].as_str().unwrap();
    let issued = credentials::issued(f.handle.reader(), &target, jti)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(issued.member.as_deref(), Some(SECOND_WORLD));
    let origin = issued.origin.as_ref().unwrap();
    assert_eq!(origin.world, SECOND_WORLD);
    assert_eq!(origin.member, MEMBER);
    let prepared = prepare_local_delegation(&f, &f.handle, &target, pending.clone()).await;
    let verified = credentials::verify_http(
        f.handle.reader(),
        &f.headers(&credential, MacType::Commit),
        &target,
        &Did::new(WORLD),
    )
    .await
    .unwrap();
    assert_private_reads(&f, &credential, StatusCode::OK).await;
    remove_world_member(&f, SECOND_WORLD, MEMBER).await;
    assert!(matches!(
        f.handle.run(prepared).await.unwrap(),
        Err(CredentialError::UserNotAuthorized)
    ));
    assert!(matches!(
        f.handle.run(RecheckCredential(verified)).await.unwrap(),
        Err(CredentialError::CredentialRevoked)
    ));
    assert!(
        credentials::issued(f.handle.reader(), &target, jti)
            .await
            .unwrap()
            .unwrap()
            .revoked
    );
    assert_private_reads(&f, &credential, StatusCode::UNAUTHORIZED).await;
    let refused = f.mint_token(&pending, &target).await;
    assert_eq!(refused.status(), StatusCode::FORBIDDEN);
    assert_delegation_revocation_survives_recovery(&f, &credential, pending).await;
}

/// Both Worlds must continue to authorize a locally delegated credential.
/// Catches retaining access when the Space authority removes the issuing World.
#[tokio::test]
async fn cross_world_delegation_revokes_when_space_authority_removes_issuing_world() {
    let f = Fixture::new().await;
    f.handle.run(InitializeSecondWorld).await.unwrap().unwrap();
    let (credential, pending) = cross_world_credential(&f).await;
    assert_private_reads(&f, &credential, StatusCode::OK).await;
    remove_world_member(&f, WORLD, SECOND_WORLD).await;
    assert_private_reads(&f, &credential, StatusCode::UNAUTHORIZED).await;
    let refused = f.mint_token(&pending, &space()).await;
    assert_eq!(refused.status(), StatusCode::FORBIDDEN);
    assert_eq!(json_body(refused).await["error"], "UserNotAuthorized");
}

struct MarkLegacyCrossWorldCredential(String);

impl Op for MarkLegacyCrossWorldCredential {
    type Output = ();
    type Error = SpaceError;

    async fn run(self, txn: &mut Txn<'_>) -> Result<(), SpaceError> {
        let key = keys::space_credential(&space().to_string(), &Did::new(WORLD), &self.0);
        let value = txn.get(&key).await?.ok_or(SpaceError::NotFound)?;
        let mut issued: IssuedCredential = serde_json::from_slice(&value)?;
        issued.origin = None;
        issued.member = Some(SECOND_WORLD.into());
        txn.delete(keys::space_delegated_credential(&Did::new(SECOND_WORLD), &key));
        txn.put(key, serde_json::to_vec(&issued)?.into());
        Ok(())
    }
}

/// Old credentials with a hosted World as issuer lack trustworthy member provenance.
/// Catches granting them World access after upgrading before their original expiry.
#[tokio::test]
async fn legacy_cross_world_credentials_fail_closed_and_reconcile_when_encountered() {
    let f = Fixture::new().await;
    f.handle.run(InitializeSecondWorld).await.unwrap().unwrap();
    let (credential, _) = cross_world_credential(&f).await;
    let payload: Value =
        serde_json::from_slice(&URL_SAFE_NO_PAD.decode(credential.split('.').nth(1).unwrap()).unwrap()).unwrap();
    let jti = payload["jti"].as_str().unwrap().to_owned();
    let verified = credentials::verify_http(
        f.handle.reader(),
        &f.headers(&credential, MacType::Commit),
        &space(),
        &Did::new(WORLD),
    )
    .await
    .unwrap();
    f.handle
        .run(MarkLegacyCrossWorldCredential(jti.clone()))
        .await
        .unwrap()
        .unwrap();
    assert_private_reads(&f, &credential, StatusCode::UNAUTHORIZED).await;
    assert!(matches!(
        f.handle.run(RecheckCredential(verified)).await.unwrap(),
        Err(CredentialError::CredentialRevoked)
    ));
    remove_world_member(&f, WORLD, SECOND_WORLD).await;
    assert!(
        credentials::issued(f.handle.reader(), &space(), &jti)
            .await
            .unwrap()
            .unwrap()
            .revoked
    );
}

/// Delegation authority ends when the origin member's role no longer exists.
/// Catches checking only whether the membership record still names the member.
#[tokio::test]
async fn cross_world_delegation_rechecks_the_requesters_surviving_role() {
    let f = Fixture::new().await;
    f.handle.run(InitializeSecondWorld).await.unwrap().unwrap();
    let (credential, pending) = cross_world_credential(&f).await;
    let members = spaces::world_space(&Did::new(SECOND_WORLD), spaces::MEMBERS).unwrap();
    let deletion = json!({"space": members.to_string(), "repo": SECOND_WORLD,
        "collection": Role::NSID, "rkey": spaces::OWNER_ROLE});
    assert_eq!(
        f.write_to(
            OWNER,
            &f.owner,
            SECOND_WORLD,
            "com.atproto.space.deleteRecord",
            deletion,
        )
        .await
        .status(),
        StatusCode::OK
    );
    assert!(
        spaces::record(f.handle.reader(), &members, Membership::NSID, MEMBER)
            .await
            .unwrap()
            .is_some()
    );
    assert_private_reads(&f, &credential, StatusCode::UNAUTHORIZED).await;
    let refused = f.mint_token(&pending, &space()).await;
    assert_eq!(refused.status(), StatusCode::FORBIDDEN);
    assert_eq!(json_body(refused).await["error"], "UserNotAuthorized");
}

/// Public target access cannot preserve a removed member's cross-World credential.
/// Catches checking the public target while skipping the issuing World's origin policy.
#[tokio::test]
async fn public_cross_world_delegation_requires_current_origin_authority() {
    let f = Fixture::new().await;
    f.handle.run(PublishMeta).await.unwrap().unwrap();
    f.handle.run(InitializeSecondWorld).await.unwrap().unwrap();
    let (credential, pending) = cross_world_credential(&f).await;
    assert_private_reads(&f, &credential, StatusCode::OK).await;
    remove_world_member(&f, SECOND_WORLD, MEMBER).await;
    assert_private_reads(&f, &credential, StatusCode::UNAUTHORIZED).await;
    assert_eq!(f.mint_token(&pending, &space()).await.status(), StatusCode::FORBIDDEN);
    assert_eq!(f.mint_public(&space()).await.status(), StatusCode::OK);
}
