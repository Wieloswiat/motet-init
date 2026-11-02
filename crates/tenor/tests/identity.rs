//! World identity properties through the service-auth router and a real
//! in-process PLC directory, with the shard's durable store and real keys.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use atproto_identity::key::{KeyData, KeyType, generate_key, identify_key, sign, to_public};
use atproto_identity::model::{Document, VerificationMethod};
use atproto_identity::plc::Operation;
use atproto_space::http_signature::sign_credential_use;
use atproto_space::kem::encapsulate;
use atproto_space::mac::{
    MAC_HEADER, MacType, WANT_MAC_HEADER, format_want_mac_header, parse_mac_header, verify_record_mac,
};
use atproto_testkit::highport::{FakePlcDirectory, Fault};
use axum::body::{Body, to_bytes};
use axum::http::{HeaderName, HeaderValue, Request, StatusCode};
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
use p256::ecdsa::{
    Signature, SigningKey,
    signature::{Signer as _, Verifier as _},
};
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

    fn router(&self) -> axum::Router {
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
        .expect("registry")
    }

    fn token(&self, issuer: &str, audience: &str, method: &str) -> String {
        let jti = atproto_record::tid::Tid::new().to_string();
        self.token_with_jti(issuer, audience, method, Some(&jti))
    }

    fn token_with_jti(&self, issuer: &str, audience: &str, method: &str, jti: Option<&str>) -> String {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).expect("clock").as_secs();
        let header = URL_SAFE_NO_PAD.encode(br#"{"alg":"ES256K","typ":"JWT"}"#);
        let mut claims = json!({"iss": issuer, "aud": audience, "lxm": method, "iat": now, "exp": now + 60});
        if let Some(jti) = jti {
            claims
                .as_object_mut()
                .expect("service claims are an object")
                .insert("jti".into(), json!(jti));
        }
        let claims = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&claims).expect("claims"));
        let input = format!("{header}.{claims}");
        let signature = sign(&self.signing, input.as_bytes()).expect("signature");
        format!("{input}.{}", URL_SAFE_NO_PAD.encode(signature))
    }

    async fn post(&self, method: &str, body: Value, issuer: &str, audience: &str) -> (StatusCode, Value) {
        self.post_token(method, body, &self.token(issuer, audience, method))
            .await
    }

    async fn post_token(&self, method: &str, body: Value, token: &str) -> (StatusCode, Value) {
        let request = Request::post(format!("/xrpc/{method}"))
            .header("authorization", format!("Bearer {token}"))
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(&body).expect("request")))
            .expect("request");
        response_json(self.router().oneshot(request).await.expect("response")).await
    }

    async fn reserve(&self, handle: &str, tenor_rotation: bool) -> ReserveWorldOutput {
        self.reserve_with(handle, tenor_rotation, None, None).await
    }

    async fn reserve_with(
        &self,
        handle: &str,
        tenor_rotation: bool,
        visibility: Option<&str>,
        name: Option<&str>,
    ) -> ReserveWorldOutput {
        let mut body = json!({
            "op": atproto_record::tid::Tid::new().to_string(),
            "handle": handle,
            "rotationKey": to_public(&self.rotation).expect("public rotation key").to_string(),
            "tenorRotationKey": tenor_rotation,
        });
        if let Some(visibility) = visibility {
            body.as_object_mut()
                .expect("reservation body is an object")
                .insert("visibility".into(), json!(visibility));
        }
        if let Some(name) = name {
            body.as_object_mut()
                .expect("reservation body is an object")
                .insert("name".into(), json!(name));
        }
        let request: ReserveWorldInput = serde_json::from_value(body).expect("generated reservation input contract");
        let (status, body) = self
            .post(
                api::RESERVE_WORLD.nsid,
                serde_json::to_value(request).expect("request"),
                OWNER,
                self.hosting.service_did(),
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
            self.hosting.service_did(),
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

async fn mint_anonymous(f: &Fixture, space: &str, holder: &KeyData) -> (StatusCode, Value) {
    let request = Request::post("/xrpc/com.atproto.space.getSpaceCredential")
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::to_vec(&json!({
                "space": space,
                "credentialKid": to_public(holder).expect("reader public key").to_string()
            }))
            .expect("mint request"),
        ))
        .expect("mint request");
    response_json(f.router().oneshot(request).await.expect("mint response")).await
}

async fn read_public_profile(
    f: &Fixture,
    reserved: &ReserveWorldOutput,
    did: &str,
    meta: &str,
    holder: &KeyData,
) -> Value {
    let (status, minted) = mint_anonymous(f, meta, holder).await;
    assert_eq!(status, StatusCode::OK, "{minted}");
    let credential = minted
        .get("credential")
        .and_then(Value::as_str)
        .expect("public reader credential");
    let signed_headers = sign_credential_use(credential, did, holder).expect("reader proof");
    let kem_key = identify_key(
        reserved
            .operation
            .get("verificationMethods")
            .and_then(|methods| methods.get("atproto_kem"))
            .and_then(Value::as_str)
            .expect("reserved public KEM key"),
    )
    .expect("KEM did:key");
    let kem = encapsulate(&kem_key).expect("reader encapsulates to the World's public KEM key");
    let params = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("space", meta)
        .append_pair("repo", did)
        .append_pair("collection", "group.opensocial.profile")
        .append_pair("rkey", "self")
        .finish();
    let mut request = Request::get(format!("/xrpc/com.atproto.space.getRecord?{params}"))
        .body(Body::empty())
        .expect("public profile query");
    for (name, value) in signed_headers.iter() {
        request.headers_mut().insert(
            HeaderName::from_static(name),
            HeaderValue::from_str(value).expect("credential proof header"),
        );
    }
    request.headers_mut().insert(
        WANT_MAC_HEADER,
        HeaderValue::from_str(&format_want_mac_header(MacType::Record, &kem.key_share).expect("MAC request"))
            .expect("MAC header"),
    );
    let response = f.router().oneshot(request).await.expect("profile response");
    assert_eq!(response.status(), StatusCode::OK);
    let mac = parse_mac_header(
        response.headers().get(MAC_HEADER).and_then(|value| value.to_str().ok()),
        MacType::Record,
    )
    .expect("profile MAC");
    let (_, profile) = response_json(response).await;
    verify_record_mac(
        &kem.shared_secret,
        profile.get("uri").and_then(Value::as_str).expect("profile URI"),
        &profile
            .get("cid")
            .and_then(Value::as_str)
            .expect("profile CID")
            .parse()
            .expect("CID"),
        &mac,
    )
    .expect("profile MAC verifies against the reserved KEM key");
    profile
}
/// A lost response must not create new keys or strand the reserved handle.
/// Catches generating a second reservation before checking the owner's op.
#[tokio::test]
async fn reservation_retries_recover_the_original_and_other_owners_cannot_take_its_handle() {
    let f = Fixture::new().await;
    let method = api::RESERVE_WORLD.nsid;
    let audience = f.hosting.service_did();
    let request = json!({
        "op": "world-attempt",
        "handle": "retry.motet.test",
        "rotationKey": to_public(&f.rotation).expect("public rotation key").to_string(),
        "tenorRotationKey": false,
    });
    let (status, first) = f.post(method, request.clone(), OWNER, audience).await;
    assert_eq!(status, StatusCode::OK, "{first}");
    let (status, retried) = f.post(method, request.clone(), OWNER, audience).await;
    assert_eq!(status, StatusCode::OK, "{retried}");
    assert_eq!(retried, first);

    let mut changed = request.clone();
    changed["handle"] = json!("different.motet.test");
    let (status, body) = f.post(method, changed, OWNER, audience).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(body["error"], "InvalidRequest");

    let mut other_op = request;
    other_op["op"] = json!("another-attempt");
    let (status, body) = f.post(method, other_op, OTHER, audience).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(body["error"], "HandleTaken");
}

/// Reloading with a new owner key replaces only the owner's unsigned draft.
/// Catches leaving two live reservations or sealed keys for one handle.
#[tokio::test]
async fn owner_can_replace_an_unsigned_reservation_without_leaving_a_creatable_old_draft() {
    let f = Fixture::new().await;
    let method = api::RESERVE_WORLD.nsid;
    let audience = f.hosting.service_did();
    let old = f.reserve("reload.motet.test", true).await;
    let signed = Fixture::signed(&old, &f.rotation);
    let new_key = generate_key(KeyType::K256Private).expect("new owner key");
    let request = json!({
        "op": "after-reload",
        "handle": "reload.motet.test",
        "rotationKey": to_public(&new_key).expect("new public key").to_string(),
        "tenorRotationKey": false,
    });
    let (status, replacement) = f.post(method, request, OWNER, audience).await;
    assert_eq!(status, StatusCode::OK, "{replacement}");
    assert_ne!(replacement["reservation"], old.reservation);
    let (status, body) = f
        .post(
            api::CREATE_WORLD.nsid,
            json!({"reservation": old.reservation, "signedOperation": signed}),
            OWNER,
            audience,
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    for purpose in [Purpose::Signing, Purpose::Kem, Purpose::Rotation] {
        assert!(
            f.shard
                .reader()
                .get(&keys::reserved_key(&old.reservation, purpose))
                .await
                .expect("key slot")
                .is_none()
        );
    }
}

/// Tenor-scoped reservation and creation intents spend signed tokens in the
/// shard. Catches changed-body replay before a World exists, cross-method jti
/// reuse and a failed validation consuming the owner's only token.
#[tokio::test]
async fn world_creation_intents_require_fresh_tokens_and_roll_back_failed_attempts() {
    let f = Fixture::new().await;
    let audience = f.hosting.service_did();
    let reserve = api::RESERVE_WORLD.nsid;
    let create = api::CREATE_WORLD.nsid;
    let rotation = to_public(&f.rotation).expect("public rotation key").to_string();
    let reserve_body =
        |handle: &str| json!({"op":handle,"handle":handle,"rotationKey":rotation,"tenorRotationKey":false});
    let without_jti = f.token_with_jti(OWNER, audience, reserve, None);
    assert_eq!(
        f.post_token(reserve, reserve_body("first.motet.test"), &without_jti)
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    let reserve_token = f.token_with_jti(OWNER, audience, reserve, Some("reservation-token"));
    assert_eq!(
        f.post_token(reserve, reserve_body("bad/handle"), &reserve_token)
            .await
            .0,
        StatusCode::BAD_REQUEST
    );
    let (status, result) = f
        .post_token(reserve, reserve_body("first.motet.test"), &reserve_token)
        .await;
    assert_eq!(status, StatusCode::OK, "{result}");
    let first: ReserveWorldOutput = serde_json::from_value(result).expect("reservation");
    assert_eq!(
        f.post_token(reserve, reserve_body("second.motet.test"), &reserve_token)
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    let second = f.reserve("second.motet.test", false).await;
    let first_request = json!({
        "reservation":first.reservation,
        "signedOperation":Fixture::signed(&first, &f.rotation),
    });
    let second_request = json!({
        "reservation":second.reservation,
        "signedOperation":Fixture::signed(&second, &f.rotation),
    });
    let no_create_jti = f.token_with_jti(OWNER, audience, create, None);
    assert_eq!(
        f.post_token(create, first_request.clone(), &no_create_jti).await.0,
        StatusCode::FORBIDDEN
    );
    let cross_method = f.token_with_jti(OWNER, audience, create, Some("reservation-token"));
    assert_eq!(
        f.post_token(create, first_request.clone(), &cross_method).await.0,
        StatusCode::FORBIDDEN
    );
    let create_token = f.token_with_jti(OWNER, audience, create, Some("creation-token"));
    let stranger = generate_key(KeyType::P256Private).expect("stranger rotation key");
    let invalid = json!({
        "reservation":first.reservation,
        "signedOperation":Fixture::signed(&first, &stranger),
    });
    assert_eq!(
        f.post_token(create, invalid, &create_token).await.0,
        StatusCode::BAD_REQUEST
    );
    let (status, created) = f.post_token(create, first_request.clone(), &create_token).await;
    assert_eq!(status, StatusCode::OK, "{created}");
    assert_eq!(
        f.post_token(create, second_request.clone(), &create_token).await.0,
        StatusCode::FORBIDDEN
    );
    let (status, retried) = f.post(create, first_request, OWNER, audience).await;
    assert_eq!(status, StatusCode::OK, "{retried}");
    assert_eq!(retried, created);
    let (status, created_second) = f.post(create, second_request, OWNER, audience).await;
    assert_eq!(status, StatusCode::OK, "{created_second}");
    assert_ne!(created["did"], created_second["did"]);
}

/// A new client can discover the tenor before any World exists and use its DID
/// as the creation-token audience. Catches an absent document, a service ID
/// bound to another DID, an endpoint derived from an untrusted Host header, or
/// discovery of an audience the creation procedure refuses.
#[tokio::test]
async fn tenor_did_document_discovers_a_usable_world_creation_service() {
    let mut f = Fixture::new().await;
    for (index, (origin, host, did)) in [
        (
            "https://tenor.motet.test",
            "tenor.motet.test",
            "did:web:tenor.motet.test",
        ),
        (
            "https://tenor.motet.test:8443",
            "attacker.example",
            "did:web:tenor.motet.test%3A8443",
        ),
    ]
    .into_iter()
    .enumerate()
    {
        f.hosting = Arc::new(
            Hosting::new(
                &origin.parse().expect("origin"),
                vec![".motet.test".into()],
                f.plc.origin().into(),
                f.fetcher.clone(),
                Vec::new(),
            )
            .expect("hosting configuration"),
        );
        let request = Request::get("/.well-known/did.json")
            .header("host", host)
            .body(Body::empty())
            .expect("DID document request");
        let (status, value) = response_json(f.router().oneshot(request).await.expect("DID document response")).await;
        assert_eq!(status, StatusCode::OK, "{value}");
        let document: Document = serde_json::from_value(value).expect("a DID document");
        assert_eq!(document.id, did);
        assert_eq!(document.service_endpoint("motet_tenor"), Some(origin));

        let (status, body) = f
            .post(
                api::RESERVE_WORLD.nsid,
                json!({
                    "op": format!("discovery-{index}"),
                    "handle": format!("discovery-{index}.motet.test"),
                    "rotationKey": to_public(&f.rotation).expect("public rotation key").to_string(),
                    "tenorRotationKey": false,
                }),
                OWNER,
                &document.id,
            )
            .await;
        assert_eq!(status, StatusCode::OK, "{body}");
    }
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
    let (status, body) = f
        .post(api::CREATE_WORLD.nsid, request, OTHER, f.hosting.service_did())
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["error"], "Forbidden");
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

/// Only the owner's canonical P-256 signature over the raw returned bytes
/// can start creation. Catches accepting the tenor's lower-priority key,
/// double-hashing, DER, padded or malleable signatures, or storing a bad intent.
#[tokio::test]
async fn p256_genesis_accepts_only_the_owner_low_s_signature_over_raw_signing_bytes() {
    let f = Fixture::new().await;
    let reserved = f.reserve("p256.motet.test", true).await;
    let bytes = URL_SAFE_NO_PAD.decode(&reserved.signing_bytes).expect("raw DAG-CBOR");
    let signing = SigningKey::from_slice(f.rotation.bytes()).expect("owner P-256 key");
    let low: Signature = signing.sign(&bytes);
    let low = low.normalize_s();
    let high = Signature::from_scalars(low.r(), -low.s()).expect("high-S twin");
    signing
        .verifying_key()
        .verify(&bytes, &high)
        .expect("the high-S twin is a mathematically valid P-256 signature");

    let operation_with = |sig: String| -> Operation {
        let mut operation = reserved.operation.clone();
        operation
            .as_object_mut()
            .expect("PLC operation is an object")
            .insert("sig".into(), json!(sig));
        serde_json::from_value(operation).expect("signed PLC operation")
    };
    let high = operation_with(URL_SAFE_NO_PAD.encode(high.to_bytes()));
    assert_eq!(f.create(&reserved, &high).await.0, StatusCode::BAD_REQUEST);

    let digest = atproto_identity::plc::encoding::sha256(&bytes);
    let double_hashed: Signature = signing.sign(&digest);
    let double_hashed = operation_with(URL_SAFE_NO_PAD.encode(double_hashed.normalize_s().to_bytes()));
    assert_eq!(f.create(&reserved, &double_hashed).await.0, StatusCode::BAD_REQUEST);

    let padded = operation_with(format!("{}=", URL_SAFE_NO_PAD.encode(low.to_bytes())));
    assert_eq!(f.create(&reserved, &padded).await.0, StatusCode::BAD_REQUEST);

    let der = operation_with(URL_SAFE_NO_PAD.encode(low.to_der().as_bytes()));
    assert_eq!(f.create(&reserved, &der).await.0, StatusCode::BAD_REQUEST);

    let stranger = generate_key(KeyType::P256Private).expect("another P-256 rotation key");
    assert_eq!(
        f.create(&reserved, &Fixture::signed(&reserved, &stranger)).await.0,
        StatusCode::BAD_REQUEST
    );
    let sealed = f
        .shard
        .reader()
        .get(&keys::reserved_key(&reserved.reservation, Purpose::Rotation))
        .await
        .expect("read reserved tenor key")
        .expect("tenor rotation key was opted in");
    let tenor = f
        .master
        .unseal(
            &Did::new(format!("reservation/{}", reserved.reservation)),
            Purpose::Rotation,
            &sealed,
        )
        .expect("tenor can open its reserved recovery key");
    assert_eq!(
        reserved.operation["rotationKeys"][1],
        to_public(&tenor).expect("tenor public key").to_string()
    );
    assert_eq!(
        f.create(&reserved, &Fixture::signed(&reserved, &tenor)).await.0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(f.plc.submission_count(), 0);

    let valid = operation_with(URL_SAFE_NO_PAD.encode(low.to_bytes()));
    let (status, created) = f.create(&reserved, &valid).await;
    assert_eq!(status, StatusCode::OK, "{created}");
    assert_eq!(f.plc.submission_count(), 1);
}

/// A public selection and profile name survive an unfinished PLC submission
/// and shard replay. Catches losing or replacing reserved fields, publishing
/// no group declaration, or marking meta public without readable profiles.
#[tokio::test]
async fn public_world_creation_publishes_its_named_meta_after_retry_and_replay() {
    let mut f = Fixture::new().await;
    let reserved = f
        .reserve_with(
            "lantern.motet.test",
            false,
            Some("public"),
            Some("The Lantern – Étoile"),
        )
        .await;
    let operation = Fixture::signed(&reserved, &f.rotation);
    f.reopen().await;
    f.plc.script(Fault::refuse(503, "Unavailable"));
    assert_eq!(
        f.create(&reserved, &operation).await.0,
        StatusCode::INTERNAL_SERVER_ERROR
    );
    f.reopen().await;
    // A retry cannot override the options fixed by the original reservation.
    let (status, created) = f
        .post(
            api::CREATE_WORLD.nsid,
            json!({
                "reservation": reserved.reservation,
                "signedOperation": operation,
                "visibility": "private",
                "name": "Replaced",
            }),
            OWNER,
            f.hosting.service_did(),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{created}");
    let created: CreateWorldOutput = serde_json::from_value(created).expect("creation response");
    f.reopen().await;
    let (status, retry) = f.create(&reserved, &operation).await;
    assert_eq!(status, StatusCode::OK, "{retry}");
    assert_eq!(retry["did"], created.did);
    assert_eq!(f.plc.submission_count(), 2);

    let declaration_path = format!(
        "/xrpc/com.atproto.repo.getRecord?repo={}&collection=group.opensocial.declaration&rkey=self",
        created.did
    );
    let (status, declaration) = query(f.router(), &declaration_path).await;
    assert_eq!(status, StatusCode::OK, "{declaration}");
    let world = Did::new(&created.did);
    let meta = motet_tenor::spaces::world_space(&world, motet_tenor::spaces::META)
        .expect("World meta Space")
        .to_string();
    assert_eq!(declaration["value"]["meta"], meta);

    // An unrelated reader obtains a public credential without a delegation.
    let holder = generate_key(KeyType::P256Private).expect("reader credential key");
    let profile = read_public_profile(&f, &reserved, &created.did, &meta, &holder).await;
    assert_eq!(profile["value"]["displayName"], "The Lantern – Étoile");
    for space_type in [motet_tenor::spaces::MEMBERS, motet_tenor::spaces::GMS] {
        let private = motet_tenor::spaces::world_space(&world, space_type)
            .expect("private World Space")
            .to_string();
        let (status, body) = mint_anonymous(&f, &private, &holder).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
        assert_eq!(body["error"], "SpaceNotFound");
    }
}

/// Explicit and omitted private visibility keep names out of public discovery.
/// Catches treating any supplied visibility as public or changing old defaults.
#[tokio::test]
async fn private_worlds_never_publish_a_declaration_or_anonymous_meta_access() {
    let f = Fixture::new().await;
    for (handle, visibility, name) in [
        ("default.motet.test", None, None),
        ("private.motet.test", Some("private"), Some("Hidden Lantern")),
    ] {
        let reserved = f.reserve_with(handle, false, visibility, name).await;
        let operation = Fixture::signed(&reserved, &f.rotation);
        let (status, created) = f.create(&reserved, &operation).await;
        assert_eq!(status, StatusCode::OK, "{created}");
        let did = created["did"].as_str().expect("World DID");
        let declaration_path =
            format!("/xrpc/com.atproto.repo.getRecord?repo={did}&collection=group.opensocial.declaration&rkey=self");
        let (status, missing) = query(f.router(), &declaration_path).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{missing}");
        assert_eq!(missing["error"], "RecordNotFound");
        let meta =
            motet_tenor::spaces::world_space(&Did::new(did), motet_tenor::spaces::META).expect("World meta Space");
        let profile = motet_tenor::spaces::record(f.shard.reader(), &meta, "group.opensocial.profile", "self")
            .await
            .expect("profile read")
            .expect("profile exists");
        let profile = atproto_dasl::from_slice_to_json(&profile.value).expect("profile DRISL");
        assert_eq!(profile["displayName"], name.unwrap_or(handle));
        let holder = generate_key(KeyType::P256Private).expect("reader credential key");
        let (status, body) = mint_anonymous(&f, &meta.to_string(), &holder).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
        assert_eq!(body["error"], "SpaceNotFound");
    }
}

/// Invalid profile names must fail before irreversible PLC publication.
/// Catches accepting a reservation which could never bootstrap its profile.
#[tokio::test]
async fn invalid_profile_cannot_reserve_a_world_or_strand_its_handle() {
    let f = Fixture::new().await;
    let request = json!({
        "op": "invalid-profile",
        "handle": "reusable.motet.test",
        "rotationKey": to_public(&f.rotation).expect("public key").to_string(),
        "tenorRotationKey": false,
        "visibility": "public",
        "name": "x".repeat(65),
    });
    let (status, body) = f
        .post(api::RESERVE_WORLD.nsid, request, OWNER, f.hosting.service_did())
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    let invalid_visibility = json!({
        "op": "invalid-visibility",
        "handle": "reusable.motet.test",
        "rotationKey": to_public(&f.rotation).expect("public key").to_string(),
        "tenorRotationKey": false,
        "visibility": "friends",
    });
    let (status, body) = f
        .post(
            api::RESERVE_WORLD.nsid,
            invalid_visibility,
            OWNER,
            f.hosting.service_did(),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(f.plc.submission_count(), 0);
    let reserved = f
        .reserve_with("reusable.motet.test", false, Some("public"), Some("Reusable"))
        .await;
    let operation = Fixture::signed(&reserved, &f.rotation);
    let (status, body) = f.create(&reserved, &operation).await;
    assert_eq!(status, StatusCode::OK, "{body}");
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
    let (status, before) = query(f.router(), &path).await;
    assert_eq!(status, StatusCode::OK, "{before}");
    f.reopen().await;
    let (status, resolved) = query(
        f.router(),
        "/xrpc/com.atproto.identity.resolveHandle?handle=DURABLE.MOTET.TEST",
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{resolved}");
    assert_eq!(resolved["did"], created.did);
    let (status, after) = query(f.router(), &path).await;
    assert_eq!(status, StatusCode::OK, "{after}");
    assert_eq!(before, after);
    let (status, description) = query(
        f.router(),
        "/xrpc/com.atproto.repo.describeRepo?repo=durable.motet.test",
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{description}");
    assert_eq!(description["did"], created.did);
    assert_eq!(description["handle"], "durable.motet.test");
    assert_eq!(description["handleIsCorrect"], true);
    let response = f
        .router()
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
    let (status, description) = query(f.router(), "/xrpc/com.atproto.server.describeServer").await;
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
                op: "expired".into(),
                handle: "expired.motet.test".into(),
                rotation_key: to_public(&f.rotation).expect("public key").to_string(),
                tenor_rotation_key: true,
                visibility: identity::Visibility::Private,
                name: None,
            },
            hosting: Arc::clone(&f.hosting),
            now: now - Duration::from_hours(24),
        })
        .await
        .expect("writer")
        .expect("reservation");
    let operation = reserved.operation.sign(&f.rotation).expect("signature");
    let (status, _) = f
        .post(
            api::CREATE_WORLD.nsid,
            json!({"reservation": reserved.reservation.clone(), "signedOperation": operation}),
            OWNER,
            f.hosting.service_did(),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
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

/// World creation requires the bare tenor service DID. Catches letting a
/// World-scoped token or another service consume an owner's reservation.
#[tokio::test]
async fn world_creation_requires_the_bare_tenor_service_audience() {
    let f = Fixture::new().await;
    let reserved = f.reserve("direct.motet.test", false).await;
    let operation = Fixture::signed(&reserved, &f.rotation);
    let request = json!({ "reservation": reserved.reservation, "signedOperation": operation });
    for wrong_audience in [
        f.hosting.audience(),
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
    let (status, body) = f
        .post(api::CREATE_WORLD.nsid, request, OWNER, f.hosting.service_did())
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let created: CreateWorldOutput = serde_json::from_value(body).expect("created World");
    assert_eq!(
        identity::resolve_handle(f.shard.reader(), "direct.motet.test")
            .await
            .expect("hosted handle"),
        Did::new(created.did)
    );
    assert_eq!(f.plc.submission_count(), 1);
}

/// Creation rejects tokens for another audience and handles outside its DNS
/// domain. Catches confusing the tenor service with a World or sibling domain.
#[tokio::test]
async fn service_audiences_and_handle_domains_do_not_cross_scopes() {
    let f = Fixture::new().await;
    let public_key = to_public(&f.rotation).expect("public key").to_string();
    let request =
        json!({ "op": "audience", "handle": "alice.motet.test", "rotationKey": public_key, "tenorRotationKey": false });
    for audience in [
        f.hosting.audience(),
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
        let request =
            json!({ "op": "invalid-handle", "handle": handle, "rotationKey": public_key, "tenorRotationKey": false });
        assert_eq!(
            f.post(api::RESERVE_WORLD.nsid, request, OWNER, f.hosting.service_did())
                .await
                .0,
            StatusCode::BAD_REQUEST,
            "{handle}"
        );
    }
    assert_eq!(f.plc.submission_count(), 0);
}

/// Admission accepts both hosted World service audiences but not another
/// World's. Catches confusing creation auth with a hosted World target.
#[tokio::test]
async fn admission_requires_the_named_world_audience() {
    let f = Fixture::new().await;
    let reserved = f.reserve("admission.motet.test", false).await;
    let operation = Fixture::signed(&reserved, &f.rotation);
    let (status, created) = f.create(&reserved, &operation).await;
    assert_eq!(status, StatusCode::OK, "{created}");
    let did = created["did"].as_str().expect("World DID");
    let body = json!({
        "op": "one", "world": did,
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
    let fragment = format!("{did}#motet_tenor");
    for audience in [did, fragment.as_str()] {
        // The author's DID document names no PDS, so read-back stays pending.
        let (status, response) = f.post(api::SUBMIT_ADMISSION.nsid, body.clone(), OWNER, audience).await;
        assert_eq!(status, StatusCode::OK, "{response}");
        assert_eq!(response, json!({ "status": "pending" }));
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
            f.router(),
            "/xrpc/com.atproto.identity.resolveHandle?handle=confirmation.motet.test"
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    let response = f
        .router()
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
