//! Credentials follow the shard's current membership, not the access observed before queueing.
//! Real delegation/JWS and HTTP signatures exercise the authority boundary without external services.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use atproto_identity::key::{KeyData, KeyType, generate_key, to_public};
use atproto_identity::model::Document;
use atproto_identity::traits::IdentityResolver;
use atproto_record::datetime::Datetime;
use atproto_space::credential::{
    DELEGATION_TOKEN_TTL_SECS, MintedSpaceCredential, create_delegation_token, sign_compact_jws,
};
use atproto_space::http_signature::sign_credential_use;
use atproto_space::kem::encapsulate;
use atproto_space::mac::{MacType, WANT_MAC_HEADER, format_want_mac_header};
use atproto_space::space_repo::OpAction;
use atproto_space::types::SpaceUri;
use axum::http::{HeaderMap, HeaderName, HeaderValue};
use motet_lexicons::Record;
use motet_lexicons::group_opensocial_defs::SpaceGrant;
use motet_lexicons::group_opensocial_membership::Membership;
use motet_lexicons::group_opensocial_role::Role;
use motet_net::{Fetcher, Policy};
use motet_rules::Did;
use motet_tenor::keys::{MasterKey, Purpose};
use motet_tenor::shard::{Handle, Op, Options, Shard, Txn, spawn};
use motet_tenor::spaces::credentials::{
    self, CredentialError, MintRequest, PreparedMint, VerifiedCredential, prepare_mint, response_secret, verify_http,
};
use motet_tenor::spaces::{self, AccessRecord, MEMBERS, ProfileInput, SpaceError, SpaceWrite, world_space};
use object_store::{ObjectStore, memory::InMemory};
use serde_json::{Value, json};

const WORLD: &str = "did:plc:aaaaaaaaaaaaaaaaaaaaaaaa";
const OWNER: &str = "did:plc:oooooooooooooooooooooooo";
const ALICE: &str = "did:plc:aaaaaaaaaaaaaaaaaaaaaaab";
const BOB: &str = "did:plc:bbbbbbbbbbbbbbbbbbbbbbbb";

fn world() -> Did {
    Did::new(WORLD)
}

fn scene() -> SpaceUri {
    format!("at://{WORLD}/space/at.motet.pilot.space.scene/private")
        .parse()
        .expect("Space URI")
}

fn membership(member: &str) -> Membership {
    Membership {
        created_at: Datetime::now().into_string(),
        member: member.into(),
        roles: vec!["reader".into()],
        extra: serde_json::Map::new(),
    }
}

#[derive(Debug)]
struct Initialize;

impl Op for Initialize {
    type Output = ();
    type Error = SpaceError;

    async fn run(self, txn: &mut Txn<'_>) -> Result<(), SpaceError> {
        txn.begin_world(world(), 1).expect("new World's command stream");
        txn.generate_key(&world(), Purpose::Signing)
            .expect("generate Signing key");
        txn.generate_key(&world(), Purpose::Kem).expect("generate KEM key");
        motet_tenor::repo::genesis(txn, &world()).await?;
        spaces::bootstrap(txn, &world(), &Did::new(OWNER), ProfileInput::default()).await?;
        let role = Role {
            default: Some(false),
            description: None,
            display_name: "Reader".into(),
            extra: serde_json::Map::new(),
        };
        spaces::write_as_world(
            txn,
            &world_space(&world(), MEMBERS)?,
            vec![
                SpaceWrite::create(Role::NSID, "reader", &role)?,
                SpaceWrite::create(Membership::NSID, ALICE, &membership(ALICE))?,
                SpaceWrite::create(Membership::NSID, BOB, &membership(BOB))?,
            ],
        )
        .await?;
        spaces::create(
            txn,
            &scene(),
            AccessRecord {
                public: Some(false),
                grants: vec![SpaceGrant {
                    role: "reader".into(),
                    scopes: vec!["space:*?collection=*&action=read".into()],
                    extra: serde_json::Map::new(),
                }],
                extra: serde_json::Map::new(),
            },
            false,
        )
        .await?;
        Ok(())
    }
}

#[derive(Debug)]
struct Mutate {
    space: SpaceUri,
    writes: Vec<SpaceWrite>,
}

impl Op for Mutate {
    type Output = ();
    type Error = SpaceError;

    async fn run(self, txn: &mut Txn<'_>) -> Result<(), SpaceError> {
        spaces::write_as_world(txn, &self.space, self.writes).await?;
        Ok(())
    }
}

fn remove_alice() -> Mutate {
    Mutate {
        space: world_space(&world(), MEMBERS).expect("members Space"),
        writes: vec![SpaceWrite {
            action: OpAction::Delete,
            collection: Membership::NSID.into(),
            rkey: ALICE.into(),
            value: None,
            swap: None,
        }],
    }
}

#[derive(Debug)]
struct Recheck(VerifiedCredential);

impl Op for Recheck {
    type Output = ();
    type Error = CredentialError;

    async fn run(self, txn: &mut Txn<'_>) -> Result<(), CredentialError> {
        credentials::recheck(txn, &self.0).await
    }
}

#[derive(Debug)]
struct DeriveSecret {
    credential: VerifiedCredential,
    headers: HeaderMap,
    kind: MacType,
}

impl Op for DeriveSecret {
    type Output = atproto_space::kem::SharedSecret;
    type Error = CredentialError;

    async fn run(self, txn: &mut Txn<'_>) -> Result<Self::Output, CredentialError> {
        credentials::recheck(txn, &self.credential).await?;
        response_secret(txn, &self.headers, &self.credential, self.kind).await
    }
}

async fn derive_secret(
    fixture: &Fixture,
    headers: &HeaderMap,
    kind: MacType,
) -> Result<atproto_space::kem::SharedSecret, CredentialError> {
    let credential = verify_http(fixture.handle.reader(), headers, &scene(), &world()).await?;
    fixture
        .handle
        .run(DeriveSecret {
            credential,
            headers: headers.clone(),
            kind,
        })
        .await
        .expect("MAC operation")
}

#[derive(Debug)]
struct Documents(BTreeMap<String, Value>);

#[async_trait]
impl IdentityResolver for Documents {
    async fn resolve(&self, subject: &str) -> anyhow::Result<Document> {
        let document = self.0.get(subject).ok_or_else(|| anyhow::anyhow!("unknown test DID"))?;
        Ok(serde_json::from_value(document.clone())?)
    }
}

struct Fixture {
    store: Arc<dyn ObjectStore>,
    handle: Handle,
    alice: KeyData,
    bob: KeyData,
    holder: KeyData,
    resolver: Documents,
    fetcher: Fetcher,
}

impl Fixture {
    async fn new() -> Self {
        let store: Arc<dyn ObjectStore> = Arc::new(InMemory::new());
        let shard = Shard::open(
            Arc::clone(&store),
            "credentials",
            Arc::new(MasterKey::from_bytes(&[19; 32])),
        )
        .await
        .expect("open shard");
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
            .expect("durable setup")
            .expect("World creation");
        let alice = generate_key(KeyType::K256Private).expect("Alice key");
        let bob = generate_key(KeyType::P256Private).expect("Bob key");
        let holder = generate_key(KeyType::P256Private).expect("credential-use key");
        let documents = [(ALICE, &alice), (BOB, &bob)]
            .into_iter()
            .map(|(did, key)| {
                let public = to_public(key).expect("public key").to_string();
                let multibase = public.strip_prefix("did:key:").expect("public did:key");
                (
                    did.into(),
                    json!({
                        "id": did,
                        "verificationMethod": [{
                            "id": format!("{did}#atproto"),
                            "type": "Multikey",
                            "controller": did,
                            "publicKeyMultibase": multibase,
                        }],
                    }),
                )
            })
            .collect();
        Self {
            store,
            handle,
            alice,
            bob,
            holder,
            resolver: Documents(documents),
            fetcher: Fetcher::new(Policy::default()).expect("fetcher"),
        }
    }

    fn token(&self, member: &str) -> String {
        let key = match member {
            ALICE => &self.alice,
            BOB => &self.bob,
            _ => panic!("fixture member must be Alice or Bob"),
        };
        create_delegation_token(member, &scene(), key, DELEGATION_TOKEN_TTL_SECS).expect("delegation token")
    }

    fn request(&self, token: Option<String>) -> MintRequest {
        MintRequest {
            space: scene(),
            credential_kid: to_public(&self.holder).expect("holder public key").to_string(),
            delegation_token: token,
            client_attestation: None,
        }
    }

    async fn prepare(&self, token: String) -> PreparedMint {
        prepare_mint(
            self.handle.reader(),
            &self.resolver,
            &self.fetcher,
            self.request(Some(token)),
        )
        .await
        .expect("verified delegation")
    }

    async fn mint(&self, member: &str) -> MintedSpaceCredential {
        self.handle
            .run(self.prepare(self.token(member)).await)
            .await
            .expect("durable mint")
            .expect("authorized mint")
    }

    async fn reopen(&self) -> Shard {
        Shard::open(
            Arc::clone(&self.store),
            "credentials",
            Arc::new(MasterKey::from_bytes(&[19; 32])),
        )
        .await
        .expect("recover committed credentials")
    }
}

fn headers(token: &str, audience: &str, holder: &KeyData) -> HeaderMap {
    let signed = sign_credential_use(token, audience, holder).expect("sign credential use");
    signed
        .iter()
        .map(|(name, value)| {
            (
                HeaderName::from_static(name),
                HeaderValue::from_str(value).expect("signed header"),
            )
        })
        .collect()
}

/// Removing Alice revokes her exact JTIs and pending mint, while Bob continues reading.
/// Catches a shared Space-level credential cache and an access check made only before queueing.
#[tokio::test]
async fn alice_is_revoked_while_bob_remains_and_prepared_alice_mint_is_refused() {
    let fixture = Fixture::new().await;
    let alice = fixture.mint(ALICE).await;
    let bob = fixture.mint(BOB).await;
    let alice_headers = headers(&alice.token, WORLD, &fixture.holder);
    let bob_headers = headers(&bob.token, WORLD, &fixture.holder);
    let admitted_before_removal = verify_http(fixture.handle.reader(), &alice_headers, &scene(), &world())
        .await
        .expect("Alice starts authorized");
    let pending = fixture.prepare(fixture.token(ALICE)).await;

    fixture
        .handle
        .run(remove_alice())
        .await
        .expect("durable removal")
        .expect("remove member");

    assert!(matches!(
        fixture.handle.run(pending).await.expect("mint decided"),
        Err(CredentialError::UserNotAuthorized)
    ));
    assert!(matches!(
        fixture
            .handle
            .run(Recheck(admitted_before_removal))
            .await
            .expect("recheck decided"),
        Err(CredentialError::CredentialRevoked)
    ));
    assert!(matches!(
        verify_http(fixture.handle.reader(), &alice_headers, &scene(), &world()).await,
        Err(CredentialError::CredentialRevoked)
    ));
    verify_http(fixture.handle.reader(), &bob_headers, &scene(), &world())
        .await
        .expect("Bob remains authorized");
    assert!(
        credentials::issued(fixture.handle.reader(), &scene(), &alice.claims.jti)
            .await
            .expect("ledger")
            .expect("Alice issued")
            .revoked
    );
    assert!(
        !credentials::issued(fixture.handle.reader(), &scene(), &bob.claims.jti)
            .await
            .expect("ledger")
            .expect("Bob issued")
            .revoked
    );

    let replayed = fixture.reopen().await;
    assert!(matches!(
        verify_http(&replayed.reader(), &alice_headers, &scene(), &world()).await,
        Err(CredentialError::CredentialRevoked)
    ));
    verify_http(&replayed.reader(), &bob_headers, &scene(), &world())
        .await
        .expect("Bob survives replay");
}

/// Concurrent spends of one verified delegation mint at most once, including after restart.
/// Catches replay guards kept only in memory or checked outside the shard transaction.
#[tokio::test]
async fn delegation_replay_is_refused_in_the_writer_and_after_recovery() {
    let fixture = Fixture::new().await;
    let token = fixture.token(ALICE);
    let first = fixture.prepare(token.clone()).await;
    let second = fixture.prepare(token.clone()).await;
    let (first, second) = tokio::join!(fixture.handle.run(first), fixture.handle.run(second));
    let results = [first.expect("first decided"), second.expect("second decided")];
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|result| matches!(result, Err(CredentialError::InvalidDelegationToken(_))))
            .count(),
        1
    );

    let recovered = fixture.reopen().await;
    let recovered = spawn(
        recovered,
        Options {
            max_batch: 2,
            checkpoint_every: Duration::from_secs(3600),
        },
    );
    let prepared = prepare_mint(
        recovered.reader(),
        &fixture.resolver,
        &fixture.fetcher,
        fixture.request(Some(token)),
    )
    .await
    .expect("signature remains valid");
    assert!(matches!(
        recovered.run(prepared).await.expect("replay decided"),
        Err(CredentialError::InvalidDelegationToken(_))
    ));
}

/// Knowing a bound credential is insufficient: its request must prove possession at the named audience.
/// Catches accepting the request's key on trust, a bearer fallback, or a signature for another account.
#[tokio::test]
async fn credential_key_proof_is_checked_at_use_not_merely_at_mint() {
    let fixture = Fixture::new().await;
    let credential = fixture.mint(ALICE).await;
    let attacker = generate_key(KeyType::P256Private).expect("attacker key");
    let wrong_key = headers(&credential.token, WORLD, &attacker);
    assert!(matches!(
        verify_http(fixture.handle.reader(), &wrong_key, &scene(), &world()).await,
        Err(CredentialError::BadSpaceSignature(_))
    ));
    let other_audience = headers(&credential.token, BOB, &fixture.holder);
    assert!(matches!(
        verify_http(fixture.handle.reader(), &other_audience, &scene(), &world()).await,
        Err(CredentialError::BadSpaceAudience)
    ));
    let mut valid = headers(&credential.token, WORLD, &fixture.holder);
    verify_http(fixture.handle.reader(), &valid, &scene(), &world())
        .await
        .expect("correct holder and audience");
    valid.append(
        "authorization",
        HeaderValue::from_static("Atproto-Space-Credential other"),
    );
    assert!(matches!(
        verify_http(fixture.handle.reader(), &valid, &scene(), &world()).await,
        Err(CredentialError::BadSpaceSignature(_))
    ));
}

/// A resolved member key, not the unverified issuer string, authorizes a mint.
/// Catches decoding a delegation and trusting its iss without verifying the member's #atproto signature.
#[tokio::test]
async fn another_members_signature_cannot_claim_alices_membership() {
    let fixture = Fixture::new().await;
    let forged =
        create_delegation_token(ALICE, &scene(), &fixture.bob, DELEGATION_TOKEN_TTL_SECS).expect("forged issuer");
    assert!(matches!(
        prepare_mint(
            fixture.handle.reader(),
            &fixture.resolver,
            &fixture.fetcher,
            fixture.request(Some(forged))
        )
        .await,
        Err(CredentialError::InvalidDelegationToken(_))
    ));
}

/// Private Spaces stay concealed, and a World signature alone cannot assert a member.
/// Catches diagnostic leaks and unattributed delegation bypassing member removal.
#[tokio::test]
async fn private_spaces_are_concealed_and_unattributed_world_delegations_are_refused() {
    let fixture = Fixture::new().await;
    let private = prepare_mint(
        fixture.handle.reader(),
        &fixture.resolver,
        &fixture.fetcher,
        fixture.request(None),
    )
    .await
    .expect_err("private Space concealed");
    let mut absent_request = fixture.request(None);
    absent_request.space = format!("at://{WORLD}/space/at.motet.pilot.space.scene/absent")
        .parse()
        .expect("absent Space");
    let absent = prepare_mint(
        fixture.handle.reader(),
        &fixture.resolver,
        &fixture.fetcher,
        absent_request,
    )
    .await
    .expect_err("absent Space");
    assert_eq!(private.error_name(), absent.error_name());
    assert_eq!(private.to_string(), absent.to_string());

    let signing = fixture
        .handle
        .reader()
        .key(&world(), Purpose::Signing)
        .await
        .expect("World key read")
        .expect("World key");
    let token =
        create_delegation_token(WORLD, &scene(), &signing, DELEGATION_TOKEN_TTL_SECS).expect("World delegation");
    assert!(matches!(
        fixture
            .handle
            .run(fixture.prepare(token).await)
            .await
            .expect("durable mint"),
        Err(CredentialError::InvalidDelegationToken(_))
    ));
}

/// MAC-enabled reads derive from the served World's KEM key and require the right response type.
/// Catches silently omitting a requested MAC or treating the credential-holder key as the KEM key.
#[tokio::test]
async fn credential_reads_require_a_valid_kem_request_before_data_is_read() {
    let fixture = Fixture::new().await;
    let credential = fixture.mint(ALICE).await;
    let mut headers = headers(&credential.token, WORLD, &fixture.holder);
    assert!(matches!(
        derive_secret(&fixture, &headers, MacType::Record).await,
        Err(CredentialError::BadSpaceMac(_))
    ));
    let kem = fixture
        .handle
        .reader()
        .key(&world(), Purpose::Kem)
        .await
        .expect("KEM read")
        .expect("KEM exists");
    let encapsulated = encapsulate(&to_public(&kem).expect("public KEM key")).expect("encapsulation");
    headers.insert(
        WANT_MAC_HEADER,
        HeaderValue::from_str(&format_want_mac_header(MacType::Record, &encapsulated.key_share).expect("MAC request"))
            .expect("MAC header"),
    );
    let secret = derive_secret(&fixture, &headers, MacType::Record)
        .await
        .expect("decapsulation");
    assert_eq!(secret, encapsulated.shared_secret);
    assert!(matches!(
        derive_secret(&fixture, &headers, MacType::Commit).await,
        Err(CredentialError::BadSpaceMac(_))
    ));
}

/// A valid signature cannot make wrong-type, wrong-audience, future or unbounded assertions usable.
/// Catches checks limited to exp or to JWS validity before fetching attacker-selected metadata.
#[tokio::test]
async fn client_attestation_type_audience_time_and_nonce_are_checked_before_fetching() {
    let fixture = Fixture::new().await;
    let now = SystemTime::now().duration_since(UNIX_EPOCH).expect("clock").as_secs();
    let base = json!({
        "iss": "https://app.example/client.json",
        "sub": "https://app.example/client.json",
        "aud": format!("{WORLD}#atproto_pds"),
        "iat": now,
        "exp": now + 60,
        "jti": "attestation-nonce",
    });
    for (typ, field, value) in [
        ("JWT", "jti", json!("attestation-nonce")),
        ("atproto-client-attestation+jwt", "aud", json!(WORLD)),
        ("atproto-client-attestation+jwt", "iat", json!(now + 600)),
        ("atproto-client-attestation+jwt", "exp", json!(now + 3600)),
        ("atproto-client-attestation+jwt", "jti", json!("")),
    ] {
        let mut claims = base.clone();
        claims[field] = value;
        let token = sign_compact_jws(&json!({"typ":typ,"alg":"ES256","kid":"app"}), &claims, &fixture.holder)
            .expect("signed client assertion");
        let mut request = fixture.request(Some(fixture.token(ALICE)));
        request.client_attestation = Some(token);
        assert!(matches!(
            prepare_mint(fixture.handle.reader(), &fixture.resolver, &fixture.fetcher, request).await,
            Err(CredentialError::InvalidClientAttestation(_))
        ));
    }
}
