//! App assertions use real signatures; only fetching their already-known JWKS is outside these tests.

use std::sync::Arc;
use std::time::Duration;

use atproto_identity::key::{KeyType, generate_key};
use atproto_oauth::jwk::generate;
use atproto_space::credential::sign_compact_jws;
use object_store::{ObjectStore, memory::InMemory};
use serde_json::json;

use super::*;
use crate::keys::MasterKey;
use crate::shard::{Handle, Options, Shard, spawn};
use crate::spaces::{META, ProfileInput, bootstrap, world_space};

const WORLD: &str = "did:plc:cccccccccccccccccccccccc";
const FIRST_APP: &str = "https://first.example/client.json";
const SECOND_APP: &str = "https://second.example/client.json";

#[derive(Debug)]
struct PublicWorld;

impl Op for PublicWorld {
    type Output = ();
    type Error = SpaceError;

    async fn run(self, txn: &mut Txn<'_>) -> Result<(), SpaceError> {
        let world = Did::new(WORLD);
        txn.begin_world(world.clone(), 1).expect("new World's command stream");
        txn.generate_key(&world, Purpose::Signing).expect("World Signing key");
        txn.generate_key(&world, Purpose::Kem).expect("World KEM key");
        crate::repo::genesis(txn, &world).await?;
        bootstrap(
            txn,
            &world,
            &Did::new("did:plc:oooooooooooooooooooooooo"),
            ProfileInput {
                public: true,
                ..ProfileInput::default()
            },
        )
        .await?;
        let space = world_space(&world, META)?;
        let mut state = info_txn(txn, &space).await?;
        state.allowed_clients = Some(vec![FIRST_APP.into(), SECOND_APP.into()]);
        txn.put(keys::space_info(&space), encode(&state)?);
        Ok(())
    }
}

async fn public_world() -> Handle {
    let store: Arc<dyn ObjectStore> = Arc::new(InMemory::new());
    let shard = Shard::open(store, "attestations", Arc::new(MasterKey::from_bytes(&[23; 32])))
        .await
        .expect("open shard");
    let handle = spawn(
        shard,
        Options {
            max_batch: 4,
            checkpoint_every: Duration::from_secs(3600),
        },
    );
    handle
        .run(PublicWorld)
        .await
        .expect("durable World")
        .expect("public World");
    handle
}

fn app_jwks(key: &KeyData) -> WrappedJsonWebKeySet {
    let mut wrapped = generate(key).expect("public app JWK");
    wrapped.kid = Some("app-signing".into());
    WrappedJsonWebKeySet { keys: vec![wrapped] }
}

fn signed_app(key: &KeyData, issuer: &str, jti: &str, algorithm: &str) -> String {
    let now = now_secs();
    sign_compact_jws(
        &json!({"typ":ATTESTATION_TYP,"alg":algorithm,"kid":"app-signing"}),
        &json!({
            "iss": issuer,
            "sub": issuer,
            "aud": space_host_audience(WORLD),
            "iat": now,
            "exp": now + 60,
            "jti": jti,
        }),
        key,
    )
    .expect("signed app attestation")
}

fn verify_app(
    token: &str,
    jwks: &WrappedJsonWebKeySet,
    space: &SpaceUri,
) -> Result<AttestationClaims, CredentialError> {
    let (header, payload, signature) = compact_parts(token).expect("test compact JWT");
    let header: AttestationHeader = decode_part(header).expect("header");
    let claims: AttestationClaims = decode_part(payload).expect("claims");
    validate_attestation_claims(&claims, space, now_secs())?;
    verify_attestation_signature(token, signature, &header, jwks)?;
    Ok(claims)
}

async fn app_request(handle: &Handle, key: &KeyData, issuer: &str, jti: &str) -> PreparedMint {
    let space = world_space(&Did::new(WORLD), META).expect("meta Space");
    let token = signed_app(key, issuer, jti, "ES256");
    let attestation = verify_app(&token, &app_jwks(key), &space).expect("verified app signature");
    PreparedMint {
        generation: info(handle.reader(), &space).await.expect("Space info").generation,
        space,
        credential_kid: to_public(key).expect("holder public key").to_string(),
        delegation: None,
        attestation: Some(attestation),
    }
}

/// A JWKS verifies only its actual app key and declared algorithm, not merely a matching kid.
/// Catches removing signature verification or trusting the unverified alg independently of the key.
#[test]
fn an_app_signature_cannot_select_another_key_or_algorithm() {
    let key = generate_key(KeyType::P256Private).expect("app key");
    let impostor = generate_key(KeyType::P256Private).expect("other app key");
    let space = world_space(&Did::new(WORLD), META).expect("Space");
    let token = signed_app(&key, FIRST_APP, "nonce", "ES256");
    verify_app(&token, &app_jwks(&key), &space).expect("real signature verifies");
    assert!(matches!(
        verify_app(&token, &app_jwks(&impostor), &space),
        Err(CredentialError::InvalidClientAttestation(_))
    ));
    let confused = signed_app(&key, FIRST_APP, "nonce", "ES256K");
    assert!(matches!(
        verify_app(&confused, &app_jwks(&key), &space),
        Err(CredentialError::InvalidClientAttestation(_))
    ));
}

/// An app's assertion is spent once; another issuer's same nonce does not collide.
/// Catches a non-atomic replay guard, a guard keyed only by jti, or ignoring the app allow-list.
#[tokio::test]
async fn app_replay_is_atomic_and_scoped_to_its_verified_issuer() {
    let handle = public_world().await;
    let first_key = generate_key(KeyType::P256Private).expect("first app key");
    let second_key = generate_key(KeyType::P256Private).expect("second app key");
    let first = app_request(&handle, &first_key, FIRST_APP, "same-nonce").await;
    let replay = app_request(&handle, &first_key, FIRST_APP, "same-nonce").await;
    let second = app_request(&handle, &second_key, SECOND_APP, "same-nonce").await;
    let (first, replay) = tokio::join!(handle.run(first), handle.run(replay));
    let results = [first.expect("first decided"), replay.expect("replay decided")];
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|result| matches!(result, Err(CredentialError::InvalidClientAttestation(_))))
            .count(),
        1
    );
    handle
        .run(second)
        .await
        .expect("second app decided")
        .expect("issuer-scoped nonce");
    let refused = app_request(
        &handle,
        &second_key,
        "https://unlisted.example/client.json",
        "fresh-nonce",
    )
    .await;
    assert!(matches!(
        handle.run(refused).await.expect("app access decided"),
        Err(CredentialError::AppNotAuthorized)
    ));
}
