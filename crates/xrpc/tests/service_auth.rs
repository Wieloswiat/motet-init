//! Service auth against real signatures: tokens minted with generated keys,
//! checked against DID documents served by an in-memory resolver.

use std::collections::{BTreeMap, HashMap};
use std::time::{SystemTime, UNIX_EPOCH};

use atproto_identity::key::{KeyData, KeyType, generate_key, to_public};
use atproto_identity::model::{Document, VerificationMethod};
use atproto_identity::traits::IdentityResolver;
use atproto_oauth::jwt::{Claims, Header, JoseClaims, mint};
use motet_xrpc::Caller;
use motet_xrpc::service_auth::{MAX_LIFETIME_SECS, ServiceAuthError, verify};
use proptest::prelude::*;

const ALICE: &str = "did:plc:alice";
const APPVIEW: &str = "did:web:api.motet.test#motet_appview";
const METHOD: &str = "at.motet.pilot.play.submitAdmission";

struct Directory(BTreeMap<String, Document>);

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
        verify(token, &self.directory, |aud| aud == APPVIEW, now()).await
    }
}

#[tokio::test]
async fn a_well_formed_token_names_its_issuer_and_method() {
    let f = Fixture::new();
    assert_eq!(
        f.verify(&f.token(|_| {})).await,
        Ok(Caller::Service {
            did: ALICE.into(),
            lxm: METHOD.into(),
            audience: APPVIEW.into(),
        })
    );
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

#[tokio::test]
async fn a_key_that_isnt_the_atproto_key_doesnt_sign() {
    let mut f = Fixture::new();
    // Alice's document lists the same key, but under another fragment.
    f.directory
        .0
        .insert(ALICE.into(), document(ALICE, "atproto_label", &f.key));
    assert_eq!(f.verify(&f.token(|_| {})).await, Err(ServiceAuthError::NoSigningKey));
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
