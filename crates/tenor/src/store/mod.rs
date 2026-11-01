//! A shard's current state: one SlateDB database, written only by the
//! shard's writer (see `shard`), read by everyone else through [`Reader`].
//!
//! SlateDB's own write-ahead log is off. The shard's log is the only one
//! (docs/adr/0014-tenor-state-in-slatedb.md): a batch reaches the database
//! only after its log segment is durable, together with the marker saying
//! which segment it was, so the database always holds a whole prefix of the
//! log and replay starts after the marker. The database can lag the log after
//! a crash, never lead it.

pub mod keys;

use std::sync::Arc;
use std::time::Duration;

use atproto_dasl::{BlockStorage, Cid, StorageError};
use bytes::Bytes;
use motet_rules::Did;
use object_store::ObjectStore;
use slatedb::{Db, Settings};

use crate::keys::{KeyError, MasterKey, Purpose};

/// The database a shard keeps its state in: `state/{shard}/` beside the
/// shard's log, through the same object-store client so conditional writes
/// behave the same for both.
pub async fn open(store: Arc<dyn ObjectStore>, shard: &str) -> Result<Db, slatedb::Error> {
    Db::builder(format!("state/{shard}"), store)
        .with_settings(settings())
        .build()
        .await
}

fn settings() -> Settings {
    Settings {
        wal_enabled: false,
        // The shard flushes at its own checkpoints; a timer flush would only
        // add PUTs between them.
        flush_interval: None,
        // How soon a fenced writer notices when it isn't writing: its writes
        // notice at once. vlpds uses the same.
        manifest_poll_interval: Duration::from_secs(10),
        ..Settings::default()
    }
}

/// Why the database couldn't be read.
#[derive(Debug, thiserror::Error)]
pub enum ReadError {
    #[error(transparent)]
    Db(#[from] slatedb::Error),
    #[error(transparent)]
    Key(#[from] KeyError),
}

/// Committed state, for everything outside the shard's writer: sync
/// endpoints, queries, credential minting. It never sees an operation that
/// hasn't been acknowledged.
// Not derived: SlateDB's `Db` has no `Debug`.
#[derive(Clone)]
pub struct Reader {
    db: Arc<Db>,
    master: Arc<MasterKey>,
}

impl std::fmt::Debug for Reader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Reader").finish_non_exhaustive()
    }
}

impl Reader {
    pub(crate) fn new(db: Arc<Db>, master: Arc<MasterKey>) -> Self {
        Self { db, master }
    }

    pub async fn get(&self, key: &[u8]) -> Result<Option<Bytes>, ReadError> {
        Ok(self.db.get(key).await?)
    }

    /// Up to `limit` entries under `prefix`, in key order, starting after
    /// `after` (a full key) when given.
    pub async fn scan(
        &self,
        prefix: &[u8],
        after: Option<&[u8]>,
        limit: usize,
    ) -> Result<Vec<(Bytes, Bytes)>, ReadError> {
        let mut iter = self.db.scan_prefix(prefix, ..).await?;
        if let Some(after) = after {
            iter.seek(after).await?;
        }
        let mut out = Vec::new();
        while out.len() < limit {
            let Some(kv) = iter.next().await? else { break };
            if after.is_some_and(|a| kv.key.as_ref() == a) {
                continue;
            }
            out.push((kv.key, kv.value));
        }
        Ok(out)
    }

    /// A World's key, unsealed.
    pub async fn key(
        &self,
        world: &Did,
        purpose: Purpose,
    ) -> Result<Option<atproto_identity::key::KeyData>, ReadError> {
        let Some(sealed) = self.get(&keys::sealed_key(world, purpose)).await? else {
            return Ok(None);
        };
        Ok(Some(self.master.unseal(world, purpose, &sealed)?))
    }

    pub(crate) fn unseal_secret(&self, context: &[u8], sealed: &[u8]) -> Result<Vec<u8>, KeyError> {
        self.master.unseal_secret(context, sealed)
    }

    /// A World's committed repo blocks, read-only.
    #[must_use]
    pub fn blocks(&self, world: &Did) -> ReadBlocks {
        ReadBlocks {
            db: Arc::clone(&self.db),
            world: world.clone(),
        }
    }
}

/// A World's committed repo blocks as atproto-dasl block storage, for reading
/// a repo (MST walks, CAR export). Writing through it is refused: blocks are
/// written only by operations on the shard's writer.
#[derive(Clone)]
pub struct ReadBlocks {
    db: Arc<Db>,
    world: Did,
}

impl std::fmt::Debug for ReadBlocks {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReadBlocks")
            .field("world", &self.world)
            .finish_non_exhaustive()
    }
}

fn storage_error(e: &slatedb::Error) -> StorageError {
    StorageError::Io(std::io::Error::other(e.to_string()))
}

fn read_only() -> StorageError {
    StorageError::Io(std::io::Error::new(
        std::io::ErrorKind::PermissionDenied,
        "committed blocks are read-only; write through the shard",
    ))
}

impl BlockStorage for ReadBlocks {
    fn put(&mut self, _cid: &Cid, _data: Vec<u8>) -> impl Future<Output = Result<(), StorageError>> + Send {
        std::future::ready(Err(read_only()))
    }

    async fn get(&self, cid: &Cid) -> Result<Option<Vec<u8>>, StorageError> {
        let key = keys::block(&self.world, &cid.to_bytes());
        let value = self.db.get(&key).await.map_err(|e| storage_error(&e))?;
        Ok(value.map(|v| v.to_vec()))
    }

    async fn contains(&self, cid: &Cid) -> Result<bool, StorageError> {
        Ok(self.get(cid).await?.is_some())
    }

    fn remove(&mut self, _cid: &Cid) -> impl Future<Output = Result<Option<Vec<u8>>, StorageError>> + Send {
        std::future::ready(Err(read_only()))
    }

    fn memory_usage(&self) -> usize {
        0
    }

    fn block_count(&self) -> usize {
        0
    }

    fn cids(&self) -> Box<dyn Iterator<Item = Cid> + '_> {
        // Listing every block needs an async scan; nothing that reads a
        // committed repo walks storage this way (CAR export walks from the
        // commit instead).
        Box::new(std::iter::empty())
    }

    fn clear(&mut self) -> impl Future<Output = Result<(), StorageError>> + Send {
        std::future::ready(Err(read_only()))
    }
}
