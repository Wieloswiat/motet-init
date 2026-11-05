//! Properties of the tenor's decisions over generated histories.

mod support;

use std::collections::{BTreeMap, BTreeSet};

use motet_rules::world::{Command, CommandError, Outcome, Place, ReadBack, Refusal, World};
use motet_rules::{Cid, Did, OperationId, RecordUri, SceneId};
use proptest::prelude::*;
use support::{admission_history, history, run};

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

    /// An author's preflight agrees with a fresh admission at every point and
    /// changes no World state. Catches preflights that ignore changed authority
    /// or read-back versions, or consume a place before the real admission.
    #[test]
    fn preflight_agrees_with_fresh_admission_without_changing_state(actions in admission_history()) {
        let (_, run) = run(&actions);
        let mut world = World::new(0);
        let mut facts = Facts::default();
        for (op, command, _) in &run.log {
            let checked = if let Command::Admit { post, replaces } = command {
                let before = world.clone();
                let checked = world.check(post, replaces.as_ref());
                prop_assert_eq!(&world, &before, "preflight changed the World");
                prop_assert_eq!(
                    checked.is_ok(),
                    facts.can_admit(post, replaces.as_ref()),
                    "preflight disagreed with command facts for {:?}",
                    command
                );
                Some(checked)
            } else {
                None
            };
            prop_assert!(!world.has_operation(op), "comparison must use a fresh operation");
            let outcome = world.apply(op.clone(), command.clone()).expect("fresh operation ID");
            if let Some(checked) = checked {
                match outcome {
                    Outcome::Admitted(_) => prop_assert_eq!(checked, Ok(())),
                    Outcome::Refused(why) => prop_assert_eq!(checked, Err(why)),
                    other => prop_assert!(false, "admission produced {:?}", other),
                }
            }
            facts.record(command);
        }
    }

    /// Each scene numbers new admissions consecutively in the selected epoch;
    /// edits keep their whole place. Catches counters shared between scenes,
    /// consumed by edits or refusals, reset on reopen, or reused after deletion.
    #[test]
    fn places_are_unique_ordered_and_kept_by_edits(actions in admission_history(), epoch in any::<u32>()) {
        let (_, run) = run(&actions);
        let mut world = World::new(epoch);
        let mut facts = Facts::default();
        let mut place_of: BTreeMap<RecordUri, (SceneId, Place)> = BTreeMap::new();
        let mut next_seq: BTreeMap<SceneId, u64> = BTreeMap::new();
        for (op, command, _) in &run.log {
            let outcome = world.apply(op.clone(), command.clone()).expect("fresh operation ID");
            match command {
                Command::Admit { post, replaces } if facts.can_admit(post, replaces.as_ref()) => {
                    let expected = place_of.entry(post.post.uri.clone()).or_insert_with(|| {
                        let seq = next_seq.entry(post.scene.clone()).or_default();
                        let place = Place { epoch, seq: *seq };
                        *seq += 1;
                        (post.scene.clone(), place)
                    });
                    let Outcome::Admitted(admission) = outcome else {
                        prop_assert!(false, "eligible admission produced {:?}", outcome);
                        return Ok(());
                    };
                    prop_assert_eq!(&admission.post, &post.post);
                    prop_assert_eq!(&admission.author, &post.author);
                    prop_assert_eq!(&admission.scene, &expected.0);
                    prop_assert_eq!(admission.place, expected.1);
                    prop_assert_eq!(admission.authority_rev, facts.rev);
                    prop_assert_eq!(&admission.replaces, replaces);
                }
                Command::Admit { .. } => {
                    prop_assert!(matches!(outcome, Outcome::Refused(_)), "ineligible admission took a place");
                }
                Command::ObserveDeletion { post } => {
                    if let Some((scene, place)) = place_of.remove(post) {
                        let Outcome::Closed(admission) = outcome else {
                            prop_assert!(false, "deletion did not close its admission");
                            return Ok(());
                        };
                        prop_assert_eq!(admission.place, place);
                        prop_assert_eq!(admission.scene, scene);
                    } else {
                        prop_assert_eq!(outcome, Outcome::Unchanged);
                    }
                }
                _ => {}
            }
            facts.record(command);
        }
        let actual: BTreeMap<_, _> = world.admissions()
            .map(|a| (a.post.uri.clone(), (a.scene.clone(), a.place)))
            .collect();
        prop_assert_eq!(actual, place_of);
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
                (
                    Command::CloseScene { scene }
                    | Command::SetSceneCast { scene, .. }
                    | Command::JoinCast { scene, .. }
                    | Command::LeaveCast { scene, .. },
                    Outcome::Refused(Refusal::UnknownScene),
                ) => prop_assert!(!f.known.contains(scene), "{:?} refused, but its scene exists", command),
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

    fn can_admit(&self, post: &ReadBack, replaces: Option<&Cid>) -> bool {
        self.open.contains(&post.scene)
            && self
                .casts
                .get(&post.scene)
                .is_some_and(|cast| cast.contains(&post.author))
            && !self.suspended.contains(&post.author)
            && self.admitted.get(&post.post.uri) == replaces
            && self.admitted.get(&post.post.uri) != Some(&post.post.cid)
            && self
                .scene_of
                .get(&post.post.uri)
                .is_none_or(|scene| scene == &post.scene)
    }

    /// Advance the model from command facts, never from the World's outcome.
    fn record(&mut self, command: &Command) {
        match command {
            Command::Admit { post, replaces } => {
                if self.can_admit(post, replaces.as_ref()) {
                    self.admitted.insert(post.post.uri.clone(), post.post.cid.clone());
                    self.scene_of.insert(post.post.uri.clone(), post.scene.clone());
                }
            }
            Command::ObserveDeletion { post } => {
                self.admitted.remove(post);
                self.scene_of.remove(post);
            }
            command => {
                let known = match command {
                    Command::CloseScene { scene }
                    | Command::SetSceneCast { scene, .. }
                    | Command::JoinCast { scene, .. }
                    | Command::LeaveCast { scene, .. } => self.known.contains(scene),
                    _ => true,
                };
                if known {
                    self.rev += 1;
                    self.apply(command);
                }
            }
        }
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
            Command::SetSceneCast { scene, cast } => {
                self.casts.insert(scene.clone(), cast.clone());
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
