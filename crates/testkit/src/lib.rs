//! Shared test machinery.
//!
//! Motet's tests check properties of whole behaviours (two consumers agree,
//! a rebuild gives the same projections, a retry never doubles a write) rather
//! than restating implementations. This crate holds what those tests share:
//!
//! - [`delivery`]: proptest strategies for how the network delivers records:
//!   in order within a repository, interleaved arbitrarily across them, with
//!   at-least-once redelivery.
//! - `services` (feature `services`): isolated PostgreSQL databases and
//!   object-storage prefixes for integration tests, from the services
//!   `devenv up` starts.
//!
//! Planned here as the backend grows: an in-process network of fake PDSes and
//! a fake PLC directory (atproto-crates' `atproto-testkit` has both, and is
//! the first thing to try), fault injection for object storage (latency,
//! ambiguous writes, lost acknowledgements), and a clock under test control.

pub mod delivery;

#[cfg(feature = "services")]
pub mod services;
