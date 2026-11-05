//! What the indexer receives and archives.
//!
//! The shapes follow what Tap delivers (verified record operations, identity
//! and account changes, per repo in order), normalized so the archive
//! doesn't depend on Tap's wire format. Operations and content are separate:
//! content can be deduplicated by CID, operations can't, or a delete
//! followed by a re-post would vanish.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Event {
    Record(RecordEvent),
    /// A snapshot of a record in a publicly readable Space repository.
    SpaceRecord(SpaceRecordEvent),
    /// A DID's handle changed (or was confirmed).
    Identity {
        did: String,
        handle: Option<String>,
    },
    /// An account was activated, deactivated, taken down or deleted.
    Account {
        did: String,
        active: bool,
        status: Option<String>,
    },
}

/// One record operation in one repository.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordEvent {
    pub did: String,
    /// The repository revision (a TID) the operation was committed in.
    /// TIDs sort as strings in commit order.
    pub rev: String,
    pub collection: String,
    pub rkey: String,
    pub action: Action,
    /// Absent for deletions.
    pub cid: Option<String>,
    /// The record as JSON, verified by Tap. Lexicon validation is a separate
    /// ingestion stage, not yet implemented. Absent for deletions.
    pub record: Option<serde_json::Value>,
}

/// One current record in a publicly readable Space repository.
///
/// Ingestion must verify that the Space is public before archiving its
/// records. Space revisions are independent of public repository revisions.
/// A missing record is represented by a deletion at the snapshot revision.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpaceRecordEvent {
    pub space: String,
    /// The DID of the repository within the Space.
    pub did: String,
    /// The Space repository revision observed consistently with the record.
    pub rev: String,
    pub collection: String,
    pub rkey: String,
    pub action: Action,
    pub cid: Option<String>,
    pub record: Option<serde_json::Value>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Action {
    Create,
    Update,
    Delete,
}

impl RecordEvent {
    #[must_use]
    pub fn uri(&self) -> String {
        format!("at://{}/{}/{}", self.did, self.collection, self.rkey)
    }
}
