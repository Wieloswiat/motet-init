//! World identity properties through the service-auth router and a real
//! in-process PLC directory, with the shard's durable store and real keys.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use atproto_identity::key::{KeyData, KeyType, generate_key, sign, to_public};
use atproto_identity::model::{Document, VerificationMethod};
use atproto_identity::plc::Operation;
use atproto_testkit::highport::{FakePlcDirectory, Fault};
use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use motet_lexicons::at_motet_pilot_tenor_createworld::{CreateWorldInput, CreateWorldOutput};
use motet_lexicons::at_motet_pilot_tenor_reserveworld::{ReserveWorldInput, ReserveWorldOutput};
use motet_net::identity::DidResolver;
use motet_net::{Fetcher, Policy};
use motet_rules::Did;
use motet_tenor::identity::{self, Hosting, IdentityError, ReserveRequest, ReserveWorld, SweepReservations};
use motet_tenor::keys::{MasterKey, Purpose};
use motet_tenor::shard::{self, Handle, Options, Shard};
use motet_tenor::store::keys;
use motet_tenor::{api, pds};
use object_store::ObjectStore;
use object_store::memory::InMemory;
use serde_json::{Value, json};
use tower::ServiceExt as _;

const OWNER: &str = "did:plc:owner";
const OTHER: &str = "did:plc:other";

struct Fixture {
    store: Arc<dyn ObjectStore>,
    master: Arc<MasterKey>,
    shard: Handle,
    plc: FakePlcDirectory,
    hosting: Arc<Hosting>,
    fetcher: Fetcher,
    signing: KeyData,
    rotation: KeyData,
}

fn running(shard: Shard) -> Handle {
    shard::spawn(
        shard,
        Options {
            max_batch: 16,
            checkpoint_every: Duration::from_secs(3600),
        },
    )
}

impl Fixture {
    async fn new() -> Self {
        let store: Arc<dyn ObjectStore> = Arc::new(InMemory::new());
        let master = Arc::new(MasterKey::from_bytes(&[41; 32]));
        let shard = running(
            Shard::open(Arc::clone(&store), "identity", Arc::clone(&master))
                .await
                .expect("shard"),
        );
        let plc = FakePlcDirectory::start().await;
        plc.auto_publish();
        let mut policy = Policy::default().allow_private();
        policy.timeout = Duration::from_secs(1);
        let fetcher = Fetcher::new(policy).expect("client");
        let hosting = Arc::new(
            Hosting::new(
                &"https://tenor.motet.test".parse().expect("origin"),
                vec![".motet.test".into()],
                plc.origin().into(),
                fetcher.clone(),
                Vec::new(),
            )
            .expect("hosting configuration"),
        );
        let signing = generate_key(KeyType::K256Private).expect("owner's PDS key");
        let rotation = generate_key(KeyType::P256Private).expect("owner's rotation key");
        for did in [OWNER, OTHER] {
            let public = to_public(&signing).expect("public key").to_string();
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
            plc.set_document(did, serde_json::to_value(document).expect("document"));
        }
        Self {
            store,
            master,
            shard,
            plc,
            hosting,
            fetcher,
            signing,
            rotation,
        }
    }

    async fn router(&self) -> axum::Router {
        let resolver = Arc::new(DidResolver::new(self.fetcher.clone(), self.plc.origin()));
        let state = Arc::new(api::State {
            shard: self.shard.clone(),
            records: pds::Pds::new(self.fetcher.clone(), Arc::<DidResolver>::clone(&resolver)),
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
        .await
        .expect("registry")
    }

    fn token(&self, issuer: &str, audience: &str, method: &str) -> String {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).expect("clock").as_secs();
        let header = URL_SAFE_NO_PAD.encode(br#"{"alg":"ES256K","typ":"JWT"}"#);
        let claims = URL_SAFE_NO_PAD.encode(
            serde_json::to_vec(&json!({
                "iss": issuer, "aud": audience, "lxm": method, "iat": now, "exp": now + 60,
            }))
            .expect("claims"),
        );
        let input = format!("{header}.{claims}");
        let signature = sign(&self.signing, input.as_bytes()).expect("signature");
        format!("{input}.{}", URL_SAFE_NO_PAD.encode(signature))
    }

    async fn post(&self, method: &str, body: Value, issuer: &str, audience: &str) -> (StatusCode, Value) {
        let request = Request::post(format!("/xrpc/{method}"))
            .header(
                "authorization",
                format!("Bearer {}", self.token(issuer, audience, method)),
            )
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(&body).expect("request")))
            .expect("request");
        response_json(self.router().await.oneshot(request).await.expect("response")).await
    }

    async fn reserve(&self, name: &str, tenor_rotation: bool) -> ReserveWorldOutput {
        let request = ReserveWorldInput {
            handle: name.into(),
            rotation_key: to_public(&self.rotation).expect("public rotation key").to_string(),
            tenor_rotation_key: tenor_rotation,
            extra: serde_json::Map::new(),
        };
        let (status, body) = self
            .post(
                api::RESERVE_WORLD.nsid,
                serde_json::to_value(request).expect("request"),
                OWNER,
                self.hosting.audience(),
            )
            .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        serde_json::from_value(body).expect("generated reserveWorld output contract")
    }

    fn signed(reserved: &ReserveWorldOutput, key: &KeyData) -> Operation {
        let signing_bytes = URL_SAFE_NO_PAD.decode(&reserved.signing_bytes).expect("signing bytes");
        let sig = URL_SAFE_NO_PAD.encode(sign(key, &signing_bytes).expect("owner signs supplied bytes"));
        let mut operation = reserved.operation.clone();
        operation
            .as_object_mut()
            .expect("PLC object")
            .insert("sig".into(), sig.into());
        serde_json::from_value(operation).expect("signed operation")
    }

    async fn create(&self, reserved: &ReserveWorldOutput, operation: &Operation) -> (StatusCode, Value) {
        let request = CreateWorldInput {
            reservation: reserved.reservation.clone(),
            signed_operation: serde_json::to_value(operation).expect("signed operation"),
            extra: serde_json::Map::new(),
        };
        self.post(
            api::CREATE_WORLD.nsid,
            serde_json::to_value(request).expect("request"),
            OWNER,
            self.hosting.audience(),
        )
        .await
    }

    async fn reopen(&mut self) {
        self.shard = running(
            Shard::open(Arc::clone(&self.store), "identity", Arc::clone(&self.master))
                .await
                .expect("reopened shard"),
        );
    }
}

async fn response_json(response: axum::response::Response) -> (StatusCode, Value) {
    let status = response.status();
    let body = to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("response body");
    (status, serde_json::from_slice(&body).expect("JSON response"))
}

async fn query(router: axum::Router, path: &str) -> (StatusCode, Value) {
    response_json(
        router
            .oneshot(Request::get(path).body(Body::empty()).expect("query"))
            .await
            .expect("response"),
    )
    .await
}

/// The owner can recover from the tenor and signs exactly the bytes returned
/// to their device. Catches swapped rotation priority, accidental opt-in and
/// signing-byte encodings that differ from PLC's canonical representation.
#[tokio::test]
async fn owners_sign_genesis_and_outrank_only_opted_in_tenor_keys() {
    let f = Fixture::new().await;
    let owner_key = to_public(&f.rotation).expect("public rotation key");
    for (name, opt_in) in [("owner-only.motet.test", false), ("backup.motet.test", true)] {
        let reserved = f.reserve(name, opt_in).await;
        let operation = Fixture::signed(&reserved, &f.rotation);
        let rotations = operation.rotation_keys().expect("PLC rotation keys");
        assert_eq!(rotations.first(), Some(&owner_key.to_string()));
        assert_eq!(rotations.len(), if opt_in { 2 } else { 1 });
        operation
            .verify(std::slice::from_ref(&owner_key))
            .expect("owner signature");
        let (status, body) = f.create(&reserved, &operation).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        let created: CreateWorldOutput = serde_json::from_value(body).expect("generated creation output");
        let did = Did::new(created.did);
        let tenor_key = f.shard.reader().key(&did, Purpose::Rotation).await.expect("read key");
        assert_eq!(tenor_key.is_some(), opt_in);
        if let Some(key) = tenor_key {
            assert_eq!(
                rotations.get(1),
                Some(&to_public(&key).expect("public tenor key").to_string())
            );
        }
        for purpose in [Purpose::Signing, Purpose::Kem] {
            let key = f
                .shard
                .reader()
                .key(&did, purpose)
                .await
                .expect("World key")
                .expect("adopted key");
            assert_eq!(key.key_type(), &purpose.key_type());
            assert!(
                f.shard
                    .reader()
                    .get(&keys::reserved_key(&reserved.reservation, purpose))
                    .await
                    .expect("reservation key")
                    .is_none()
            );
        }
        let submitted = f.plc.submissions_for(did.as_str());
        let accepted: Operation =
            serde_json::from_value(submitted.first().expect("submission").operation.clone()).expect("PLC op");
        accepted
            .verify(std::slice::from_ref(&owner_key))
            .expect("submitted owner signature");
        assert_eq!(accepted, operation);
    }
}

/// An unrelated signing key or another service-auth issuer cannot consume a
/// reservation. Catches trusting all listed rotation keys or only the token,
/// and accepting a changed operation merely because the owner signed it.
#[tokio::test]
async fn another_key_or_owner_cannot_create_the_reserved_world() {
    let f = Fixture::new().await;
    let reserved = f.reserve("protected.motet.test", true).await;
    let stranger = generate_key(KeyType::K256Private).expect("stranger");
    let forged = Fixture::signed(&reserved, &stranger);
    assert_eq!(f.create(&reserved, &forged).await.0, StatusCode::BAD_REQUEST);
    let valid = Fixture::signed(&reserved, &f.rotation);
    let request = json!({ "reservation": reserved.reservation, "signedOperation": valid });
    assert_eq!(
        f.post(api::CREATE_WORLD.nsid, request, OTHER, f.hosting.audience())
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    let mut changed = reserved.operation.clone();
    changed["alsoKnownAs"] = json!(["at://stolen.motet.test"]);
    let changed: atproto_identity::plc::UnsignedOperation = serde_json::from_value(changed).expect("unsigned op");
    let changed = changed.sign(&f.rotation).expect("owner signature on altered op");
    assert_eq!(f.create(&reserved, &changed).await.0, StatusCode::BAD_REQUEST);
    assert_eq!(f.plc.submission_count(), 0);
    assert_eq!(
        f.shard
            .reader()
            .scan(keys::WORLDS, None, usize::MAX)
            .await
            .expect("Worlds"),
        []
    );
    assert!(matches!(
        identity::resolve_handle(f.shard.reader(), "protected.motet.test").await,
        Err(IdentityError::HandleNotFound)
    ));
    assert_eq!(f.create(&reserved, &valid).await.0, StatusCode::OK);
}

/// Acknowledged identity and repo state survive takeover together. Catches
/// keeping only an in-memory handle map, losing the genesis, or publishing a
/// World before the sealed keys and public repo reach the same durable batch.
#[tokio::test]
async fn created_world_resolves_and_serves_its_repo_after_reopen() {
    let mut f = Fixture::new().await;
    let reserved = f.reserve("durable.motet.test", false).await;
    let operation = Fixture::signed(&reserved, &f.rotation);
    let (status, body) = f.create(&reserved, &operation).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let created: CreateWorldOutput = serde_json::from_value(body).expect("creation response");
    let path = format!("/xrpc/com.atproto.sync.getLatestCommit?did={}", created.did);
    let (status, before) = query(f.router().await, &path).await;
    assert_eq!(status, StatusCode::OK, "{before}");
    f.reopen().await;
    let (status, resolved) = query(
        f.router().await,
        "/xrpc/com.atproto.identity.resolveHandle?handle=DURABLE.MOTET.TEST",
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{resolved}");
    assert_eq!(resolved["did"], created.did);
    let (status, after) = query(f.router().await, &path).await;
    assert_eq!(status, StatusCode::OK, "{after}");
    assert_eq!(before, after);
    let (status, description) = query(
        f.router().await,
        "/xrpc/com.atproto.repo.describeRepo?repo=durable.motet.test",
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{description}");
    assert_eq!(description["did"], created.did);
    assert_eq!(description["handle"], "durable.motet.test");
    assert_eq!(description["handleIsCorrect"], true);
    let response = f
        .router()
        .await
        .oneshot(
            Request::get("/.well-known/atproto-did")
                .header("host", "durable.motet.test:2584")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("well-known response");
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        to_bytes(response.into_body(), 1024).await.expect("DID body").as_ref(),
        created.did.as_bytes()
    );
    let (status, description) = query(f.router().await, "/xrpc/com.atproto.server.describeServer").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(description["availableUserDomains"], json!([".motet.test"]));
    assert_eq!(description["did"], "did:web:tenor.motet.test");
    assert_eq!(f.create(&reserved, &operation).await.0, StatusCode::OK);
    assert_eq!(f.plc.submission_count(), 1);
}

/// Expiry is checked even before the periodic sweep. Catches treating the
/// sweeper as the authorization check, or retaining an expired handle lock
/// and key blobs after cleanup so nobody else can reserve the name.
#[tokio::test]
async fn expired_reservations_cannot_be_created_and_release_their_keys() {
    let f = Fixture::new().await;
    let now = SystemTime::now();
    let reserved = f
        .shard
        .run(ReserveWorld {
            owner: Did::new(OWNER),
            request: ReserveRequest {
                handle: "expired.motet.test".into(),
                rotation_key: to_public(&f.rotation).expect("public key").to_string(),
                tenor_rotation_key: true,
            },
            hosting: Arc::clone(&f.hosting),
            now: now - Duration::from_hours(24),
        })
        .await
        .expect("writer")
        .expect("reservation");
    let operation = reserved.operation.sign(&f.rotation).expect("signature");
    let result = identity::create(
        &f.shard,
        &f.hosting,
        Did::new(OWNER),
        identity::CreateRequest {
            reservation: reserved.reservation.clone(),
            signed_operation: operation,
        },
    )
    .await;
    assert!(matches!(result, Err(IdentityError::ReservationExpired)));
    assert_eq!(f.plc.submission_count(), 0);
    assert_eq!(
        f.shard
            .run(SweepReservations { now, limit: 10 })
            .await
            .expect("writer")
            .expect("sweep"),
        1
    );
    for purpose in [Purpose::Signing, Purpose::Kem, Purpose::Rotation] {
        assert!(
            f.shard
                .reader()
                .get(&keys::reserved_key(&reserved.reservation, purpose))
                .await
                .expect("key slot")
                .is_none()
        );
    }
    f.reserve("expired.motet.test", false).await;
}

/// A lost PLC response is not a new identity. Catches retrying genesis POST
/// without reading PLC: the directory refuses even an identical second one.
#[tokio::test]
async fn lost_plc_response_is_reconciled_before_another_submission() {
    let f = Fixture::new().await;
    let reserved = f.reserve("ambiguous.motet.test", false).await;
    let operation = Fixture::signed(&reserved, &f.rotation);
    f.plc.script(Fault::Delay(Duration::from_secs(30)));
    let (status, body) = f.create(&reserved, &operation).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(f.plc.submission_count(), 1);
    let (status, repeated) = f.create(&reserved, &operation).await;
    assert_eq!(status, StatusCode::OK, "{repeated}");
    assert_eq!(body, repeated);
    assert_eq!(f.plc.submission_count(), 1);
}

/// PDS proxies mint bare-DID audiences, while direct callers may name the
/// service fragment. Catches rejecting either form at authentication or the
/// creation handler, and letting a World-scoped token consume a reservation.
#[tokio::test]
async fn proxy_and_service_audiences_can_reserve_and_create_worlds() {
    let f = Fixture::new().await;
    for (handle, audience) in [
        ("proxy.motet.test", f.hosting.service_did()),
        ("direct.motet.test", f.hosting.audience()),
    ] {
        let (status, body) = f
            .post(
                api::RESERVE_WORLD.nsid,
                json!({
                    "handle": handle,
                    "rotationKey": to_public(&f.rotation).expect("public key").to_string(),
                    "tenorRotationKey": false,
                }),
                OWNER,
                audience,
            )
            .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        let reserved: ReserveWorldOutput = serde_json::from_value(body).expect("reservation");
        let operation = Fixture::signed(&reserved, &f.rotation);
        let request = json!({ "reservation": reserved.reservation, "signedOperation": operation });
        for wrong_audience in [
            "did:plc:someworld",
            "did:plc:someworld#motet_tenor",
            "did:web:other.motet.test",
            "did:web:other.motet.test#motet_tenor",
            "did:web:tenor.motet.test#atproto_pds",
        ] {
            assert_eq!(
                f.post(api::CREATE_WORLD.nsid, request.clone(), OWNER, wrong_audience)
                    .await
                    .0,
                StatusCode::UNAUTHORIZED,
                "{wrong_audience}"
            );
        }
        let (status, body) = f.post(api::CREATE_WORLD.nsid, request, OWNER, audience).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        let created: CreateWorldOutput = serde_json::from_value(body).expect("created World");
        assert_eq!(
            identity::resolve_handle(f.shard.reader(), handle)
                .await
                .expect("hosted handle"),
            Did::new(created.did)
        );
    }
    assert_eq!(f.plc.submission_count(), 2);
}

/// Tenor creation auth and per-World admission auth never substitute for one
/// another. Catches broadening `accepts_audience` without checking the method's
/// scope, and DNS suffix checks that accept an attacker-controlled sibling.
#[tokio::test]
async fn service_audiences_and_handle_domains_do_not_cross_scopes() {
    let f = Fixture::new().await;
    let public_key = to_public(&f.rotation).expect("public key").to_string();
    let request = json!({ "handle": "alice.motet.test", "rotationKey": public_key, "tenorRotationKey": false });
    for audience in [
        "did:web:other.motet.test",
        "did:web:other.motet.test#motet_tenor",
        "did:web:tenor.motet.test#atproto_pds",
        "did:plc:someworld",
        "did:plc:someworld#motet_tenor",
    ] {
        assert_eq!(
            f.post(api::RESERVE_WORLD.nsid, request.clone(), OWNER, audience)
                .await
                .0,
            StatusCode::UNAUTHORIZED
        );
    }
    for handle in [
        "alice.notmotet.test",
        "alice.motet.test.evil.test",
        "-alice.motet.test",
        "at://alice.motet.test",
        "motet.test",
    ] {
        let request = json!({ "handle": handle, "rotationKey": public_key, "tenorRotationKey": false });
        assert_eq!(
            f.post(api::RESERVE_WORLD.nsid, request, OWNER, f.hosting.audience())
                .await
                .0,
            StatusCode::BAD_REQUEST,
            "{handle}"
        );
    }
    assert_eq!(f.plc.submission_count(), 0);
}

/// Admission accepts the PDS proxy's bare World DID and its service fragment,
/// but neither authorizes another World or the creation service. Catches
/// fragment-only admission gates and dropping the request body's World binding.
#[tokio::test]
async fn proxy_audiences_reach_admission_only_for_the_named_world() {
    let f = Fixture::new().await;
    let body = json!({
        "op": "one", "world": "did:plc:worlda",
        "post": { "uri": format!("at://{OWNER}/at.motet.pilot.post/one"), "cid": "cid" }
    });
    for audience in [
        f.hosting.service_did(),
        f.hosting.audience(),
        "did:plc:worldb",
        "did:plc:worldb#motet_tenor",
        "did:plc:worlda#atproto_pds",
    ] {
        assert_eq!(
            f.post(api::SUBMIT_ADMISSION.nsid, body.clone(), OWNER, audience)
                .await
                .0,
            StatusCode::UNAUTHORIZED
        );
    }
    for audience in ["did:plc:worlda", "did:plc:worlda#motet_tenor"] {
        let (status, response) = f.post(api::SUBMIT_ADMISSION.nsid, body.clone(), OWNER, audience).await;
        assert_eq!(status, StatusCode::OK, "{audience}: {response}");
        // The author's DID document names no PDS, so read-back stays pending.
        assert_eq!(response, json!({ "status": "pending" }));
    }
    let mut creation_body = body;
    creation_body["world"] = json!(f.hosting.service_did());
    for audience in [f.hosting.service_did(), f.hosting.audience()] {
        assert_eq!(
            f.post(api::SUBMIT_ADMISSION.nsid, creation_body.clone(), OWNER, audience)
                .await
                .0,
            StatusCode::UNAUTHORIZED,
            "a matching body must not turn creation auth into admission auth"
        );
    }
}

/// A submit that did not land can be retried after takeover without changing
/// the keys or losing its handle. Catches sweeping a submitting intent as if
/// it were still an unsigned draft, which could strand a PLC-accepted DID.
#[tokio::test]
async fn pending_creation_keeps_its_keys_across_expiry_and_restart() {
    let mut f = Fixture::new().await;
    let reserved = f.reserve("pending.motet.test", true).await;
    let operation = Fixture::signed(&reserved, &f.rotation);
    f.plc.script(Fault::refuse(503, "Unavailable"));
    assert_eq!(
        f.create(&reserved, &operation).await.0,
        StatusCode::INTERNAL_SERVER_ERROR
    );
    f.reopen().await;
    let removed = f
        .shard
        .run(SweepReservations {
            now: SystemTime::now() + Duration::from_hours(48),
            limit: 100,
        })
        .await
        .expect("writer")
        .expect("sweep");
    assert_eq!(removed, 0);
    let (status, body) = f.create(&reserved, &operation).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let created: CreateWorldOutput = serde_json::from_value(body).expect("creation response");
    assert_eq!(
        created.did,
        atproto_identity::plc::Did::from_genesis(&operation)
            .expect("DID")
            .to_string()
    );
    assert_eq!(f.plc.submission_count(), 2);
    for submission in f.plc.submissions() {
        let submitted: Operation = serde_json::from_value(submission.operation).expect("PLC operation");
        assert_eq!(submitted, operation);
    }
}

/// The handle index alone is not identity proof. Catches serving a cached
/// handle after the stored DID document no longer claims it.
#[tokio::test]
async fn a_handle_without_the_did_documents_confirmation_is_not_served() {
    let f = Fixture::new().await;
    let reserved = f.reserve("confirmation.motet.test", false).await;
    let operation = Fixture::signed(&reserved, &f.rotation);
    let (status, body) = f.create(&reserved, &operation).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let created: CreateWorldOutput = serde_json::from_value(body).expect("creation response");
    let did = Did::new(created.did);
    let mut info = motet_tenor::world::get(f.shard.reader(), &did)
        .await
        .expect("World info")
        .expect("World");
    info.document.also_known_as.clear();
    f.shard
        .submit(shard::Entry::Write(shard::Write {
            mutations: vec![shard::Mutation {
                key: keys::world_info(&did).into(),
                value: Some(serde_json::to_vec(&info).expect("refreshed identity").into()),
            }],
            events: Vec::new(),
        }))
        .await
        .expect("writer")
        .expect("identity refresh");
    assert_eq!(
        query(
            f.router().await,
            "/xrpc/com.atproto.identity.resolveHandle?handle=confirmation.motet.test"
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    let response = f
        .router()
        .await
        .oneshot(
            Request::get("/.well-known/atproto-did")
                .header("host", "confirmation.motet.test")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
