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
//! write), from Spaces write notifications, and from sync. Automatic
//! ingestion uses [`admit`]; member requests also check current membership
//! and record their actor in the decision's transaction.

use motet_rules::world::{Command, Outcome, ReadBack};
use motet_rules::{Cid, Did, OperationId, SceneId, StrongRef};
use thiserror::Error;

use crate::authorization::Actor;
use crate::shard::{Applied, Entry, Handle, Op, SubmitError, Txn};
use crate::spaces::SpaceError;

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
    #[error("the authenticated actor does not match the requested World or post author")]
    ActorMismatch,
    #[error(transparent)]
    Shard(#[from] SubmitError),
    #[error(transparent)]
    Space(#[from] SpaceError),
}

/// Read back and decide an admission brought by automatic ingestion.
pub async fn admit(shard: &Handle, records: &impl Records, request: Request) -> Result<Decision, AdmitError> {
    admit_inner(shard, records, request, None).await
}

/// Read back and decide an admission requested by its authenticated author.
pub(crate) async fn admit_as(
    shard: &Handle,
    records: &impl Records,
    request: Request,
    actor: Actor,
) -> Result<Decision, AdmitError> {
    admit_inner(shard, records, request, Some(actor)).await
}

async fn admit_inner(
    shard: &Handle,
    records: &impl Records,
    request: Request,
    actor: Option<Actor>,
) -> Result<Decision, AdmitError> {
    if actor.as_ref().is_some_and(|actor| actor.world != request.world) {
        return Err(AdmitError::ActorMismatch);
    }
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
    if actor.as_ref().is_some_and(|actor| actor.actor != fetched.author) {
        return Err(AdmitError::ActorMismatch);
    }
    let read_back = ReadBack {
        post: request.post,
        author: fetched.author,
        scene: fetched.scene,
    };
    let applied = match actor {
        Some(actor) => {
            shard
                .run(AdmitAs {
                    actor,
                    op: request.op,
                    post: read_back,
                    replaces: request.replaces,
                })
                .await??
        }
        None => {
            shard
                .submit(Entry::Apply {
                    world: request.world,
                    op: request.op,
                    command: Command::Admit {
                        post: read_back,
                        replaces: request.replaces,
                    },
                })
                .await?
        }
    };
    Ok(Decision::Decided(applied))
}

struct AdmitAs {
    actor: Actor,
    op: OperationId,
    post: ReadBack,
    replaces: Option<Cid>,
}

impl Op for AdmitAs {
    type Output = Applied;
    type Error = AdmitError;

    async fn run(self, txn: &mut Txn<'_>) -> Result<Applied, AdmitError> {
        self.actor.policy(txn).await?;
        let seen = txn
            .world(&self.actor.world)
            .is_some_and(|world| world.has_operation(&self.op));
        let subject = (!seen).then(|| self.post.post.uri.clone());
        let applied = txn
            .command(
                self.actor.world.clone(),
                self.op,
                Command::Admit {
                    post: self.post,
                    replaces: self.replaces,
                },
            )
            .await
            .map_err(SubmitError::from)?;
        // Refusals also consume an operation ID durably. Attribute the first
        // decision, but never attribute a retry or a reused operation ID.
        if let Some(subject) = subject {
            let action = match &applied {
                Ok(Outcome::Admitted(_)) => Some("admit"),
                Ok(Outcome::Refused(_)) => Some("refuseAdmission"),
                _ => None,
            };
            if let Some(action) = action {
                self.actor.audit(txn, action, subject.as_str()).await?;
            }
        }
        Ok(applied)
    }
}
