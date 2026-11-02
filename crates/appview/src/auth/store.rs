//! Sealed OAuth state and browser sessions, shared by AppView instances.
//!
//! A cookie is a random bearer secret; PostgreSQL stores only its hash. Both
//! the grant and pending login are sealed under an operator-provided key and
//! bound to their row and purpose. No secret is included in errors or `Debug`.
//!
//! Refresh holds a per-session transaction advisory lock while an independently
//! committed poison marker is present. If a task or connection disappears
//! after the PDS may have consumed the token, the next reader removes the
//! poisoned session rather than ever returning that token again.

use std::collections::{HashMap, hash_map::Entry};
use std::fmt;
use std::future::Future;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use base64::Engine as _;
use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use chacha20poly1305::aead::{AeadInPlace as _, KeyInit};
use chacha20poly1305::{Tag, XChaCha20Poly1305, XNonce};
use chrono::{DateTime, Utc};
use serde::Serialize;
use serde::de::DeserializeOwned;
use sha2::{Digest as _, Sha256};
use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Postgres, Transaction};
use thiserror::Error;

const NONCE_LEN: usize = 24;
const TAG_LEN: usize = 16;
const SECRET_LEN: usize = 32;
const COOKIE_LEN: usize = 43;
const FORMAT: u8 = 1;
const SESSION_PURPOSE: &[u8; 28] = b"motet/appview/oauth/session\0";
const PENDING_PURPOSE: &[u8; 28] = b"motet/appview/oauth/pending\0";
const MAX_WAITERS_PER_SESSION: usize = 32;
const LOCK_TIMEOUT: Duration = Duration::from_secs(5);
const POOL_TIMEOUT: Duration = Duration::from_secs(2);
const INITIAL_BACKOFF: Duration = Duration::from_millis(10);
const MAX_BACKOFF: Duration = Duration::from_millis(200);

type RowId = [u8; 32];
type StoredSession = (Vec<u8>, i64, bool);

/// A persistent reference to a session, not a browser bearer cookie.
///
/// The reference is a hash of the random cookie. Reconstruct it from the
/// database with [`SessionRef::from_db`] and never send it to a browser.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct SessionRef(RowId);

impl fmt::Debug for SessionRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SessionRef([redacted])")
    }
}

impl SessionRef {
    /// Hashes a syntactically valid opaque browser cookie.
    #[must_use]
    pub fn from_cookie(cookie: &str) -> Option<Self> {
        cookie_id(cookie).map(Self)
    }

    /// Restores a previously persisted 32-byte session reference.
    #[must_use]
    pub fn from_db(bytes: &[u8]) -> Option<Self> {
        <RowId>::try_from(bytes).ok().map(Self)
    }

    /// Returns the persistence-safe reference bytes, never the browser cookie.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

/// A storage error, with no bearer or cryptographic material in its message.
#[derive(Debug, Error)]
pub enum StoreError {
    #[error("OAuth storage database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("the OAuth storage key file could not be read: {0}")]
    KeyFile(#[source] std::io::Error),
    #[error("the OAuth storage key must contain 32 raw bytes or standard base64")]
    InvalidKey,
    #[error("the operating system random source failed")]
    Random,
    #[error("OAuth state could not be serialized")]
    Serialize,
    #[error("sealed OAuth state could not be opened")]
    Unseal,
    #[error("the remote OAuth refresh failed; the session was invalidated")]
    Refresh,
    #[error("the session generation moved backwards")]
    Generation,
    #[error("the OAuth session is busy; try again")]
    Busy,
}

/// A decrypted session and the generation observed by its reader.
///
/// Pass `generation` to [`SessionStore::refresh`] when the grant needs renewal.
/// The grant is intentionally omitted from `Debug`.
pub struct Session<T> {
    pub grant: T,
    pub generation: i64,
}

impl<T> fmt::Debug for Session<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Session")
            .field("grant", &"[redacted]")
            .field("generation", &self.generation)
            .finish()
    }
}

/// PostgreSQL OAuth state sealed with an external 256-bit key.
#[derive(Clone)]
pub struct SessionStore {
    pool: PgPool,
    locks: PgPool,
    waiters: Arc<Mutex<HashMap<RowId, usize>>>,
    cipher: XChaCha20Poly1305,
}

impl fmt::Debug for SessionStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SessionStore([redacted])")
    }
}

impl SessionStore {
    /// Uses a key supplied by the operator; do not store it in PostgreSQL.
    #[must_use]
    pub fn new(pool: PgPool, key: [u8; 32]) -> Self {
        // A held coordinator transaction must not consume a connection needed
        // to commit the poison marker or replacement grant.
        let locks = PgPoolOptions::new()
            .max_connections(5)
            .acquire_timeout(POOL_TIMEOUT)
            .connect_lazy_with((*pool.connect_options()).clone());
        Self {
            pool,
            locks,
            waiters: Arc::new(Mutex::new(HashMap::new())),
            cipher: XChaCha20Poly1305::new((&key).into()),
        }
    }

    /// Loads 32 raw bytes, or standard base64 with surrounding whitespace.
    pub fn from_file(pool: PgPool, path: &Path) -> Result<Self, StoreError> {
        let contents = std::fs::read(path).map_err(StoreError::KeyFile)?;
        let key = if let Ok(raw) = <[u8; SECRET_LEN]>::try_from(contents.as_slice()) {
            raw
        } else {
            let text = std::str::from_utf8(&contents).map_err(|_| StoreError::InvalidKey)?;
            STANDARD
                .decode(text.trim())
                .ok()
                .and_then(|bytes| <[u8; SECRET_LEN]>::try_from(bytes).ok())
                .ok_or(StoreError::InvalidKey)?
        };
        Ok(Self::new(pool, key))
    }

    /// Stores a complete OAuth grant and returns a fresh, opaque browser secret.
    ///
    /// `expires_at` is the browser session's absolute expiry, not the access
    /// token's expiry. The caller must also check the access token's lifetime.
    pub async fn create_session<T: Serialize + Sync>(
        &self,
        grant: &T,
        expires_at: DateTime<Utc>,
    ) -> Result<String, StoreError> {
        let mut secret = [0; SECRET_LEN];
        getrandom::fill(&mut secret).map_err(|_| StoreError::Random)?;
        let cookie = URL_SAFE_NO_PAD.encode(secret);
        let id = hash(&cookie);
        let sealed = self.seal(SESSION_PURPOSE, &id, grant)?;
        let mut tx = self.pool.begin().await?;
        sqlx::query("INSERT INTO service.oauth_sessions (id, sealed, expires_at) VALUES ($1, $2, $3)")
            .bind(id.as_slice())
            .bind(sealed)
            .bind(expires_at)
            .execute(&mut *tx)
            .await?;
        sqlx::query("INSERT INTO service.oauth_session_locks (id) VALUES ($1)")
            .bind(id.as_slice())
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(cookie)
    }

    /// Reads a live grant. An active refresh is awaited rather than mistaken
    /// for a logout; an abandoned refresh invalidates the session. If the
    /// wait limit is reached, it returns [`StoreError::Busy`] without a grant.
    pub async fn get_session<T: DeserializeOwned>(&self, cookie: &str) -> Result<Option<Session<T>>, StoreError> {
        let Some(reference) = SessionRef::from_cookie(cookie) else {
            return Ok(None);
        };
        self.get_session_by_ref(&reference).await
    }

    /// Reads a session from its persistent reference, with the same expiry,
    /// revocation and refresh guarantees as [`Self::get_session`].
    pub async fn get_session_by_ref<T: DeserializeOwned>(
        &self,
        reference: &SessionRef,
    ) -> Result<Option<Session<T>>, StoreError> {
        let id = &reference.0;
        let Some(row) = self.read_session(id).await? else {
            return Ok(None);
        };
        if !row.2 {
            return self.open_session(id, row).map(Some);
        }
        let Some(mut tx) = self.lock_session(id).await? else {
            return Ok(None);
        };
        let result = match self.read_session(id).await? {
            Some(row) if !row.2 => Some(self.open_session(id, row)?),
            Some(_) => {
                invalidate_locked(&mut tx, id).await?;
                None
            }
            None => None,
        };
        tx.commit().await?;
        Ok(result)
    }

    /// Renews one generation at most once across all AppView instances.
    ///
    /// A concurrent caller that observed an older generation receives the
    /// committed replacement without invoking `refresh`. Any remote error is
    /// treated as ambiguous and invalidates the session. If this future is
    /// cancelled after the marker commits, the next reader invalidates it.
    /// If the replacement commit is ambiguous, only a newer committed grant
    /// read back from PostgreSQL may be returned.
    /// Contention past the bounded wait returns [`StoreError::Busy`], without
    /// changing or exposing the grant.
    pub async fn refresh<T, F, Fut, E>(
        &self,
        cookie: &str,
        observed_generation: i64,
        refresh: F,
    ) -> Result<Option<Session<T>>, StoreError>
    where
        T: Serialize + DeserializeOwned,
        F: FnOnce(T) -> Fut,
        Fut: Future<Output = Result<T, E>>,
    {
        let Some(reference) = SessionRef::from_cookie(cookie) else {
            return Ok(None);
        };
        self.refresh_by_ref(&reference, observed_generation, refresh).await
    }

    /// Renews a session by persistent reference with the same serialized,
    /// fail-closed refresh semantics as [`Self::refresh`].
    pub async fn refresh_by_ref<T, F, Fut, E>(
        &self,
        reference: &SessionRef,
        observed_generation: i64,
        refresh: F,
    ) -> Result<Option<Session<T>>, StoreError>
    where
        T: Serialize + DeserializeOwned,
        F: FnOnce(T) -> Fut,
        Fut: Future<Output = Result<T, E>>,
    {
        let id = &reference.0;
        let Some(mut tx) = self.lock_session(id).await? else {
            return Ok(None);
        };
        let Some(row) = self.read_session(id).await? else {
            tx.commit().await?;
            return Ok(None);
        };
        if row.2 {
            invalidate_locked(&mut tx, id).await?;
            tx.commit().await?;
            return Ok(None);
        }
        if row.1 > observed_generation {
            let session = self.open_session(id, row)?;
            tx.commit().await?;
            return Ok(Some(session));
        }
        if row.1 < observed_generation {
            return Err(StoreError::Generation);
        }
        let grant = self.open_session::<T>(id, row)?.grant;

        // This commit precedes the network call. Rolling back the lock
        // transaction can never restore a possibly consumed refresh token.
        let marked = sqlx::query(
            "UPDATE service.oauth_sessions SET refreshing = true \
             WHERE id = $1 AND generation = $2 AND NOT refreshing AND NOT revoked AND expires_at > clock_timestamp()",
        )
        .bind(id.as_slice())
        .bind(observed_generation)
        .execute(&self.pool)
        .await?;
        if marked.rows_affected() != 1 {
            tx.commit().await?;
            return Ok(None);
        }
        let Ok(grant) = refresh(grant).await else {
            invalidate_locked(&mut tx, id).await?;
            tx.commit().await?;
            return Err(StoreError::Refresh);
        };
        let sealed = match self.seal(SESSION_PURPOSE, id, &grant) {
            Ok(sealed) => sealed,
            Err(error) => {
                invalidate_locked(&mut tx, id).await?;
                tx.commit().await?;
                return Err(error);
            }
        };
        let updated: Result<Option<(i64, bool)>, sqlx::Error> = sqlx::query_as(
            "UPDATE service.oauth_sessions SET sealed = $1, generation = generation + 1, refreshing = false \
             WHERE id = $2 AND generation = $3 AND refreshing AND expires_at > clock_timestamp() \
             RETURNING generation, revoked",
        )
        .bind(sealed)
        .bind(id.as_slice())
        .bind(observed_generation)
        .fetch_optional(&self.pool)
        .await;
        match updated {
            Ok(Some((generation, revoked))) => {
                tx.commit().await?;
                Ok((!revoked).then_some(Session { grant, generation }))
            }
            Ok(None) => {
                tx.commit().await?;
                Ok(None)
            }
            Err(error) => {
                // A lost database response is not evidence that UPDATE failed.
                // If it did commit, its newer generation is the only safe grant.
                if let Some(row) = self.read_session(id).await? {
                    if !row.2 && row.1 > observed_generation {
                        let session = self.open_session(id, row)?;
                        tx.commit().await?;
                        return Ok(Some(session));
                    }
                    invalidate_locked(&mut tx, id).await?;
                }
                tx.commit().await?;
                Err(StoreError::Database(error))
            }
        }
    }

    /// Revokes the browser session and its grant, including during refresh.
    /// Deletion commits before waiting for an in-flight refresh to finish.
    pub async fn revoke(&self, cookie: &str) -> Result<(), StoreError> {
        let Some(id) = cookie_id(cookie) else {
            return Ok(());
        };
        sqlx::query("DELETE FROM service.oauth_sessions WHERE id = $1")
            .bind(id.as_slice())
            .execute(&self.pool)
            .await?;
        if let Some(mut tx) = self.lock_session(&id).await? {
            invalidate_locked(&mut tx, &id).await?;
            tx.commit().await?;
        }
        Ok(())
    }

    /// Immediately revokes a session and takes its latest safe grant for
    /// remote revocation. A refresh already in progress finishes first; if
    /// it was abandoned, its possibly consumed grant is never returned. A
    /// caller may retry after a busy error to finish an earlier revocation.
    pub async fn take_session<T: DeserializeOwned>(&self, cookie: &str) -> Result<Option<Session<T>>, StoreError> {
        let Some(id) = cookie_id(cookie) else {
            return Ok(None);
        };
        let marked = sqlx::query("UPDATE service.oauth_sessions SET revoked = true WHERE id = $1")
            .bind(id.as_slice())
            .execute(&self.pool)
            .await?;
        if marked.rows_affected() != 1 {
            return Ok(None);
        }
        let Some(mut tx) = self.lock_session(&id).await? else {
            return Ok(None);
        };
        let row: Option<StoredSession> =
            sqlx::query_as("SELECT sealed, generation, refreshing FROM service.oauth_sessions WHERE id = $1")
                .bind(id.as_slice())
                .fetch_optional(&mut *tx)
                .await?;
        let session = match row {
            Some(row) if !row.2 => self.open_session(&id, row).map(Some),
            _ => Ok(None),
        };
        // Even corrupt ciphertext must not leave a usable local session.
        invalidate_locked(&mut tx, &id).await?;
        tx.commit().await?;
        session
    }

    /// Stores the entire pending login, bound to unpredictable OAuth state
    /// and a separate unpredictable browser cookie. Both secrets are hashed.
    pub async fn store_pending<T: Serialize + Sync>(
        &self,
        state: &str,
        browser_binding: &str,
        pending: &T,
        expires_at: DateTime<Utc>,
    ) -> Result<(), StoreError> {
        let id = hash(state);
        let binding = hash(browser_binding);
        let sealed = self.seal(PENDING_PURPOSE, &id, pending)?;
        sqlx::query(
            "INSERT INTO service.oauth_pending_logins (id, browser_binding, sealed, expires_at) \
             VALUES ($1, $2, $3, $4)",
        )
        .bind(id.as_slice())
        .bind(binding.as_slice())
        .bind(sealed)
        .bind(expires_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Atomically consumes a live pending login for its original browser.
    /// A wrong binding does not consume another browser's login.
    pub async fn consume_pending<T: DeserializeOwned>(
        &self,
        state: &str,
        browser_binding: &str,
    ) -> Result<Option<T>, StoreError> {
        let id = hash(state);
        let binding = hash(browser_binding);
        let row: Option<(Vec<u8>,)> = sqlx::query_as(
            "DELETE FROM service.oauth_pending_logins \
             WHERE id = $1 AND browser_binding = $2 AND expires_at > clock_timestamp() RETURNING sealed",
        )
        .bind(id.as_slice())
        .bind(binding.as_slice())
        .fetch_optional(&self.pool)
        .await?;
        row.map(|(sealed,)| self.open(PENDING_PURPOSE, &id, sealed)).transpose()
    }

    /// Removes expired sessions and pending logins and returns the number
    /// removed. Expiry is enforced by reads even when cleanup has not run.
    pub async fn expire(&self) -> Result<u64, StoreError> {
        // Delete sessions first; marker cleanup must never remove a live row's
        // coordinator, including one created by a concurrent transaction.
        let sessions = sqlx::query("DELETE FROM service.oauth_sessions WHERE expires_at <= clock_timestamp()")
            .execute(&self.pool)
            .await?
            .rows_affected();
        let pending = sqlx::query("DELETE FROM service.oauth_pending_logins WHERE expires_at <= clock_timestamp()")
            .execute(&self.pool)
            .await?
            .rows_affected();
        sqlx::query(
            "DELETE FROM service.oauth_session_locks AS l WHERE l.id IN ( \
             SELECT candidate.id FROM service.oauth_session_locks AS candidate \
             WHERE NOT EXISTS (SELECT 1 FROM service.oauth_sessions AS s WHERE s.id = candidate.id) \
             FOR UPDATE OF candidate SKIP LOCKED)",
        )
        .execute(&self.locks)
        .await?;
        Ok(sessions + pending)
    }

    async fn read_session(&self, id: &RowId) -> Result<Option<StoredSession>, StoreError> {
        Ok(sqlx::query_as(
            "SELECT sealed, generation, refreshing FROM service.oauth_sessions \
             WHERE id = $1 AND NOT revoked AND expires_at > clock_timestamp()",
        )
        .bind(id.as_slice())
        .fetch_optional(&self.pool)
        .await?)
    }

    async fn lock_session(&self, id: &RowId) -> Result<Option<Transaction<'static, Postgres>>, StoreError> {
        let waiter = SessionWaiter::register(&self.waiters, id)?;
        // Two-int advisory keys are disjoint from the bigint outbox namespace.
        let result = tokio::time::timeout(LOCK_TIMEOUT, async {
            let mut backoff = INITIAL_BACKOFF;
            loop {
                let mut tx = self.locks.begin().await?;
                let row: Option<(i64,)> =
                    sqlx::query_as("SELECT advisory_key FROM service.oauth_session_locks WHERE id = $1")
                        .bind(id.as_slice())
                        .fetch_optional(&mut *tx)
                        .await?;
                let Some((key,)) = row else {
                    tx.commit().await?;
                    return Ok(None);
                };
                let [a, b, c, d, e, f, g, h] = key.to_be_bytes();
                let (acquired,): (bool,) = sqlx::query_as("SELECT pg_try_advisory_xact_lock($1, $2)")
                    .bind(i32::from_be_bytes([a, b, c, d]))
                    .bind(i32::from_be_bytes([e, f, g, h]))
                    .fetch_one(&mut *tx)
                    .await?;
                if acquired {
                    // Migration 0002's row-lock comment predates the advisory
                    // lock cutover. Pin this exact marker without waiting so a
                    // deleted row cannot be recreated under a different key.
                    let row: Option<(i32,)> = sqlx::query_as(
                        "SELECT 1 FROM service.oauth_session_locks \
                         WHERE id = $1 AND advisory_key = $2 FOR KEY SHARE SKIP LOCKED",
                    )
                    .bind(id.as_slice())
                    .bind(key)
                    .fetch_optional(&mut *tx)
                    .await?;
                    if row.is_none() {
                        tx.commit().await?;
                        return Ok(None);
                    }
                    return Ok(Some(tx));
                }
                // Never occupy a connection while another session holder runs.
                tx.rollback().await?;
                tokio::time::sleep(backoff).await;
                backoff = (backoff * 2).min(MAX_BACKOFF);
            }
        })
        .await
        .map_err(|_| StoreError::Busy)?;
        drop(waiter);
        result
    }

    fn open_session<T: DeserializeOwned>(&self, id: &RowId, row: StoredSession) -> Result<Session<T>, StoreError> {
        Ok(Session {
            grant: self.open(SESSION_PURPOSE, id, row.0)?,
            generation: row.1,
        })
    }

    fn seal<T: Serialize>(&self, purpose: &[u8; 28], id: &RowId, value: &T) -> Result<Vec<u8>, StoreError> {
        let mut nonce = [0; NONCE_LEN];
        getrandom::fill(&mut nonce).map_err(|_| StoreError::Random)?;
        let aad = associated_data(purpose, id);
        let mut sealed = Vec::with_capacity(1 + NONCE_LEN + TAG_LEN + 128);
        sealed.push(FORMAT);
        sealed.extend_from_slice(&nonce);
        serde_json::to_writer(&mut sealed, value).map_err(|_| StoreError::Serialize)?;
        let plaintext = sealed.get_mut(1 + NONCE_LEN..).ok_or(StoreError::Unseal)?;
        let tag = self
            .cipher
            .encrypt_in_place_detached(&XNonce::from(nonce), &aad, plaintext)
            .map_err(|_| StoreError::Unseal)?;
        sealed.extend_from_slice(&tag);
        Ok(sealed)
    }

    fn open<T: DeserializeOwned>(&self, purpose: &[u8; 28], id: &RowId, mut sealed: Vec<u8>) -> Result<T, StoreError> {
        let (&format, rest) = sealed.split_first().ok_or(StoreError::Unseal)?;
        if format != FORMAT {
            return Err(StoreError::Unseal);
        }
        let (nonce, _) = rest.split_at_checked(NONCE_LEN).ok_or(StoreError::Unseal)?;
        let nonce = <[u8; NONCE_LEN]>::try_from(nonce).map_err(|_| StoreError::Unseal)?;
        let aad = associated_data(purpose, id);
        let body = sealed.get_mut(1 + NONCE_LEN..).ok_or(StoreError::Unseal)?;
        let tag_offset = body.len().checked_sub(TAG_LEN).ok_or(StoreError::Unseal)?;
        let (ciphertext, tag) = body.split_at_mut(tag_offset);
        let tag = <[u8; TAG_LEN]>::try_from(&*tag).map_err(|_| StoreError::Unseal)?;
        self.cipher
            .decrypt_in_place_detached(&XNonce::from(nonce), &aad, ciphertext, &Tag::from(tag))
            .map_err(|_| StoreError::Unseal)?;
        serde_json::from_slice(ciphertext).map_err(|_| StoreError::Unseal)
    }
}

// The guard is dropped on success, error, timeout and cancellation. Its
// synchronous mutex is held only for the count update, never across an await.
struct SessionWaiter<'a> {
    waiters: &'a Mutex<HashMap<RowId, usize>>,
    id: RowId,
}

impl<'a> SessionWaiter<'a> {
    fn register(waiters: &'a Mutex<HashMap<RowId, usize>>, id: &RowId) -> Result<Self, StoreError> {
        let mut counts = waiters.lock().map_err(|_| StoreError::Busy)?;
        let count = counts.entry(*id).or_default();
        if *count >= MAX_WAITERS_PER_SESSION {
            return Err(StoreError::Busy);
        }
        *count += 1;
        Ok(Self { waiters, id: *id })
    }
}

impl Drop for SessionWaiter<'_> {
    fn drop(&mut self) {
        let Ok(mut counts) = self.waiters.lock() else {
            return;
        };
        if let Entry::Occupied(mut entry) = counts.entry(self.id) {
            if *entry.get() == 1 {
                entry.remove();
            } else {
                *entry.get_mut() -= 1;
            }
        }
    }
}

async fn invalidate_locked(tx: &mut Transaction<'_, Postgres>, id: &RowId) -> Result<(), StoreError> {
    sqlx::query("DELETE FROM service.oauth_sessions WHERE id = $1")
        .bind(id.as_slice())
        .execute(&mut **tx)
        .await?;
    sqlx::query("DELETE FROM service.oauth_session_locks WHERE id = $1")
        .bind(id.as_slice())
        .execute(&mut **tx)
        .await?;
    Ok(())
}

fn hash(secret: &str) -> RowId {
    Sha256::digest(secret.as_bytes()).into()
}

fn cookie_id(cookie: &str) -> Option<RowId> {
    let mut decoded = [0; SECRET_LEN + 1];
    if cookie.len() != COOKIE_LEN || URL_SAFE_NO_PAD.decode_slice(cookie, &mut decoded).ok()? != SECRET_LEN {
        return None;
    }
    Some(hash(cookie))
}

fn associated_data(purpose: &[u8; 28], id: &RowId) -> [u8; 60] {
    let mut aad = [0; 60];
    let (purpose_slot, id_slot) = aad.split_at_mut(28);
    purpose_slot.copy_from_slice(purpose);
    id_slot.copy_from_slice(id);
    aad
}
