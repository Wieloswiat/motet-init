//! Motet's AppView.
//!
//! Motet runs one AppView; it doesn't need to be self-hostable, and nothing
//! about the data depends on it. Any authorized AppView reading the same
//! records, admissions included, derives an equivalent World.
//!
//! - [`event`] and [`archive`]: what arrives from sync, appended to a bucket
//!   before anything else happens to it.
//! - [`projection`]: the PostgreSQL tables pages are built from, rebuildable
//!   from the archive alone.
//! - [`indexer`]: archive, apply, acknowledge.
//! - [`outbox`]: writes made on people's behalf, leased and retried safely.
//! - [`api`]: the XRPC methods, each declared with who may call it.
//!
//! Still to come, each behind its own module: OAuth sign-in as a confidential
//! client (sessions sealed outside the database, one refresh at a time per
//! session), the Tap and Spaces inputs to the indexer, server-sent events
//! per reader (`LISTEN/NOTIFY` only as a wake-up), search with ParadeDB, and
//! the render and image workers.

pub mod api;
pub mod archive;
pub mod config;
pub mod event;
pub mod indexer;
pub mod outbox;
pub mod projection;

/// Service-state migrations (`migrations/`). Projections are not migrated;
/// see [`projection`].
pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!();
