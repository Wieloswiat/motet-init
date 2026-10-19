//! The tenor: Motet's group host.
//!
//! A tenor hosts each World as a group account, doing for the World what a
//! PDS does for a person: its identity, its public repo and its Spaces. GMs
//! act as the World through it with their own accounts. It admits posts,
//! issues dice and runs the World's moderation. Motet runs managed tenors and
//! anyone can run their own; the AppView treats both the same.
//!
//! - [`shard`]: the fenced, group-committed log of each World's commands in
//!   object storage, and the state replaying it gives. Everything the tenor
//!   promises rests on this: acknowledged means durable.
//! - [`admission`]: reading a post back from its author and deciding it in
//!   the World's command stream.
//! - [`pds`]: the read-back over `com.atproto.repo.getRecord`.
//! - [`dice`]: faces from the OS's secure random source.
//! - [`api`]: the XRPC methods, each declared with who may call it.
//!
//! Still to come, each behind its own module: the account host (repo, blobs,
//! PLC operations, per-GM sessions), the Space host (via `motet-spaces`),
//! the labeler, publishing outcomes to the World's repo, and the owner's
//! encrypted export.

pub mod admission;
pub mod api;
pub mod config;
pub mod dice;
pub mod pds;
pub mod shard;
