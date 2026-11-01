//! Space writer tracking, subscriptions and durable notification delivery.
//!
//! Adapted from atproto-pds `src/space/{notify,sequencer,write_delivery,
//! deletion_delivery,revocation,revoke,recipient}.rs` and
//! `src/http/space_handlers.rs` at revision 09a868cd (MIT).
//! Motet commits every intent with its state change, uses the shard's single
//! writer instead of SQL, and delivers only through motet-net outside an Op.

use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use atproto_identity::traits::IdentityResolver;
use atproto_identity::validation::validate_service_endpoint;
use atproto_record::tid::Tid;
use atproto_space::credential::{CLOCK_SKEW_SECS, SPACE_CREDENTIAL_MAX_AGE_SECS};
use atproto_space::set_hash::{LtHash, SetHash};
use atproto_space::{LexBytes, SpaceUri};
use motet_net::Fetcher;
use motet_rules::Did;
use serde::{Deserialize, Serialize};
use tokio::task::JoinHandle;

use super::credentials::{self, VerifiedCredential};
use super::{SpaceError, access, access_txn, decode, encode, info, info_txn, storage};
use crate::keys::Purpose;
use crate::shard::{Handle, Op, SubmitError, Txn};
use crate::store::{Reader, keys};

pub const NOTIFY_WRITE_NSID: &str = "com.atproto.space.notifyWrite";
pub const NOTIFY_SPACE_DELETED_NSID: &str = "com.atproto.space.notifySpaceDeleted";
pub const NOTIFY_CREDENTIAL_REVOKED_NSID: &str = "com.atproto.space.notifyCredentialRevoked";
/// Host policy: a subscription may outlive the credential that created it.
pub const DEFAULT_REGISTER_NOTIFY_TTL_SECS: i64 = 60 * 24 * 60 * 60;
const REVOCATION_RETENTION_SECS: u64 = SPACE_CREDENTIAL_MAX_AGE_SECS + 2 * CLOCK_SKEW_SECS;
const PAGE_SIZE: usize = 100;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NotifyWrite {
    pub space: String,
    pub repo: String,
    pub repo_rev: String,
    pub data: LexBytes,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub space_rev: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prev_space_rev: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoRef {
    pub did: String,
    pub repo_rev: String,
    pub data: LexBytes,
    pub space_rev: String,
}

#[derive(Debug, Serialize)]
pub struct ReposPage {
    pub repos: Vec<RepoRef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
struct Writer {
    generation: String,
    repo: RepoRef,
}

#[derive(Debug, Deserialize, Serialize)]
struct Sequence {
    generation: String,
    rev: String,
}

/// A service identity bound to its validated DID-document endpoint.
/// Construct this outside the writer with [`resolve_service`].
#[derive(Debug)]
pub struct ResolvedService {
    service: String,
    endpoint: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct Registration {
    service: String,
    endpoint: String,
    generation: String,
    expires: i64,
}

#[derive(Debug, Deserialize, Serialize)]
enum Notification {
    Write(NotifyWrite),
    Revoked { credentials: Vec<String> },
    Deleted,
}

impl Notification {
    const fn method(&self) -> &'static str {
        match self {
            Self::Write(_) => NOTIFY_WRITE_NSID,
            Self::Revoked { .. } => NOTIFY_CREDENTIAL_REVOKED_NSID,
            Self::Deleted => NOTIFY_SPACE_DELETED_NSID,
        }
    }

    fn body(&self, space: &SpaceUri) -> Result<Vec<u8>, SpaceError> {
        Ok(match self {
            Self::Write(payload) => serde_json::to_vec(payload)?,
            Self::Revoked { credentials } => serde_json::to_vec(&serde_json::json!({
                "space": space.to_string(), "credentials": credentials,
            }))?,
            Self::Deleted => serde_json::to_vec(&serde_json::json!({ "space": space.to_string() }))?,
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct Intent {
    id: String,
    space: SpaceUri,
    generation: String,
    audience: String,
    /// `None` resolves a writer's current `#atproto_pds` at delivery time.
    endpoint: Option<String>,
    notification: Notification,
    deadline_ms: u64,
    due_ms: u64,
    attempts: u32,
}

fn now_ms() -> Result<u64, SpaceError> {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| SpaceError::Invalid(format!("system clock precedes Unix epoch: {error}")))?;
    u64::try_from(elapsed.as_millis()).map_err(|error| SpaceError::Invalid(error.to_string()))
}

fn deadline_ms(expires: i64) -> u64 {
    u64::try_from(expires).unwrap_or(0).saturating_mul(1000)
}

fn service_parts(service: &str) -> Result<(&str, Option<&str>), SpaceError> {
    let (did, fragment) = service
        .split_once('#')
        .map_or((service, None), |(did, fragment)| (did, Some(fragment)));
    if !(did.starts_with("did:plc:") || did.starts_with("did:web:"))
        || did.contains(['/', '?', '\0'])
        || fragment.is_some_and(|value| value.is_empty() || value.contains(['#', '/', '?', '\0']))
    {
        return Err(SpaceError::Invalid(
            "service must be a DID with an optional service fragment".into(),
        ));
    }
    Ok((did, fragment))
}

/// Resolve before submitting an Op. `resolver` must use motet-net's policy.
/// Delivery repeats endpoint validation and its client rejects private DNS.
pub async fn resolve_service(resolver: &dyn IdentityResolver, service: &str) -> Result<ResolvedService, SpaceError> {
    let (did, fragment) = service_parts(service)?;
    let document = resolver
        .resolve(did)
        .await
        .map_err(|error| SpaceError::ServiceNotResolvable(error.to_string()))?;
    if document.id != did {
        return Err(SpaceError::ServiceNotResolvable(
            "DID document names another subject".into(),
        ));
    }
    let endpoint = document
        .service
        .iter()
        .find_map(|entry| {
            let entry_fragment = entry
                .id
                .strip_prefix('#')
                .or_else(|| entry.id.strip_prefix(did).and_then(|suffix| suffix.strip_prefix('#')));
            if entry_fragment.is_none() || fragment.is_some_and(|wanted| entry_fragment != Some(wanted)) {
                return None;
            }
            entry.service_endpoint.as_uri()
        })
        .ok_or_else(|| SpaceError::ServiceNotResolvable("DID document has no matching service endpoint".into()))?;
    validate_service_endpoint(endpoint).map_err(|error| SpaceError::Invalid(error.to_string()))?;
    Ok(ResolvedService {
        service: service.to_owned(),
        endpoint: endpoint.trim_end_matches('/').to_owned(),
    })
}

/// Register the whole Space, or renew its registration, after HTTP proof-of-possession verification.
/// Rechecks the credential inside the writer; returns Unix seconds for `expiresAt`.
pub async fn register(
    txn: &mut Txn<'_>,
    space: &SpaceUri,
    credential: &VerifiedCredential,
    service: ResolvedService,
) -> Result<i64, SpaceError> {
    check_credential(txn, space, credential).await?;
    let current = info_txn(txn, space).await?;
    if current.deleted {
        return Err(SpaceError::Deleted);
    }
    let expires = i64::try_from(now_ms()? / 1000)
        .map_err(|error| SpaceError::Invalid(error.to_string()))?
        .saturating_add(DEFAULT_REGISTER_NOTIFY_TTL_SECS);
    let registration = Registration {
        service: service.service,
        endpoint: service.endpoint,
        generation: current.generation,
        expires,
    };
    txn.put(
        keys::space_notify_registration(space, &registration.service),
        encode(&registration)?,
    );
    Ok(expires)
}

/// Withdraw a registration idempotently. A credential for another Space is refused.
pub async fn unregister(
    txn: &mut Txn<'_>,
    space: &SpaceUri,
    credential: &VerifiedCredential,
    service: &str,
) -> Result<(), SpaceError> {
    check_credential(txn, space, credential).await?;
    service_parts(service)?;
    txn.delete(keys::space_notify_registration(space, service));
    Ok(())
}

async fn check_credential(txn: &Txn<'_>, space: &SpaceUri, credential: &VerifiedCredential) -> Result<(), SpaceError> {
    if credential.space() != space {
        return Err(SpaceError::NotAuthorized);
    }
    credentials::recheck(txn, credential)
        .await
        .map_err(credentials::CredentialError::into_space_error)
}

/// Record a local commit and its durable notification intents in the same Op.
/// The writer must still have write access, even for a duplicate revision.
pub async fn record_write(txn: &mut Txn<'_>, space: &SpaceUri, writer: &str, rev: &str) -> Result<(), SpaceError> {
    let state = storage::current_state_txn(txn, &Did::new(writer), space).await?;
    if state.rev.as_deref() != Some(rev) {
        return Err(SpaceError::Conflict);
    }
    let hash = state
        .set_hash
        .ok_or_else(|| SpaceError::Invalid("Space repo has no commitment state".into()))?;
    let data = LtHash::from_state_bytes(&hash)?.digest();
    accept_write(txn, space, writer, rev, data).await
}

/// Receive a writer's service-authenticated notification. HTTP verifies the signature and method;
/// this boundary binds the verified issuer/audience and rechecks write access atomically.
pub async fn receive_write(
    txn: &mut Txn<'_>,
    space: &SpaceUri,
    issuer: &str,
    audience: &str,
    payload: NotifyWrite,
) -> Result<(), SpaceError> {
    if issuer != payload.repo
        || payload.space != space.to_string()
        || (audience != space.space_did && audience != format!("{}#atproto_pds", space.space_did))
    {
        return Err(SpaceError::NotAuthorized);
    }
    // Forwarded sequence fields are untrusted; only this authority assigns them.
    accept_write(txn, space, &payload.repo, &payload.repo_rev, payload.data.0).await
}

async fn accept_write(
    txn: &mut Txn<'_>,
    space: &SpaceUri,
    writer: &str,
    rev: &str,
    data: Vec<u8>,
) -> Result<(), SpaceError> {
    if !access_txn(txn, space, Some(writer)).await?.write {
        return Err(SpaceError::NotAuthorized);
    }
    let now = now_ms()?;
    let tid = Tid::decode(rev).map_err(|error| SpaceError::Invalid(format!("repoRev is not a TID: {error}")))?;
    if tid.timestamp_micros() > now.saturating_add(300_000).saturating_mul(1000) {
        return Err(SpaceError::FutureRev);
    }
    if data.len() != 32 {
        return Err(SpaceError::Invalid("commit data must be a 32-byte digest".into()));
    }
    let current = info_txn(txn, space).await?;
    let writer_key = keys::space_notify_writer(space, writer);
    if let Some(stored) = txn.get(&writer_key).await? {
        let stored: Writer = decode(&stored)?;
        if stored.generation == current.generation && rev <= stored.repo.repo_rev.as_str() {
            return Ok(());
        }
        txn.delete(keys::space_notify_revision(space, &stored.repo.space_rev));
    }
    let sequence_key = keys::space_notify_sequence(space);
    let previous = txn
        .get(&sequence_key)
        .await?
        .map(|bytes| decode::<Sequence>(&bytes))
        .transpose()?;
    let previous = previous
        .filter(|sequence| sequence.generation == current.generation)
        .map(|sequence| sequence.rev);
    let next = match previous.as_deref() {
        Some(previous) => {
            Tid::next_after(&Tid::decode(previous).map_err(|error| SpaceError::Invalid(error.to_string()))?)
        }
        None => Tid::new(),
    }
    .to_string();
    let payload = NotifyWrite {
        space: space.to_string(),
        repo: writer.to_owned(),
        repo_rev: rev.to_owned(),
        data: LexBytes(data),
        space_rev: Some(next.clone()),
        prev_space_rev: previous,
    };
    let row = Writer {
        generation: current.generation.clone(),
        repo: RepoRef {
            did: payload.repo.clone(),
            repo_rev: payload.repo_rev.clone(),
            data: payload.data.clone(),
            space_rev: next.clone(),
        },
    };
    let encoded = encode(&row)?;
    txn.put(writer_key, encoded.clone());
    txn.put(keys::space_notify_revision(space, &next), encoded);
    txn.put(
        sequence_key,
        encode(&Sequence {
            generation: current.generation.clone(),
            rev: next,
        })?,
    );
    for (_, value) in txn.scan(&keys::space_notify_registrations(space), usize::MAX).await? {
        let recipient: Registration = decode(&value)?;
        if recipient.generation != current.generation || deadline_ms(recipient.expires) <= now {
            continue;
        }
        enqueue(
            txn,
            &Intent {
                id: Tid::new().to_string(),
                space: space.clone(),
                generation: current.generation.clone(),
                audience: recipient.service,
                endpoint: Some(recipient.endpoint),
                notification: Notification::Write(payload.clone()),
                deadline_ms: deadline_ms(recipient.expires),
                due_ms: now,
                attempts: 0,
            },
        )?;
    }
    Ok(())
}

/// Read current authorized writers in increasing authority revision order.
/// HTTP must first authenticate a credential for this Space. Every nonempty page has a cursor.
pub async fn list_repos(
    reader: &Reader,
    space: &SpaceUri,
    cursor: Option<&str>,
    limit: usize,
) -> Result<ReposPage, SpaceError> {
    let current = info(reader, space).await?;
    if current.deleted {
        return Err(SpaceError::Deleted);
    }
    if let Some(cursor) = cursor {
        Tid::decode(cursor).map_err(|error| SpaceError::Invalid(error.to_string()))?;
    }
    let limit = limit.clamp(1, 1000);
    let prefix = keys::space_notify_revisions(space);
    let mut after = cursor.map(|cursor| keys::space_notify_revision(space, cursor));
    let mut repos = Vec::with_capacity(limit);
    loop {
        let rows = reader.scan(&prefix, after.as_deref(), PAGE_SIZE).await?;
        if rows.is_empty() {
            break;
        }
        for (key, value) in rows {
            after = Some(key.to_vec());
            let writer: Writer = decode(&value)?;
            if writer.generation == current.generation && access(reader, space, Some(&writer.repo.did)).await?.write {
                repos.push(writer.repo);
                if repos.len() == limit {
                    let cursor = repos.last().map(|repo| repo.space_rev.clone());
                    return Ok(ReposPage { repos, cursor });
                }
            }
        }
    }
    let cursor = repos.last().map(|repo| repo.space_rev.clone());
    Ok(ReposPage { repos, cursor })
}

/// Read a page in the same Op that rechecks its credential, with no revocation race.
pub async fn list_repos_txn(
    txn: &Txn<'_>,
    space: &SpaceUri,
    cursor: Option<&str>,
    limit: usize,
) -> Result<ReposPage, SpaceError> {
    let current = info_txn(txn, space).await?;
    if current.deleted {
        return Err(SpaceError::Deleted);
    }
    if let Some(cursor) = cursor {
        Tid::decode(cursor).map_err(|error| SpaceError::Invalid(error.to_string()))?;
    }
    let limit = limit.clamp(1, 1000);
    let mut repos = Vec::with_capacity(limit);
    // Txn exposes prefix scans, not cursor seeks. Filtering stays inside this
    // read-only Op so credential revocation and writer access share one view.
    for (_, value) in txn.scan(&keys::space_notify_revisions(space), usize::MAX).await? {
        let writer: Writer = decode(&value)?;
        if cursor.is_some_and(|cursor| writer.repo.space_rev.as_str() <= cursor)
            || writer.generation != current.generation
        {
            continue;
        }
        if access_txn(txn, space, Some(&writer.repo.did)).await?.write {
            repos.push(writer.repo);
            if repos.len() == limit {
                break;
            }
        }
    }
    let cursor = repos.last().map(|repo| repo.space_rev.clone());
    Ok(ReposPage { repos, cursor })
}

/// Snapshot live syncers after the core writes its tombstone, before registrations are erased.
/// Every intent retains this generation; recreation cancels it before delivery.
pub async fn queue_deletion(txn: &mut Txn<'_>, space: &SpaceUri, generation: &str) -> Result<(), SpaceError> {
    let current = info_txn(txn, space).await?;
    if !current.deleted || current.generation != generation {
        return Err(SpaceError::Conflict);
    }
    let now = now_ms()?;
    for (key, value) in txn.scan(&keys::space_notify_registrations(space), usize::MAX).await? {
        let recipient: Registration = decode(&value)?;
        if recipient.generation == generation && deadline_ms(recipient.expires) > now {
            enqueue(
                txn,
                &Intent {
                    id: Tid::new().to_string(),
                    space: space.clone(),
                    generation: generation.to_owned(),
                    audience: recipient.service,
                    endpoint: Some(recipient.endpoint),
                    notification: Notification::Deleted,
                    deadline_ms: deadline_ms(recipient.expires),
                    due_ms: now,
                    attempts: 0,
                },
            )?;
        }
        txn.delete(key.to_vec());
    }
    Ok(())
}

/// Revoke locally and queue every remote writer's repo host before the credential ledger commits.
/// Unresolved hosts remain queued until the credential deadline; no network runs in this Op.
pub async fn queue_revocation(txn: &mut Txn<'_>, space: &SpaceUri, jti: &str, expires: i64) -> Result<(), SpaceError> {
    if jti.is_empty() {
        return Err(SpaceError::Invalid("credential jti must not be empty".into()));
    }
    let now = now_ms()?;
    store_revocation(txn, space, jti, now).await?;
    if deadline_ms(expires) <= now {
        return Ok(());
    }
    let current = info_txn(txn, space).await?;
    for (_, value) in txn.scan(&keys::space_notify_writers(space), usize::MAX).await? {
        let writer: Writer = decode(&value)?;
        if writer.generation != current.generation
            || txn
                .get(&keys::sealed_key(&Did::new(&writer.repo.did), Purpose::Signing))
                .await?
                .is_some()
        {
            continue;
        }
        enqueue(
            txn,
            &Intent {
                id: Tid::new().to_string(),
                space: space.clone(),
                generation: current.generation.clone(),
                audience: writer.repo.did,
                endpoint: None,
                notification: Notification::Revoked {
                    credentials: vec![jti.to_owned()],
                },
                deadline_ms: deadline_ms(expires),
                due_ms: now,
                attempts: 0,
            },
        )?;
    }
    Ok(())
}

async fn store_revocation(txn: &mut Txn<'_>, space: &SpaceUri, jti: &str, now: u64) -> Result<(), SpaceError> {
    let key = keys::space_notify_revoked(space, jti);
    let previous = txn
        .get(&key)
        .await?
        .map(|bytes| decode::<u64>(&bytes))
        .transpose()?
        .unwrap_or(0);
    let expires = now.saturating_add(REVOCATION_RETENTION_SECS * 1000).max(previous);
    txn.put(key, encode(&expires)?);
    Ok(())
}

/// Accept only the authenticated authority. HTTP also verifies the method and a hosted bare-DID audience.
pub async fn receive_revocation(
    txn: &mut Txn<'_>,
    space: &SpaceUri,
    issuer: &str,
    jtis: &[String],
) -> Result<(), SpaceError> {
    if issuer != space.space_did {
        return Err(SpaceError::NotAuthorized);
    }
    if jtis.is_empty() || jtis.len() > 100 || jtis.iter().any(String::is_empty) {
        return Err(SpaceError::Invalid(
            "credentials must name 1 to 100 nonempty jtis".into(),
        ));
    }
    let now = now_ms()?;
    for jti in jtis {
        store_revocation(txn, space, jti, now).await?;
    }
    Ok(())
}

pub async fn is_revoked(reader: &Reader, space: &SpaceUri, jti: &str) -> Result<bool, SpaceError> {
    let expires = reader.get(&keys::space_notify_revoked(space, jti)).await?;
    revoked_at(expires.as_deref(), now_ms()?)
}

pub async fn is_revoked_txn(txn: &Txn<'_>, space: &SpaceUri, jti: &str) -> Result<bool, SpaceError> {
    let expires = txn.get(&keys::space_notify_revoked(space, jti)).await?;
    revoked_at(expires.as_deref(), now_ms()?)
}

fn revoked_at(expires: Option<&[u8]>, now: u64) -> Result<bool, SpaceError> {
    Ok(expires
        .map(decode::<u64>)
        .transpose()?
        .is_some_and(|expires| expires > now))
}

/// A repo host keeps its members' own records after an authority deletes a Space.
///
/// The October 22 alpha assigns purging to syncers, a role this host does not implement.
/// HTTP verifies service auth and its hosted audience before this authority check.
pub fn receive_deletion(
    _txn: &mut Txn<'_>,
    space: &SpaceUri,
    issuer: &str,
) -> impl Future<Output = Result<(), SpaceError>> + Send {
    std::future::ready(if issuer == space.space_did {
        Ok(())
    } else {
        Err(SpaceError::NotAuthorized)
    })
}

fn enqueue(txn: &mut Txn<'_>, intent: &Intent) -> Result<(), SpaceError> {
    txn.put(keys::space_notify_outbox(intent.due_ms, &intent.id), encode(intent)?);
    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum NotifyTaskError {
    #[error(transparent)]
    Space(#[from] SpaceError),
    #[error(transparent)]
    Submit(#[from] SubmitError),
}

/// Start one delivery task per shard. Await the returned task and propagate errors;
/// aborting it is safe because an unacknowledged delivery remains in the durable outbox.
#[must_use]
pub fn spawn(
    handle: Handle,
    fetcher: Fetcher,
    resolver: Arc<dyn IdentityResolver>,
) -> JoinHandle<Result<(), NotifyTaskError>> {
    tokio::spawn(async move {
        let mut timer = tokio::time::interval(Duration::from_secs(1));
        timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            timer.tick().await;
            deliver_due(&handle, &fetcher, resolver.as_ref()).await?;
        }
    })
}

/// Deliver a bounded page. State reads and network I/O happen outside the writer;
/// prepare/finish operations recheck the persisted intent before acting on it.
pub async fn deliver_due(
    handle: &Handle,
    fetcher: &Fetcher,
    resolver: &dyn IdentityResolver,
) -> Result<(), NotifyTaskError> {
    let now = now_ms()?;
    let rows = handle
        .reader()
        .scan(keys::SPACE_NOTIFY_OUTBOX, None, PAGE_SIZE)
        .await
        .map_err(SpaceError::from)?;
    for (key, _) in rows {
        let due = keys::space_notify_due(&key)
            .ok_or_else(|| SpaceError::Invalid("invalid notification outbox key".into()))?;
        if due > now {
            break;
        }
        let prepared = handle
            .run(Prepare {
                key: key.to_vec(),
                now_ms: now_ms()?,
            })
            .await??;
        let Some(intent) = prepared else { continue };
        let result = deliver(handle.reader(), fetcher, resolver, &intent).await?;
        handle
            .run(Finish {
                key: key.to_vec(),
                intent,
                result,
                now_ms: now_ms()?,
            })
            .await??;
    }
    Ok(())
}

struct Prepare {
    key: Vec<u8>,
    now_ms: u64,
}

impl Op for Prepare {
    type Output = Option<Intent>;
    type Error = SpaceError;

    async fn run(self, txn: &mut Txn<'_>) -> Result<Self::Output, Self::Error> {
        let Some(value) = txn.get(&self.key).await? else {
            return Ok(None);
        };
        let intent: Intent = decode(&value)?;
        if intent.due_ms > self.now_ms {
            return Ok(None);
        }
        let reason = if intent.deadline_ms <= self.now_ms {
            Some("notification deadline expired")
        } else {
            stale_reason(txn, &intent, self.now_ms).await?
        };
        if let Some(reason) = reason {
            tracing::info!(space = %intent.space, audience = %intent.audience, reason, "retiring Space notification");
            txn.delete(self.key);
            return Ok(None);
        }
        Ok(Some(intent))
    }
}

async fn stale_reason(txn: &Txn<'_>, intent: &Intent, now: u64) -> Result<Option<&'static str>, SpaceError> {
    if matches!(intent.notification, Notification::Revoked { .. }) {
        return Ok(None);
    }
    let current = match info_txn(txn, &intent.space).await {
        Ok(current) => current,
        Err(SpaceError::NotFound) => return Ok(Some("Space no longer exists")),
        Err(error) => return Err(error),
    };
    if current.generation != intent.generation {
        return Ok(Some("Space was recreated"));
    }
    match &intent.notification {
        Notification::Deleted if !current.deleted => Ok(Some("Space is live")),
        Notification::Write(payload) => {
            if current.deleted {
                return Ok(Some("Space was deleted"));
            }
            if !access_txn(txn, &intent.space, Some(&payload.repo)).await?.write {
                return Ok(Some("writer is no longer authorized"));
            }
            let Some(registration) = txn
                .get(&keys::space_notify_registration(&intent.space, &intent.audience))
                .await?
            else {
                return Ok(Some("registration was withdrawn"));
            };
            let registration: Registration = decode(&registration)?;
            if registration.generation != intent.generation || deadline_ms(registration.expires) <= now {
                return Ok(Some("registration expired"));
            }
            if intent.endpoint.as_deref() != Some(registration.endpoint.as_str()) {
                return Ok(Some("registration endpoint changed"));
            }
            Ok(None)
        }
        Notification::Deleted | Notification::Revoked { .. } => Ok(None),
    }
}

enum Delivery {
    Delivered,
    Retry(String),
    Terminal(String),
}

async fn deliver(
    reader: &Reader,
    fetcher: &Fetcher,
    resolver: &dyn IdentityResolver,
    intent: &Intent,
) -> Result<Delivery, SpaceError> {
    let endpoint = match &intent.endpoint {
        Some(endpoint) => endpoint.clone(),
        None => match resolve_service(resolver, &format!("{}#atproto_pds", intent.audience)).await {
            Ok(service) => service.endpoint,
            Err(error) => return Ok(Delivery::Retry(format!("repo host resolution: {error}"))),
        },
    };
    if let Err(error) = validate_service_endpoint(&endpoint) {
        return Ok(Delivery::Terminal(format!("endpoint refused: {error}")));
    }
    if intent.deadline_ms <= now_ms()? {
        return Ok(Delivery::Terminal(
            "notification deadline expired during resolution".into(),
        ));
    }
    let authority = Did::new(&intent.space.space_did);
    let Some(key) = reader.key(&authority, Purpose::Signing).await? else {
        return Ok(Delivery::Terminal("authority signing key no longer exists".into()));
    };
    let token = atproto_xrpcs::service_auth::mint_service_auth(
        &key,
        &intent.space.space_did,
        &intent.audience,
        intent.notification.method(),
        Duration::from_secs(60),
    )
    .map_err(|error| SpaceError::Invalid(format!("cannot sign notification: {error}")))?;
    let url = format!(
        "{}/xrpc/{}",
        endpoint.trim_end_matches('/'),
        intent.notification.method()
    );
    let body = intent.notification.body(&intent.space)?;
    let response = fetcher
        .client()
        .post(url)
        .bearer_auth(token)
        .header("content-type", "application/json")
        .body(body)
        .send()
        .await;
    Ok(match response {
        Ok(response) if response.status().is_success() => Delivery::Delivered,
        Ok(response) if retryable_status(response.status().as_u16()) => {
            Delivery::Retry(format!("HTTP {}", response.status()))
        }
        Ok(response) => Delivery::Terminal(format!("HTTP {}", response.status())),
        Err(error) => Delivery::Retry(format!("transport: {error}")),
    })
}

const fn retryable_status(status: u16) -> bool {
    matches!(status, 408 | 425 | 429) || (status >= 500 && status <= 599 && status != 501)
}

/// The alpha's backoff: one minute doubling to one hour, then 50–100% jitter.
fn retry_delay_ms(attempts: u32) -> Result<u64, SpaceError> {
    let base = 60_000_u64
        .saturating_mul(1_u64 << attempts.saturating_sub(1).min(16))
        .min(3_600_000);
    let mut entropy = [0; 8];
    getrandom::fill(&mut entropy)
        .map_err(|error| SpaceError::Invalid(format!("retry jitter entropy unavailable: {error}")))?;
    Ok(base / 2 + u64::from_le_bytes(entropy) % (base / 2 + 1))
}

struct Finish {
    key: Vec<u8>,
    intent: Intent,
    result: Delivery,
    now_ms: u64,
}

impl Op for Finish {
    type Output = ();
    type Error = SpaceError;

    async fn run(self, txn: &mut Txn<'_>) -> Result<(), SpaceError> {
        let Some(value) = txn.get(&self.key).await? else {
            return Ok(());
        };
        let current: Intent = decode(&value)?;
        if current.id != self.intent.id || current.attempts != self.intent.attempts {
            return Ok(());
        }
        txn.delete(self.key);
        match self.result {
            Delivery::Delivered => {}
            Delivery::Terminal(reason) => {
                tracing::warn!(space = %current.space, audience = %current.audience, reason, "Space notification refused permanently");
            }
            Delivery::Retry(reason) => {
                if current.deadline_ms <= self.now_ms || stale_reason(txn, &current, self.now_ms).await?.is_some() {
                    tracing::info!(space = %current.space, audience = %current.audience, reason, "Space notification retired after retry deadline or generation change");
                    return Ok(());
                }
                let mut next = current;
                next.attempts = next.attempts.saturating_add(1);
                next.due_ms = self
                    .now_ms
                    .saturating_add(retry_delay_ms(next.attempts)?)
                    .min(next.deadline_ms);
                tracing::debug!(space = %next.space, audience = %next.audience, reason, attempts = next.attempts, "retrying Space notification later");
                enqueue(txn, &next)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "notify_tests.rs"]
mod tests;
