//! An append-only log in an object-storage bucket.
//!
//! Both of Motet's services keep what must last this way, after vlpds: the
//! bucket is the only durable store and every node is replaceable.
//!
//! - A tenor group-commits each shard's World commands into its log and
//!   acknowledges a command only once the segment holding it is durable.
//!   SlateDB will hold current state and replay checkpoints on top (see
//!   `docs/adr/0006-storage.md`); the log stays the only write-ahead log.
//! - The AppView appends every verified record to its archive before
//!   PostgreSQL applies it, so projections rebuild from the bucket without
//!   re-crawling the network.
//!
//! **Fencing is the key space.** Segment `n` is written with a create-only
//! PUT. Two processes that both think they own a shard race for the same key;
//! exactly one succeeds and the other learns it was fenced, so no two writers
//! can ever acknowledge different contents for the same position. There is no
//! lease to expire and no clock involved.
//!
//! **Ambiguity is explicit.** A PUT that fails with anything but "already
//! exists" may or may not have happened. [`Writer::append`] returns
//! [`AppendError::Unknown`], and the only way forward is
//! [`Writer::reconcile`], which reads the key back. A write is never retried
//! blindly.

use std::sync::Arc;

use bytes::Bytes;
use futures::TryStreamExt;
use object_store::path::Path;
use object_store::{ObjectStore, ObjectStoreExt, PutMode, PutOptions, PutPayload};
use thiserror::Error;

/// A position in the log. Segments are numbered from zero without gaps.
pub type Seq = u64;

#[derive(Debug, Error)]
pub enum AppendError {
    /// Another writer holds this position. This writer must stop: it no longer
    /// owns the log.
    #[error("fenced: segment {0} was written by another writer")]
    Fenced(Seq),
    /// The write may or may not be durable. Call [`Writer::reconcile`].
    #[error("segment {seq} may or may not have been written: {source}")]
    Unknown {
        seq: Seq,
        #[source]
        source: object_store::Error,
    },
    /// The writer is waiting on a reconciliation and accepts nothing else.
    #[error("segment {0} is unresolved; reconcile before appending")]
    Unresolved(Seq),
}

#[derive(Debug, Error)]
pub enum ReadError {
    #[error(transparent)]
    Store(#[from] object_store::Error),
    #[error("segment {0} is missing: the log has a gap")]
    Gap(Seq),
}

/// A log under one prefix of a bucket.
#[derive(Clone, Debug)]
pub struct Log {
    store: Arc<dyn ObjectStore>,
    prefix: Path,
}

impl Log {
    pub fn new(store: Arc<dyn ObjectStore>, prefix: impl Into<Path>) -> Self {
        Self {
            store,
            prefix: prefix.into(),
        }
    }

    fn key(&self, seq: Seq) -> Path {
        // Zero-padded so lexicographic listing is numeric order.
        self.prefix.clone().join(format!("{seq:020}"))
    }

    /// Positions present in the bucket, in order.
    async fn positions(&self) -> Result<Vec<Seq>, ReadError> {
        let mut seqs: Vec<Seq> = self
            .store
            .list(Some(&self.prefix))
            .map_ok(|meta| meta.location.filename().and_then(|f| f.parse::<Seq>().ok()))
            .try_collect::<Vec<_>>()
            .await?
            .into_iter()
            .flatten()
            .collect();
        seqs.sort_unstable();
        Ok(seqs)
    }

    /// Takes over the log: the writer appends after the last durable segment.
    /// Taking over never fences anyone by itself; an older writer is fenced the
    /// moment both try the same position.
    pub async fn writer(&self) -> Result<Writer, ReadError> {
        let next = self.positions().await?.last().map_or(0, |s| s + 1);
        Ok(Writer {
            log: self.clone(),
            next,
            unresolved: None,
        })
    }

    /// Every segment from `from` on, in order. A missing segment in the middle
    /// is an error, never silently skipped.
    pub async fn read_from(&self, from: Seq) -> Result<Vec<(Seq, Bytes)>, ReadError> {
        let mut out = Vec::new();
        let mut expected = from;
        for seq in self.positions().await?.into_iter().filter(|s| *s >= from) {
            if seq != expected {
                return Err(ReadError::Gap(expected));
            }
            let bytes = self.store.get(&self.key(seq)).await?.bytes().await?;
            out.push((seq, bytes));
            // Nothing can follow the last representable position, so stop instead of
            // wrapping the cursor back to zero.
            expected = match expected.checked_add(1) {
                Some(next) => next,
                None => break,
            };
        }
        Ok(out)
    }
}

/// The single writer of a log, until another writer fences it.
#[derive(Debug)]
pub struct Writer {
    log: Log,
    next: Seq,
    unresolved: Option<Seq>,
}

/// The outcome of reconciling an ambiguous append.
#[derive(Debug, PartialEq, Eq)]
pub enum Reconciled {
    /// It was written: acknowledge it.
    Durable(Seq),
    /// It was not written: the batch may be appended again.
    Absent,
}

impl Writer {
    /// The position the next append will take.
    #[must_use]
    pub fn next(&self) -> Seq {
        self.next
    }

    /// Makes `batch` durable as the next segment. Only an `Ok` may be
    /// acknowledged to anyone.
    pub async fn append(&mut self, batch: Bytes) -> Result<Seq, AppendError> {
        if let Some(seq) = self.unresolved {
            return Err(AppendError::Unresolved(seq));
        }
        let seq = self.next;
        let options = PutOptions {
            mode: PutMode::Create,
            ..PutOptions::default()
        };
        match self
            .log
            .store
            .put_opts(&self.log.key(seq), PutPayload::from(batch), options)
            .await
        {
            Ok(_) => {
                self.next += 1;
                Ok(seq)
            }
            Err(object_store::Error::AlreadyExists { .. }) => Err(AppendError::Fenced(seq)),
            Err(source) => {
                self.unresolved = Some(seq);
                Err(AppendError::Unknown { seq, source })
            }
        }
    }

    /// Resolves an [`AppendError::Unknown`] by reading the position back.
    /// Different contents there mean another writer took it: fenced.
    pub async fn reconcile(&mut self, batch: &Bytes) -> Result<Reconciled, AppendError> {
        let Some(seq) = self.unresolved else {
            return Ok(Reconciled::Absent);
        };
        match self.log.store.get(&self.log.key(seq)).await {
            Ok(found) => {
                let found = found
                    .bytes()
                    .await
                    .map_err(|source| AppendError::Unknown { seq, source })?;
                if found == *batch {
                    self.unresolved = None;
                    self.next = seq + 1;
                    Ok(Reconciled::Durable(seq))
                } else {
                    Err(AppendError::Fenced(seq))
                }
            }
            Err(object_store::Error::NotFound { .. }) => {
                self.unresolved = None;
                Ok(Reconciled::Absent)
            }
            Err(source) => Err(AppendError::Unknown { seq, source }),
        }
    }
}
