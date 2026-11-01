//! Firehose events, as the shard numbers and stores them.
//!
//! An operation emits events while it runs; the shard gives each one the next
//! sequence number when its batch is committed, stores it under `e/` in the
//! same database batch, and hands it to live subscribers afterwards. Replay
//! assigns the same numbers, because the events are part of the log entry.
//! Encoding these as `com.atproto.sync.subscribeRepos` frames is the sync
//! module's job; the fields here are what those frames need.

use bytes::Bytes;
use motet_rules::Did;
use serde::{Deserialize, Serialize};

/// One event of a World's account, in atproto's sync vocabulary.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Event {
    /// A repo commit: `#commit`.
    Commit(Commit),
    /// The repo's state, forced: `#sync` (a World's first commit, a recovery).
    Sync(Sync),
    /// The account's handle or DID document changed: `#identity`.
    Identity {
        did: Did,
        handle: Option<String>,
        time: String,
    },
    /// The account's hosting status changed: `#account`.
    Account {
        did: Did,
        active: bool,
        status: Option<String>,
        time: String,
    },
}

impl Event {
    /// The account the event is about.
    #[must_use]
    pub fn did(&self) -> &Did {
        match self {
            Self::Commit(c) => &c.repo,
            Self::Sync(s) => &s.repo,
            Self::Identity { did, .. } | Self::Account { did, .. } => did,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[expect(
    clippy::struct_field_names,
    reason = "atproto's #commit event names its commit CID field `commit`"
)]
pub struct Commit {
    pub repo: Did,
    pub rev: String,
    /// The previous commit's revision, if any.
    pub since: Option<String>,
    /// The commit's CID, as a string.
    pub commit: String,
    /// A CAR of the blocks a consumer needs to apply the commit: the commit,
    /// the changed records and the MST nodes covering them.
    #[serde(with = "crate::b64")]
    pub blocks: Bytes,
    pub ops: Vec<RepoOp>,
    /// The previous commit's MST root, for inductive verification.
    pub prev_data: Option<String>,
    pub time: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoOp {
    pub action: Action,
    /// `collection/rkey`.
    pub path: String,
    /// The record's new CID; absent for deletions.
    pub cid: Option<String>,
    /// The record's previous CID; absent for creations.
    pub prev: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Action {
    Create,
    Update,
    Delete,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Sync {
    pub repo: Did,
    pub rev: String,
    /// A CAR holding just the commit block.
    #[serde(with = "crate::b64")]
    pub blocks: Bytes,
    pub time: String,
}

/// An event with the sequence number the shard gave it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sequenced {
    pub seq: u64,
    pub event: Event,
}
