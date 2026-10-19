//! A storage shard: the fenced log of its Worlds' commands, and the state
//! those commands produce.
//!
//! Each World belongs to exactly one shard, and each shard has exactly one
//! writer, so every World has one command stream: "was the author in the cast
//! when this post was admitted?" has one answer. The log in the bucket is the
//! only write-ahead log. State in memory is what replaying it gives, and a
//! node that loses its memory replays.
//!
//! The order of a commit is the contract:
//!
//! 1. apply the batch to a staged copy of the Worlds it touches;
//! 2. append the batch to the log as one segment;
//! 3. only once that segment is durable, adopt the staged state and return
//!    the outcomes.
//!
//! So an outcome is never returned for anything that a replay wouldn't
//! reproduce. A write whose fate is unknown is reconciled by reading it back
//! (see `motet_log`), never by trying again.
//!
//! Group commit lives in [`spawn`]: requests queue while a segment is being
//! written and go out together in the next one.
//!
//! SlateDB (current state and replay checkpoints, so a restart needn't replay
//! from zero) and log compaction come later; neither changes this contract.

use std::collections::BTreeMap;
use std::sync::Arc;

use bytes::Bytes;
use motet_log::{AppendError, Log, ReadError, Reconciled, Seq, Writer};
use motet_rules::world::{Command, CommandError, Outcome, World};
use motet_rules::{Did, OperationId};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::{mpsc, oneshot};

/// The segment format this code writes and reads. Replaying a log written in
/// a format this code doesn't know is an error, never a guess.
pub const FORMAT: u32 = 1;

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
}

impl Entry {
    pub fn world(&self) -> &Did {
        match self {
            Self::Begin { world, .. } | Self::Apply { world, .. } => world,
        }
    }
}

/// What applying one entry produced.
pub type Applied = Result<Outcome, Rejected>;

/// An entry that changed nothing, recorded in the log like any other so a
/// replay reaches the same verdict.
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
    /// Another node writes this shard now. Stop serving it.
    #[error("fenced at segment {0}: another writer owns this shard")]
    Fenced(Seq),
    /// The batch was not written and nothing was applied; it may be submitted
    /// again.
    #[error("the batch was not written")]
    NotWritten,
    /// Whether the batch was written can't be established now. Nothing more is
    /// accepted; reopening the shard (which replays) settles it.
    #[error("segment {0} is unresolved; reopen the shard")]
    Unresolved(Seq),
    #[error(transparent)]
    Read(#[from] ReadError),
    #[error("segment {seq} can't be read: {reason}")]
    Corrupt { seq: Seq, reason: String },
}

#[derive(Serialize, Deserialize)]
struct Segment {
    format: u32,
    entries: Vec<Entry>,
}

/// A shard and its Worlds, owned by the one process writing it.
#[derive(Debug)]
pub struct Shard {
    writer: Writer,
    worlds: BTreeMap<Did, World>,
}

impl Shard {
    /// Replays the log and takes over as its writer.
    pub async fn open(log: Log) -> Result<Self, ShardError> {
        let writer = log.writer().await?;
        let mut worlds = BTreeMap::new();
        for (seq, bytes) in log.read_from(0).await? {
            if seq >= writer.next() {
                break; // written after this writer took over: it will be fenced there
            }
            for entry in decode(seq, &bytes)? {
                // Replay reaches the same verdicts; a rejection changes nothing.
                let _ = apply(&mut worlds, &entry);
            }
        }
        Ok(Self { writer, worlds })
    }

    pub fn world(&self, world: &Did) -> Option<&World> {
        self.worlds.get(world)
    }

    /// Applies `entries` in order and makes them durable as one segment. The
    /// outcomes are returned only once they are durable.
    pub async fn commit(&mut self, entries: &[Entry]) -> Result<Vec<Applied>, ShardError> {
        let mut staged: BTreeMap<Did, World> = BTreeMap::new();
        let results = entries
            .iter()
            .map(|entry| {
                let world = entry.world();
                if !staged.contains_key(world)
                    && let Some(current) = self.worlds.get(world)
                {
                    staged.insert(world.clone(), current.clone());
                }
                apply(&mut staged, entry)
            })
            .collect();

        let bytes = encode(entries);
        let durable = match self.writer.append(bytes.clone()).await {
            Ok(_) => true,
            Err(AppendError::Unknown { .. }) => match self.writer.reconcile(&bytes).await {
                Ok(Reconciled::Durable(_)) => true,
                Ok(Reconciled::Absent) => false,
                Err(e) => return Err(append_error(&e)),
            },
            Err(e) => return Err(append_error(&e)),
        };
        if !durable {
            return Err(ShardError::NotWritten);
        }
        self.worlds.extend(staged);
        Ok(results)
    }
}

fn apply(worlds: &mut BTreeMap<Did, World>, entry: &Entry) -> Applied {
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
    if segment.format != FORMAT {
        return Err(ShardError::Corrupt {
            seq,
            reason: format!("format {} is not {FORMAT}", segment.format),
        });
    }
    Ok(segment.entries)
}

/// A running shard, shared by the tasks that submit to it.
#[derive(Clone, Debug)]
pub struct Handle {
    tx: mpsc::Sender<Request>,
}

type Request = (Entry, oneshot::Sender<Result<Applied, Arc<ShardError>>>);

#[derive(Debug, Error)]
pub enum SubmitError {
    #[error(transparent)]
    Shard(Arc<ShardError>),
    #[error("the shard has stopped")]
    Stopped,
}

impl Handle {
    /// Submits one entry and waits until it is durable.
    pub async fn submit(&self, entry: Entry) -> Result<Applied, SubmitError> {
        let (tx, rx) = oneshot::channel();
        self.tx.send((entry, tx)).await.map_err(|_| SubmitError::Stopped)?;
        rx.await.map_err(|_| SubmitError::Stopped)?.map_err(SubmitError::Shard)
    }
}

/// Runs `shard` with group commit: whatever queued while one segment was
/// being written goes into the next, up to `max_batch` entries. Stops when
/// fenced or unresolved; a new writer then has to open the shard.
pub fn spawn(mut shard: Shard, max_batch: usize) -> Handle {
    let (tx, mut rx) = mpsc::channel::<Request>(max_batch * 4);
    tokio::spawn(async move {
        let mut batch = Vec::with_capacity(max_batch);
        while rx.recv_many(&mut batch, max_batch).await > 0 {
            let (entries, waiters): (Vec<Entry>, Vec<_>) = batch.drain(..).unzip();
            match shard.commit(&entries).await {
                Ok(results) => {
                    for (waiter, result) in waiters.into_iter().zip(results) {
                        let _ = waiter.send(Ok(result));
                    }
                }
                Err(e) => {
                    let stop = !matches!(e, ShardError::NotWritten);
                    let e = Arc::new(e);
                    for waiter in waiters {
                        let _ = waiter.send(Err(e.clone()));
                    }
                    if stop {
                        tracing::error!(error = %e, "shard stopped");
                        break;
                    }
                }
            }
        }
    });
    Handle { tx }
}
