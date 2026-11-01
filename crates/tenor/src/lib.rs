//! The tenor: Motet's group host.
//!
//! A tenor hosts each World as a group account, doing for the World what a
//! PDS does for a person: its identity, its public repo and its Spaces. GMs
//! act as the World through it with their own accounts. It admits posts,
//! issues dice and runs the World's moderation. Motet runs managed tenors and
//! anyone can run their own; the AppView treats both the same.
//!
//! - [`shard`]: the one writer of a shard's Worlds. Every change is an
//!   operation run against staged state, made durable in the shard's fenced
//!   log, then written to the shard's database. Everything the tenor promises
//!   rests on this: acknowledged means durable.
//! - [`store`]: the shard's database (SlateDB in the same bucket) and its key
//!   registry; [`store::Reader`] reads committed state.
//! - [`keys`]: World keys, sealed under the tenor's master key.
//! - [`event`]: firehose events, numbered and stored by the shard.
//! - [`identity`]: owner-signed PLC reservations, handle resolution and discovery.
//! - [`world`]: the atomic transition from a reservation to a hosted World.
//! - [`spaces`]: the World's role-derived Space authority and repositories.
//! - [`oauth`]: `DPoP`-bound World sessions, GM authentication and audited writes.
//! - [`admission`]: reading a post back from its author and deciding it in
//!   the World's command stream.
//! - [`publication`]: durable admission and closing publication to each scene's audience.
//! - [`pds`]: the read-back over `com.atproto.repo.getRecord`.
//! - [`dice`]: faces from the OS's secure random source.
//! - [`api`]: the XRPC methods, each declared with who may call it.
//!
//! Still to come, each behind its own module: blobs, the labeler and the
//! owner's encrypted export.

pub mod admission;
pub mod api;
mod b64;
pub mod config;
pub mod crawl;
pub mod dice;
pub mod event;
pub mod identity;
pub mod keys;
pub mod oauth;
pub mod pds;
mod plc_genesis;
pub mod publication;
pub mod repo;
pub mod shard;
pub mod spaces;
pub mod store;
pub mod sync;
pub mod world;
