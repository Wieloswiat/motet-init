//! The archive: every verified event, appended to a bucket before
//! PostgreSQL applies it.
//!
//! It is what makes projections disposable. "Rebuild gives the same
//! projections" means replaying this archive, with no network access, gives
//! the tables the live indexer built. It is trusted, not proof: Tap delivers
//! verified records as JSON, not signed commits, so the archive rebuilds a
//! deployment that trusts its own ingestion.
//!
//! One indexer writes the archive at a time; the log's fencing makes sure of
//! it. Segments are group-committed batches of events.

use std::sync::Arc;

use bytes::Bytes;
use motet_log::{AppendError, Log, ReadError, Reconciled, Seq, Writer};
use object_store::ObjectStore;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::event::Event;

/// The segment format this code writes and reads.
pub const FORMAT: u32 = 1;

#[derive(Serialize, Deserialize)]
struct Segment {
    format: u32,
    events: Vec<Event>,
}

#[derive(Debug, Error)]
pub enum ArchiveError {
    #[error("another indexer writes this archive (fenced at {0})")]
    Fenced(Seq),
    #[error("segment {0} may or may not be durable; restart the indexer")]
    Unresolved(Seq),
    #[error("the batch was not written")]
    NotWritten,
    #[error(transparent)]
    Read(#[from] ReadError),
    #[error("segment {seq} can't be read: {reason}")]
    Corrupt { seq: Seq, reason: String },
}

/// Read access to the archive, for rebuilds and catch-up.
#[derive(Clone, Debug)]
pub struct Archive {
    log: Log,
}

impl Archive {
    pub fn new(store: Arc<dyn ObjectStore>, prefix: &str) -> Self {
        Self {
            log: Log::new(store, prefix),
        }
    }

    /// Every segment from `from` on, decoded.
    pub async fn read_from(&self, from: Seq) -> Result<Vec<(Seq, Vec<Event>)>, ArchiveError> {
        self.log
            .read_from(from)
            .await?
            .into_iter()
            .map(|(seq, bytes)| Ok((seq, decode(seq, &bytes)?)))
            .collect()
    }

    /// Takes over as the archive's only writer.
    pub async fn writer(&self) -> Result<ArchiveWriter, ArchiveError> {
        Ok(ArchiveWriter {
            writer: self.log.writer().await?,
        })
    }
}

#[derive(Debug)]
pub struct ArchiveWriter {
    writer: Writer,
}

impl ArchiveWriter {
    /// Makes `events` durable as one segment and returns its position. Only
    /// then may they be applied and acknowledged to Tap.
    pub async fn append(&mut self, events: &[Event]) -> Result<Seq, ArchiveError> {
        #[derive(Serialize)]
        struct SegmentRef<'a> {
            format: u32,
            events: &'a [Event],
        }
        let bytes =
            Bytes::from(serde_json::to_vec(&SegmentRef { format: FORMAT, events }).expect("events always serialize"));
        let fail = |e| match e {
            AppendError::Fenced(seq) => ArchiveError::Fenced(seq),
            AppendError::Unknown { seq, .. } | AppendError::Unresolved(seq) => ArchiveError::Unresolved(seq),
        };
        match self.writer.append(bytes.clone()).await {
            Ok(seq) => Ok(seq),
            Err(AppendError::Unknown { .. }) => match self.writer.reconcile(&bytes).await.map_err(fail)? {
                Reconciled::Durable(seq) => Ok(seq),
                Reconciled::Absent => Err(ArchiveError::NotWritten),
            },
            Err(e) => Err(fail(e)),
        }
    }
}

fn decode(seq: Seq, bytes: &[u8]) -> Result<Vec<Event>, ArchiveError> {
    let segment: Segment = serde_json::from_slice(bytes).map_err(|e| ArchiveError::Corrupt {
        seq,
        reason: e.to_string(),
    })?;
    if segment.format != FORMAT {
        return Err(ArchiveError::Corrupt {
            seq,
            reason: format!("format {} is not {FORMAT}", segment.format),
        });
    }
    Ok(segment.events)
}
