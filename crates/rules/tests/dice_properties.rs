//! Independent integer-key histories protect durable dice's no-reroll contract.

use std::collections::BTreeMap;

use motet_rules::dice::{
    Audience, Dice, Eligibility, Epoch, Failure, Fence, Inputs, Opportunity, Outcome, Phase, Publication, Refusal,
    Request, RequestKey, SlotKey, SlotState,
};
use motet_rules::{Cid, Did, OperationId, RecordUri, SceneId, StrongRef};
use proptest::prelude::*;

fn reference(name: &str) -> StrongRef {
    StrongRef::new(RecordUri::new(format!("at://world/{name}")), Cid::new(name))
}

fn audience(private: bool) -> Audience {
    if private {
        Audience::Space {
            space: RecordUri::new("at://world/space/scene"),
        }
    } else {
        Audience::Public
    }
}

fn slot(number: u8) -> SlotKey {
    SlotKey {
        world: Did::new("world"),
        action: RecordUri::new(format!("at://world/action/{number}")),
        actor: Did::new("author"),
        slot: "attack".into(),
    }
}

fn inputs(number: u8, private: bool) -> Inputs {
    Inputs {
        scene: SceneId::new("scene"),
        audience: audience(private),
        audience_generation: 4,
        action: Some(reference(&format!("action/{number}"))),
        expression: "2d6".into(),
        grammar: "motet-dice/1".into(),
        rules_version: "game/1".into(),
        sheet: None,
        algorithm_version: "os-csprng/1".into(),
        limits_policy_version: "limits/1".into(),
    }
}

fn opportunity(number: u8, private: bool) -> Opportunity {
    Opportunity {
        reference: reference(&format!("opportunity/{number}")),
        slot: slot(number),
        epoch: Epoch(0),
        inputs: inputs(number, private),
    }
}

fn request(number: u8, cid: u8, epoch: u32, private: bool) -> Request {
    Request {
        world: Did::new("world"),
        key: RequestKey {
            audience: audience(private),
            author: Did::new("author"),
            collection: "roll.request".into(),
            rkey: number.to_string(),
        },
        cid: Cid::new(cid.to_string()),
        epoch: Epoch(epoch),
        opportunity: (number != 7).then(|| reference(&format!("opportunity/{}", number % 3))),
        inputs: inputs(number % 3, private),
        authority_revision: 9,
        result_uri: reference(&format!("result/{}", number % 5)).uri,
    }
}

fn publication(id: u8, value: u8) -> Publication {
    Publication {
        reference: StrongRef::new(
            reference(&format!("result/{}", id % 5)).uri,
            Cid::new(value.to_string()),
        ),
        signed_record: vec![value, 0, 255, value],
    }
}

fn fresh(private: bool) -> Dice {
    let mut dice = Dice::new(Did::new("world"), Epoch(0), Fence(0));
    for number in 0..3 {
        dice.open(opportunity(number, private))
            .expect("fresh slots have distinct actions and valid bound inputs");
    }
    dice
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Answer {
    Faces(Vec<u8>),
    Limit,
    Interrupted,
}

impl Answer {
    fn outcome(&self) -> Outcome {
        match self {
            Self::Faces(bytes) => Outcome::Rolled(bytes.clone()),
            Self::Limit => Outcome::Failed {
                code: Failure::LimitsExceeded,
                details: vec![9, 0, 9],
            },
            Self::Interrupted => Outcome::Failed {
                code: Failure::Interrupted,
                details: Vec::new(),
            },
        }
    }
}

#[derive(Clone, Debug)]
struct Accepted {
    cid: u8,
    epoch: u32,
    claimed: bool,
    uncertain: bool,
    answer: Option<Answer>,
    published: Option<u8>,
}

#[derive(Clone, Default)]
struct Facts {
    epoch: u32,
    fence: u64,
    accepted: BTreeMap<u8, Accepted>,
    aliases: BTreeMap<u8, u8>,
    winners: BTreeMap<u8, u8>,
    destinations: BTreeMap<u8, u8>,
    frozen: bool,
}

impl Facts {
    fn reserve(&mut self, id: u8, cid: u8, op: u8, epoch: u32, authorized: bool) -> Result<(), Refusal> {
        if self.aliases.get(&op).is_some_and(|first| *first != id) {
            return Err(Refusal::Conflict);
        }
        if let Some(first) = self.accepted.get(&id) {
            if (first.cid, first.epoch) != (cid, epoch) {
                return Err(Refusal::Conflict);
            }
        } else {
            if !authorized {
                return Err(Refusal::Unauthorized);
            }
            if epoch != self.epoch {
                return Err(Refusal::RecoveryUnresolved);
            }
            if self.destinations.contains_key(&(id % 5)) {
                return Err(Refusal::Conflict);
            }
            if id != 7 {
                // All opportunities in this history predate recovery.
                if epoch != 0 {
                    return Err(Refusal::OpportunityMismatch);
                }
                if self.winners.contains_key(&(id % 3)) {
                    return Err(Refusal::SlotConsumed);
                }
                if self.frozen {
                    return Err(Refusal::RecoveryUnresolved);
                }
                self.winners.insert(id % 3, id);
            }
            self.destinations.insert(id % 5, id);
            self.accepted.insert(
                id,
                Accepted {
                    cid,
                    epoch,
                    claimed: false,
                    uncertain: false,
                    answer: None,
                    published: None,
                },
            );
        }
        self.aliases.insert(op, id);
        Ok(())
    }

    fn begin(&mut self, id: u8, fence: u64) -> Result<(), Refusal> {
        if fence != self.fence {
            return Err(Refusal::StaleFence);
        }
        let first = self.accepted.get_mut(&id).ok_or(Refusal::Unknown)?;
        if first.claimed || first.uncertain || first.answer.is_some() {
            return Err(Refusal::WrongPhase);
        }
        first.claimed = true;
        Ok(())
    }

    fn record(&mut self, id: u8, fence: u64, answer: Answer) -> Result<(), Refusal> {
        if fence != self.fence {
            return Err(Refusal::StaleFence);
        }
        let first = self.accepted.get_mut(&id).ok_or(Refusal::Unknown)?;
        if let Some(previous) = &first.answer {
            return if previous == &answer {
                Ok(())
            } else {
                Err(Refusal::Conflict)
            };
        }
        if !first.claimed || first.uncertain {
            return Err(Refusal::WrongPhase);
        }
        first.answer = Some(answer);
        Ok(())
    }

    fn publish(&mut self, id: u8, value: u8) -> Result<(), Refusal> {
        let first = self.accepted.get_mut(&id).ok_or(Refusal::Unknown)?;
        if let Some(previous) = first.published {
            return if previous == value {
                Ok(())
            } else {
                Err(Refusal::Conflict)
            };
        }
        if first.answer.is_none() {
            return Err(Refusal::WrongPhase);
        }
        first.published = Some(value);
        Ok(())
    }

    fn takeover(&mut self) {
        self.fence += 1;
        for first in self.accepted.values_mut() {
            if first.claimed && !first.uncertain && first.answer.is_none() {
                first.answer = Some(Answer::Interrupted);
            }
        }
    }

    fn recover(&mut self, epoch: u32, fence: u64) {
        self.epoch = epoch;
        self.fence = fence;
        self.frozen = true;
        for first in self.accepted.values_mut() {
            if first.answer.is_none() {
                first.uncertain = true;
            }
        }
    }

    fn check(&self, dice: &Dice, private: bool) {
        assert_eq!(dice.epoch(), Epoch(self.epoch));
        for id in 0..8 {
            let key = request(id, 0, 0, private).key;
            let actual = dice.receipt(&key);
            let Some(first) = self.accepted.get(&id) else {
                assert!(actual.is_none());
                continue;
            };
            let receipt = actual.expect("the model accepted this request");
            assert_eq!(receipt.request, request(id, first.cid, first.epoch, private));
            let expected = first.answer.as_ref().map_or_else(
                || {
                    if first.uncertain {
                        Phase::RecoveryUnresolved
                    } else if first.claimed {
                        Phase::Drawing(Fence(self.fence))
                    } else {
                        Phase::Accepted
                    }
                },
                |answer| {
                    first.published.map_or_else(
                        || Phase::Recorded(answer.outcome()),
                        |value| Phase::Published {
                            outcome: answer.outcome(),
                            publication: publication(id, value),
                        },
                    )
                },
            );
            assert_eq!(receipt.phase, expected);
        }
        for number in 0..3 {
            let winner = self
                .winners
                .get(&number)
                .and_then(|id| self.accepted.get(id).map(|first| (*id, first)));
            let expected = match winner {
                Some((id, first)) if !first.uncertain => SlotState::Reserved(request(id, 0, 0, private).key),
                _ if self.frozen => SlotState::RecoveryUnresolved,
                _ => SlotState::Open,
            };
            assert_eq!(dice.slot(&slot(number)), Some(&expected));
            let outcomes = (0u8..7)
                .filter(|id| *id % 3 == number)
                .filter_map(|id| dice.receipt(&request(id, 0, 0, private).key))
                .filter(|receipt| matches!(receipt.phase, Phase::Recorded(_) | Phase::Published { .. }))
                .count();
            assert!(outcomes <= 1, "a restricted slot produced a second terminal outcome");
        }
    }
}

#[derive(Clone, Debug)]
enum Action {
    Reserve {
        id: u8,
        cid: u8,
        op: u8,
        old_epoch: bool,
        authorized: bool,
    },
    Begin {
        id: u8,
        stale: bool,
    },
    Record {
        id: u8,
        stale: bool,
        answer: Answer,
    },
    Publish {
        id: u8,
        value: u8,
    },
    Takeover,
    Recover,
    Checkpoint,
    Restore,
}

fn actions() -> impl Strategy<Value = Vec<Action>> {
    let action = prop_oneof![
        6 => (0u8..8, 0u8..3, 0u8..12, any::<bool>(), any::<bool>()).prop_map(|(id, cid, op, old_epoch, authorized)| Action::Reserve { id, cid, op, old_epoch, authorized }),
        4 => (0u8..8, any::<bool>()).prop_map(|(id, stale)| Action::Begin { id, stale }),
        4 => (0u8..8, any::<bool>(), prop_oneof![proptest::collection::vec(any::<u8>(), 0..32).prop_map(Answer::Faces), Just(Answer::Limit)])
            .prop_map(|(id, stale, answer)| Action::Record { id, stale, answer }),
        3 => (0u8..8, any::<u8>()).prop_map(|(id, value)| Action::Publish { id, value }),
        1 => Just(Action::Takeover),
        1 => Just(Action::Recover),
        1 => Just(Action::Checkpoint),
        1 => Just(Action::Restore),
    ];
    proptest::collection::vec(action, 1..180)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// Request/slot uniqueness and exact receipts survive adversarial histories.
    /// Catches op-ID-only deduplication, editable first CIDs, failure releasing a
    /// slot, duplicate Drawing claims and older restores permitting fresh entropy.
    /// JSON round-trips catch compound map keys breaking shard snapshots. Refusal
    /// precedence matches the API contract so a conflict cannot consume a slot.
    #[test]
    fn histories_never_reroll_or_replace_terminal_bytes(history in actions(), private in any::<bool>()) {
        let mut dice = fresh(private);
        let mut facts = Facts::default();
        let mut checkpoint = (dice.clone(), facts.clone());
        for action in history {
            match action {
                Action::Reserve { id, cid, op, old_epoch, authorized } => {
                    let epoch = if old_epoch { 0 } else { facts.epoch };
                    let expected = facts.reserve(id, cid, op, epoch, authorized);
                    let actual = dice.reserve(OperationId::new(op.to_string()), request(id, cid, epoch, private),
                        if authorized { Eligibility::Eligible } else { Eligibility::Unauthorized }).map(|_| ());
                    prop_assert_eq!(actual, expected);
                }
                Action::Begin { id, stale } => {
                    let fence = if stale { facts.fence.saturating_sub(1) } else { facts.fence };
                    prop_assert_eq!(dice.begin(&request(id, 0, 0, private).key, Fence(fence)), facts.begin(id, fence));
                }
                Action::Record { id, stale, answer } => {
                    let fence = if stale { facts.fence.saturating_sub(1) } else { facts.fence };
                    prop_assert_eq!(dice.record(&request(id, 0, 0, private).key, Fence(fence), answer.outcome()), facts.record(id, fence, answer));
                }
                Action::Publish { id, value } => {
                    prop_assert_eq!(dice.publish(&request(id, 0, 0, private).key, publication(id, value)), facts.publish(id, value));
                }
                Action::Takeover => {
                    facts.takeover();
                    dice.takeover(Fence(facts.fence)).unwrap();
                }
                Action::Recover => {
                    facts.recover(facts.epoch + 1, facts.fence + 1);
                    dice.recover(Epoch(facts.epoch), Fence(facts.fence)).unwrap();
                }
                Action::Checkpoint => checkpoint = (dice.clone(), facts.clone()),
                Action::Restore => {
                    let epoch = facts.epoch + 1;
                    let fence = facts.fence + 1;
                    (dice, facts) = checkpoint.clone();
                    facts.recover(epoch, fence);
                    dice.recover(Epoch(epoch), Fence(fence)).unwrap();
                }
            }
            facts.check(&dice, private);
        }
        // The shard stores JSON; compound map keys must not panic its snapshot path.
        let encoded = serde_json::to_vec(&dice).unwrap();
        let restored: Dice = serde_json::from_slice(&encoded).unwrap();
        prop_assert_eq!(restored, dice);
    }

    /// First terminal bytes win across arbitrary retries and takeover. Catches
    /// a later evaluator or publication attempt overwriting an unpublished roll.
    #[test]
    fn terminal_receipts_are_immutable(bytes in proptest::collection::vec(any::<u8>(), 0..256), other in any::<u8>()) {
        let mut dice = fresh(false);
        let req = request(0, 0, 0, false);
        dice.reserve(OperationId::new("first"), req.clone(), Eligibility::Eligible).unwrap();
        dice.begin(&req.key, Fence(0)).unwrap();
        let outcome = Outcome::Rolled(bytes);
        dice.record(&req.key, Fence(0), outcome).unwrap();
        dice.takeover(Fence(1)).unwrap();
        let first = dice.receipt(&req.key).unwrap().clone();
        let _ = dice.record(&req.key, Fence(1), Outcome::Rolled(vec![other]));
        prop_assert_eq!(dice.receipt(&req.key), Some(&first));
        prop_assert_eq!(dice.reserve(OperationId::new("fresh-op"), req.clone(), Eligibility::Unauthorized), Ok(&first));
        dice.publish(&req.key, publication(0, other)).unwrap();
        let published = dice.receipt(&req.key).unwrap().clone();
        let _ = dice.publish(&req.key, publication(0, other.wrapping_add(1)));
        prop_assert_eq!(dice.receipt(&req.key), Some(&published));
        dice.recover(Epoch(1), Fence(2)).unwrap();
        prop_assert_eq!(dice.receipt(&req.key), Some(&published));
        prop_assert_eq!(dice.reserve(OperationId::new("third-op"), req, Eligibility::Eligible), Ok(&published));
    }

    /// Restoring an apparently open opportunity cannot erase an unseen outcome.
    /// Catches epoch-in-slot keys and treating a missing old request as free-form.
    #[test]
    fn older_checkpoint_freezes_open_slots_and_missing_requests(private in any::<bool>(), id in 0u8..7) {
        let mut live = fresh(private);
        let mut old = live.clone();
        let req = request(id, 0, 0, private);
        live.reserve(OperationId::new("lost"), req.clone(), Eligibility::Eligible).unwrap();
        live.begin(&req.key, Fence(0)).unwrap();
        live.record(&req.key, Fence(0), Outcome::Rolled(vec![6])).unwrap();
        old.recover(Epoch(1), Fence(1)).unwrap();
        prop_assert_eq!(old.reserve(OperationId::new("retry"), req, Eligibility::Eligible), Err(Refusal::RecoveryUnresolved));
        let mut recreated = opportunity(id % 3, private);
        recreated.epoch = Epoch(1);
        recreated.reference = reference("new-opportunity");
        prop_assert_eq!(old.open(recreated), Err(Refusal::Conflict));
        let free = request(7, 0, 0, private);
        prop_assert_eq!(old.reserve(OperationId::new("free"), free, Eligibility::Eligible), Err(Refusal::RecoveryUnresolved));
    }

    /// A fenced takeover interrupts once; an older checkpoint does not invent
    /// an interrupted result. Catches restarting entropy from a Drawing marker
    /// or overwriting an unseen success when restoring that marker.
    #[test]
    fn drawing_cannot_be_reclaimed_after_takeover_or_restore(private in any::<bool>(), bytes in proptest::collection::vec(any::<u8>(), 0..32)) {
        let mut dice = fresh(private);
        let req = request(0, 0, 0, private);
        dice.reserve(OperationId::new("first"), req.clone(), Eligibility::Eligible).unwrap();
        dice.begin(&req.key, Fence(0)).unwrap();
        let mut checkpoint = dice.clone();
        prop_assert_eq!(dice.begin(&req.key, Fence(0)), Err(Refusal::WrongPhase));
        dice.takeover(Fence(1)).unwrap();
        let interrupted = dice.receipt(&req.key).unwrap().clone();
        prop_assert_eq!(&interrupted.phase, &Phase::Recorded(Outcome::Failed { code: Failure::Interrupted, details: Vec::new() }));
        prop_assert_eq!(dice.begin(&req.key, Fence(1)), Err(Refusal::WrongPhase));
        prop_assert_eq!(dice.record(&req.key, Fence(0), Outcome::Rolled(bytes.clone())), Err(Refusal::StaleFence));
        prop_assert_eq!(dice.record(&req.key, Fence(1), Outcome::Rolled(bytes.clone())), Err(Refusal::Conflict));
        dice.takeover(Fence(2)).unwrap();
        prop_assert_eq!(dice.receipt(&req.key), Some(&interrupted));
        checkpoint.recover(Epoch(1), Fence(1)).unwrap();
        prop_assert_eq!(&checkpoint.receipt(&req.key).unwrap().phase, &Phase::RecoveryUnresolved);
        prop_assert_eq!(checkpoint.begin(&req.key, Fence(1)), Err(Refusal::WrongPhase));
        prop_assert_eq!(checkpoint.record(&req.key, Fence(1), Outcome::Rolled(bytes)), Err(Refusal::WrongPhase));
    }

    /// Every failure consumes the restricted slot, including zero-face entropy
    /// failures. Catches cleanup that releases a slot after failed evaluation.
    #[test]
    fn failures_permanently_consume_the_winning_slot(code in 0u8..7, private in any::<bool>()) {
        let failure = match code {
            0 => Failure::InvalidNotation,
            1 => Failure::Unsupported,
            2 => Failure::LimitsExceeded,
            3 => Failure::ArithmeticOverflow,
            4 => Failure::EntropyUnavailable,
            5 => Failure::EntropyBudgetExhausted,
            _ => Failure::Interrupted,
        };
        let mut dice = fresh(private);
        let winner = request(0, 0, 0, private);
        dice.reserve(OperationId::new("winner"), winner.clone(), Eligibility::Eligible).unwrap();
        dice.begin(&winner.key, Fence(0)).unwrap();
        let outcome = Outcome::Failed { code: failure, details: vec![0, 255, code] };
        dice.record(&winner.key, Fence(0), outcome).unwrap();
        let first = dice.receipt(&winner.key).unwrap().clone();
        dice.takeover(Fence(1)).unwrap();
        prop_assert_eq!(dice.reserve(OperationId::new("retry"), winner.clone(), Eligibility::Eligible), Ok(&first));
        prop_assert_eq!(dice.reserve(OperationId::new("competitor"), request(3, 1, 0, private), Eligibility::Eligible), Err(Refusal::SlotConsumed));
        prop_assert_eq!(dice.begin(&winner.key, Fence(1)), Err(Refusal::WrongPhase));
    }

    /// Stable identity includes audience, author, collection and rkey. Catches
    /// accidental global deduplication by rkey or omission of Space identity.
    #[test]
    fn all_request_identity_dimensions_are_distinct(dimension in 0u8..5) {
        let mut dice = fresh(false);
        let first = request(7, 0, 0, false);
        let mut other = first.clone();
        match dimension {
            0 => other.key.audience = audience(true),
            1 => other.key.author = Did::new("another"),
            2 => other.key.collection = "another.request".into(),
            3 => other.key.rkey = "another".into(),
            _ => other.key.audience = Audience::Space { space: RecordUri::new("at://world/space/other") },
        }
        other.inputs.audience = other.key.audience.clone();
        other.result_uri = reference("result/other").uri;
        dice.reserve(OperationId::new("first"), first.clone(), Eligibility::Eligible).unwrap();
        dice.reserve(OperationId::new("other"), other.clone(), Eligibility::Eligible).unwrap();
        dice.begin(&first.key, Fence(0)).unwrap();
        dice.record(&first.key, Fence(0), Outcome::Rolled(vec![1])).unwrap();
        dice.begin(&other.key, Fence(0)).unwrap();
        dice.record(&other.key, Fence(0), Outcome::Rolled(vec![2])).unwrap();
        prop_assert_eq!(&dice.receipt(&first.key).unwrap().phase, &Phase::Recorded(Outcome::Rolled(vec![1])));
        prop_assert_eq!(&dice.receipt(&other.key).unwrap().phase, &Phase::Recorded(Outcome::Rolled(vec![2])));
    }

    /// One destination cannot represent two requests, while separate Spaces may
    /// reuse an AT-URI. Catches result overwrites and URI-only private deduplication.
    #[test]
    fn destinations_are_unique_within_their_audience(separate_space in any::<bool>()) {
        let mut dice = fresh(false);
        let first = request(7, 0, 0, false);
        let mut other = request(6, 0, 0, false);
        other.opportunity = None;
        other.result_uri = first.result_uri.clone();
        if separate_space {
            other.key.audience = audience(true);
            other.inputs.audience = audience(true);
        }
        dice.reserve(OperationId::new("first"), first, Eligibility::Eligible).unwrap();
        let before = dice.clone();
        let actual = dice.reserve(OperationId::new("other"), other, Eligibility::Eligible).map(|_| ());
        if separate_space { prop_assert_eq!(actual, Ok(())); }
        else {
            prop_assert_eq!(actual, Err(Refusal::Conflict));
            prop_assert_eq!(dice, before);
        }
    }

    /// Publication must use the captured repo, collection and rkey, but its CID
    /// comes from the signed bytes. Catches publishing a correct roll elsewhere.
    #[test]
    fn publication_matches_the_frozen_uri_not_the_request_cid(part in 0u8..3, value in any::<u8>()) {
        let mut dice = fresh(false);
        let req = request(0, 0, 0, false);
        dice.reserve(OperationId::new("first"), req.clone(), Eligibility::Eligible).unwrap();
        dice.begin(&req.key, Fence(0)).unwrap();
        dice.record(&req.key, Fence(0), Outcome::Rolled(vec![value])).unwrap();
        let before = dice.clone();
        let mut wrong = publication(0, value);
        wrong.reference.uri = RecordUri::new(match part {
            0 => "at://another/result/0",
            1 => "at://world/another/0",
            _ => "at://world/result/another",
        });
        prop_assert_eq!(dice.publish(&req.key, wrong), Err(Refusal::Conflict));
        prop_assert_eq!(&dice, &before);
        let mut valid = publication(0, value);
        valid.reference.cid = Cid::new("the-signed-result-cid");
        dice.publish(&req.key, valid.clone()).unwrap();
        prop_assert!(matches!(&dice.receipt(&req.key).unwrap().phase, Phase::Published { publication, .. } if publication == &valid), "the pinned URI accepts the signed result CID");
    }

    /// Recovery freezes old actions, not legitimate new play. Catches using a
    /// global recovery lock that also prevents a GM opening a new action.
    #[test]
    fn new_epoch_actions_can_roll_while_old_slots_remain_frozen(private in any::<bool>()) {
        let mut dice = fresh(private);
        dice.recover(Epoch(1), Fence(1)).unwrap();
        let mut new_opportunity = opportunity(3, private);
        new_opportunity.epoch = Epoch(1);
        dice.open(new_opportunity.clone()).unwrap();
        let mut req = request(6, 0, 1, private);
        req.opportunity = Some(new_opportunity.reference);
        req.inputs = new_opportunity.inputs;
        dice.reserve(OperationId::new("new-action"), req.clone(), Eligibility::Eligible).unwrap();
        dice.begin(&req.key, Fence(1)).unwrap();
        dice.record(&req.key, Fence(1), Outcome::Rolled(vec![6])).unwrap();
        dice.publish(&req.key, publication(6, 77)).unwrap();
        prop_assert!(matches!(dice.receipt(&req.key).unwrap().phase, Phase::Published { .. }), "new actions remain publishable after recovery");
        prop_assert_eq!(dice.slot(&slot(0)), Some(&SlotState::RecoveryUnresolved));
        prop_assert_eq!(dice.reserve(OperationId::new("old-action"), request(0, 0, 0, private), Eligibility::Eligible), Err(Refusal::RecoveryUnresolved));
    }

    /// Changing a pinned context never creates a second result. Catches matching
    /// only opportunity IDs while accepting a different actor, trace policy or CID.
    #[test]
    fn changed_bound_inputs_conflict_without_mutation(field in 0u8..13, private in any::<bool>()) {
        let mut dice = fresh(private);
        let req = request(0, 0, 0, private);
        dice.reserve(OperationId::new("first"), req.clone(), Eligibility::Eligible).unwrap();
        let before = dice.clone();
        let mut changed = req;
        match field {
            0 => changed.cid = Cid::new("different"),
            1 => changed.epoch = Epoch(1),
            2 => changed.inputs.expression = "3d6".into(),
            3 => changed.inputs.grammar = "other".into(),
            4 => changed.inputs.rules_version = "other".into(),
            5 => changed.inputs.sheet = Some(reference("sheet")),
            6 => changed.inputs.algorithm_version = "other".into(),
            7 => changed.inputs.limits_policy_version = "other".into(),
            8 => changed.inputs.audience_generation += 1,
            9 => changed.inputs.scene = SceneId::new("other"),
            10 => changed.opportunity = Some(reference("other")),
            11 => changed.world = Did::new("other"),
            _ => changed.result_uri = reference("result/other").uri,
        }
        prop_assert_eq!(dice.reserve(OperationId::new("retry"), changed, Eligibility::Eligible), Err(Refusal::Conflict));
        prop_assert_eq!(dice, before);
    }
}
