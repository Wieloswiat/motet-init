//! Motet's seam over atproto Spaces.
//!
//! Spaces are alpha, and their Rust crates aren't released, so Motet keeps
//! one seam between them and everything else. The tenor uses the host side
//! (credentials, writer lists, write notifications, revocation) and the
//! AppView the client side (credentials, syncing members' space repos). Both
//! go through this crate, so a breaking change upstream is fixed here once.
//!
//! What is settled, and lives here already:
//!
//! - [`audience`]: a credential reads a whole Space, so every audience is its
//!   own Space: the GMs, each private scene's cast, each application under
//!   review.
//! - [`serving`]: a Space credential lets Motet sync a Space, not serve it to
//!   everyone. Each reader's access is checked separately, with a freshness
//!   bound, and a known revocation takes effect at once.
//!
//! What comes with the Groundwork spike: [`SpaceReader`]'s implementation over
//! `atproto-space-client`, and the host side over `atproto-space`.

pub mod audience;
pub mod serving;

use std::future::Future;

pub use audience::{Audience, SpaceId};

/// One record in one member's space repo.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpaceRecord {
    pub space: SpaceId,
    pub writer: String,
    pub collection: String,
    pub rkey: String,
    pub cid: String,
    pub json: String,
}

#[derive(Debug, thiserror::Error)]
pub enum SyncError {
    /// No usable credential: the last member delegation lapsed, or access
    /// was revoked. The Space stops being served until one is renewed.
    #[error("no usable credential for {0}")]
    NoCredential(SpaceId),
    /// The writer's PDS couldn't be read. Not a deletion: the last verified
    /// state stays, marked stale.
    #[error("{writer}'s repo in {space} couldn't be read: {reason}")]
    Unreachable {
        space: SpaceId,
        writer: String,
        reason: String,
    },
}

/// The client side: what the AppView (and a tenor, for admission) reads.
pub trait SpaceReader: Send + Sync {
    /// Everyone who can write to `space` now, per its host.
    fn writers(&self, space: &SpaceId) -> impl Future<Output = Result<Vec<String>, SyncError>> + Send;

    /// `writer`'s records in `space`, verified against their space repo's
    /// commit. Reconciliation calls this for former writers too: write
    /// notifications are for speed, not completeness.
    fn records(
        &self,
        space: &SpaceId,
        writer: &str,
    ) -> impl Future<Output = Result<Vec<SpaceRecord>, SyncError>> + Send;
}
