//! What every consumer derives.
//!
//! Records arrive per repository, in order within a repository and in no
//! particular order across them: one consumer may see Alice's post before
//! her removal from the scene, another the other way round. A [`WorldView`]
//! never decides who could post. It follows the World's admissions, which
//! have one writer, so any two consumers that received the same records agree
//! on what each scene shows, whatever the interleaving.
//!
//! Delivery is at least once. Each event carries its repository revision and
//! the view ignores anything at or below the revision it has already applied
//! for that repository, so a redelivered or late event can't roll it back.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::id::{Cid, Did, RecordUri, SceneId, StrongRef};
use crate::world::{Admission, Place};

/// One change to one repository, as sync delivers it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoEvent {
    pub repo: Did,
    pub rev: u64,
    pub op: Op,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Op {
    /// A post record created or edited in its author's repo.
    PutPost {
        post: StrongRef,
        scene: SceneId,
        body: String,
    },
    DeletePost {
        uri: RecordUri,
    },
    /// An admission record written in the World's repo.
    PutAdmission(Admission),
    /// The World closed an admission after observing the post's deletion.
    CloseAdmission(Admission),
}

/// One entry in a scene, at its admitted place.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    pub place: Place,
    pub post: RecordUri,
    pub author: Did,
    pub shown: Shown,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Shown {
    /// The admitted version, as its author wrote it.
    Version { cid: Cid, body: String },
    /// The admitted version was replaced or deleted before this consumer
    /// kept it. Shown as "this version isn't available", never as the
    /// current text in its place.
    Unavailable { cid: Cid },
    /// The post was deleted; a tombstone keeps its place.
    Tombstone,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct PostVersions {
    author: Option<Did>,
    current: Option<Cid>,
    /// Versions observed since the post was last deleted. No archive by
    /// default: a deletion drops them.
    bodies: BTreeMap<Cid, String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum AdmissionState {
    Open(Admission),
    Closed(Admission),
}

impl AdmissionState {
    fn admission(&self) -> &Admission {
        match self {
            Self::Open(a) | Self::Closed(a) => a,
        }
    }
}

/// One World, as one consumer has seen it so far.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorldView {
    world: Did,
    revs: BTreeMap<Did, u64>,
    posts: BTreeMap<RecordUri, PostVersions>,
    admissions: BTreeMap<RecordUri, AdmissionState>,
}

impl WorldView {
    #[must_use]
    pub fn new(world: Did) -> Self {
        Self {
            world,
            revs: BTreeMap::new(),
            posts: BTreeMap::new(),
            admissions: BTreeMap::new(),
        }
    }

    pub fn apply(&mut self, event: &RepoEvent) {
        let seen = self.revs.entry(event.repo.clone()).or_default();
        if event.rev <= *seen {
            return;
        }
        *seen = event.rev;

        match &event.op {
            Op::PutPost { post, body, .. } => {
                // Anyone can write anything in their own repo, and only there.
                if !is_in_repo(&post.uri, &event.repo) {
                    return;
                }
                let versions = self.posts.entry(post.uri.clone()).or_default();
                versions.author = Some(event.repo.clone());
                versions.current = Some(post.cid.clone());
                versions.bodies.insert(post.cid.clone(), body.clone());
            }
            Op::DeletePost { uri } => {
                if !is_in_repo(uri, &event.repo) {
                    return;
                }
                let versions = self.posts.entry(uri.clone()).or_default();
                versions.author = Some(event.repo.clone());
                versions.current = None;
                versions.bodies.clear();
            }
            // Admissions count only from the World's own repo.
            Op::PutAdmission(admission) if event.repo == self.world => {
                self.admissions
                    .insert(admission.post.uri.clone(), AdmissionState::Open(admission.clone()));
            }
            Op::CloseAdmission(admission) if event.repo == self.world => {
                self.admissions
                    .insert(admission.post.uri.clone(), AdmissionState::Closed(admission.clone()));
            }
            Op::PutAdmission(_) | Op::CloseAdmission(_) => {}
        }
    }

    /// The scene's entries in admission order. An admission whose post this
    /// consumer hasn't received yet is left out: it waits for the post and is
    /// never shown alone.
    #[must_use]
    pub fn scene(&self, scene: &SceneId) -> Vec<Entry> {
        let mut entries: Vec<Entry> = self
            .admissions
            .values()
            .filter(|state| &state.admission().scene == scene)
            .filter_map(|state| {
                let admission = state.admission();
                let shown = match state {
                    AdmissionState::Closed(_) => Shown::Tombstone,
                    AdmissionState::Open(_) => {
                        let versions = self.posts.get(&admission.post.uri)?;
                        match (&versions.current, versions.bodies.get(&admission.post.cid)) {
                            (None, _) => Shown::Tombstone,
                            (Some(_), Some(body)) => Shown::Version {
                                cid: admission.post.cid.clone(),
                                body: body.clone(),
                            },
                            (Some(_), None) => Shown::Unavailable {
                                cid: admission.post.cid.clone(),
                            },
                        }
                    }
                };
                Some(Entry {
                    place: admission.place,
                    post: admission.post.uri.clone(),
                    author: admission.author.clone(),
                    shown,
                })
            })
            .collect();
        entries.sort_by_key(|e| e.place);
        entries
    }

    /// An author's posts whose current version isn't admitted (yet): shown to
    /// them as pending, outside the scene.
    #[must_use]
    pub fn pending(&self, author: &Did) -> Vec<StrongRef> {
        self.posts
            .iter()
            .filter(|(_, v)| v.author.as_ref() == Some(author))
            .filter_map(|(uri, v)| {
                let current = v.current.as_ref()?;
                let admitted = match self.admissions.get(uri) {
                    Some(AdmissionState::Open(a)) => Some(&a.post.cid),
                    _ => None,
                };
                (admitted != Some(current)).then(|| StrongRef::new(uri.clone(), current.clone()))
            })
            .collect()
    }
}

/// Whether `uri` names a record in `repo`: `at://<repo>/…`.
fn is_in_repo(uri: &RecordUri, repo: &Did) -> bool {
    uri.as_str()
        .strip_prefix("at://")
        .and_then(|rest| rest.strip_prefix(repo.as_str()))
        .is_some_and(|rest| rest.starts_with('/'))
}
