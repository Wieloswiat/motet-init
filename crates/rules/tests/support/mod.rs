//! Generated World histories.
//!
//! A history is what happens in a small World: authors write, edit and delete
//! posts in their own repos, GMs change a scene's cast, and the tenor admits
//! what it reads back. Running it through the real [`World`] gives both the
//! tenor's outcomes and the per-repo record streams other consumers receive.

#![allow(dead_code, unreachable_pub)]

use std::collections::BTreeMap;

use motet_rules::view::{Op, RepoEvent};
use motet_rules::world::{Command, Outcome, ReadBack, World};
use motet_rules::{Cid, Did, OperationId, RecordUri, SceneId, StrongRef};
use proptest::prelude::*;

pub const AUTHORS: usize = 3;
pub const POSTS_PER_AUTHOR: usize = 3;
pub const SCENES: usize = 2;

pub fn world_did() -> Did {
    Did::new("did:plc:world")
}

pub fn author(i: usize) -> Did {
    Did::new(format!("did:plc:author{i}"))
}

pub fn scene(i: usize) -> SceneId {
    SceneId::new(format!("scene{i}"))
}

pub fn post_uri(author_index: usize, post: usize) -> RecordUri {
    RecordUri::new(format!(
        "at://{}/at.motet.pilot.post/p{post}",
        author(author_index)
    ))
}

/// One step of a history.
#[derive(Clone, Debug)]
pub enum Action {
    /// An author writes (or rewrites) one of their posts in a scene.
    Write { author: usize, post: usize, scene: usize, body: u8 },
    Delete { author: usize, post: usize },
    JoinCast { author: usize, scene: usize },
    LeaveCast { author: usize, scene: usize },
    CloseScene { scene: usize },
    ReopenScene { scene: usize },
    Suspend { author: usize },
    Reinstate { author: usize },
    /// The tenor reads the post back from its author's PDS and asks to admit
    /// whatever version is there now. `claim` is which version the author's
    /// client said the edit replaces: clients can be stale or wrong.
    Admit { author: usize, post: usize, claim: Claim },
    /// The tenor notices a deletion through sync.
    ObserveDeletion { author: usize, post: usize },
}

/// What an admission request says it replaces.
#[derive(Clone, Copy, Debug)]
pub enum Claim {
    /// The version actually admitted now (what an up-to-date client sends).
    Current,
    /// Nothing: a client that thinks this is a new post.
    Nothing,
    /// A version that was never admitted.
    Bogus,
}

pub fn action() -> impl Strategy<Value = Action> {
    let a = 0..AUTHORS;
    let p = 0..POSTS_PER_AUTHOR;
    let s = 0..SCENES;
    prop_oneof![
        3 => (a.clone(), p.clone(), s.clone(), any::<u8>())
            .prop_map(|(author, post, scene, body)| Action::Write { author, post, scene, body }),
        1 => (a.clone(), p.clone()).prop_map(|(author, post)| Action::Delete { author, post }),
        1 => (a.clone(), s.clone()).prop_map(|(author, scene)| Action::JoinCast { author, scene }),
        1 => (a.clone(), s.clone()).prop_map(|(author, scene)| Action::LeaveCast { author, scene }),
        1 => s.clone().prop_map(|scene| Action::CloseScene { scene }),
        1 => s.clone().prop_map(|scene| Action::ReopenScene { scene }),
        1 => a.clone().prop_map(|author| Action::Suspend { author }),
        1 => a.clone().prop_map(|author| Action::Reinstate { author }),
        6 => (a.clone(), p.clone(), prop_oneof![8 => Just(Claim::Current), 1 => Just(Claim::Nothing), 1 => Just(Claim::Bogus)])
            .prop_map(|(author, post, claim)| Action::Admit { author, post, claim }),
        1 => (a, p).prop_map(|(author, post)| Action::ObserveDeletion { author, post }),
    ]
}

pub fn history() -> impl Strategy<Value = Vec<Action>> {
    proptest::collection::vec(action(), 0..60)
}

/// What a history produced.
#[derive(Debug, Default)]
pub struct Run {
    /// Every command the tenor applied, with its operation ID and outcome.
    pub log: Vec<(OperationId, Command, Outcome)>,
    /// Each repository's events in commit order: one stream per author, then
    /// the World's own.
    pub streams: Vec<Vec<RepoEvent>>,
    /// Each post as its author's PDS holds it at the end.
    pub current: BTreeMap<RecordUri, StrongRef>,
}

/// Runs `actions` against a World whose scenes start open with everyone cast.
pub fn run(actions: &[Action]) -> (World, Run) {
    let mut world = World::new(0);
    let mut out = Run::default();
    let mut repos: BTreeMap<Did, Vec<RepoEvent>> = BTreeMap::new();
    // Each author's repo as its PDS holds it now: post -> (version, scene).
    let mut current: BTreeMap<RecordUri, (StrongRef, SceneId)> = BTreeMap::new();
    let mut ops = 0u64;

    let emit = |repos: &mut BTreeMap<Did, Vec<RepoEvent>>, repo: Did, op: Op| {
        let stream = repos.entry(repo.clone()).or_default();
        let rev = stream.len() as u64 + 1;
        stream.push(RepoEvent { repo, rev, op });
    };
    let mut tenor = |world: &mut World, out: &mut Run, command: Command| -> Outcome {
        ops += 1;
        let op = OperationId::new(format!("op{ops}"));
        let outcome = world
            .apply(op.clone(), command.clone())
            .expect("fresh operation IDs are never reused");
        out.log.push((op, command, outcome.clone()));
        outcome
    };

    for s in 0..SCENES {
        tenor(
            &mut world,
            &mut out,
            Command::OpenScene {
                scene: scene(s),
                cast: (0..AUTHORS).map(author).collect(),
            },
        );
    }

    for action in actions {
        match *action {
            Action::Write { author: a, post, scene: s, body } => {
                let uri = post_uri(a, post);
                // An edit stays in its scene; a new post goes where it's written.
                let scene_id = current.get(&uri).map_or_else(|| scene(s), |(_, sc)| sc.clone());
                let cid = Cid::new(format!("bafy-{a}-{post}-{body}"));
                let version = StrongRef::new(uri.clone(), cid);
                current.insert(uri, (version.clone(), scene_id.clone()));
                emit(
                    &mut repos,
                    author(a),
                    Op::PutPost { post: version, scene: scene_id, body: format!("body {body}") },
                );
            }
            Action::Delete { author: a, post } => {
                let uri = post_uri(a, post);
                if current.remove(&uri).is_some() {
                    emit(&mut repos, author(a), Op::DeletePost { uri });
                }
            }
            Action::JoinCast { author: a, scene: s } => {
                tenor(&mut world, &mut out, Command::JoinCast { scene: scene(s), member: author(a) });
            }
            Action::LeaveCast { author: a, scene: s } => {
                tenor(&mut world, &mut out, Command::LeaveCast { scene: scene(s), member: author(a) });
            }
            Action::CloseScene { scene: s } => {
                tenor(&mut world, &mut out, Command::CloseScene { scene: scene(s) });
            }
            Action::ReopenScene { scene: s } => {
                let cast = cast_of(&out, &scene(s));
                tenor(&mut world, &mut out, Command::OpenScene { scene: scene(s), cast });
            }
            Action::Suspend { author: a } => {
                tenor(&mut world, &mut out, Command::Suspend { member: author(a) });
            }
            Action::Reinstate { author: a } => {
                tenor(&mut world, &mut out, Command::Reinstate { member: author(a) });
            }
            Action::Admit { author: a, post, claim } => {
                let uri = post_uri(a, post);
                // Read-back: only a version that exists can be admitted.
                let Some((version, scene_id)) = current.get(&uri).cloned() else { continue };
                let replaces = match claim {
                    Claim::Current => world.admission(&uri).map(|adm| adm.post.cid.clone()),
                    Claim::Nothing => None,
                    Claim::Bogus => Some(Cid::new("bafy-never-admitted")),
                };
                let outcome = tenor(
                    &mut world,
                    &mut out,
                    Command::Admit {
                        post: ReadBack { post: version, author: author(a), scene: scene_id },
                        replaces,
                    },
                );
                if let Outcome::Admitted(admission) = outcome {
                    emit(&mut repos, world_did(), Op::PutAdmission(admission));
                }
            }
            Action::ObserveDeletion { author: a, post } => {
                let uri = post_uri(a, post);
                if current.contains_key(&uri) {
                    continue; // nothing deleted to observe
                }
                if let Outcome::Closed(admission) =
                    tenor(&mut world, &mut out, Command::ObserveDeletion { post: uri })
                {
                    emit(&mut repos, world_did(), Op::CloseAdmission(admission));
                }
            }
        }
    }

    out.current = current.into_iter().map(|(uri, (version, _))| (uri, version)).collect();
    let world_stream = repos.remove(&world_did()).unwrap_or_default();
    out.streams = repos.into_values().collect();
    out.streams.push(world_stream);
    (world, out)
}

/// A scene's cast as the commands so far left it.
fn cast_of(run: &Run, scene: &SceneId) -> std::collections::BTreeSet<Did> {
    let mut cast = std::collections::BTreeSet::new();
    for (_, command, _) in &run.log {
        match command {
            Command::OpenScene { scene: s, cast: c } if s == scene => cast.clone_from(c),
            Command::JoinCast { scene: s, member } if s == scene => {
                cast.insert(member.clone());
            }
            Command::LeaveCast { scene: s, member } if s == scene => {
                cast.remove(member);
            }
            _ => {}
        }
    }
    cast
}
