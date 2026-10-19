//! When a reader may still be served private content.
//!
//! Motet syncs a Space with a credential obtained through some member's
//! delegation; that says nothing about whether any particular reader may see
//! it. Each reader's access is verified on its own, and the verdict is used
//! only while it is fresh. If it can't be renewed within the bound, that
//! reader stops being served until it can. A revocation Motet knows about
//! takes effect immediately, whatever the bound.
//!
//! This covers every serving path: pages, cached pages, media, downloads,
//! search, the live stream's replay and queued notifications all ask
//! [`decide`] before handing anything over.

use serde::{Deserialize, Serialize};

/// Starting target for private content, from the backend plan.
pub const DEFAULT_BOUND_MILLIS: u64 = 60_000;

/// The last verification of one reader's access to one Space.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Verified {
    /// When the host last confirmed the reader may read, in milliseconds on
    /// the serving clock.
    pub at_millis: u64,
    /// Whether a revocation has been seen since. Sticky until a later
    /// verification says otherwise.
    pub revoked: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decision {
    Serve,
    /// Never verified, or revoked: don't serve.
    Refuse,
    /// The verification is too old: don't serve, and re-verify.
    Stale,
}

/// Whether to serve a reader now.
pub fn decide(verified: Option<Verified>, now_millis: u64, bound_millis: u64) -> Decision {
    match verified {
        None | Some(Verified { revoked: true, .. }) => Decision::Refuse,
        Some(Verified { at_millis, .. }) if now_millis.saturating_sub(at_millis) > bound_millis => Decision::Stale,
        // A verification from the future (clock trouble) is treated as fresh
        // only up to the bound, by the saturating subtraction above.
        Some(_) => Decision::Serve,
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    proptest! {
        /// Nothing is served after a known revocation, or longer than the
        /// bound after the last verification.
        #[test]
        fn serving_never_outlives_access(at in any::<u32>(), age in any::<u32>(), bound in 0..120_000u64, revoked: bool) {
            let (at, now) = (u64::from(at), u64::from(at) + u64::from(age));
            let decision = decide(Some(Verified { at_millis: at, revoked }), now, bound);
            if decision == Decision::Serve {
                prop_assert!(!revoked);
                prop_assert!(now - at <= bound);
            }
        }
    }

    #[test]
    fn fresh_verified_access_is_served_up_to_the_bound() {
        let v = Some(Verified {
            at_millis: 1_000,
            revoked: false,
        });
        assert_eq!(
            decide(v, 1_000 + DEFAULT_BOUND_MILLIS, DEFAULT_BOUND_MILLIS),
            Decision::Serve
        );
        assert_eq!(
            decide(v, 1_001 + DEFAULT_BOUND_MILLIS, DEFAULT_BOUND_MILLIS),
            Decision::Stale
        );
        assert_eq!(decide(None, 0, DEFAULT_BOUND_MILLIS), Decision::Refuse);
    }
}
