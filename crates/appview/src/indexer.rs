//! The indexer: archive first, then apply, then acknowledge.
//!
//! Tap delivers at least once, per repository in order. Each batch is made
//! durable in the archive before PostgreSQL applies it, and acknowledged to
//! Tap only after both. A crash between the two is repaired on start by
//! applying whatever the archive holds past the last applied segment, so the
//! projections never hold anything the archive doesn't.
//!
//! Public admission matching and public meta Space snapshots are projected
//! after archiving. Lexicon validation remains a separate future stage.

use motet_log::Seq;
use sqlx::PgPool;

use crate::archive::{Archive, ArchiveError, ArchiveWriter};
use crate::event::Event;
use crate::projection::{self, ProjectionError};

#[derive(Debug, thiserror::Error)]
pub enum IndexError {
    #[error(transparent)]
    Archive(#[from] ArchiveError),
    #[error(transparent)]
    Projection(#[from] ProjectionError),
    #[error(transparent)]
    Db(#[from] sqlx::Error),
    #[error("the indexer stopped after a previous incomplete batch")]
    Poisoned,
}

/// The one process indexing into this deployment.
#[derive(Debug)]
pub struct Indexer {
    archive: ArchiveWriter,
    pool: PgPool,
    poisoned: bool,
}

impl Indexer {
    /// Takes over the archive, then brings projections up to date with it.
    /// Taking over first means nothing can be appended while catching up.
    pub async fn start(pool: PgPool, archive: &Archive) -> Result<Self, IndexError> {
        let writer = archive.writer().await?;
        projection::ensure(&pool, archive).await?;
        Ok(Self {
            archive: writer,
            pool,
            poisoned: false,
        })
    }

    /// Archives and applies one batch. When this returns, the batch may be
    /// acknowledged to Tap.
    pub async fn ingest(&mut self, events: &[Event]) -> Result<Seq, IndexError> {
        if self.poisoned {
            return Err(IndexError::Poisoned);
        }
        // Cancellation or an error after append may leave a durable segment
        // unapplied. Never advance the watermark past it in this process.
        self.poisoned = true;
        let seq = self.archive.append(events).await?;
        let mut conn = self.pool.acquire().await?;
        projection::apply(&mut conn, projection::LIVE, seq, events).await?;
        self.poisoned = false;
        Ok(seq)
    }
}
