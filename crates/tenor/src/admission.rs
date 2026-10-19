//! Admitting a post: read it back from where its author keeps it, then
//! commit the decision in the World's command stream.
//!
//! The read-back is what makes an admission trustworthy. The tenor never
//! admits a version it hasn't fetched from the author's PDS (or their Space
//! repo), so an admission can't point at a record that doesn't exist, and a
//! player can't collect admissions for unpublished posts while in the cast
//! and publish them after being removed. What counts is the record as
//! fetched: its author, its CID and the scene it names. Whatever the request
//! claimed about them is only a hint.
//!
//! Requests come from Motet's API as people post (in parallel with the PDS
//! write), from Spaces write notifications, and from sync. All of them reach
//! [`admit`], so a post is admitted the same way whichever arrives first.

use motet_rules::world::{Command, ReadBack};
use motet_rules::{Cid, Did, OperationId, SceneId, StrongRef};
use thiserror::Error;

use crate::shard::{Applied, Entry, Handle, SubmitError};

/// A post version as its author's PDS returned it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fetched {
    pub cid: Cid,
    pub author: Did,
    pub world: Did,
    pub scene: SceneId,
}

#[derive(Debug, Error)]
pub enum FetchError {
    /// The PDS couldn't be asked, or didn't answer usefully. Not evidence
    /// that the record doesn't exist.
    #[error("couldn't read the record: {0}")]
    Unavailable(String),
}

/// Where posts are read back from: the author's PDS for public posts, their
/// Space repo for private ones.
pub trait Records: Send + Sync {
    /// The record's current version, or `None` if its repo says it has none.
    fn fetch(&self, post: &StrongRef) -> impl Future<Output = Result<Option<Fetched>, FetchError>> + Send;
}

/// One request to admit a post version.
#[derive(Clone, Debug)]
pub struct Request {
    pub op: OperationId,
    pub world: Did,
    pub post: StrongRef,
    /// For an edit, the admitted version it replaces.
    pub replaces: Option<Cid>,
}

/// What became of a request.
#[derive(Debug, PartialEq, Eq)]
pub enum Decision {
    /// The World's command stream decided, durably: admitted or refused.
    Decided(Applied),
    /// The pinned version isn't (or isn't yet) what the author's repo holds.
    /// Nothing was written to the World. The caller keeps the request
    /// pending: the PDS write may still land, and sync or a write
    /// notification will bring it back here.
    NotFound,
    /// The PDS couldn't be read. Pending, as above; not a refusal.
    Unreachable,
    /// The record names another World. Nothing to decide here.
    OtherWorld,
}

#[derive(Debug, Error)]
pub enum AdmitError {
    #[error(transparent)]
    Shard(#[from] SubmitError),
}

pub async fn admit(shard: &Handle, records: &impl Records, request: Request) -> Result<Decision, AdmitError> {
    let fetched = match records.fetch(&request.post).await {
        Ok(Some(fetched)) => fetched,
        Ok(None) => return Ok(Decision::NotFound),
        Err(FetchError::Unavailable(reason)) => {
            tracing::debug!(post = %request.post.uri, %reason, "read-back failed; request stays pending");
            return Ok(Decision::Unreachable);
        }
    };
    if fetched.cid != request.post.cid {
        // The repo holds another version now. That version gets its own
        // request; this one never pins something that isn't there.
        return Ok(Decision::NotFound);
    }
    if fetched.world != request.world {
        return Ok(Decision::OtherWorld);
    }
    let read_back = ReadBack {
        post: request.post,
        author: fetched.author,
        scene: fetched.scene,
    };
    let entry = Entry::Apply {
        world: request.world,
        op: request.op,
        command: Command::Admit {
            post: read_back,
            replaces: request.replaces,
        },
    };
    Ok(Decision::Decided(shard.submit(entry).await?))
}
