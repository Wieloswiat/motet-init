//! Owner-signed World identities.
//!
//! The writer reserves keys and records intent; PLC requests run outside it.
//! An accepted creation intent keeps its keys until it becomes a World,
//! even if the client disappears during submission.

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use atproto_identity::errors::PLCDIDError;
use atproto_identity::key::{KeyType, identify_key};
use atproto_identity::model::Document;
use atproto_identity::plc::{self, Operation, PlcState, UnsignedOperation};
use atproto_identity::validation::is_valid_handle;
use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use bytes::Bytes;
use motet_net::Fetcher;
use motet_rules::Did;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use url::Url;

use crate::authorization::ServiceToken;
use crate::keys::{KeyError, Purpose};
use crate::shard::{Handle, Op, SubmitError, Txn, TxnError};
use crate::store::{ReadError, Reader, keys};
use crate::world::{self, WorldInfo};

const RESERVATION_SECONDS: u64 = 24 * 60 * 60;

#[derive(Debug, Error)]
pub enum IdentityError {
    #[error("invalid tenor public origin or handle domain")]
    Configuration,
    #[error("handle is invalid or outside the tenor's configured domains")]
    InvalidHandle,
    #[error("handle is already in use or reserved")]
    HandleTaken,
    #[error("World profile is invalid: {0}")]
    InvalidProfile(String),
    #[error("rotationKey must be a public P-256 or K-256 did:key")]
    InvalidRotationKey,
    #[error("reservation does not exist")]
    ReservationNotFound,
    #[error("reservation has expired")]
    ReservationExpired,
    #[error("only the reservation's owner may create its World")]
    NotOwner,
    #[error("signed operation differs from the reservation or lacks the owner's signature")]
    InvalidOperation,
    #[error("handle does not name an active World with a confirming DID document")]
    HandleNotFound,
    #[error("PLC document does not match the reserved genesis")]
    DocumentMismatch,
    #[error("identity state is inconsistent")]
    Inconsistent,
    #[error("the system clock is outside the supported range")]
    Clock,
    #[error(transparent)]
    Key(#[from] KeyError),
    #[error(transparent)]
    Plc(#[from] PLCDIDError),
    #[error(transparent)]
    Txn(#[from] TxnError),
    #[error(transparent)]
    Read(#[from] ReadError),
    #[error(transparent)]
    Submit(#[from] SubmitError),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Repo(#[from] crate::repo::RepoError),
    #[error(transparent)]
    Space(#[from] crate::spaces::SpaceError),
    #[error("PLC operation could not be encoded: {0}")]
    Encoding(String),
}

/// Identity configuration shared by HTTP handlers and the reservation writer.
#[derive(Debug)]
pub struct Hosting {
    endpoint: String,
    service_did: String,
    audience: String,
    domains: Vec<String>,
    plc_directory: String,
    fetcher: Fetcher,
    relay_urls: Vec<Url>,
    hostname: String,
}

impl Hosting {
    pub fn new(
        public_url: &Url,
        domains: Vec<String>,
        plc_directory: String,
        fetcher: Fetcher,
        relay_urls: Vec<Url>,
    ) -> Result<Self, IdentityError> {
        if !matches!(public_url.scheme(), "https" | "http")
            || public_url.host_str().is_none()
            || public_url.path() != "/"
            || !public_url.username().is_empty()
            || public_url.password().is_some()
            || public_url.query().is_some()
            || public_url.fragment().is_some()
        {
            return Err(IdentityError::Configuration);
        }
        let domains = domains
            .into_iter()
            .map(|domain| {
                if !domain.starts_with('.') || is_valid_handle(&format!("world{domain}")).is_none() {
                    return Err(IdentityError::Configuration);
                }
                Ok(domain.to_ascii_lowercase())
            })
            .collect::<Result<Vec<_>, _>>()?;
        let hostname = public_url[url::Position::BeforeHost..url::Position::AfterPort].to_owned();
        let service_did = format!("did:web:{}", hostname.replace(':', "%3A"));
        Ok(Self {
            endpoint: public_url.as_str().trim_end_matches('/').to_owned(),
            audience: format!("{service_did}#{}", crate::api::SERVICE_FRAGMENT),
            service_did,
            domains,
            plc_directory,
            fetcher,
            relay_urls,
            hostname,
        })
    }

    #[must_use]
    pub fn service_did(&self) -> &str {
        &self.service_did
    }

    #[must_use]
    pub fn audience(&self) -> &str {
        &self.audience
    }

    #[must_use]
    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    #[must_use]
    pub fn domains(&self) -> &[String] {
        &self.domains
    }

    /// The shared outbound client, including the tenor's network policy.
    #[must_use]
    pub fn fetcher(&self) -> &Fetcher {
        &self.fetcher
    }

    #[must_use]
    pub fn plc_directory(&self) -> &str {
        &self.plc_directory
    }

    fn handle(&self, handle: &str) -> Result<String, IdentityError> {
        let handle = normalize_handle(handle)?;
        if self.domains.iter().any(|domain| handle.ends_with(domain)) {
            Ok(handle)
        } else {
            Err(IdentityError::InvalidHandle)
        }
    }

    /// Relay failure cannot undo a durable World. Startup announces again.
    pub async fn announce(&self) {
        if let Err(error) = crate::crawl::announce(&self.fetcher, &self.relay_urls, &self.hostname).await {
            tracing::warn!(%error, "tenor crawl announcement failed");
        }
    }
}

/// Handle fields are bare DNS names, not at:// URIs or @-prefixed input.
fn normalize_handle(handle: &str) -> Result<String, IdentityError> {
    if handle.starts_with('@') || handle.contains('/') {
        return Err(IdentityError::InvalidHandle);
    }
    is_valid_handle(handle).ok_or(IdentityError::InvalidHandle)
}

/// The initial audience of a World's meta Space.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Visibility {
    Public,
    #[default]
    Private,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReserveRequest {
    pub handle: String,
    pub rotation_key: String,
    pub tenor_rotation_key: bool,
    #[serde(default)]
    pub visibility: Visibility,
    pub name: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Reserved {
    pub reservation: String,
    pub operation: UnsignedOperation,
    /// Base64url, without padding, of the canonical DAG-CBOR signing bytes.
    pub signing_bytes: String,
    /// Unix seconds. Creation must begin before this instant.
    pub expires_at: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateRequest {
    pub reservation: String,
    pub signed_operation: Operation,
}

/// Drafts expire; submitting intents and completed receipts do not. Keeping
/// the exact signed operation prevents a retry from deriving a second DID.
#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct Reservation {
    pub(crate) owner: Did,
    pub(crate) handle: String,
    #[serde(default)]
    pub(crate) visibility: Visibility,
    #[serde(default)]
    pub(crate) name: Option<String>,
    pub(crate) operation: UnsignedOperation,
    pub(crate) expires: u64,
    pub(crate) signed: Option<Operation>,
    pub(crate) created: bool,
}

/// A reservation and its verified token share one shard transaction.
pub(crate) struct AuthorizedReserve {
    pub token: ServiceToken,
    pub request: ReserveRequest,
    pub hosting: Arc<Hosting>,
    pub now: SystemTime,
}

impl Op for AuthorizedReserve {
    type Output = Reserved;
    type Error = IdentityError;

    async fn run(self, txn: &mut Txn<'_>) -> Result<Reserved, IdentityError> {
        self.token.consume(txn).await?;
        ReserveWorld {
            owner: self.token.issuer,
            request: self.request,
            hosting: self.hosting,
            now: self.now,
        }
        .run(txn)
        .await
    }
}

/// Reserves a handle and sealed keys on the one writer. The caller supplies
/// its clock instant so expiry is the same decision for the whole operation.
#[derive(Debug)]
pub struct ReserveWorld {
    pub owner: Did,
    pub request: ReserveRequest,
    pub hosting: Arc<Hosting>,
    pub now: SystemTime,
}

impl Op for ReserveWorld {
    type Output = Reserved;
    type Error = IdentityError;

    async fn run(self, txn: &mut Txn<'_>) -> Result<Reserved, IdentityError> {
        let now = unix_seconds(self.now)?;
        let handle = self.hosting.handle(&self.request.handle)?;
        let rotation = identify_key(&self.request.rotation_key).map_err(|_| IdentityError::InvalidRotationKey)?;
        if !self.request.rotation_key.starts_with("did:key:")
            || !matches!(rotation.key_type(), KeyType::P256Public | KeyType::K256Public)
        {
            return Err(IdentityError::InvalidRotationKey);
        }
        // An invalid profile must not strand a DID after PLC accepts genesis.
        let profile = serde_json::json!({
            "$type": "group.opensocial.profile",
            "displayName": self.request.name.as_deref().unwrap_or(&handle),
            "joinPolicy": "invite",
        });
        let profile = crate::repo::record_from_json(&profile)?;
        crate::repo::validate_record("group.opensocial.profile", "self", &profile).map_err(|error| match error {
            crate::repo::RepoError::InvalidRecord(message) => IdentityError::InvalidProfile(message),
            other => other.into(),
        })?;
        if txn.get(&keys::handle(&handle)).await?.is_some() {
            return Err(IdentityError::HandleTaken);
        }
        if let Some(id) = txn.get(&keys::reserved_handle(&handle)).await? {
            let id = std::str::from_utf8(&id).map_err(|_| IdentityError::Inconsistent)?;
            let reserved = load(txn, id).await?;
            if reserved.signed.is_some() || reserved.expires > now {
                return Err(IdentityError::HandleTaken);
            }
            remove_draft(txn, id, &reserved);
        }
        let mut random = [0; 32];
        getrandom::fill(&mut random).map_err(|_| KeyError::Random)?;
        let id = URL_SAFE_NO_PAD.encode(random);
        if txn.get(&keys::reservation(&id)).await?.is_some() {
            return Err(IdentityError::Inconsistent);
        }
        let signing = txn.generate_reserved_key(&id, Purpose::Signing)?;
        let kem = txn.generate_reserved_key(&id, Purpose::Kem)?;
        let tenor_rotation = self
            .request
            .tenor_rotation_key
            .then(|| txn.generate_reserved_key(&id, Purpose::Rotation))
            .transpose()?;
        let operation =
            crate::plc_genesis::prepare(&handle, &self.hosting.endpoint, rotation, signing, kem, tenor_rotation)?;
        let signing_bytes = URL_SAFE_NO_PAD
            .encode(atproto_dasl::to_vec(&operation).map_err(|e| IdentityError::Encoding(e.to_string()))?);
        let expires = now.checked_add(RESERVATION_SECONDS).ok_or(IdentityError::Clock)?;
        let reserved = Reservation {
            owner: self.owner,
            handle,
            visibility: self.request.visibility,
            name: self.request.name,
            operation,
            expires,
            signed: None,
            created: false,
        };
        txn.put(keys::reservation(&id), serde_json::to_vec(&reserved)?.into());
        txn.put(
            keys::reserved_handle(&reserved.handle),
            Bytes::copy_from_slice(id.as_bytes()),
        );
        txn.put(
            keys::reservation_expiry(&id, expires),
            Bytes::copy_from_slice(id.as_bytes()),
        );
        Ok(Reserved {
            reservation: id,
            operation: reserved.operation,
            signing_bytes,
            expires_at: expires,
        })
    }
}

pub(crate) async fn load(txn: &Txn<'_>, id: &str) -> Result<Reservation, IdentityError> {
    let raw = txn
        .get(&keys::reservation(id))
        .await?
        .ok_or(IdentityError::ReservationNotFound)?;
    Ok(serde_json::from_slice(&raw)?)
}

fn remove_draft(txn: &mut Txn<'_>, id: &str, reservation: &Reservation) {
    txn.delete(keys::reservation(id));
    txn.delete(keys::reserved_handle(&reservation.handle));
    txn.delete(keys::reservation_expiry(id, reservation.expires));
    for purpose in [Purpose::Signing, Purpose::Kem, Purpose::Rotation] {
        txn.delete(keys::reserved_key(id, purpose));
    }
}

/// Bounded expiry work, ordered oldest first so live drafts cannot starve
/// expired ones. A submitting intent has left this index before any network I/O.
#[derive(Debug)]
pub struct SweepReservations {
    pub now: SystemTime,
    pub limit: usize,
}

impl Op for SweepReservations {
    type Output = usize;
    type Error = IdentityError;

    async fn run(self, txn: &mut Txn<'_>) -> Result<usize, IdentityError> {
        let now = unix_seconds(self.now)?;
        let mut removed = 0;
        for (_, id) in txn.scan(keys::RESERVATION_EXPIRIES, self.limit).await? {
            let id = std::str::from_utf8(&id).map_err(|_| IdentityError::Inconsistent)?;
            let reservation = load(txn, id).await?;
            if reservation.expires > now {
                break;
            }
            if reservation.signed.is_some() {
                return Err(IdentityError::Inconsistent);
            }
            remove_draft(txn, id, &reservation);
            removed += 1;
        }
        Ok(removed)
    }
}

struct BeginCreate {
    token: ServiceToken,
    request: CreateRequest,
}

struct Prepared {
    id: String,
    did: Did,
    operation: Operation,
    created: bool,
}

impl Op for BeginCreate {
    type Output = Prepared;
    type Error = IdentityError;

    async fn run(self, txn: &mut Txn<'_>) -> Result<Prepared, IdentityError> {
        self.token.consume(txn).await?;
        let id = self.request.reservation;
        if id.len() != 43 || !id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_') {
            return Err(IdentityError::ReservationNotFound);
        }
        let mut reserved = load(txn, &id).await?;
        if reserved.owner != self.token.issuer {
            return Err(IdentityError::NotOwner);
        }
        if reserved.signed.is_none() && reserved.expires <= unix_seconds(SystemTime::now())? {
            return Err(IdentityError::ReservationExpired);
        }
        verify_reserved(&reserved.operation, &self.request.signed_operation)?;
        let did = Did::new(plc::Did::from_genesis(&self.request.signed_operation)?.to_string());
        if let Some(signed) = &reserved.signed {
            if signed != &self.request.signed_operation {
                return Err(IdentityError::InvalidOperation);
            }
        } else {
            reserved.signed = Some(self.request.signed_operation.clone());
            txn.put(keys::reservation(&id), serde_json::to_vec(&reserved)?.into());
            // From this point PLC may accept the operation even after a lost
            // response. Sweeping its keys would strand an irreversible DID.
            txn.delete(keys::reservation_expiry(&id, reserved.expires));
        }
        Ok(Prepared {
            id,
            did,
            operation: self.request.signed_operation,
            created: reserved.created,
        })
    }
}

fn verify_reserved(reserved: &UnsignedOperation, signed: &Operation) -> Result<(), IdentityError> {
    let (
        UnsignedOperation::PlcOperation {
            rotation_keys,
            verification_methods,
            also_known_as,
            services,
            prev,
        },
        Operation::PlcOperation {
            rotation_keys: signed_keys,
            verification_methods: signed_methods,
            also_known_as: signed_aliases,
            services: signed_services,
            prev: signed_prev,
            ..
        },
    ) = (reserved, signed)
    else {
        return Err(IdentityError::InvalidOperation);
    };
    if prev.is_some()
        || signed_prev.is_some()
        || rotation_keys != signed_keys
        || verification_methods != signed_methods
        || also_known_as != signed_aliases
        || services != signed_services
    {
        return Err(IdentityError::InvalidOperation);
    }
    let owner = rotation_keys.first().ok_or(IdentityError::Inconsistent)?;
    let owner = identify_key(owner).map_err(|_| IdentityError::Inconsistent)?;
    signed.verify(&[owner]).map_err(|_| IdentityError::InvalidOperation)
}

/// Publishes the exact owner-signed operation, then atomically creates the
/// World. A committed intent spends its token even if the PLC response fails;
/// retrying with a fresh token reconciles PLC before another submission.
pub(crate) async fn create(
    shard: &Handle,
    hosting: &Hosting,
    token: ServiceToken,
    request: CreateRequest,
) -> Result<WorldInfo, IdentityError> {
    let prepared = shard.run(BeginCreate { token, request }).await??;
    if prepared.created {
        return world::get(shard.reader(), &prepared.did)
            .await?
            .ok_or(IdentityError::Inconsistent);
    }
    let document = publish(hosting, &prepared.did, &prepared.operation).await?;
    let info = shard
        .run(world::CreateWorld {
            reservation: prepared.id,
            did: prepared.did,
            document,
        })
        .await??;
    hosting.announce().await;
    Ok(info)
}

async fn publish(hosting: &Hosting, did: &Did, operation: &Operation) -> Result<Document, IdentityError> {
    let mut expected = PlcState::new();
    operation.apply_to(&mut expected);
    // A previous request may have reached PLC but lost its response or died
    // before the local commit. Query first, including after a process restart.
    match plc::query(hosting.fetcher.client(), &hosting.plc_directory, did.as_str()).await {
        Ok(document) => return matching_document(did, &expected, document),
        Err(PLCDIDError::HttpRequestFailed { error, .. }) if error.status().is_some_and(|s| s.as_u16() == 404) => {}
        Err(error) => return Err(error.into()),
    }
    match plc::submit(
        hosting.fetcher.client(),
        &hosting.plc_directory,
        did.as_str(),
        operation,
    )
    .await
    {
        Ok(()) => Ok(expected.to_document(did.as_str())),
        Err(error) => {
            // Even a failed response can follow an accepted POST. Never mint
            // new keys or change the operation to recover from that ambiguity.
            if let Ok(document) = plc::query(hosting.fetcher.client(), &hosting.plc_directory, did.as_str()).await {
                return matching_document(did, &expected, document);
            }
            Err(error.into())
        }
    }
}

fn matching_document(did: &Did, expected: &PlcState, document: Document) -> Result<Document, IdentityError> {
    let actual = PlcState::from_document(&document).map_err(|_| IdentityError::DocumentMismatch)?;
    if document.id != did.as_str()
        || actual.verification_methods != expected.verification_methods
        || actual.services != expected.services
        || actual.also_known_as != expected.also_known_as
    {
        return Err(IdentityError::DocumentMismatch);
    }
    Ok(document)
}

/// Resolves only a hosted, active World whose DID document confirms the name.
pub async fn resolve_handle(reader: &Reader, handle: &str) -> Result<Did, IdentityError> {
    let handle = normalize_handle(handle)?;
    let raw = reader
        .get(&keys::handle(&handle))
        .await?
        .ok_or(IdentityError::HandleNotFound)?;
    let did = Did::new(std::str::from_utf8(&raw).map_err(|_| IdentityError::Inconsistent)?);
    let info = world::get(reader, &did).await?.ok_or(IdentityError::HandleNotFound)?;
    if info.status != world::Status::Active || info.handle != handle || !info.document.confirms_handle(&handle) {
        return Err(IdentityError::HandleNotFound);
    }
    Ok(did)
}

fn unix_seconds(now: SystemTime) -> Result<u64, IdentityError> {
    now.duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .map_err(|_| IdentityError::Clock)
}
