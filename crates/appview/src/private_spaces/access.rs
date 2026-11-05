//! Local reader leases; the authority and shared invalidation store remain future work.
//!
//! Callers serialize mutations and output, supply a monotonic serving clock, and discard
//! leases on clock discontinuity. These types do not authenticate identities or credentials.

use std::{fmt, sync::Arc};

use motet_spaces::{
    SpaceId,
    serving::{self, Decision, Verified},
};

/// One incarnation of an audience. A URI alone cannot distinguish recreation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpaceGeneration {
    pub space: SpaceId,
    pub generation: u64,
}

/// An in-flight authority check. Dropping it leaves the previous lease unchanged.
#[derive(Debug)]
pub struct Verification {
    instance: Arc<()>,
    invalidation: u64,
    started_at_millis: u64,
}

/// An authenticated authority result, not a network protocol or credential verifier.
#[derive(Clone, Copy, Debug)]
pub enum VerificationResult {
    Granted {
        expires_at_millis: u64,
    },
    Denied,
    /// A failed check cannot extend the previous permission age.
    Unavailable,
}

#[derive(Debug)]
struct Grant {
    started_at_millis: u64,
    expires_at_millis: u64,
}

/// Permission to attempt output, rechecked by [`PrivateResponse::release`].
#[derive(Debug)]
#[must_use]
pub struct Permit {
    instance: Arc<()>,
    invalidation: u64,
}

/// One reader's access to exactly one Space incarnation.
///
/// Own one lease per active reader/audience; the owner must bound that set. No lease
/// state is serializable. Restart and clock discontinuity require [`Self::restart`].
#[derive(Debug)]
pub struct ReaderLease {
    reader: String,
    audience: SpaceGeneration,
    // Strong ticket/permit references keep this allocation alive after lease replacement.
    // This makes pointer identity safe against allocator reuse; do not use Weak or raw pointers.
    instance: Arc<()>,
    invalidation: u64,
    grant: Option<Grant>,
}

impl ReaderLease {
    #[must_use]
    pub fn new(reader: String, audience: SpaceGeneration) -> Self {
        Self {
            reader,
            audience,
            instance: Arc::new(()),
            invalidation: 0,
            grant: None,
        }
    }

    /// Capture permission age before any authority round trips, regardless of their count.
    #[must_use]
    pub fn start_verification(&self, now_millis: u64) -> Verification {
        Verification {
            instance: Arc::clone(&self.instance),
            invalidation: self.invalidation,
            started_at_millis: now_millis,
        }
    }

    /// Install only a check from this lease's current invalidation generation.
    /// Authoritative denials invalidate even when their request started before another check.
    /// Keep the grant with the latest verification start; ties preserve the installed grant.
    #[expect(
        clippy::needless_pass_by_value,
        reason = "consume the ticket to prevent replaying one authority result"
    )]
    pub fn finish_verification(&mut self, check: Verification, result: VerificationResult) {
        if !Arc::ptr_eq(&self.instance, &check.instance) {
            return;
        }
        match result {
            VerificationResult::Denied => self.invalidate(),
            VerificationResult::Granted { expires_at_millis }
                if check.invalidation == self.invalidation
                    && self
                        .grant
                        .as_ref()
                        .is_none_or(|grant| check.started_at_millis > grant.started_at_millis) =>
            {
                self.grant = Some(Grant {
                    started_at_millis: check.started_at_millis,
                    expires_at_millis,
                });
            }
            VerificationResult::Granted { .. } | VerificationResult::Unavailable => {}
        }
    }

    /// Known removal wins over every check already in flight and every issued permit.
    pub fn invalidate(&mut self) {
        self.grant = None;
        if let Some(next) = self.invalidation.checked_add(1) {
            self.invalidation = next;
        } else {
            // Change identity instead of wrapping and accepting an ancient check.
            self.restart();
        }
    }

    /// Forget all authority, including outstanding pre-restart checks and permits.
    pub fn restart(&mut self) {
        self.instance = Arc::new(());
        self.invalidation = 0;
        self.grant = None;
    }

    /// Expiry is exclusive. Freshness and future-clock rejection belong to motet-spaces.
    #[must_use]
    pub fn decision(&self, now_millis: u64) -> Decision {
        let Some(grant) = &self.grant else {
            return Decision::Refuse;
        };
        if now_millis >= grant.expires_at_millis {
            // Expiry requires a fresh check, not an authoritative denial.
            return Decision::Stale;
        }
        serving::decide(
            Some(Verified {
                at_millis: grant.started_at_millis,
                revoked: false,
            }),
            now_millis,
            serving::DEFAULT_BOUND_MILLIS,
        )
    }

    /// `reader` must come from a live authenticated session, never a query DID or sponsor.
    /// The caller supplies the current authoritative generation and sponsor availability.
    #[must_use]
    pub fn permit(
        &self,
        reader: Option<&str>,
        audience: &SpaceGeneration,
        sponsor_available: bool,
        now_millis: u64,
    ) -> Option<Permit> {
        self.allows_output(reader, audience, sponsor_available, now_millis)
            .then(|| Permit {
                instance: Arc::clone(&self.instance),
                invalidation: self.invalidation,
            })
    }

    fn allows_output(
        &self,
        reader: Option<&str>,
        audience: &SpaceGeneration,
        sponsor_available: bool,
        now_millis: u64,
    ) -> bool {
        reader == Some(self.reader.as_str())
            && audience == &self.audience
            && sponsor_available
            && self.decision(now_millis) == Decision::Serve
    }
}

/// Audience-bound bytes with no unguarded content accessor.
pub struct PrivateResponse {
    audience: SpaceGeneration,
    content: Vec<u8>,
}

impl fmt::Debug for PrivateResponse {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Diagnostics must not reveal private prose.
        formatter
            .debug_struct("PrivateResponse")
            .field("audience", &self.audience)
            .finish_non_exhaustive()
    }
}

impl PrivateResponse {
    #[must_use]
    pub const fn new(audience: SpaceGeneration, content: Vec<u8>) -> Self {
        Self { audience, content }
    }

    /// Consume one response at the output boundary. Recheck after any queued delay.
    ///
    /// Call this for each output chunk while holding the owner's serialization boundary.
    /// A returned byte vector is delivered output; it cannot subsequently be recalled.
    #[must_use]
    pub fn release(
        self,
        lease: &ReaderLease,
        permit: &Permit,
        reader: Option<&str>,
        current_audience: &SpaceGeneration,
        sponsor_available: bool,
        now_millis: u64,
    ) -> Option<Vec<u8>> {
        if self.audience != *current_audience
            || !Arc::ptr_eq(&lease.instance, &permit.instance)
            || lease.invalidation != permit.invalidation
        {
            return None;
        }
        lease
            .allows_output(reader, current_audience, sponsor_available, now_millis)
            .then_some(self.content)
    }
}
