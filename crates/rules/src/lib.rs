//! The World's authority model.
//!
//! A post becomes part of a World when the World's tenor *admits* it: a record
//! written as the World that pins the post's exact version and gives it a
//! place in its scene. This crate holds the two halves of that rule as pure
//! functions:
//!
//! - [`world`]: what the tenor decides. A [`world::World`] applies one
//!   [`world::Command`] at a time, in the World's single command stream, and
//!   returns an [`world::Outcome`] to persist and publish.
//! - [`view`]: what every consumer derives. A [`view::WorldView`] folds the
//!   records it has received, from members' repos and the World's, into what
//!   a scene shows. It never re-runs the tenor's rules: it follows admissions.
//!
//! Both run in the tenor and in the AppView (which uses [`world`] to warn an
//! author before sending a post the tenor would refuse), so they live here,
//! away from any transport, storage or SDK type. See
//! `docs/architecture/overview.md` for where this sits.
//!
//! The record shapes and refusal reasons are provisional: the exact admission
//! state machine is a Groundwork decision. What is not provisional are the
//! invariants the tests pin down:
//!
//! - consumers fed the same records in any cross-repo order agree on what each
//!   scene shows (`tests/view_convergence.rs`);
//! - retrying an operation never admits twice, and reusing its ID for a
//!   different request is refused (`tests/world_properties.rs`);
//! - an edit keeps its post's place, and a stale edit is a conflict;
//! - a removal governs everything the tenor commits after it.

pub mod id;
pub mod view;
pub mod world;

pub use id::{Cid, Did, OperationId, RecordUri, SceneId, StrongRef};
