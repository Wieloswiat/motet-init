//! Properties of the tenor's decisions over generated histories.

mod support;

use std::collections::{BTreeMap, BTreeSet};

use motet_rules::world::{Command, CommandError, Outcome, Refusal, World};
use motet_rules::{Cid, Did, OperationId, RecordUri, SceneId};
use proptest::prelude::*;
use support::{history, run};

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// A retry with the same operation ID returns the same outcome and changes
    /// nothing, however many times and wherever it lands. (Acceptance test:
    /// "stop the tenor while players post … none lost or doubled".)
    #[test]
    fn retries_never_change_the_outcome_or_the_state(
        actions in history(),
        retries in proptest::collection::vec((any::<prop::sample::Index>(), any::<prop::sample::Index>()), 0..20),
    ) {
        let (reference, run) = run(&actions);
        prop_assume!(!run.log.is_empty());

        // Replay the log, re-sending earlier operations at random later points.
        let mut retried = World::new(0);
        let mut schedule: Vec<usize> = (0..run.log.len()).collect();
        for (which, after) in retries {
            let i = which.index(run.log.len());
            // Strictly after the original send: a retry is never early.
            let first = schedule.iter().position(|&x| x == i).expect("every op is scheduled");
            let at = first + 1 + after.index(schedule.len() - first);
            schedule.insert(at, i);
        }
        let mut first_outcome: BTreeMap<OperationId, Outcome> = BTreeMap::new();
        for i in schedule {
            let (op, command, _) = &run.log[i];
            let outcome = retried.apply(op.clone(), command.clone()).expect("same command, same ID");
            let first = first_outcome.entry(op.clone()).or_insert_with(|| outcome.clone());
            prop_assert_eq!(&outcome, first);
        }
        for (op, _, outcome) in &run.log {
            prop_assert_eq!(first_outcome.get(op), Some(outcome));
        }
        prop_assert_eq!(retried, reference);
    }

    /// Reusing an operation ID for a different command is an error, not an
    /// outcome, and leaves the World untouched.
    #[test]
    fn reusing_an_operation_id_is_refused_without_effect(actions in history()) {
        let (mut world, run) = run(&actions);
        let Some((op, command, _)) = run.log.last().cloned() else { return Ok(()) };
        let different = match command {
            Command::CloseScene { .. } => Command::Suspend { member: Did::new("did:plc:someone") },
            _ => Command::CloseScene { scene: SceneId::new("scene0") },
        };
        let before = world.clone();
        prop_assert_eq!(world.apply(op.clone(), different), Err(CommandError::OperationReused(op)));
        prop_assert_eq!(world, before);
    }

    /// Places in a scene are unique and handed out in admission order; an edit
    /// keeps the place of the post it replaces.
    #[test]
    fn places_are_unique_ordered_and_kept_by_edits(actions in history()) {
        let (_, run) = run(&actions);
        let mut place_of: BTreeMap<RecordUri, (SceneId, u64)> = BTreeMap::new();
        let mut last_new: BTreeMap<SceneId, u64> = BTreeMap::new();
        let mut taken: BTreeSet<(SceneId, u64)> = BTreeSet::new();
        for (_, _, outcome) in &run.log {
            match outcome {
                Outcome::Admitted(a) if a.replaces.is_some() => {
                    prop_assert_eq!(
                        place_of.get(&a.post.uri),
                        Some(&(a.scene.clone(), a.place.seq)),
                        "an edit moved its post"
                    );
                }
                Outcome::Admitted(a) => {
                    if let Some(prev) = last_new.get(&a.scene) {
                        prop_assert!(a.place.seq > *prev, "places went backwards");
                    }
                    prop_assert!(taken.insert((a.scene.clone(), a.place.seq)), "place reused");
                    last_new.insert(a.scene.clone(), a.place.seq);
                    place_of.insert(a.post.uri.clone(), (a.scene.clone(), a.place.seq));
                }
                Outcome::Closed(a) => {
                    place_of.remove(&a.post.uri);
                }
                _ => {}
            }
        }
    }

    /// Every outcome is justified by the authority facts at the point the
    /// tenor committed it, tracked here independently from the outcomes
    /// themselves: an admission needs an open scene, an author in its cast
    /// and not suspended, and an edit must name the version it replaces; a
    /// refusal must give a reason that is actually true. In particular, a
    /// removal governs everything committed after it, however early the
    /// request. (Acceptance test: "remove a player while their post is in
    /// flight".)
    #[test]
    fn every_outcome_is_justified_by_the_facts_at_that_point(actions in history()) {
        let (world, run) = run(&actions);
        let mut f = Facts::default();
        for (_, command, outcome) in &run.log {
            match (command, outcome) {
                (Command::Admit { post, replaces }, Outcome::Admitted(a)) => {
                    prop_assert!(f.open.contains(&a.scene), "admitted into a closed scene");
                    prop_assert!(f.cast(&a.scene).contains(&a.author), "{:?} admitted while out of the cast", a.author);
                    prop_assert!(!f.suspended.contains(&a.author), "admitted while suspended");
                    let previous = f.admitted.get(&post.post.uri).cloned();
                    prop_assert_eq!(replaces.as_ref(), previous.as_ref(), "edit named the wrong version");
                    prop_assert_eq!(&a.replaces, replaces);
                    prop_assert_ne!(Some(&a.post.cid), previous.as_ref(), "same version admitted twice");
                    prop_assert_eq!(a.authority_rev, f.rev, "checked against the wrong authority revision");
                    f.admitted.insert(post.post.uri.clone(), a.post.cid.clone());
                    f.scene_of.insert(post.post.uri.clone(), a.scene.clone());
                }
                (Command::Admit { post, replaces }, Outcome::Refused(why)) => {
                    let previous = f.admitted.get(&post.post.uri);
                    let true_reason = match why {
                        Refusal::UnknownScene => !f.known.contains(&post.scene),
                        Refusal::SceneClosed => !f.open.contains(&post.scene),
                        Refusal::NotInCast => !f.cast(&post.scene).contains(&post.author),
                        Refusal::Suspended => f.suspended.contains(&post.author),
                        Refusal::AlreadyAdmitted => previous == Some(&post.post.cid),
                        Refusal::StaleEdit { expected, current } => {
                            expected.as_ref() == replaces.as_ref() && current.as_ref() == previous && previous != replaces.as_ref()
                        }
                        // A post recreated in another scene before its old
                        // admission is closed (deletion not yet observed).
                        Refusal::SceneChanged => f.scene_of.get(&post.post.uri).is_some_and(|s| *s != post.scene),
                    };
                    prop_assert!(true_reason, "refused with an untrue reason {:?}", why);
                }
                (Command::ObserveDeletion { post }, Outcome::Closed(a)) => {
                    let removed = f.admitted.remove(post);
                    f.scene_of.remove(post);
                    prop_assert_eq!(removed.as_ref(), Some(&a.post.cid));
                }
                (Command::ObserveDeletion { post }, Outcome::Unchanged) => {
                    prop_assert!(!f.admitted.contains_key(post));
                }
                (_, Outcome::AuthorityChanged { authority_rev }) => {
                    f.rev += 1;
                    prop_assert_eq!(*authority_rev, f.rev);
                    f.apply(command);
                }
                (_, Outcome::Refused(Refusal::UnknownScene)) => {}
                (c, o) => prop_assert!(false, "unexpected outcome {:?} for {:?}", o, c),
            }
        }
        prop_assert_eq!(world.authority_rev(), f.rev);
        let open: BTreeMap<RecordUri, Cid> =
            world.admissions().map(|a| (a.post.uri.clone(), a.post.cid.clone())).collect();
        prop_assert_eq!(open, f.admitted);
    }
}

/// The authority facts, tracked from commands alone.
#[derive(Default)]
struct Facts {
    rev: u64,
    known: BTreeSet<SceneId>,
    open: BTreeSet<SceneId>,
    casts: BTreeMap<SceneId, BTreeSet<Did>>,
    suspended: BTreeSet<Did>,
    admitted: BTreeMap<RecordUri, Cid>,
    scene_of: BTreeMap<RecordUri, SceneId>,
}

impl Facts {
    fn cast(&self, scene: &SceneId) -> BTreeSet<Did> {
        self.casts.get(scene).cloned().unwrap_or_default()
    }

    fn apply(&mut self, command: &Command) {
        match command {
            Command::OpenScene { scene, cast } => {
                self.known.insert(scene.clone());
                self.open.insert(scene.clone());
                self.casts.insert(scene.clone(), cast.clone());
            }
            Command::CloseScene { scene } => {
                self.open.remove(scene);
            }
            Command::JoinCast { scene, member } => {
                self.casts.entry(scene.clone()).or_default().insert(member.clone());
            }
            Command::LeaveCast { scene, member } => {
                self.casts.entry(scene.clone()).or_default().remove(member);
            }
            Command::Suspend { member } => {
                self.suspended.insert(member.clone());
            }
            Command::Reinstate { member } => {
                self.suspended.remove(member);
            }
            Command::Admit { .. } | Command::ObserveDeletion { .. } => {}
        }
    }
}
