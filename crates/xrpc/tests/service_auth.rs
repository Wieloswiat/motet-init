//! Service auth against real signatures: tokens minted with generated keys,
//! checked against DID documents served by an in-memory resolver.

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use atproto_identity::key::{KeyData, KeyType, generate_key, to_public};
use atproto_identity::model::{Document, VerificationMethod};
use atproto_identity::traits::IdentityResolver;
use atproto_oauth::jwt::{Claims, Header, JoseClaims, mint};
use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::response::IntoResponse;
use motet_xrpc::server::Service;
use motet_xrpc::service_auth::{MAX_LIFETIME_SECS, ServiceAuthError, check_claims, verify};
use motet_xrpc::{Access, Caller, Kind, Method};
use proptest::prelude::*;
use tower::ServiceExt;

const ALICE: &str = "did:plc:alice";
const APPVIEW: &str = "did:web:api.motet.test#motet_appview";
const METHOD: &str = "at.motet.pilot.play.submitAdmission";

#[derive(Clone)]
struct Directory(BTreeMap<String, Document>);

struct CountingDirectory {
    directory: Directory,
    resolutions: AtomicUsize,
}

#[async_trait::async_trait]
impl IdentityResolver for CountingDirectory {
    async fn resolve(&self, subject: &str) -> anyhow::Result<Document> {
        self.resolutions.fetch_add(1, Ordering::SeqCst);
        self.directory.resolve(subject).await
    }
}

struct InterruptibleDirectory {
    directory: Directory,
    unavailable: AtomicBool,
}

#[async_trait::async_trait]
impl IdentityResolver for InterruptibleDirectory {
    async fn resolve(&self, subject: &str) -> anyhow::Result<Document> {
        if self.unavailable.load(Ordering::SeqCst) {
            anyhow::bail!("directory unavailable");
        }
        self.directory.resolve(subject).await
    }
}

#[async_trait::async_trait]
impl IdentityResolver for Directory {
    async fn resolve(&self, subject: &str) -> anyhow::Result<Document> {
        self.0
            .get(subject)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("unknown DID {subject}"))
    }
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after 1970")
        .as_secs()
}

fn document(did: &str, fragment: &str, key: &KeyData) -> Document {
    let public = to_public(key).expect("public half of a generated key");
    let multibase = public.to_string();
    let multibase = multibase.strip_prefix("did:key:").unwrap_or(&multibase).to_owned();
    Document {
        context: Vec::new(),
        id: did.to_owned(),
        also_known_as: Vec::new(),
        service: Vec::new(),
        verification_method: vec![VerificationMethod::Multikey {
            id: format!("{did}#{fragment}"),
            controller: did.to_owned(),
            public_key_multibase: multibase,
            extra: HashMap::default(),
        }],
        extra: HashMap::default(),
    }
}

struct Fixture {
    key: KeyData,
    directory: Directory,
}

impl Fixture {
    fn new() -> Self {
        let key = generate_key(KeyType::K256Private).expect("key generation");
        let directory = Directory(BTreeMap::from([(ALICE.to_owned(), document(ALICE, "atproto", &key))]));
        Self { key, directory }
    }

    fn token(&self, edit: impl FnOnce(&mut Claims)) -> String {
        let now = now();
        let mut claims = Claims::new(JoseClaims {
            issuer: Some(ALICE.into()),
            audience: Some(APPVIEW.into()),
            issued_at: Some(now),
            expiration: Some(now + 60),
            ..JoseClaims::default()
        });
        claims.private.insert("lxm".into(), METHOD.into());
        edit(&mut claims);
        let header = Header::try_from(self.key.clone()).expect("header for a K256 key");
        mint(&self.key, &header, &claims).expect("minting")
    }

    async fn verify(&self, token: &str) -> Result<Caller, ServiceAuthError> {
        verify(token, &self.directory, |aud| aud == APPVIEW, METHOD, now()).await
    }
}

/// The replay ledger needs the exact signed identifier and expiry. Dropping
/// either would let a token be reused or recorded for the wrong interval.
#[tokio::test]
async fn verified_service_token_retains_identifier_and_expiry() {
    let f = Fixture::new();
    let expiry = now() + 50;
    let token = f.token(|claims| {
        claims.jose.expiration = Some(expiry);
        claims.jose.json_web_token_id = Some("token-from-the-pds".into());
    });
    assert_eq!(
        f.verify(&token).await,
        Ok(Caller::Service {
            did: ALICE.into(),
            lxm: METHOD.into(),
            audience: APPVIEW.into(),
            jti: Some("token-from-the-pds".into()),
            expires_at: expiry,
        })
    );
    let token = f.token(|claims| claims.jose.expiration = Some(expiry));
    assert_eq!(
        f.verify(&token).await,
        Ok(Caller::Service {
            did: ALICE.into(),
            lxm: METHOD.into(),
            audience: APPVIEW.into(),
            jti: None,
            expires_at: expiry,
        })
    );
}

/// A service issuer must remain distinguishable from its owning account.
/// Stripping the fragment would let a service be mistaken for a World member.
#[tokio::test]
async fn verified_service_issuer_keeps_its_signed_fragment() {
    let f = Fixture::new();
    for issuer in ["did:plc:alice#atproto_pds", "did:plc:alice#atproto_labeler"] {
        let expiry = now() + 50;
        let token = f.token(|claims| {
            claims.jose.issuer = Some(issuer.into());
            claims.jose.expiration = Some(expiry);
        });
        assert_eq!(
            f.verify(&token).await,
            Ok(Caller::Service {
                did: issuer.into(),
                lxm: METHOD.into(),
                audience: APPVIEW.into(),
                jti: None,
                expires_at: expiry,
            })
        );
    }
}

#[tokio::test]
async fn tokens_for_another_service_are_refused() {
    let f = Fixture::new();
    let token = f.token(|c| c.jose.audience = Some("did:web:elsewhere.test".into()));
    assert_eq!(f.verify(&token).await, Err(ServiceAuthError::WrongAudience));
}

#[tokio::test]
async fn tokens_without_a_method_are_refused() {
    let f = Fixture::new();
    let token = f.token(|c| {
        c.private.remove("lxm");
    });
    assert_eq!(f.verify(&token).await, Err(ServiceAuthError::NoMethod));
}

#[tokio::test]
async fn long_lived_or_endless_tokens_are_refused() {
    let f = Fixture::new();
    let endless = f.token(|c| c.jose.expiration = None);
    let long = f.token(|c| c.jose.expiration = c.jose.issued_at.map(|t| t + MAX_LIFETIME_SECS + 1));
    assert_eq!(f.verify(&endless).await, Err(ServiceAuthError::BadLifetime));
    assert_eq!(f.verify(&long).await, Err(ServiceAuthError::BadLifetime));
}

/// A long-lived token cannot become acceptable merely because it is near its
/// expiry; otherwise a stolen token is valid beyond the issuer's minute.
#[tokio::test]
async fn lifetime_cap_includes_the_signed_issuance_time() {
    let f = Fixture::new();
    let now = 1_700_000_000;
    for (issued, expires, accepted) in [
        (now, now + 60, true),
        (now - 1, now + 60, false),
        (now - 60, now + 1, false),
    ] {
        let token = f.token(|claims| {
            claims.jose.issued_at = Some(issued);
            claims.jose.expiration = Some(expires);
        });
        assert_eq!(
            verify(&token, &f.directory, |aud| aud == APPVIEW, METHOD, now)
                .await
                .is_ok(),
            accepted,
        );
    }
}

/// Invalid claims must not turn unauthenticated HTTP requests into DID
/// directory traffic, while a valid request still reaches the handler.
#[tokio::test]
async fn invalid_audience_method_and_lifetime_never_resolve_through_the_router() {
    let f = Fixture::new();
    let directory = Arc::new(CountingDirectory {
        directory: f.directory.clone(),
        resolutions: AtomicUsize::new(0),
    });
    let calls = Arc::new(AtomicUsize::new(0));
    let authenticate = motet_xrpc::service_auth::authenticate(
        Arc::<CountingDirectory>::clone(&directory),
        Arc::new(|aud| aud == APPVIEW),
    );
    let router = Service::new(authenticate)
        .method(
            Method {
                nsid: METHOD,
                kind: Kind::Procedure,
                access: Access::Service,
                max_body: 0,
            },
            |calls: Arc<AtomicUsize>, _: Caller, _| async move {
                calls.fetch_add(1, Ordering::SeqCst);
                Ok(StatusCode::NO_CONTENT.into_response())
            },
        )
        .into_router(Arc::clone(&calls))
        .expect("registered method");
    let now = now();
    let invalid = [
        f.token(|c| c.jose.audience = Some("did:web:elsewhere.test".into())),
        f.token(|c| c.jose.audience = None),
        f.token(|c| {
            c.private.remove("lxm");
        }),
        f.token(|c| {
            c.private.insert("lxm".into(), "".into());
        }),
        f.token(|c| {
            c.private.insert("lxm".into(), "at.motet.pilot.play.closeScene".into());
        }),
        f.token(|c| c.jose.expiration = None),
        f.token(|c| c.jose.issued_at = None),
        f.token(|c| {
            c.jose.issued_at = None;
            c.jose.not_before = Some(now.saturating_sub(1));
        }),
        f.token(|c| c.jose.expiration = Some(now)),
        f.token(|c| {
            c.jose.issued_at = Some(now);
            c.jose.expiration = Some(now + 61);
        }),
        f.token(|c| c.jose.not_before = Some(now + 40)),
        f.token(|c| {
            c.jose.issued_at = Some(now + 30);
            c.jose.expiration = Some(now + 20);
        }),
        f.token(|c| {
            c.jose.issued_at = Some(now + 61);
            c.jose.expiration = Some(now + 40);
        }),
    ];
    for token in invalid {
        let request = Request::post(format!("/xrpc/{METHOD}"))
            .header("authorization", format!("Bearer {token}"))
            .body(Body::empty())
            .expect("request");
        let response = router.clone().oneshot(request).await.expect("router");
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(directory.resolutions.load(Ordering::SeqCst), 0);
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }

    // The expected method is the registry's decoded path, not the raw URI.
    let token = f.token(|_| {});
    let request = Request::post("/xrpc/at.motet.pilot.play.submit%41dmission?other=value")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::empty())
        .expect("request");
    let response = router.oneshot(request).await.expect("router");
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert_eq!(directory.resolutions.load(Ordering::SeqCst), 1);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

/// An outage must not consume a token, and a forged token must not consume
/// another token's identifier. Mapping every resolver error to 401, or
/// admitting unverified claims to the handler, breaks this retry.
#[tokio::test]
async fn resolver_outage_and_forgery_cannot_consume_a_valid_token() {
    let f = Fixture::new();
    let directory = Arc::new(InterruptibleDirectory {
        directory: f.directory.clone(),
        unavailable: AtomicBool::new(true),
    });
    let calls = Arc::new(AtomicUsize::new(0));
    let authenticate = motet_xrpc::service_auth::authenticate(
        Arc::<InterruptibleDirectory>::clone(&directory),
        Arc::new(|aud| aud == APPVIEW),
    );
    let router = Service::new(authenticate)
        .method(
            Method {
                nsid: METHOD,
                kind: Kind::Procedure,
                access: Access::Service,
                max_body: 0,
            },
            |calls: Arc<AtomicUsize>, caller: Caller, _| async move {
                assert!(matches!(&caller, Caller::Service { jti: Some(jti), .. } if jti == "single-use"));
                calls.fetch_add(1, Ordering::SeqCst);
                Ok(StatusCode::NO_CONTENT.into_response())
            },
        )
        .into_router(Arc::clone(&calls))
        .expect("registered method");
    let token = f.token(|claims| claims.jose.json_web_token_id = Some("single-use".into()));
    let request = |token: &str| {
        Request::post(format!("/xrpc/{METHOD}"))
            .header("authorization", format!("Bearer {token}"))
            .body(Body::empty())
            .expect("request")
    };

    let response = router.clone().oneshot(request(&token)).await.expect("router");
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    let body = axum::body::to_bytes(response.into_body(), 4096)
        .await
        .expect("error body");
    let error: serde_json::Value = serde_json::from_slice(&body).expect("XRPC error");
    assert_eq!(error["error"], "UpstreamFailure");
    assert_eq!(calls.load(Ordering::SeqCst), 0);

    directory.unavailable.store(false, Ordering::SeqCst);
    let mallory = Fixture::new();
    let forged = mallory.token(|claims| claims.jose.json_web_token_id = Some("single-use".into()));
    let response = router.clone().oneshot(request(&forged)).await.expect("router");
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(calls.load(Ordering::SeqCst), 0);

    let response = router.oneshot(request(&token)).await.expect("router");
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

/// Both verification layers use the supplied clock and distinguish signed
/// lifetime refusals from forged signatures, even when wall time differs.
#[tokio::test]
async fn signed_time_claims_use_one_clock_and_report_lifetime_refusals() {
    let f = Fixture::new();
    let issued = 1_700_000_000;
    let token = f.token(|c| {
        c.jose.issued_at = Some(issued);
        c.jose.expiration = Some(issued + 60);
        c.jose.not_before = Some(issued);
    });
    assert!(
        verify(&token, &f.directory, |aud| aud == APPVIEW, METHOD, issued)
            .await
            .is_ok()
    );
    for time in [issued - 1, issued + 60] {
        assert_eq!(
            verify(&token, &f.directory, |aud| aud == APPVIEW, METHOD, time).await,
            Err(ServiceAuthError::BadLifetime),
        );
    }
    let future = f.token(|c| {
        c.jose.issued_at = Some(issued + 61);
        c.jose.expiration = Some(issued + 120);
    });
    assert_eq!(
        verify(&future, &f.directory, |aud| aud == APPVIEW, METHOD, issued).await,
        Err(ServiceAuthError::BadLifetime),
    );
}

#[tokio::test]
async fn a_key_that_isnt_the_atproto_key_doesnt_sign() {
    let mut f = Fixture::new();
    // Alice's document lists the same key, but under another fragment.
    f.directory
        .0
        .insert(ALICE.into(), document(ALICE, "atproto_label", &f.key));
    assert_eq!(f.verify(&f.token(|_| {})).await, Err(ServiceAuthError::NoSigningKey));
}

/// A document may mention another DID's key. Selecting it by the suffix
/// alone would let that DID sign requests as the document's subject.
#[tokio::test]
async fn foreign_atproto_keys_cannot_impersonate_the_document_subject() {
    let mut f = Fixture::new();
    let attacker = Fixture {
        key: generate_key(KeyType::K256Private).expect("attacker key"),
        directory: f.directory.clone(),
    };
    let foreign = document("did:plc:mallory", "atproto", &attacker.key)
        .verification_method
        .remove(0);
    f.directory
        .0
        .get_mut(ALICE)
        .expect("Alice document")
        .verification_method
        .insert(0, foreign);
    let forged = attacker.token(|claims| claims.jose.json_web_token_id = Some("forged".into()));
    let legitimate = f.token(|_| {});
    assert_eq!(f.verify(&forged).await, Err(ServiceAuthError::BadSignature));
    assert!(f.verify(&legitimate).await.is_ok());

    let alice_document = f.directory.0.get_mut(ALICE).expect("Alice document");
    let VerificationMethod::Multikey { id, .. } = &mut alice_document.verification_method[1] else {
        panic!("Alice's verification method is a Multikey");
    };
    *id = "#atproto".into();
    assert!(f.verify(&legitimate).await.is_ok());
    assert_eq!(f.verify(&forged).await, Err(ServiceAuthError::BadSignature));

    f.directory
        .0
        .get_mut(ALICE)
        .expect("Alice document")
        .verification_method
        .truncate(1);
    assert_eq!(f.verify(&forged).await, Err(ServiceAuthError::NoSigningKey));
}

/// A foreign controller cannot authorize an issuer's service tokens merely
/// by placing its public key under the issuer's own method ID.
#[tokio::test]
async fn atproto_keys_with_foreign_controllers_cannot_impersonate_the_subject() {
    for method_id in ["#atproto", "did:plc:alice#atproto"] {
        let mut f = Fixture::new();
        let attacker = Fixture {
            key: generate_key(KeyType::K256Private).expect("attacker key"),
            directory: f.directory.clone(),
        };
        let mut foreign = document(ALICE, "atproto", &attacker.key).verification_method.remove(0);
        let VerificationMethod::Multikey { id, controller, .. } = &mut foreign else {
            panic!("fixture verification method is a Multikey");
        };
        *id = method_id.into();
        *controller = "did:plc:mallory".into();
        f.directory
            .0
            .get_mut(ALICE)
            .expect("Alice document")
            .verification_method
            .insert(0, foreign);
        let forged = attacker.token(|_| {});
        let legitimate = f.token(|_| {});
        assert_eq!(f.verify(&forged).await, Err(ServiceAuthError::BadSignature));
        assert!(f.verify(&legitimate).await.is_ok());

        f.directory
            .0
            .get_mut(ALICE)
            .expect("Alice document")
            .verification_method
            .truncate(1);
        assert_eq!(f.verify(&forged).await, Err(ServiceAuthError::NoSigningKey));
    }
}

/// A resolver returning a different subject's document must not let that
/// subject's signing key authenticate the requested issuer.
#[tokio::test]
async fn mismatched_resolved_document_cannot_authenticate_an_issuer() {
    let mut f = Fixture::new();
    f.directory
        .0
        .insert(ALICE.into(), document("did:plc:mallory", "atproto", &f.key));
    assert_eq!(f.verify(&f.token(|_| {})).await, Err(ServiceAuthError::Unresolvable));
}

#[tokio::test]
async fn someone_elses_key_doesnt_sign_for_alice() {
    let f = Fixture::new();
    let mallory = generate_key(KeyType::K256Private).expect("key generation");
    let header = Header::try_from(mallory.clone()).expect("header");
    let unsigned = f.token(|_| {});
    let claims_part = unsigned.split('.').nth(1).expect("three parts");
    let claims: Claims = serde_json::from_slice(
        &base64::Engine::decode(&base64::engine::general_purpose::URL_SAFE_NO_PAD, claims_part).expect("base64"),
    )
    .expect("claims json");
    let forged = mint(&mallory, &header, &claims).expect("minting");
    assert_eq!(f.verify(&forged).await, Err(ServiceAuthError::BadSignature));
}

/// A forged identifier must not reach the replay ledger. The inexpensive
/// pre-checks must not make unverified claims authoritative.
#[tokio::test]
async fn changing_a_signed_identifier_does_not_authenticate() {
    let f = Fixture::new();
    let token = f.token(|claims| claims.jose.json_web_token_id = Some("original".into()));
    let mut parts = token.split('.');
    let header = parts.next().expect("header");
    let payload = parts.next().expect("payload");
    let signature = parts.next().expect("signature");
    let bytes =
        base64::Engine::decode(&base64::engine::general_purpose::URL_SAFE_NO_PAD, payload).expect("encoded claims");
    let mut claims: Claims = serde_json::from_slice(&bytes).expect("claims");
    claims.jose.json_web_token_id = Some("attacker".into());
    let payload = base64::Engine::encode(
        &base64::engine::general_purpose::URL_SAFE_NO_PAD,
        serde_json::to_vec(&claims).expect("claims JSON"),
    );
    let forged = format!("{header}.{payload}.{signature}");
    assert_eq!(f.verify(&forged).await, Err(ServiceAuthError::BadSignature));
}

fn timestamp() -> impl Strategy<Value = u64> {
    prop_oneof![4 => any::<u64>(), 1 => 0..=61_u64, 1 => (u64::MAX - 61)..=u64::MAX]
}

fn claims_with_times(issued_at: Option<u64>, expiration: Option<u64>, not_before: Option<u64>) -> Claims {
    let mut claims = Claims::new(JoseClaims {
        issuer: Some(ALICE.into()),
        audience: Some(APPVIEW.into()),
        issued_at,
        expiration,
        not_before,
        ..JoseClaims::default()
    });
    claims.private.insert("lxm".into(), METHOD.into());
    claims
}

/// The credential must expire within a minute of both issuance and use.
/// Widening the arithmetic keeps this predicate independent of overflow handling.
fn authorized_time_window(now: u64, issued_at: Option<u64>, expiration: Option<u64>, not_before: Option<u64>) -> bool {
    let (Some(issued_at), Some(expiration)) = (issued_at, expiration) else {
        return false;
    };
    let (now, issued_at, expiration) = (u128::from(now), u128::from(issued_at), u128::from(expiration));
    expiration > now
        && expiration > issued_at
        && expiration <= issued_at + 60
        && expiration <= now + 60
        && issued_at <= now + 60
        && not_before.is_none_or(|not_before| u128::from(not_before) <= now)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// No timestamp can extend a credential's authority beyond its signed
    /// minute. Wrapping subtraction or accepting absent time claims breaks this.
    #[test]
    fn full_domain_time_claims_obey_the_authorization_window(
        now in timestamp(),
        issued_at in prop::option::of(timestamp()),
        expiration in prop::option::of(timestamp()),
        not_before in prop::option::of(timestamp()),
    ) {
        let claims = claims_with_times(issued_at, expiration, not_before);
        let result = check_claims(&claims, |audience| audience == APPVIEW, now);
        if authorized_time_window(now, issued_at, expiration, not_before) {
            prop_assert!(result.is_ok(), "{claims:?}: {result:?}");
        } else {
            prop_assert_eq!(result, Err(ServiceAuthError::BadLifetime));
        }
    }

    /// Exact expiry, minute and nbf boundaries also apply near the clock's
    /// maximum. Off-by-one comparisons or overflowing additions break this.
    #[test]
    fn authorization_time_boundaries_do_not_wrap(now in timestamp()) {
        for now in [now, 0, 60, u64::MAX - 60, u64::MAX - 1, u64::MAX] {
            let boundaries = [
                now.checked_sub(61),
                now.checked_sub(60),
                now.checked_sub(1),
                Some(now),
                now.checked_add(1),
                now.checked_add(59),
                now.checked_add(60),
                now.checked_add(61),
            ];
            for issued_at in boundaries.into_iter().flatten() {
                let expirations = [
                    now.checked_sub(1),
                    Some(now),
                    now.checked_add(1),
                    now.checked_add(60),
                    now.checked_add(61),
                    issued_at.checked_sub(1),
                    Some(issued_at),
                    issued_at.checked_add(1),
                    issued_at.checked_add(60),
                    issued_at.checked_add(61),
                    Some(u64::MAX),
                ];
                for expiration in expirations.into_iter().flatten() {
                    for not_before in [None, now.checked_sub(1), Some(now), now.checked_add(1), Some(u64::MAX)] {
                        let claims = claims_with_times(Some(issued_at), Some(expiration), not_before);
                        let result = check_claims(&claims, |audience| audience == APPVIEW, now);
                        let authorized = authorized_time_window(now, Some(issued_at), Some(expiration), not_before);
                        prop_assert_eq!(
                            result.is_ok(),
                            authorized,
                            "now={}, issued_at={}, expiration={}, not_before={:?}",
                            now, issued_at, expiration, not_before,
                        );
                    }
                }
            }
        }
    }

    /// A usable credential retains the authority and replay identity supplied
    /// by verification. Normalizing fragments or dropping JTI/expiry breaks this.
    #[test]
    fn accepted_claims_preserve_authority_and_replay_identity(
        now in 0..=(u64::MAX - 60),
        age in 0..60_u64,
        remaining in 1..=60_u64,
        not_before in prop::option::of(timestamp()),
        issuer in prop::sample::select(vec![ALICE, "did:plc:alice#atproto_pds", "did:plc:alice#atproto_labeler"]),
        audience in prop::sample::select(vec![APPVIEW, "did:plc:world", "did:plc:world#motet_tenor"]),
        method in prop_oneof![Just(METHOD.to_owned()), "[a-z]{1,16}(\\.[a-z]{1,16}){1,4}"],
        jti in prop::option::of(any::<String>()),
    ) {
        let issued_at = now.saturating_sub(age);
        let expiration = now + remaining.min(60 - age);
        let mut claims = claims_with_times(Some(issued_at), Some(expiration), not_before.map(|time| time.min(now)));
        claims.jose.issuer = Some(issuer.into());
        claims.jose.audience = Some(audience.into());
        claims.jose.json_web_token_id = jti.clone();
        claims.private.insert("lxm".into(), method.clone().into());
        prop_assert_eq!(
            check_claims(&claims, |aud| [APPVIEW, "did:plc:world", "did:plc:world#motet_tenor"].contains(&aud), now),
            Ok(Caller::Service {
                did: issuer.into(),
                lxm: method,
                audience: audience.into(),
                jti,
                expires_at: expiration,
            }),
        );
    }

    /// Audience and method restrictions compose even for otherwise usable
    /// credentials. Truthy non-string methods or ignored audiences break this.
    #[test]
    fn audience_and_method_claims_cannot_bypass_each_other(
        now in 0..=(u64::MAX - 60),
        remaining in 1..=60_u64,
        method in "[a-z]{1,16}(\\.[a-z]{1,16}){1,4}",
    ) {
        for audience in [None, Some(""), Some("did:web:elsewhere.test"), Some(APPVIEW), Some("did:plc:world")] {
            for method_claim in [
                None,
                Some(serde_json::Value::Null),
                Some(serde_json::json!(true)),
                Some(serde_json::json!(60)),
                Some(serde_json::json!([METHOD])),
                Some(serde_json::json!({"lxm": METHOD})),
                Some(serde_json::json!("")),
                Some(serde_json::json!(METHOD)),
                Some(serde_json::json!(method)),
            ] {
                let mut claims = claims_with_times(Some(now), Some(now + remaining), Some(now));
                claims.jose.audience = audience.map(str::to_owned);
                let method = method_claim.as_ref().and_then(serde_json::Value::as_str);
                let expected_method = method.filter(|method| !method.is_empty()).map(str::to_owned);
                claims.private.remove("lxm");
                if let Some(value) = method_claim {
                    claims.private.insert("lxm".into(), value);
                }
                let result = check_claims(&claims, |aud| [APPVIEW, "did:plc:world"].contains(&aud), now);
                if !matches!(audience, Some(APPVIEW | "did:plc:world")) {
                    prop_assert_eq!(result, Err(ServiceAuthError::WrongAudience));
                } else if let Some(method) = expected_method {
                    prop_assert_eq!(
                        result,
                        Ok(Caller::Service {
                            did: ALICE.into(),
                            lxm: method,
                            audience: audience.expect("accepted audience").into(),
                            jti: None,
                            expires_at: now + remaining,
                        }),
                    );
                } else {
                    prop_assert_eq!(result, Err(ServiceAuthError::NoMethod));
                }
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// Changing any byte of a signed token never yields a different accepted
    /// caller: it either fails or (for bytes base64 ignores) still means the
    /// same thing.
    #[test]
    fn tampering_never_changes_who_is_calling(index in any::<prop::sample::Index>(), byte in any::<u8>()) {
        let runtime = tokio::runtime::Builder::new_current_thread().build().expect("runtime");
        let f = Fixture::new();
        let token = f.token(|_| {});
        let mut bytes = token.clone().into_bytes();
        let i = index.index(bytes.len());
        prop_assume!(bytes[i] != byte);
        bytes[i] = byte;
        let Ok(tampered) = String::from_utf8(bytes) else { return Ok(()) };
        let original = runtime.block_on(f.verify(&token));
        if let Ok(caller) = runtime.block_on(f.verify(&tampered)) {
            prop_assert_eq!(Ok(caller), original);
        }
    }
}
