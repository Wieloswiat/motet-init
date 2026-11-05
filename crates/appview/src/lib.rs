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
//! - [`tap`]: verified record and identity input, acknowledged after indexing.
//! - [`outbox`]: writes made on people's behalf, leased and retried safely.
//! - [`auth`]: confidential OAuth, sealed sessions and `DPoP` PDS access.
//! - [`api`]: the XRPC methods, each declared with who may call it.
//!
//! `serve` takes over the archive and catches projections up before serving.
//! With `MOTET_APPVIEW_TAP_URL`, it also consumes Tap in batches of at most
//! 100 deliveries or 25 milliseconds. HTTPS enables TLS; the optional
//! `MOTET_APPVIEW_TAP_ADMIN_PASSWORD` authenticates the connection. Shutdown
//! finishes the current batch before flushing acknowledgements. Without a
//! Tap URL, the API runs without live ingestion.
//!
//! Still to come, each behind its own module: private Spaces input,
//! server-sent events per reader (`LISTEN/NOTIFY` only as a wake-up),
//! search with ParadeDB, and the render and image workers.

pub mod api;
pub mod archive;
pub mod auth;
pub mod config;
pub mod event;
pub mod indexer;
pub mod outbox;
pub mod private_spaces;
pub mod projection;
pub mod public_space;
pub mod tap;
pub mod writes;

/// Service-state migrations (`migrations/`). Projections are not migrated;
/// see [`projection`].
pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!();
