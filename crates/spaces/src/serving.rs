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
    /// The verification is too old or from the future: don't serve, and re-verify.
    Stale,
}

/// Whether to serve a reader now.
#[must_use]
pub fn decide(verified: Option<Verified>, now_millis: u64, bound_millis: u64) -> Decision {
    match verified {
        None | Some(Verified { revoked: true, .. }) => Decision::Refuse,
        // Future verification times cannot establish freshness on this clock.
        Some(Verified { at_millis, .. }) if at_millis > now_millis || now_millis - at_millis > bound_millis => {
            Decision::Stale
        }
        Some(_) => Decision::Serve,
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    fn clock_millis() -> impl Strategy<Value = u64> {
        prop_oneof![4 => any::<u64>(), 1 => Just(0), 1 => Just(1), 1 => Just(u64::MAX - 1), 1 => Just(u64::MAX)]
    }

    fn clock_cases() -> impl Strategy<Value = (u64, u64, u64)> {
        prop_oneof![
            4 => (clock_millis(), clock_millis(), clock_millis()),
            2 => (clock_millis(), clock_millis()).prop_map(|(at, now)| (at, now, now.abs_diff(at))),
            1 => (clock_millis(), clock_millis())
                .prop_map(|(at, now)| (at, now, now.abs_diff(at).saturating_sub(1))),
        ]
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(256))]

        /// Readers receive content exactly within a nonrevoked verification's clock window.
        /// Catches future timestamps accepted by saturating subtraction and exclusive deadlines.
        #[test]
        fn serving_never_outlives_access(
            (at, now, bound) in clock_cases(),
            revoked: bool,
            present: bool,
        ) {
            let verified = present.then_some(Verified { at_millis: at, revoked });
            let decision = decide(verified, now, bound);
            let now = u128::from(now);
            let at = u128::from(at);
            let deadline = at + u128::from(bound);
            let expected = if !present || revoked {
                Decision::Refuse
            } else if (at..=deadline).contains(&now) {
                Decision::Serve
            } else {
                Decision::Stale
            };
            prop_assert_eq!(decision, expected);
        }
    }

    /// A future verification cannot authorize a reader, even at the clock's extremes.
    /// Catches saturating subtraction turning a future timestamp into zero age.
    #[test]
    fn future_verification_never_authorizes_private_content() {
        for (at, now, bound) in [
            (u64::MAX, 0, 0),
            (u64::MAX, 0, u64::MAX),
            (1, 0, 0),
            (u64::MAX, u64::MAX - 1, 0),
        ] {
            assert_eq!(
                decide(
                    Some(Verified {
                        at_millis: at,
                        revoked: false
                    }),
                    now,
                    bound
                ),
                Decision::Stale,
                "future verification at={at}, now={now}, bound={bound}"
            );
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

#[cfg(kani)]
mod proofs {
    use super::{Decision, Verified, decide};

    // These harnesses call production decide with full-width symbolic clocks.
    // No assumptions exclude values or clock orderings. Widened sums cannot
    // overflow: two u64 values fit in u128. Kani also checks production arithmetic.

    /// Missing or revoked access never releases private content.
    /// Catches checking freshness before a known revocation.
    #[kani::proof]
    fn missing_or_revoked_access_is_refused() {
        let at_millis: u64 = kani::any();
        let now: u64 = kani::any();
        let bound: u64 = kani::any();
        assert_eq!(decide(None, now, bound), Decision::Refuse);
        assert_eq!(
            decide(
                Some(Verified {
                    at_millis,
                    revoked: true
                }),
                now,
                bound
            ),
            Decision::Refuse
        );
    }

    /// Access outside the verification's clock window requires renewal.
    /// Catches future timestamps treated as fresh and stale access served.
    #[kani::proof]
    fn future_or_expired_access_is_stale() {
        let at_millis: u64 = kani::any();
        let now: u64 = kani::any();
        let bound: u64 = kani::any();
        if u128::from(now) < u128::from(at_millis) || u128::from(now) > u128::from(at_millis) + u128::from(bound) {
            assert_eq!(
                decide(
                    Some(Verified {
                        at_millis,
                        revoked: false
                    }),
                    now,
                    bound
                ),
                Decision::Stale
            );
        }
    }

    /// Valid access is served through its inclusive deadline.
    /// Catches an exclusive comparison denying a reader at the freshness bound.
    #[kani::proof]
    fn fresh_access_including_deadline_is_served() {
        let at_millis: u64 = kani::any();
        let now: u64 = kani::any();
        let bound: u64 = kani::any();
        if u128::from(at_millis) <= u128::from(now) && u128::from(now) <= u128::from(at_millis) + u128::from(bound) {
            assert_eq!(
                decide(
                    Some(Verified {
                        at_millis,
                        revoked: false
                    }),
                    now,
                    bound
                ),
                Decision::Serve
            );
        }
    }

    /// Zero and maximum clocks and bounds preserve the reader's access window.
    /// Catches overflow in a deadline sum and zero-age acceptance of future clocks.
    #[kani::proof]
    fn clock_extremes_preserve_access_window() {
        let at_millis: u64 = kani::any();
        let now: u64 = kani::any();
        let bound: u64 = kani::any();
        let verified = Some(Verified {
            at_millis,
            revoked: false,
        });
        assert_eq!(
            decide(verified, now, u64::MAX),
            if at_millis <= now {
                Decision::Serve
            } else {
                Decision::Stale
            }
        );
        assert_eq!(
            decide(verified, now, 0),
            if at_millis == now {
                Decision::Serve
            } else {
                Decision::Stale
            }
        );
        assert_eq!(
            decide(verified, 0, bound),
            if at_millis == 0 {
                Decision::Serve
            } else {
                Decision::Stale
            }
        );
        assert_eq!(
            decide(verified, u64::MAX, bound),
            if u128::from(at_millis) + u128::from(bound) >= u128::from(u64::MAX) {
                Decision::Serve
            } else {
                Decision::Stale
            }
        );
    }
}
