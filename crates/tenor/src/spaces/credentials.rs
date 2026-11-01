//! World-issued read credentials, single-use mint assertions, and per-JTI revocation.
//!
//! Adapted from atproto-pds `src/space/mint_authz.rs`, `src/http/space_auth.rs`,
//! `src/http/space_mac.rs`, `src/space/issued.rs`, and `src/space/revoke.rs` at 09a868cd (MIT).
//! Network verification precedes the shard operation. The operation rechecks
//! current access and records both replay identities and the issued credential.

use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};

use atproto_identity::key::{KeyData, KeyType, SignaturePolicy, identify_key, to_public, validate_with_policy};
use atproto_identity::traits::IdentityResolver;
use atproto_identity::validation::validate_service_endpoint;
use atproto_oauth::jwk::{WrappedJsonWebKey, WrappedJsonWebKeySet};
use atproto_space::credential::{
    CLOCK_SKEW_SECS, DelegationToken, MintedSpaceCredential, SPACE_CREDENTIAL_TTL_SECS, SpaceCredential,
    create_space_credential, peek_delegation_token, space_host_audience, validate_cnf_kid, verify_delegation_token,
    verify_space_credential,
};
use atproto_space::http_signature::{self, SignedRequest};
use atproto_space::kem::{SharedSecret, decapsulate};
use atproto_space::mac::{MacType, WANT_MAC_HEADER, parse_want_mac_header};
use atproto_space::types::SpaceUri;
use axum::http::HeaderMap;
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use motet_net::Fetcher;
use motet_rules::Did;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use thiserror::Error;

use super::{SpaceError, SpaceInfo, access, access_txn, decode, encode, info, info_txn, notify};
use crate::keys::Purpose;
use crate::shard::{Op, Txn};
use crate::store::{Reader, keys};

const ATTESTATION_TYP: &str = "atproto-client-attestation+jwt";
const MAX_ATTESTATION_LIFETIME_SECS: u64 = 300;

/// A protocol refusal is distinct from a storage failure.
#[derive(Debug, Error)]
pub enum CredentialError {
    #[error(transparent)]
    Space(#[from] SpaceError),
    #[error("Space not found")]
    SpaceNotFound,
    #[error("Space has been deleted")]
    SpaceDeleted,
    #[error("invalid credential key: {0}")]
    InvalidCredentialKey(String),
    #[error("invalid delegation token: {0}")]
    InvalidDelegationToken(String),
    #[error("invalid client attestation: {0}")]
    InvalidClientAttestation(String),
    #[error("could not verify client attestation: {0}")]
    AttestationUnavailable(String),
    #[error("the member cannot read this Space")]
    UserNotAuthorized,
    #[error("the attested application is not allowed")]
    AppNotAuthorized,
    #[error("invalid credential: {0}")]
    InvalidToken(String),
    #[error("credential revoked")]
    CredentialRevoked,
    #[error("invalid credential-use signature: {0}")]
    BadSpaceSignature(String),
    #[error("the signed audience is not the requested account")]
    BadSpaceAudience,
    #[error("unusable Space MAC request: {0}")]
    BadSpaceMac(String),
    #[error("the hosted account has no usable KEM key")]
    KemKeyUnavailable,
}

impl CredentialError {
    /// The XRPC error name. HTTP decides the status for its method.
    #[must_use]
    pub fn error_name(&self) -> &'static str {
        match self {
            Self::SpaceNotFound | Self::Space(SpaceError::NotFound) => "SpaceNotFound",
            Self::SpaceDeleted | Self::Space(SpaceError::Deleted) => "SpaceDeleted",
            Self::InvalidCredentialKey(_) | Self::Space(SpaceError::Invalid(_)) => "InvalidRequest",
            Self::InvalidDelegationToken(_) => "InvalidDelegationToken",
            Self::InvalidClientAttestation(_) => "InvalidClientAttestation",
            Self::AttestationUnavailable(_) | Self::Space(SpaceError::NotAuthorized) => "NotAuthorized",
            Self::UserNotAuthorized => "UserNotAuthorized",
            Self::AppNotAuthorized => "AppNotAuthorized",
            Self::InvalidToken(_) => "InvalidToken",
            Self::CredentialRevoked => "CredentialRevoked",
            Self::BadSpaceSignature(_) => "BadSpaceSignature",
            Self::BadSpaceAudience => "BadSpaceAudience",
            Self::BadSpaceMac(_) => "BadSpaceMac",
            Self::KemKeyUnavailable => "KemKeyUnavailable",
            Self::Space(SpaceError::Conflict) => "InvalidSwap",
            Self::Space(_) => "InternalError",
        }
    }

    /// Preserve infrastructure failures when a transactional caller needs the core error type.
    #[must_use]
    pub fn into_space_error(self) -> SpaceError {
        match self {
            Self::Space(error) => error,
            Self::SpaceNotFound => SpaceError::NotFound,
            Self::SpaceDeleted => SpaceError::Deleted,
            Self::InvalidCredentialKey(reason)
            | Self::InvalidDelegationToken(reason)
            | Self::InvalidClientAttestation(reason)
            | Self::BadSpaceMac(reason) => SpaceError::Invalid(reason),
            Self::KemKeyUnavailable => SpaceError::Invalid("hosted account has no usable KEM key".into()),
            Self::AttestationUnavailable(_)
            | Self::UserNotAuthorized
            | Self::AppNotAuthorized
            | Self::InvalidToken(_)
            | Self::CredentialRevoked
            | Self::BadSpaceSignature(_)
            | Self::BadSpaceAudience => SpaceError::NotAuthorized,
        }
    }
}

/// Untrusted input to `getSpaceCredential`. A DID in this input never authenticates a member.
pub struct MintRequest {
    pub space: SpaceUri,
    pub credential_kid: String,
    pub delegation_token: Option<String>,
    pub client_attestation: Option<String>,
}

impl std::fmt::Debug for MintRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MintRequest")
            .field("space", &self.space)
            .field("credential_kid", &self.credential_kid)
            .field("delegation_token", &self.delegation_token.is_some())
            .field("client_attestation", &self.client_attestation.is_some())
            .finish()
    }
}

/// Verified assertions, not a cached access decision. Only `prepare_mint` can construct one.
#[derive(Debug)]
pub struct PreparedMint {
    space: SpaceUri,
    generation: String,
    credential_kid: String,
    delegation: Option<DelegationToken>,
    attestation: Option<AttestationClaims>,
}

impl Op for PreparedMint {
    type Output = MintedSpaceCredential;
    type Error = CredentialError;

    async fn run(self, txn: &mut Txn<'_>) -> Result<Self::Output, Self::Error> {
        mint(txn, self).await
    }
}

/// Durable identity of one issued credential. Revocation never depends on another member's credential.
#[derive(Debug, Serialize, Deserialize)]
pub struct IssuedCredential {
    pub space: SpaceUri,
    pub generation: String,
    pub member: Option<String>,
    pub claims: SpaceCredential,
    pub revoked: bool,
}

/// A signature-checked credential bound to one Space and one served account.
/// Mutation callers must use `recheck` inside their operation.
#[derive(Debug)]
pub struct VerifiedCredential {
    space: SpaceUri,
    audience: Did,
    generation: String,
    claims: SpaceCredential,
}

impl VerifiedCredential {
    #[must_use]
    pub fn space(&self) -> &SpaceUri {
        &self.space
    }

    #[must_use]
    pub fn audience(&self) -> &Did {
        &self.audience
    }

    #[must_use]
    pub fn claims(&self) -> &SpaceCredential {
        &self.claims
    }
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn app_allowed(state: &SpaceInfo, client: Option<&str>) -> bool {
    state
        .allowed_clients
        .as_ref()
        .is_none_or(|allowed| client.is_some_and(|client| allowed.iter().any(|candidate| candidate == client)))
}

/// Resolve and verify member/app assertions outside the shard writer.
/// Access and single-use assertions are checked again atomically by `mint`.
pub async fn prepare_mint(
    reader: &Reader,
    resolver: &dyn IdentityResolver,
    fetcher: &Fetcher,
    request: MintRequest,
) -> Result<PreparedMint, CredentialError> {
    validate_cnf_kid(&request.credential_kid)
        .map_err(|error| CredentialError::InvalidCredentialKey(error.to_string()))?;
    let state = info(reader, &request.space).await?;
    if state.deleted {
        return Err(CredentialError::SpaceDeleted);
    }
    let token = request.delegation_token.as_deref().filter(|token| !token.is_empty());
    // Conceal a private Space before consulting its application policy or documents.
    if token.is_none() && !access(reader, &request.space, None).await?.read {
        return Err(CredentialError::SpaceNotFound);
    }
    let delegation = match token {
        Some(token) => {
            let unverified = peek_delegation_token(token)
                .map_err(|error| CredentialError::InvalidDelegationToken(error.to_string()))?;
            let key = member_key(reader, resolver, &unverified.iss).await?;
            Some(
                verify_delegation_token(token, &request.space.space_did, &request.space, &key)
                    .map_err(|error| CredentialError::InvalidDelegationToken(error.to_string()))?,
            )
        }
        None => None,
    };
    let attestation = match request.client_attestation.as_deref() {
        Some(token) => Some(verify_attestation(fetcher, token, &request.space).await?),
        None => None,
    };
    Ok(PreparedMint {
        space: request.space,
        generation: state.generation,
        credential_kid: request.credential_kid,
        delegation,
        attestation,
    })
}

async fn member_key(
    reader: &Reader,
    resolver: &dyn IdentityResolver,
    member: &str,
) -> Result<KeyData, CredentialError> {
    let invalid = |reason: String| CredentialError::InvalidDelegationToken(reason);
    atproto_lexicon::validation::syntax::validate_did(member).map_err(|error| invalid(error.to_string()))?;
    if let Some(key) = reader
        .key(&Did::new(member), Purpose::Signing)
        .await
        .map_err(SpaceError::from)?
    {
        return to_public(&key).map_err(|error| invalid(error.to_string()));
    }
    let document = resolver
        .resolve(member)
        .await
        .map_err(|error| invalid(error.to_string()))?;
    if document.id != member {
        return Err(invalid("resolved DID document belongs to another account".into()));
    }
    let multibase = document
        .verification_method_multibase("atproto")
        .ok_or_else(|| invalid("DID document has no #atproto Multikey".into()))?;
    let key = identify_key(multibase).map_err(|error| invalid(error.to_string()))?;
    if !matches!(key.key_type(), KeyType::K256Public | KeyType::P256Public) {
        return Err(invalid("#atproto must be a public ES256 or ES256K key".into()));
    }
    Ok(key)
}

/// Mint with the World's Signing key, only after current access and replay checks.
/// All effects disappear if this operation fails.
pub async fn mint(txn: &mut Txn<'_>, prepared: PreparedMint) -> Result<MintedSpaceCredential, CredentialError> {
    let state = info_txn(txn, &prepared.space).await?;
    if state.deleted {
        return Err(CredentialError::SpaceDeleted);
    }
    if state.generation != prepared.generation {
        return Err(SpaceError::Conflict.into());
    }
    let member = prepared.delegation.as_ref().map(|claims| claims.iss.as_str());
    let current = access_txn(txn, &prepared.space, member).await?;
    if member.is_none() && !current.read {
        return Err(CredentialError::SpaceNotFound);
    }
    let client = prepared.attestation.as_ref().map(|claims| claims.iss.as_str());
    if !app_allowed(&state, client) {
        return Err(CredentialError::AppNotAuthorized);
    }
    if !current.read {
        return Err(CredentialError::UserNotAuthorized);
    }
    let now = now_secs();
    if let Some(claims) = &prepared.delegation {
        if claims.exp.saturating_add(CLOCK_SKEW_SECS) <= now {
            return Err(CredentialError::InvalidDelegationToken(
                "token expired before mint".into(),
            ));
        }
        let replay_key = keys::space_delegation_replay(&claims.iss, &claims.jti);
        if txn.get(&replay_key).await.map_err(SpaceError::from)?.is_some() {
            return Err(CredentialError::InvalidDelegationToken("jti already used".into()));
        }
        txn.put(replay_key, encode(&claims.exp)?);
    }
    if let Some(claims) = &prepared.attestation {
        validate_attestation_claims(claims, &prepared.space, now)?;
        let replay_key = keys::space_attestation_replay(&claims.iss, &claims.jti);
        if txn.get(&replay_key).await.map_err(SpaceError::from)?.is_some() {
            return Err(CredentialError::InvalidClientAttestation(
                "issuer and jti already used".into(),
            ));
        }
        txn.put(replay_key, encode(&claims.exp)?);
    }
    let world = Did::new(&prepared.space.space_did);
    let key = txn
        .key(&world, Purpose::Signing)
        .await
        .map_err(SpaceError::from)?
        .ok_or(SpaceError::NotFound)?;
    let minted = create_space_credential(
        world.as_str(),
        &prepared.space,
        &prepared.credential_kid,
        client,
        &key,
        SPACE_CREDENTIAL_TTL_SECS,
    )
    .map_err(SpaceError::from)?;
    let ledger_key = keys::space_credential(&prepared.space.to_string(), &world, &minted.claims.jti);
    if txn.get(&ledger_key).await.map_err(SpaceError::from)?.is_some() {
        return Err(SpaceError::Conflict.into());
    }
    let issued = IssuedCredential {
        space: prepared.space,
        generation: state.generation,
        member: prepared.delegation.map(|claims| claims.iss),
        claims: minted.claims,
        revoked: false,
    };
    txn.put(ledger_key, encode(&issued)?);
    Ok(MintedSpaceCredential {
        token: minted.token,
        claims: issued.claims,
    })
}

/// Read the durable status of this exact JTI, not an aggregate Space credential cache.
pub async fn issued(reader: &Reader, space: &SpaceUri, jti: &str) -> Result<Option<IssuedCredential>, SpaceError> {
    let key = keys::space_credential(&space.to_string(), &Did::new(&space.space_did), jti);
    reader.get(&key).await?.map(|value| decode(&value)).transpose()
}

/// Revoke every credential that its World would no longer issue, including secret Spaces.
/// Call after membership, role, access, app-policy or deletion changes, inside the same operation.
pub async fn reconcile(txn: &mut Txn<'_>, world: &Did) -> Result<(), SpaceError> {
    let rows = txn.scan(&keys::space_credentials(world), usize::MAX).await?;
    let mut cached_space: Option<(SpaceUri, Option<SpaceInfo>)> = None;
    let mut members = BTreeMap::<Option<String>, bool>::new();
    let now = now_secs();
    for (key, value) in rows {
        let mut issued: IssuedCredential = decode(&value)?;
        if issued.revoked {
            continue;
        }
        if cached_space.as_ref().is_none_or(|(space, _)| space != &issued.space) {
            let state = match info_txn(txn, &issued.space).await {
                Ok(state) => Some(state),
                Err(SpaceError::NotFound | SpaceError::Deleted) => None,
                Err(error) => return Err(error),
            };
            cached_space = Some((issued.space.clone(), state));
            members.clear();
        }
        let state = cached_space.as_ref().and_then(|(_, state)| state.as_ref());
        let active = state.is_some_and(|state| !state.deleted && state.generation == issued.generation);
        let app_ok = state.is_some_and(|state| app_allowed(state, issued.claims.client_id.as_deref()));
        let read = if active && app_ok {
            if let Some(read) = members.get(&issued.member) {
                *read
            } else {
                let read = access_txn(txn, &issued.space, issued.member.as_deref()).await?.read;
                // Cache once per member: many app credentials share the same access decision.
                members.insert(issued.member.clone(), read);
                read
            }
        } else {
            false
        };
        if read {
            continue;
        }
        issued.revoked = true;
        txn.put(key.to_vec(), encode(&issued)?);
        if issued.claims.exp.saturating_add(CLOCK_SKEW_SECS) > now {
            let expires = i64::try_from(issued.claims.exp)
                .map_err(|_| SpaceError::Invalid("credential expiry is outside Unix seconds range".into()))?;
            notify::queue_revocation(txn, &issued.space, &issued.claims.jti, expires).await?;
        }
    }
    Ok(())
}

fn check_issued(issued: &IssuedCredential, state: &SpaceInfo, claims: &SpaceCredential) -> Result<(), CredentialError> {
    if issued.revoked || issued.generation != state.generation {
        return Err(CredentialError::CredentialRevoked);
    }
    if issued.claims != *claims || issued.space != state.space {
        return Err(CredentialError::InvalidToken(
            "claims do not match the issued JTI".into(),
        ));
    }
    if claims.exp.saturating_add(CLOCK_SKEW_SECS) <= now_secs() {
        return Err(CredentialError::InvalidToken("credential expired".into()));
    }
    if state.deleted {
        return Err(CredentialError::SpaceDeleted);
    }
    if !app_allowed(state, claims.client_id.as_deref()) {
        return Err(CredentialError::CredentialRevoked);
    }
    Ok(())
}

/// Recheck a previously authenticated request against the operation's overlay.
pub async fn recheck(txn: &Txn<'_>, verified: &VerifiedCredential) -> Result<(), CredentialError> {
    let state = info_txn(txn, &verified.space).await?;
    if state.generation != verified.generation {
        return Err(CredentialError::CredentialRevoked);
    }
    let key = keys::space_credential(
        &verified.space.to_string(),
        &Did::new(&verified.space.space_did),
        &verified.claims.jti,
    );
    let row = txn
        .get(&key)
        .await
        .map_err(SpaceError::from)?
        .ok_or_else(|| CredentialError::InvalidToken("credential was not issued here".into()))?;
    let issued: IssuedCredential = decode(&row)?;
    check_issued(&issued, &state, &verified.claims)?;
    if notify::is_revoked_txn(txn, &verified.space, &verified.claims.jti).await?
        || !access_txn(txn, &verified.space, issued.member.as_deref()).await?.read
    {
        return Err(CredentialError::CredentialRevoked);
    }
    Ok(())
}

fn single_header<'a>(headers: &'a HeaderMap, name: &str) -> Result<&'a str, CredentialError> {
    let mut values = headers.get_all(name).iter();
    let value = values
        .next()
        .ok_or_else(|| CredentialError::BadSpaceSignature(format!("missing {name}")))?;
    if values.next().is_some() {
        return Err(CredentialError::BadSpaceSignature(format!("duplicate {name}")));
    }
    value
        .to_str()
        .map_err(|_| CredentialError::BadSpaceSignature(format!("invalid {name}")))
}

fn combined_header(headers: &HeaderMap, name: &str) -> Result<String, CredentialError> {
    let mut combined = String::new();
    for value in headers.get_all(name) {
        let value = value
            .to_str()
            .map_err(|_| CredentialError::BadSpaceSignature(format!("invalid {name}")))?;
        if value.trim().is_empty() {
            continue;
        }
        if !combined.is_empty() {
            combined.push_str(", ");
        }
        combined.push_str(value);
    }
    Ok(combined)
}

fn credential_token(headers: &HeaderMap) -> Result<&str, CredentialError> {
    let authorization = single_header(headers, "authorization")?;
    let (scheme, token) = authorization
        .split_once(' ')
        .ok_or_else(|| CredentialError::InvalidToken("missing credential scheme".into()))?;
    if !scheme.eq_ignore_ascii_case(http_signature::AUTH_SCHEME)
        || token.is_empty()
        || token.contains(char::is_whitespace)
    {
        return Err(CredentialError::InvalidToken(
            "expected Atproto-Space-Credential".into(),
        ));
    }
    Ok(token)
}

/// Authenticate the credential scheme before dispatch. The handler must still bind its requested Space/audience.
pub async fn authenticate_http(reader: &Reader, headers: &HeaderMap) -> Result<VerifiedCredential, CredentialError> {
    let token = credential_token(headers)?;
    let (_, payload, _) =
        compact_parts(token).ok_or_else(|| CredentialError::InvalidToken("not a compact JWS".into()))?;
    let unverified: SpaceCredential = decode_part(payload).map_err(CredentialError::InvalidToken)?;
    let space = unverified
        .sub
        .parse::<SpaceUri>()
        .map_err(|error| CredentialError::InvalidToken(error.to_string()))?;
    let audience = single_header(headers, http_signature::AUDIENCE_HEADER)?;
    verify_http(reader, headers, &space, &Did::new(audience)).await
}

/// Verify a local World credential, the HTTP Message Signature and its requested audience.
/// No bearer fallback exists. Both the authority and audience must be hosted Worlds.
pub async fn verify_http(
    reader: &Reader,
    headers: &HeaderMap,
    space: &SpaceUri,
    audience: &Did,
) -> Result<VerifiedCredential, CredentialError> {
    let signed_audience = single_header(headers, http_signature::AUDIENCE_HEADER)?;
    if signed_audience != audience.as_str()
        || atproto_lexicon::validation::syntax::validate_did(signed_audience).is_err()
    {
        return Err(CredentialError::BadSpaceAudience);
    }
    let world = Did::new(&space.space_did);
    let signing = reader
        .key(&world, Purpose::Signing)
        .await
        .map_err(SpaceError::from)?
        .ok_or(CredentialError::SpaceNotFound)?;
    if audience != &world
        && reader
            .key(audience, Purpose::Signing)
            .await
            .map_err(SpaceError::from)?
            .is_none()
    {
        return Err(CredentialError::BadSpaceAudience);
    }
    let public = to_public(&signing).map_err(|error| CredentialError::InvalidToken(error.to_string()))?;
    let claims = verify_space_credential(credential_token(headers)?, world.as_str(), space, &public)
        .map_err(|error| CredentialError::InvalidToken(error.to_string()))?;
    let signature_input = combined_header(headers, http_signature::SIGNATURE_INPUT_HEADER)?;
    let signature = combined_header(headers, http_signature::SIGNATURE_HEADER)?;
    http_signature::verify_credential_use(
        &SignedRequest {
            authorization: single_header(headers, "authorization")?,
            audience: signed_audience,
            signature_input: &signature_input,
            signature: &signature,
        },
        &claims.cnf.kid,
    )
    .map_err(|error| CredentialError::BadSpaceSignature(error.to_string()))?;
    let state = info(reader, space).await?;
    let issued = issued(reader, space, &claims.jti)
        .await?
        .ok_or_else(|| CredentialError::InvalidToken("credential was not issued here".into()))?;
    check_issued(&issued, &state, &claims)?;
    if notify::is_revoked(reader, space, &claims.jti).await?
        || !access(reader, space, issued.member.as_deref()).await?.read
    {
        return Err(CredentialError::CredentialRevoked);
    }
    Ok(VerifiedCredential {
        space: space.clone(),
        audience: audience.clone(),
        generation: state.generation,
        claims,
    })
}

/// Derive the required response MAC secret after credential authentication, before reading data.
///
/// The KEM key belongs to the signed audience, never implicitly to the Space authority.
/// Callers MAC the response and set `Vary: Atproto-Space-Want-Mac`, including on errors.
pub async fn response_secret(
    txn: &Txn<'_>,
    headers: &HeaderMap,
    credential: &VerifiedCredential,
    mac_type: MacType,
) -> Result<SharedSecret, CredentialError> {
    let key = txn
        .key(credential.audience(), Purpose::Kem)
        .await
        .map_err(SpaceError::from)?
        .ok_or(CredentialError::KemKeyUnavailable)?;
    if *key.key_type() != KeyType::P256Private {
        return Err(CredentialError::KemKeyUnavailable);
    }
    let header =
        combined_header(headers, WANT_MAC_HEADER).map_err(|error| CredentialError::BadSpaceMac(error.to_string()))?;
    let share = parse_want_mac_header(Some(&header), mac_type)
        .map_err(|error| CredentialError::BadSpaceMac(error.to_string()))?;
    decapsulate(&key, &share).map_err(|error| CredentialError::BadSpaceMac(error.to_string()))
}

#[derive(Deserialize)]
struct AttestationHeader {
    typ: String,
    alg: String,
    kid: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AttestationClaims {
    iss: String,
    sub: String,
    aud: String,
    iat: u64,
    exp: u64,
    jti: String,
}

#[derive(Deserialize)]
struct ClientMetadata {
    jwks: Option<WrappedJsonWebKeySet>,
    jwks_uri: Option<String>,
}

fn compact_parts(token: &str) -> Option<(&str, &str, &str)> {
    let (input, signature) = token.rsplit_once('.')?;
    let (header, payload) = input.split_once('.')?;
    (!header.is_empty() && !payload.is_empty() && !signature.is_empty() && !payload.contains('.'))
        .then_some((header, payload, signature))
}

fn decode_part<T: DeserializeOwned>(encoded: &str) -> Result<T, String> {
    let bytes = URL_SAFE_NO_PAD.decode(encoded).map_err(|error| error.to_string())?;
    serde_json::from_slice(&bytes).map_err(|error| error.to_string())
}

fn validate_attestation_claims(claims: &AttestationClaims, space: &SpaceUri, now: u64) -> Result<(), CredentialError> {
    let invalid = |reason: &str| CredentialError::InvalidClientAttestation(reason.into());
    if claims.iss != claims.sub {
        return Err(invalid("iss does not equal sub"));
    }
    if claims.aud != space_host_audience(&space.space_did) {
        return Err(invalid("aud must name the World's #atproto_pds service"));
    }
    if claims.iat > now.saturating_add(CLOCK_SKEW_SECS) {
        return Err(invalid("iat is in the future"));
    }
    if claims.exp <= now || claims.exp <= claims.iat || claims.exp - claims.iat > MAX_ATTESTATION_LIFETIME_SECS {
        return Err(invalid("attestation expired or exceeds the five-minute lifetime"));
    }
    if claims.jti.is_empty() {
        return Err(invalid("jti is empty"));
    }
    Ok(())
}

async fn client_document<T: DeserializeOwned>(fetcher: &Fetcher, url: &str) -> Result<T, CredentialError> {
    validate_service_endpoint(url)
        .map_err(|error| CredentialError::InvalidClientAttestation(format!("unusable client document URL: {error}")))?;
    let response = fetcher
        .get(url)
        .await
        .map_err(|error| CredentialError::AttestationUnavailable(error.to_string()))?;
    if !response.status.is_success() {
        let reason = format!("client document answered {}", response.status);
        return Err(
            if response.status.is_server_error() || response.status.as_u16() == 429 {
                CredentialError::AttestationUnavailable(reason)
            } else {
                CredentialError::InvalidClientAttestation(reason)
            },
        );
    }
    serde_json::from_slice(&response.body).map_err(|error| CredentialError::InvalidClientAttestation(error.to_string()))
}

fn attestation_key(header: &AttestationHeader, jwks: &WrappedJsonWebKeySet) -> Result<KeyData, CredentialError> {
    let invalid = |reason: &str| CredentialError::InvalidClientAttestation(reason.into());
    let key: &WrappedJsonWebKey = if let Some(kid) = header.kid.as_deref() {
        let mut matches = jwks.keys.iter().filter(|key| key.kid.as_deref() == Some(kid));
        let key = matches.next().ok_or_else(|| invalid("no JWK matches kid"))?;
        if matches.next().is_some() {
            return Err(invalid("JWKS contains duplicate kid"));
        }
        key
    } else {
        match jwks.keys.as_slice() {
            [key] => key,
            _ => return Err(invalid("an attestation without kid requires exactly one JWK")),
        }
    };
    if key.alg.as_deref().is_some_and(|alg| alg != header.alg)
        || key._use.as_deref().is_some_and(|usage| usage != "sig")
    {
        return Err(invalid("JWK is not for this signature algorithm"));
    }
    let public =
        KeyData::try_from(key).map_err(|error| CredentialError::InvalidClientAttestation(error.to_string()))?;
    let algorithm = match public.key_type() {
        KeyType::P256Public => "ES256",
        KeyType::K256Public => "ES256K",
        _ => return Err(invalid("JWKS must contain a public ES256 or ES256K key")),
    };
    if header.alg != algorithm {
        return Err(invalid("JWT alg does not match the verifying key"));
    }
    Ok(public)
}

fn verify_attestation_signature(
    token: &str,
    signature: &str,
    header: &AttestationHeader,
    jwks: &WrappedJsonWebKeySet,
) -> Result<(), CredentialError> {
    let invalid = |reason: String| CredentialError::InvalidClientAttestation(reason);
    let key = attestation_key(header, jwks)?;
    let signature = URL_SAFE_NO_PAD
        .decode(signature)
        .map_err(|error| invalid(error.to_string()))?;
    let (signing_input, _) = token
        .rsplit_once('.')
        .ok_or_else(|| invalid("not a compact JWS".into()))?;
    validate_with_policy(&key, &signature, signing_input.as_bytes(), SignaturePolicy::AnyS)
        .map_err(|error| invalid(error.to_string()))
}

async fn verify_attestation(
    fetcher: &Fetcher,
    token: &str,
    space: &SpaceUri,
) -> Result<AttestationClaims, CredentialError> {
    let invalid = |reason: String| CredentialError::InvalidClientAttestation(reason);
    let (header, payload, signature) = compact_parts(token).ok_or_else(|| invalid("not a compact JWS".into()))?;
    let header: AttestationHeader = decode_part(header).map_err(invalid)?;
    if header.typ != ATTESTATION_TYP {
        return Err(invalid("typ must be atproto-client-attestation+jwt".into()));
    }
    let claims: AttestationClaims = decode_part(payload).map_err(invalid)?;
    validate_attestation_claims(&claims, space, now_secs())?;
    let metadata: ClientMetadata = client_document(fetcher, &claims.iss).await?;
    let jwks = match (metadata.jwks, metadata.jwks_uri) {
        (Some(jwks), _) => jwks,
        (None, Some(uri)) => client_document(fetcher, &uri).await?,
        (None, None) => return Err(invalid("client metadata has neither jwks nor jwks_uri".into())),
    };
    verify_attestation_signature(token, signature, &header, &jwks)?;
    Ok(claims)
}

#[cfg(test)]
#[path = "credentials_tests.rs"]
mod tests;
