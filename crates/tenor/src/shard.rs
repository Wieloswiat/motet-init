//! A storage shard: the one writer of its Worlds, the fenced log of what it
//! decided, and the database holding the state that log produces.
//!
//! Each World belongs to exactly one shard, and each shard has exactly one
//! writer, so every World has one command stream: "was the author in the cast
//! when this post was admitted?" has one answer, and a World's repo has one
//! head. Everything that changes state is an [`Op`] run on that writer.
//!
//! The order of a commit is the contract (docs/adr/0014):
//!
//! 1. run the batch's operations in order against a staged overlay; each sees
//!    what the ones before it did, and one that fails leaves nothing;
//! 2. append what they did to the log as one segment;
//! 3. only once that segment is durable, write it to the database in one
//!    batch with the marker naming the segment;
//! 4. adopt the staged state and answer every waiter.
//!
//! So an outcome is never returned for anything a replay wouldn't reproduce.
//! A write whose fate is unknown is reconciled by reading it back (see
//! `motet_log`), never by trying again.
//!
//! The log records two kinds of entry. Rules commands are deterministic, so
//! they're logged as commands and replayed by re-running them; the rules
//! state lives in memory and is snapshotted into the database at checkpoints.
//! Everything else (keys, repo commits, Spaces) is logged as the
//! key-value mutations and firehose events it produced, and replayed by
//! re-applying them: signing and revisions aren't functions of a command.
//!
//! The database's sequence numbers come from the log, so a writer that lost
//! the shard can't write an older segment over a newer one: segment `s` is
//! written as `2s + 1`, a checkpoint after it as `2s + 2`.

use std::collections::{BTreeMap, BTreeSet};
use std::ops::Bound;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use atproto_dasl::{BlockStorage, Cid, StorageError};
use atproto_identity::key::KeyData;
use bytes::Bytes;
use futures::future::BoxFuture;
use motet_log::{AppendError, Log, Reconciled, Seq, Writer};
use motet_rules::world::{Command, CommandError, Outcome, World};
use motet_rules::{Did, OperationId};
use object_store::ObjectStore;
use serde::{Deserialize, Serialize};
use slatedb::config::{FlushOptions, FlushType, WriteOptions};
use slatedb::{CloseReason, Db, ErrorKind, WriteBatch};
use thiserror::Error;
use tokio::sync::{broadcast, mpsc, oneshot};

use crate::event::{Event, Sequenced};
use crate::keys::{self, KeyError, MasterKey, Purpose};
use crate::store::{self, Reader};

/// The segment format this code writes. It reads this one and format 1
/// (rules entries only). Replaying a format it doesn't know is an error,
/// never a guess.
pub const FORMAT: u32 = 2;
const READABLE_FORMATS: [u32; 2] = [1, 2];

/// One entry in a shard's log.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Entry {
    /// A World starts on this shard, in the given epoch.
    Begin { world: Did, epoch: u32 },
    /// One command in a World's stream.
    Apply {
        world: Did,
        op: OperationId,
        command: Command,
    },
    /// What a host operation did to the database, and the events it emitted.
    Write(Write),
}

impl Entry {
    /// The World a rules entry is for.
    #[must_use]
    pub fn world(&self) -> Option<&Did> {
        match self {
            Self::Begin { world, .. } | Self::Apply { world, .. } => Some(world),
            Self::Write(_) => None,
        }
    }
}

/// A host operation's effects, as the log keeps them.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Write {
    pub mutations: Vec<Mutation>,
    pub events: Vec<Event>,
}

/// One key set or deleted.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mutation {
    #[serde(with = "crate::b64")]
    pub key: Bytes,
    /// `None` deletes the key.
    #[serde(with = "crate::b64::option")]
    pub value: Option<Bytes>,
}

/// What applying one rules entry produced.
pub type Applied = Result<Outcome, Rejected>;

/// A rules entry that changed nothing, recorded in the log like any other so
/// a replay reaches the same verdict.
#[derive(Clone, Debug, PartialEq, Eq, Error, Serialize, Deserialize)]
pub enum Rejected {
    #[error("{0} is not hosted on this shard")]
    UnknownWorld(Did),
    #[error("{0} already began")]
    AlreadyBegun(Did),
    #[error("operation {0} was already used for a different command")]
    OperationReused(OperationId),
}

#[derive(Debug, Error)]
pub enum ShardError {
    /// Another node writes this shard's log now. Stop serving it.
    #[error("fenced at segment {0}: another writer owns this shard")]
    Fenced(Seq),
    /// Another node opened this shard's database. Stop serving it.
    #[error("fenced: another writer opened this shard's state")]
    StateFenced,
    /// The batch was not written and nothing was applied; it may be submitted
    /// again.
    #[error("the batch was not written")]
    NotWritten,
    /// Whether the batch was written can't be established now. Nothing more is
    /// accepted; reopening the shard (which replays) settles it.
    #[error("segment {0} is unresolved; reopen the shard")]
    Unresolved(Seq),
    #[error(transparent)]
    Read(#[from] motet_log::ReadError),
    #[error("segment {seq} can't be read: {reason}")]
    Corrupt { seq: Seq, reason: String },
    /// The database failed. If it failed after a segment was made durable,
    /// the log and the database disagree until the shard is reopened, so the
    /// shard stops.
    #[error("the shard's state failed: {0}")]
    State(slatedb::Error),
    #[error("the shard's state holds something unreadable at {key}: {reason}")]
    BadState { key: String, reason: String },
    #[error(transparent)]
    Publication(#[from] crate::publication::QueueError),
}

impl From<slatedb::Error> for ShardError {
    fn from(e: slatedb::Error) -> Self {
        if e.kind() == ErrorKind::Closed(CloseReason::Fenced) {
            Self::StateFenced
        } else {
            Self::State(e)
        }
    }
}

/// Why an operation couldn't read or write through its [`Txn`].
#[derive(Debug, Error)]
pub enum TxnError {
    #[error(transparent)]
    Db(#[from] slatedb::Error),
    #[error(transparent)]
    Key(#[from] KeyError),
}

/// Something the shard runs on its one writer, against staged state.
///
/// An operation that returns `Err` leaves no effects: its writes, events and
/// rules entries are dropped. Network I/O doesn't belong in `run`, which
/// holds the writer: fetch first, then run an operation with the result.
pub trait Op: Send + 'static {
    type Output: Send + 'static;
    type Error: Send + 'static;
    fn run(self, txn: &mut Txn<'_>) -> impl Future<Output = Result<Self::Output, Self::Error>> + Send;
}

/// Reclaim up to `limit` expired service-token markers on an idle shard.
///
/// The observed expiry watermark commits even when no markers are removed,
/// so a clock rollback cannot revive a previously unused expired token.
#[derive(Debug)]
pub struct SweepServiceTokens {
    pub now: SystemTime,
    pub limit: usize,
}

impl Op for SweepServiceTokens {
    type Output = usize;
    type Error = crate::spaces::SpaceError;

    async fn run(self, txn: &mut Txn<'_>) -> Result<usize, Self::Error> {
        crate::authorization::sweep_service_tokens(txn, self.now, self.limit).await
    }
}

/// Rules entries capture their publication effects before becoming durable.
/// Replay applies the captured writes instead of resolving an audience again.
impl Op for Entry {
    type Output = Applied;
    type Error = crate::publication::QueueError;

    async fn run(self, txn: &mut Txn<'_>) -> Result<Applied, Self::Error> {
        let applied = match self {
            Self::Begin { world, epoch } => txn.begin_world(world, epoch),
            Self::Apply { world, op, command } => txn.command(world, op, command).await?,
            Self::Write(write) => {
                for Mutation { key, value } in write.mutations {
                    match value {
                        Some(value) => txn.put(key.to_vec(), value),
                        None => txn.delete(key.to_vec()),
                    }
                }
                for event in write.events {
                    txn.emit(event);
                }
                Ok(Outcome::Unchanged)
            }
        };
        Ok(applied)
    }
}

/// Writes of the operations already run in this batch.
#[derive(Default)]
struct Staged {
    writes: BTreeMap<Vec<u8>, Option<Bytes>>,
    worlds: BTreeMap<Did, World>,
    entries: Vec<Entry>,
}

/// Writes of the operation running now, kept apart until it succeeds.
#[derive(Default)]
struct Scope {
    writes: BTreeMap<Vec<u8>, Option<Bytes>>,
    worlds: BTreeMap<Did, World>,
    entries: Vec<Entry>,
    events: Vec<Event>,
}

impl Staged {
    fn adopt(&mut self, scope: Scope) {
        self.entries.extend(scope.entries);
        if !scope.writes.is_empty() || !scope.events.is_empty() {
            let mutations = scope
                .writes
                .iter()
                .map(|(key, value)| Mutation {
                    key: Bytes::copy_from_slice(key),
                    value: value.clone(),
                })
                .collect();
            self.entries.push(Entry::Write(Write {
                mutations,
                events: scope.events,
            }));
        }
        self.writes.extend(scope.writes);
        self.worlds.extend(scope.worlds);
    }
}

/// An operation's view of the shard: the database, overlaid with what the
/// batch has done so far and what this operation has done.
pub struct Txn<'a> {
    db: &'a Db,
    master: &'a MasterKey,
    base: &'a BTreeMap<Did, World>,
    batch: &'a Staged,
    scope: Scope,
}

impl std::fmt::Debug for Txn<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Txn")
            .field("writes", &self.scope.writes.len())
            .field("events", &self.scope.events.len())
            .finish_non_exhaustive()
    }
}

impl Txn<'_> {
    fn staged(&self, key: &[u8]) -> Option<&Option<Bytes>> {
        self.scope.writes.get(key).or_else(|| self.batch.writes.get(key))
    }

    pub async fn get(&self, key: &[u8]) -> Result<Option<Bytes>, TxnError> {
        if let Some(value) = self.staged(key) {
            return Ok(value.clone());
        }
        Ok(self.db.get(key).await?)
    }

    /// Up to `limit` entries under `prefix`, in key order, as this operation
    /// sees them.
    pub async fn scan(&self, prefix: &[u8], limit: usize) -> Result<Vec<(Bytes, Bytes)>, TxnError> {
        let in_prefix = |map: &'_ BTreeMap<Vec<u8>, Option<Bytes>>| {
            map.range::<[u8], _>((Bound::Included(prefix), Bound::Unbounded))
                .take_while(|(k, _)| k.starts_with(prefix))
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect::<Vec<_>>()
        };
        // The overlay: the batch's writes, then this operation's over them.
        let mut overlay: BTreeMap<Vec<u8>, Option<Bytes>> = in_prefix(&self.batch.writes).into_iter().collect();
        overlay.extend(in_prefix(&self.scope.writes));
        let mut overlay = overlay.into_iter().peekable();

        let mut db = self.db.scan_prefix(prefix, ..).await?;
        let mut pending = db.next().await?;
        let mut out = Vec::new();
        loop {
            if out.len() >= limit {
                break;
            }
            let from_overlay = match (overlay.peek(), &pending) {
                (None, None) => break,
                (Some(_), None) => true,
                (None, Some(_)) => false,
                (Some((ok, _)), Some(kv)) => ok.as_slice() <= kv.key.as_ref(),
            };
            if from_overlay && let Some((key, value)) = overlay.next() {
                if pending.as_ref().is_some_and(|kv| kv.key.as_ref() == key.as_slice()) {
                    pending = db.next().await?; // the overlay replaces it
                }
                if let Some(value) = value {
                    out.push((Bytes::from(key), value));
                }
            } else if let Some(kv) = pending.take() {
                out.push((kv.key, kv.value));
                pending = db.next().await?;
            }
        }
        Ok(out)
    }

    pub fn put(&mut self, key: Vec<u8>, value: Bytes) {
        self.scope.writes.insert(key, Some(value));
    }

    pub fn delete(&mut self, key: Vec<u8>) {
        self.scope.writes.insert(key, None);
    }

    /// Emits a firehose event. It is numbered when the batch commits.
    pub fn emit(&mut self, event: Event) {
        self.scope.events.push(event);
    }

    /// A World's rules state as this operation sees it.
    #[must_use]
    pub fn world(&self, world: &Did) -> Option<&World> {
        self.scope
            .worlds
            .get(world)
            .or_else(|| self.batch.worlds.get(world))
            .or_else(|| self.base.get(world))
    }

    /// Operation IDs used by admission are unique to an author across Worlds.
    pub(crate) fn operation_in_other_world(&self, world: &Did, op: &OperationId) -> bool {
        self.scope
            .worlds
            .iter()
            .chain(&self.batch.worlds)
            .chain(self.base)
            .any(|(did, state)| did != world && state.has_operation(op))
    }

    fn rules(&mut self, entry: Entry) -> Applied {
        if let Some(world) = entry.world()
            && !self.scope.worlds.contains_key(world)
            && let Some(current) = self.batch.worlds.get(world).or_else(|| self.base.get(world))
        {
            self.scope.worlds.insert(world.clone(), current.clone());
        }
        let applied = apply_rules(&mut self.scope.worlds, &entry);
        self.scope.entries.push(entry);
        applied
    }

    /// Starts a World's command stream on this shard.
    pub fn begin_world(&mut self, world: Did, epoch: u32) -> Applied {
        self.rules(Entry::Begin { world, epoch })
    }

    /// Applies one command and captures publication in the same log segment.
    /// Propagate a queue error from the enclosing operation to abort its effects.
    pub async fn command(
        &mut self,
        world: Did,
        op: OperationId,
        command: Command,
    ) -> Result<Applied, crate::publication::QueueError> {
        let applied = self.rules(Entry::Apply {
            world: world.clone(),
            op: op.clone(),
            command,
        });
        if let Ok(outcome) = &applied {
            crate::publication::enqueue(self, &world, &op, outcome).await?;
        }
        Ok(applied)
    }

    /// Generates a World key, seals it and stores it. Returns the private
    /// key; never log it.
    pub fn generate_key(&mut self, world: &Did, purpose: Purpose) -> Result<KeyData, KeyError> {
        let key = keys::generate(purpose)?;
        let sealed = self.master.seal(world, purpose, &key)?;
        self.put(store::keys::sealed_key(world, purpose), Bytes::from(sealed));
        Ok(key)
    }

    /// A World's key, unsealed, as this operation sees it.
    pub async fn key(&self, world: &Did, purpose: Purpose) -> Result<Option<KeyData>, TxnError> {
        let Some(sealed) = self.get(&store::keys::sealed_key(world, purpose)).await? else {
            return Ok(None);
        };
        Ok(Some(self.master.unseal(world, purpose, &sealed)?))
    }

    /// Generates a key before the owner's signature determines the World DID.
    /// Its ciphertext is bound to this reservation, not to a provisional DID.
    pub fn generate_reserved_key(&mut self, reservation: &str, purpose: Purpose) -> Result<KeyData, KeyError> {
        let key = keys::generate(purpose)?;
        let sealed = self.master.seal_reserved(reservation, purpose, &key)?;
        self.put(store::keys::reserved_key(reservation, purpose), Bytes::from(sealed));
        Ok(key)
    }

    /// Reseals a reservation's key for the newly established DID, then removes
    /// the reservation copy. Copying ciphertext would retain the wrong binding.
    pub async fn adopt_reserved_key(
        &mut self,
        reservation: &str,
        world: &Did,
        purpose: Purpose,
    ) -> Result<bool, TxnError> {
        let source = store::keys::reserved_key(reservation, purpose);
        let Some(sealed) = self.get(&source).await? else {
            return Ok(false);
        };
        let key = self.master.unseal_reserved(reservation, purpose, &sealed)?;
        let sealed = self.master.seal(world, purpose, &key)?;
        self.put(store::keys::sealed_key(world, purpose), Bytes::from(sealed));
        self.delete(source);
        Ok(true)
    }

    /// A World's repo blocks as this operation sees them. Blocks put into it
    /// are only collected; hand them to [`Txn::put_blocks`] when done.
    #[must_use]
    pub fn blocks(&self, world: &Did) -> TxnBlocks<'_> {
        TxnBlocks {
            txn: self,
            world: world.clone(),
            new: BTreeMap::new(),
        }
    }

    /// Stores blocks collected by [`Txn::blocks`].
    pub fn put_blocks(&mut self, world: &Did, blocks: impl IntoIterator<Item = (Cid, Vec<u8>)>) {
        for (cid, data) in blocks {
            self.put(store::keys::block(world, &cid.to_bytes()), Bytes::from(data));
        }
    }
}

/// A World's blocks for an MST run inside an operation: reads see the
/// overlay, writes are collected for [`Txn::put_blocks`].
pub struct TxnBlocks<'t> {
    txn: &'t Txn<'t>,
    world: Did,
    new: BTreeMap<Vec<u8>, (Cid, Vec<u8>)>,
}

impl std::fmt::Debug for TxnBlocks<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TxnBlocks")
            .field("world", &self.world)
            .field("new", &self.new.len())
            .finish_non_exhaustive()
    }
}

impl TxnBlocks<'_> {
    /// The blocks written through this storage.
    #[must_use]
    pub fn into_new(self) -> Vec<(Cid, Vec<u8>)> {
        self.new.into_values().collect()
    }
}

fn storage_error(e: &TxnError) -> StorageError {
    StorageError::Io(std::io::Error::other(e.to_string()))
}

impl BlockStorage for TxnBlocks<'_> {
    fn put(&mut self, cid: &Cid, data: Vec<u8>) -> impl Future<Output = Result<(), StorageError>> + Send {
        self.new.insert(cid.to_bytes(), (*cid, data));
        std::future::ready(Ok(()))
    }

    async fn get(&self, cid: &Cid) -> Result<Option<Vec<u8>>, StorageError> {
        let bytes = cid.to_bytes();
        if let Some((_, data)) = self.new.get(&bytes) {
            return Ok(Some(data.clone()));
        }
        let key = store::keys::block(&self.world, &bytes);
        let value = self.txn.get(&key).await.map_err(|e| storage_error(&e))?;
        Ok(value.map(|v| v.to_vec()))
    }

    async fn contains(&self, cid: &Cid) -> Result<bool, StorageError> {
        Ok(self.get(cid).await?.is_some())
    }

    fn remove(&mut self, cid: &Cid) -> impl Future<Output = Result<Option<Vec<u8>>, StorageError>> + Send {
        // Blocks of earlier commits stay: older revisions are served from
        // them until garbage collection, which isn't built.
        std::future::ready(Ok(self.new.remove(&cid.to_bytes()).map(|(_, data)| data)))
    }

    fn memory_usage(&self) -> usize {
        self.new.values().map(|(_, data)| data.len()).sum()
    }

    fn block_count(&self) -> usize {
        self.new.len()
    }

    fn cids(&self) -> Box<dyn Iterator<Item = Cid> + '_> {
        Box::new(self.new.values().map(|(cid, _)| *cid))
    }

    fn clear(&mut self) -> impl Future<Output = Result<(), StorageError>> + Send {
        self.new.clear();
        std::future::ready(Ok(()))
    }
}

#[derive(Serialize, Deserialize)]
struct Segment {
    format: u32,
    entries: Vec<Entry>,
}

/// A shard and its Worlds, owned by the one process writing it.
// Not derived: SlateDB's `Db` has no `Debug`.
pub struct Shard {
    writer: Writer,
    db: Arc<Db>,
    master: Arc<MasterKey>,
    worlds: BTreeMap<Did, World>,
    /// The last segment written to the database.
    applied: Option<Seq>,
    /// The segment the rules snapshots in the database were taken at.
    rules_at: Option<Seq>,
    /// Worlds whose rules changed since their last snapshot.
    dirty: BTreeSet<Did>,
    next_event: u64,
    events: broadcast::Sender<Arc<Sequenced>>,
}

impl std::fmt::Debug for Shard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Shard")
            .field("writer", &self.writer)
            .field("worlds", &self.worlds.len())
            .field("applied", &self.applied)
            .field("rules_at", &self.rules_at)
            .field("next_event", &self.next_event)
            .finish_non_exhaustive()
    }
}

fn u64_value(key: &[u8], value: Option<&Bytes>) -> Result<Option<u64>, ShardError> {
    value
        .map(|v| {
            <[u8; 8]>::try_from(v.as_ref())
                .map(u64::from_be_bytes)
                .map_err(|_| ShardError::BadState {
                    key: key.escape_ascii().to_string(),
                    reason: "not a u64".into(),
                })
        })
        .transpose()
}

fn be(n: u64) -> Bytes {
    Bytes::copy_from_slice(&n.to_be_bytes())
}

/// How many live events a slow subscriber may fall behind before it must
/// catch up from the stored events instead.
const LIVE_EVENTS: usize = 4096;

impl Shard {
    /// Takes over the shard: its log first, then its database (which fences
    /// any earlier writer of it), then replays what the database is missing.
    pub async fn open(store: Arc<dyn ObjectStore>, name: &str, master: Arc<MasterKey>) -> Result<Self, ShardError> {
        let log = Log::new(Arc::clone(&store), name);
        let writer = log.writer().await?;
        let db = Arc::new(store::open(store, name).await?);

        let applied = u64_value(store::keys::APPLIED, db.get(store::keys::APPLIED).await?.as_ref())?;
        let rules_at = u64_value(store::keys::RULES_AT, db.get(store::keys::RULES_AT).await?.as_ref())?;
        let next_event =
            u64_value(store::keys::EVENT_SEQ, db.get(store::keys::EVENT_SEQ).await?.as_ref())?.unwrap_or(0);

        let mut worlds = BTreeMap::new();
        let mut iter = db.scan_prefix(store::keys::WORLDS, ..).await?;
        while let Some(kv) = iter.next().await? {
            let Some(did) = store::keys::rules_world(&kv.key) else {
                continue;
            };
            let world: World = serde_json::from_slice(&kv.value).map_err(|e| ShardError::BadState {
                key: kv.key.escape_ascii().to_string(),
                reason: e.to_string(),
            })?;
            worlds.insert(Did::new(did), world);
        }

        let (events, _) = broadcast::channel(LIVE_EVENTS);
        let mut shard = Self {
            writer,
            db,
            master,
            worlds,
            applied,
            rules_at,
            dirty: BTreeSet::new(),
            next_event,
            events,
        };

        let from = rules_at.map_or(0, |s| s + 1);
        for (seq, bytes) in log.read_from(from).await? {
            if seq >= shard.writer.next() {
                break; // written after this writer took over: it will be fenced there
            }
            let entries = decode(seq, &bytes)?;
            // The database already holds segments up to `applied`; only their
            // rules need re-running.
            let in_db = applied.is_some_and(|a| seq <= a);
            let mut staged = Staged::default();
            for entry in entries {
                if in_db && matches!(entry, Entry::Write(_)) {
                    continue;
                }
                let mut txn = shard.txn(&staged);
                // Audience and sequence allocation are effects, not rules.
                // Replaying them against today's scene state would change history.
                match entry {
                    Entry::Begin { .. } | Entry::Apply { .. } => {
                        let _ = txn.rules(entry);
                    }
                    Entry::Write(_) => {
                        let _ = Box::pin(entry.run(&mut txn)).await?;
                    }
                }
                let scope = txn.scope;
                staged.adopt(scope);
            }
            if in_db {
                shard.dirty.extend(staged.worlds.keys().cloned());
                shard.worlds.extend(staged.worlds);
            } else {
                shard.write_segment(seq, staged).await?;
            }
        }
        Ok(shard)
    }

    fn txn<'a>(&'a self, batch: &'a Staged) -> Txn<'a> {
        Txn {
            db: &self.db,
            master: &self.master,
            base: &self.worlds,
            batch,
            scope: Scope::default(),
        }
    }

    #[must_use]
    pub fn world(&self, world: &Did) -> Option<&World> {
        self.worlds.get(world)
    }

    #[must_use]
    pub fn reader(&self) -> Reader {
        Reader::new(Arc::clone(&self.db), Arc::clone(&self.master))
    }

    /// Runs `entries` as one batch and makes them durable as one segment. The
    /// outcomes are returned only once they are durable.
    ///
    /// # Panics
    ///
    /// Never in practice: every job of a batch is answered before the batch
    /// returns, so each receiver has its answer.
    pub async fn commit(&mut self, entries: &[Entry]) -> Result<Vec<Applied>, Arc<ShardError>> {
        let mut receivers = Vec::with_capacity(entries.len());
        let jobs: Vec<Box<dyn Job>> = entries
            .iter()
            .map(|entry| {
                let (tx, rx) = oneshot::channel();
                receivers.push(rx);
                Box::new(Pending { op: entry.clone(), tx }) as Box<dyn Job>
            })
            .collect();
        self.run(jobs).await?;
        let mut out = Vec::with_capacity(receivers.len());
        for rx in receivers {
            match rx.await.expect("run answers every job before returning") {
                Ok(Ok(applied)) => out.push(applied),
                Ok(Err(error)) => return Err(Arc::new(ShardError::Publication(error))),
                Err(e) => return Err(e),
            }
        }
        Ok(out)
    }

    /// Runs one batch of jobs: stage, append, write, adopt, answer.
    async fn run(&mut self, jobs: impl IntoIterator<Item = Box<dyn Job>>) -> Result<(), Arc<ShardError>> {
        let mut staged = Staged::default();
        let mut replies = Vec::new();
        for job in jobs {
            let mut txn = self.txn(&staged);
            let (keep, reply) = job.run(&mut txn).await;
            let scope = txn.scope;
            if keep {
                staged.adopt(scope);
            }
            replies.push(reply);
        }
        if staged.entries.is_empty() {
            // Nothing to make durable: every operation failed or only read.
            for reply in replies {
                reply(Ok(()));
            }
            return Ok(());
        }

        match self.append_and_write(staged).await {
            Ok(events) => {
                for reply in replies {
                    reply(Ok(()));
                }
                for event in events {
                    // No subscribers is fine: they catch up from storage.
                    let _ = self.events.send(event);
                }
                Ok(())
            }
            Err(e) => {
                let e = Arc::new(e);
                for reply in replies {
                    reply(Err(Arc::clone(&e)));
                }
                Err(e)
            }
        }
    }

    async fn append_and_write(&mut self, staged: Staged) -> Result<Vec<Arc<Sequenced>>, ShardError> {
        let bytes = encode(&staged.entries);
        let seq = match self.writer.append(bytes.clone()).await {
            Ok(seq) => seq,
            Err(AppendError::Unknown { .. }) => match self.writer.reconcile(&bytes).await {
                Ok(Reconciled::Durable(seq)) => seq,
                Ok(Reconciled::Absent) => return Err(ShardError::NotWritten),
                Err(e) => return Err(append_error(&e)),
            },
            Err(e) => return Err(append_error(&e)),
        };
        self.write_segment(seq, staged).await
    }

    /// Writes a durable segment's effects to the database with the marker
    /// naming it, then adopts its rules state.
    async fn write_segment(&mut self, seq: Seq, staged: Staged) -> Result<Vec<Arc<Sequenced>>, ShardError> {
        let mut batch = WriteBatch::new();
        let mut events = Vec::new();
        for entry in &staged.entries {
            let Entry::Write(write) = entry else { continue };
            for Mutation { key, value } in &write.mutations {
                match value {
                    Some(value) => batch.put(key, value),
                    None => batch.delete(key),
                }
            }
            for event in &write.events {
                let sequenced = Sequenced {
                    seq: self.next_event,
                    event: event.clone(),
                };
                let stored = serde_json::to_vec(&sequenced).expect("events always serialize");
                batch.put(store::keys::event(self.next_event), stored);
                events.push(Arc::new(sequenced));
                self.next_event += 1;
            }
        }
        batch.put(store::keys::EVENT_SEQ, be(self.next_event));
        batch.put(store::keys::APPLIED, be(seq));
        self.db
            .write_with_options(batch, &WriteOptions { seqnum: db_seq(seq) })
            .await?;
        self.applied = Some(seq);
        self.dirty.extend(staged.worlds.keys().cloned());
        self.worlds.extend(staged.worlds);
        Ok(events)
    }

    /// Snapshots the rules of Worlds changed since the last checkpoint and
    /// flushes the database, so a takeover replays only what came after.
    ///
    /// # Panics
    ///
    /// If a World's rules state can't be serialized, which its types rule out.
    pub async fn checkpoint(&mut self) -> Result<(), ShardError> {
        let Some(applied) = self.applied else { return Ok(()) };
        if self.rules_at == Some(applied) {
            return Ok(());
        }
        let mut batch = WriteBatch::new();
        for did in std::mem::take(&mut self.dirty) {
            if let Some(world) = self.worlds.get(&did) {
                let snapshot = serde_json::to_vec(world).expect("rules state always serializes");
                batch.put(store::keys::world_rules(&did), snapshot);
            }
        }
        batch.put(store::keys::RULES_AT, be(applied));
        self.db
            .write_with_options(
                batch,
                &WriteOptions {
                    seqnum: db_seq(applied) + 1,
                },
            )
            .await?;
        self.db
            .flush_with_options(FlushOptions {
                flush_type: FlushType::MemTable,
            })
            .await?;
        self.rules_at = Some(applied);
        Ok(())
    }

    /// Closes the database cleanly, flushing what it holds.
    pub async fn close(self) -> Result<(), ShardError> {
        Ok(self.db.close().await?)
    }
}

/// Segment `s` is the database's sequence number `2s + 1`; a checkpoint
/// after it is `2s + 2`. Both strictly increase with the log.
fn db_seq(seq: Seq) -> u64 {
    seq.checked_mul(2)
        .and_then(|s| s.checked_add(1))
        .expect("log positions stay far below u64::MAX / 2")
}

fn apply_rules(worlds: &mut BTreeMap<Did, World>, entry: &Entry) -> Applied {
    match entry {
        Entry::Begin { world, epoch } => {
            if worlds.contains_key(world) {
                return Err(Rejected::AlreadyBegun(world.clone()));
            }
            worlds.insert(world.clone(), World::new(*epoch));
            Ok(Outcome::Unchanged)
        }
        Entry::Apply { world, op, command } => {
            let state = worlds
                .get_mut(world)
                .ok_or_else(|| Rejected::UnknownWorld(world.clone()))?;
            state.apply(op.clone(), command.clone()).map_err(|e| match e {
                CommandError::OperationReused(op) => Rejected::OperationReused(op),
            })
        }
        Entry::Write(_) => Ok(Outcome::Unchanged),
    }
}

fn append_error(e: &AppendError) -> ShardError {
    match *e {
        AppendError::Fenced(seq) => ShardError::Fenced(seq),
        AppendError::Unknown { seq, .. } | AppendError::Unresolved(seq) => ShardError::Unresolved(seq),
    }
}

fn encode(entries: &[Entry]) -> Bytes {
    #[derive(Serialize)]
    struct SegmentRef<'a> {
        format: u32,
        entries: &'a [Entry],
    }
    let segment = SegmentRef {
        format: FORMAT,
        entries,
    };
    Bytes::from(serde_json::to_vec(&segment).expect("entries always serialize"))
}

fn decode(seq: Seq, bytes: &[u8]) -> Result<Vec<Entry>, ShardError> {
    let segment: Segment = serde_json::from_slice(bytes).map_err(|e| ShardError::Corrupt {
        seq,
        reason: e.to_string(),
    })?;
    if !READABLE_FORMATS.contains(&segment.format) {
        return Err(ShardError::Corrupt {
            seq,
            reason: format!("format {} is not one of {READABLE_FORMATS:?}", segment.format),
        });
    }
    Ok(segment.entries)
}

type Reply = Box<dyn FnOnce(Result<(), Arc<ShardError>>) + Send>;

/// An operation waiting in the queue, with whoever waits for it.
trait Job: Send {
    /// Runs the operation; returns whether its effects are kept, and what to
    /// tell its waiter once the batch's fate is known.
    fn run<'t>(self: Box<Self>, txn: &'t mut Txn<'_>) -> BoxFuture<'t, (bool, Reply)>;
}

type Answer<O> = Result<Result<<O as Op>::Output, <O as Op>::Error>, Arc<ShardError>>;

struct Pending<O: Op> {
    op: O,
    tx: oneshot::Sender<Answer<O>>,
}

impl<O: Op> Job for Pending<O> {
    fn run<'t>(self: Box<Self>, txn: &'t mut Txn<'_>) -> BoxFuture<'t, (bool, Reply)> {
        Box::pin(async move {
            let Self { op, tx } = *self;
            let result = op.run(txn).await;
            let keep = result.is_ok();
            let reply: Reply = Box::new(move |batch| {
                let _ = tx.send(batch.map(|()| result));
            });
            (keep, reply)
        })
    }
}

/// A running shard, shared by the tasks that submit to it.
#[derive(Clone, Debug)]
pub struct Handle {
    tx: mpsc::Sender<Box<dyn Job>>,
    reader: Reader,
    events: broadcast::Sender<Arc<Sequenced>>,
}

impl std::fmt::Debug for dyn Job {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Job")
    }
}

#[derive(Debug, Error)]
pub enum SubmitError {
    #[error(transparent)]
    Shard(Arc<ShardError>),
    #[error("the shard has stopped")]
    Stopped,
    #[error(transparent)]
    Publication(#[from] crate::publication::QueueError),
}

impl Handle {
    /// Runs an operation on the shard's writer and waits until its effects
    /// are durable. The outer error is the shard's; the inner one is the
    /// operation's own.
    pub async fn run<O: Op>(&self, op: O) -> Result<Result<O::Output, O::Error>, SubmitError> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(Box::new(Pending { op, tx }))
            .await
            .map_err(|_| SubmitError::Stopped)?;
        rx.await.map_err(|_| SubmitError::Stopped)?.map_err(SubmitError::Shard)
    }

    /// Submits one rules entry and waits until it is durable.
    pub async fn submit(&self, entry: Entry) -> Result<Applied, SubmitError> {
        self.run(entry).await?.map_err(SubmitError::Publication)
    }

    /// Committed state, for reading outside the writer.
    #[must_use]
    pub fn reader(&self) -> &Reader {
        &self.reader
    }

    /// Events as they are committed. A subscriber that falls more than
    /// `LIVE_EVENTS` (4096) behind gets `Lagged` and catches up from storage.
    #[must_use]
    pub fn subscribe(&self) -> broadcast::Receiver<Arc<Sequenced>> {
        self.events.subscribe()
    }
}

/// How a running shard batches and checkpoints.
#[derive(Clone, Copy, Debug)]
pub struct Options {
    /// Operations per group commit.
    pub max_batch: usize,
    /// How often to checkpoint when something changed.
    pub checkpoint_every: Duration,
}

/// Runs `shard` with group commit and periodic checkpoints.
///
/// Whatever queued while one segment was being written goes into the next,
/// up to `max_batch` operations, and the shard checkpoints every
/// `checkpoint_every` when anything changed. It stops when fenced or when log
/// and database may disagree; a new writer then has to open the shard.
#[must_use]
pub fn spawn(mut shard: Shard, options: Options) -> Handle {
    let (tx, mut rx) = mpsc::channel::<Box<dyn Job>>(options.max_batch * 4);
    let handle = Handle {
        tx,
        reader: shard.reader(),
        events: shard.events.clone(),
    };
    tokio::spawn(async move {
        let mut checkpoints = tokio::time::interval(options.checkpoint_every);
        checkpoints.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        let mut batch = Vec::with_capacity(options.max_batch);
        loop {
            let result = tokio::select! {
                received = rx.recv_many(&mut batch, options.max_batch) => {
                    if received == 0 {
                        break; // every handle dropped
                    }
                    #[expect(
                        clippy::iter_with_drain,
                        reason = "recv_many reuses the batch allocation on the next iteration"
                    )]
                    let jobs = batch.drain(..);
                    shard.run(jobs).await
                }
                _ = checkpoints.tick() => shard.checkpoint().await.map_err(Arc::new),
            };
            if let Err(e) = result
                && !matches!(*e, ShardError::NotWritten)
            {
                tracing::error!(error = %e, "shard stopped");
                return;
            }
        }
        if let Err(e) = shard.checkpoint().await {
            tracing::error!(error = %e, "final checkpoint failed; the next writer replays");
        }
        if let Err(e) = shard.close().await {
            tracing::error!(error = %e, "closing the shard's state failed");
        }
    });
    handle
}
