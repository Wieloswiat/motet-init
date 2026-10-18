//! What the tenor decides.
//!
//! A [`World`] is the state one World's command stream has reached. The tenor
//! applies commands one at a time, persists each [`Outcome`] atomically with
//! the state change, and only then publishes the records the outcome implies
//! (admissions, closures) to the World's repo or Spaces. Because one writer
//! orders every command, "was the author in the cast?" has exactly one answer
//! for every post, whatever order other consumers later receive things in.
//!
//! Field names and refusal reasons are provisional (Groundwork decides the
//! admission state machine); the invariants in `tests/world_properties.rs`
//! are not.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::id::{Cid, Did, OperationId, RecordUri, SceneId, StrongRef};

/// A position in a scene's sequence, numbered within a recovery epoch so a
/// place from before a recovery never takes on a new meaning after it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Place {
    pub epoch: u32,
    pub seq: u64,
}

/// The tenor's statement, written as the World, that a post version counts.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Admission {
    pub post: StrongRef,
    pub author: Did,
    pub scene: SceneId,
    pub place: Place,
    /// The authority revision the post was checked against. It moves with
    /// every cast, sanction or scene change, so anyone can check an admission
    /// against the facts it relied on.
    pub authority_rev: u64,
    /// For an edit, the version this admission supersedes. The place is kept.
    pub replaces: Option<Cid>,
}

/// A post version the tenor has read back from its author's PDS.
///
/// Only the read-back path in the tenor constructs one, after fetching the
/// record: an admission must never point at a record that doesn't exist, or a
/// player could collect admissions for unpublished posts while in the cast and
/// publish them after being removed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadBack {
    pub post: StrongRef,
    pub author: Did,
    pub scene: SceneId,
}

/// One authoritative mutation in a World's command stream.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Command {
    OpenScene { scene: SceneId, cast: BTreeSet<Did> },
    CloseScene { scene: SceneId },
    JoinCast { scene: SceneId, member: Did },
    LeaveCast { scene: SceneId, member: Did },
    /// A World-scoped posting suspension. A sanction, not a role: roles only
    /// ever add access, so taking one away can leave another route open.
    Suspend { member: Did },
    Reinstate { member: Did },
    /// Admit a post version, or for an edit, the new version of an admitted
    /// post. `replaces` must name the currently admitted version.
    Admit { post: ReadBack, replaces: Option<Cid> },
    /// The author deleted the post. A deletion is honoured without the tenor;
    /// closing the admission makes sure a later re-creation, even byte for
    /// byte, needs an admission of its own.
    ObserveDeletion { post: RecordUri },
}

/// What applying a command produced. Persisted with its operation ID, and
/// returned again, unchanged, to any retry.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Outcome {
    /// An authority fact changed; `authority_rev` is the new revision.
    AuthorityChanged { authority_rev: u64 },
    Admitted(Admission),
    /// An admitted post was deleted and its admission is now closed.
    Closed(Admission),
    Refused(Refusal),
    /// Nothing to do, e.g. deleting a post that was never admitted.
    Unchanged,
}

/// Why a post was not admitted. Shown to its author, who may delete it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Error)]
pub enum Refusal {
    #[error("the scene doesn't exist")]
    UnknownScene,
    #[error("the scene is closed")]
    SceneClosed,
    #[error("the author isn't in the scene's cast")]
    NotInCast,
    #[error("the author is suspended from posting in this World")]
    Suspended,
    #[error("this version is already admitted")]
    AlreadyAdmitted,
    #[error("the edit replaces {expected:?}, but the admitted version is {current:?}")]
    StaleEdit { expected: Option<Cid>, current: Option<Cid> },
    #[error("an edit must stay in its post's scene")]
    SceneChanged,
}

#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum CommandError {
    /// The operation ID was already used for a different command. Never a
    /// refusal: it means a client bug, and must not be cached as an outcome.
    #[error("operation {0} was already used for a different command")]
    OperationReused(OperationId),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Scene {
    open: bool,
    cast: BTreeSet<Did>,
    next_seq: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Recorded {
    command: Command,
    outcome: Outcome,
}

/// One World's authority state, as its command stream has left it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct World {
    epoch: u32,
    authority_rev: u64,
    scenes: BTreeMap<SceneId, Scene>,
    suspended: BTreeSet<Did>,
    /// The open admission of each admitted post, by post URI.
    admitted: BTreeMap<RecordUri, Admission>,
    operations: BTreeMap<OperationId, Recorded>,
}

impl World {
    /// A World starting a new epoch: at creation, or when a recovery from an
    /// older export is declared.
    pub fn new(epoch: u32) -> Self {
        Self {
            epoch,
            authority_rev: 0,
            scenes: BTreeMap::new(),
            suspended: BTreeSet::new(),
            admitted: BTreeMap::new(),
            operations: BTreeMap::new(),
        }
    }

    pub fn authority_rev(&self) -> u64 {
        self.authority_rev
    }

    /// The open admission for a post, if it has one.
    pub fn admission(&self, post: &RecordUri) -> Option<&Admission> {
        self.admitted.get(post)
    }

    /// Every open admission, in no particular order.
    pub fn admissions(&self) -> impl Iterator<Item = &Admission> {
        self.admitted.values()
    }

    /// Applies one command. Retrying with the same operation ID and command
    /// returns the recorded outcome without changing anything.
    pub fn apply(&mut self, op: OperationId, command: Command) -> Result<Outcome, CommandError> {
        if let Some(recorded) = self.operations.get(&op) {
            return if recorded.command == command {
                Ok(recorded.outcome.clone())
            } else {
                Err(CommandError::OperationReused(op))
            };
        }
        let outcome = self.decide(&command);
        self.operations.insert(
            op,
            Recorded {
                command,
                outcome: outcome.clone(),
            },
        );
        Ok(outcome)
    }

    /// Would this post be admitted now? The same check `apply` makes, without
    /// changing anything: the AppView uses it to warn an author before sending.
    pub fn check(&self, post: &ReadBack, replaces: Option<&Cid>) -> Result<(), Refusal> {
        let scene = self.scenes.get(&post.scene).ok_or(Refusal::UnknownScene)?;
        if !scene.open {
            return Err(Refusal::SceneClosed);
        }
        if !scene.cast.contains(&post.author) {
            return Err(Refusal::NotInCast);
        }
        if self.suspended.contains(&post.author) {
            return Err(Refusal::Suspended);
        }
        let current = self.admitted.get(&post.post.uri);
        if let Some(current) = current {
            if current.scene != post.scene {
                return Err(Refusal::SceneChanged);
            }
            if current.post.cid == post.post.cid {
                return Err(Refusal::AlreadyAdmitted);
            }
        }
        let current_cid = current.map(|a| &a.post.cid);
        if current_cid != replaces {
            return Err(Refusal::StaleEdit {
                expected: replaces.cloned(),
                current: current_cid.cloned(),
            });
        }
        Ok(())
    }

    fn decide(&mut self, command: &Command) -> Outcome {
        match command {
            Command::OpenScene { scene, cast } => {
                let entry = self.scenes.entry(scene.clone()).or_insert_with(|| Scene {
                    open: true,
                    cast: BTreeSet::new(),
                    next_seq: 0,
                });
                entry.open = true;
                entry.cast.clone_from(cast);
                self.authority_changed()
            }
            Command::CloseScene { scene } => self.with_scene(scene, |s| s.open = false),
            Command::JoinCast { scene, member } => self.with_scene(scene, |s| {
                s.cast.insert(member.clone());
            }),
            Command::LeaveCast { scene, member } => self.with_scene(scene, |s| {
                s.cast.remove(member);
            }),
            Command::Suspend { member } => {
                self.suspended.insert(member.clone());
                self.authority_changed()
            }
            Command::Reinstate { member } => {
                self.suspended.remove(member);
                self.authority_changed()
            }
            Command::Admit { post, replaces } => match self.check(post, replaces.as_ref()) {
                Err(refusal) => Outcome::Refused(refusal),
                Ok(()) => Outcome::Admitted(self.admit(post, replaces.clone())),
            },
            Command::ObserveDeletion { post } => match self.admitted.remove(post) {
                Some(admission) => Outcome::Closed(admission),
                None => Outcome::Unchanged,
            },
        }
    }

    fn admit(&mut self, post: &ReadBack, replaces: Option<Cid>) -> Admission {
        let place = if let Some(previous) = self.admitted.get(&post.post.uri) {
            previous.place
        } else {
            let scene = self
                .scenes
                .get_mut(&post.scene)
                .unwrap_or_else(|| unreachable!("check() verified the scene exists"));
            let seq = scene.next_seq;
            scene.next_seq += 1;
            Place {
                epoch: self.epoch,
                seq,
            }
        };
        let admission = Admission {
            post: post.post.clone(),
            author: post.author.clone(),
            scene: post.scene.clone(),
            place,
            authority_rev: self.authority_rev,
            replaces,
        };
        self.admitted
            .insert(post.post.uri.clone(), admission.clone());
        admission
    }

    fn with_scene(&mut self, scene: &SceneId, change: impl FnOnce(&mut Scene)) -> Outcome {
        match self.scenes.get_mut(scene) {
            Some(s) => {
                change(s);
                self.authority_changed()
            }
            None => Outcome::Refused(Refusal::UnknownScene),
        }
    }

    fn authority_changed(&mut self) -> Outcome {
        self.authority_rev += 1;
        Outcome::AuthorityChanged {
            authority_rev: self.authority_rev,
        }
    }
}
