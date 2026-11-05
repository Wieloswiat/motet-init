//! Durable dice reservations, without entropy or publication I/O (ADR 0024).
//!
//! The shard serializes these transitions and persists the complete change before
//! starting entropy or returning a published result. Adapters validate bounded
//! envelopes, read-back CIDs and current authority in that same transaction.
//! Receipts are never evicted: admission quotas must reject overload beforehand.
//! Receipts grow with total requests, retry operation IDs and opportunities per
//! World, as do checkpoint and export costs. Slice 2 quotas must count attempts,
//! not only distinct requests. Pruning needs a separate reachability/compaction policy.
//! Normal transitions use ordered indexes; takeover and recovery scan retained
//! requests in O(requests) time at takeover. Recovery also checks each slot's
//! receipt, costing O(requests + opportunities * log(requests + 1)).
//! Publication is not fenced here: the shard must gate it on writer fencing and
//! log reconciliation, never permit a stale worker to commit these transitions.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{Cid, Did, OperationId, RecordUri, SceneId, StrongRef};

/// Recovery provenance, not part of a restricted slot's identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Epoch(pub u32);

/// A proven fenced writer generation. An HTTP timeout cannot advance it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Fence(pub u64);

/// Adapters supply canonical Space AT-URIs, including authority, type and key.
/// This pure module compares identities without parsing or normalization.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Audience {
    Public,
    /// The full authority/type/key Space URI; the author identifies its repo.
    Space {
        space: RecordUri,
    },
}

/// Neither the record CID nor the operation ID can create a fresh request.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct RequestKey {
    pub audience: Audience,
    pub author: Did,
    pub collection: String,
    pub rkey: String,
}

/// Action identity deliberately excludes its CID and the recovery epoch.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SlotKey {
    pub world: Did,
    pub action: RecordUri,
    pub actor: Did,
    pub slot: String,
}

/// Frozen inputs shared by an opportunity and its request.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Inputs {
    pub scene: SceneId,
    pub audience: Audience,
    pub audience_generation: u64,
    pub action: Option<StrongRef>,
    pub expression: String,
    pub grammar: String,
    pub rules_version: String,
    pub sheet: Option<StrongRef>,
    pub algorithm_version: String,
    pub limits_policy_version: String,
}

/// Opened by a separately authorized GM transaction, never by a roll request.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Opportunity {
    pub reference: StrongRef,
    pub slot: SlotKey,
    pub epoch: Epoch,
    pub inputs: Inputs,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Request {
    pub world: Did,
    pub key: RequestKey,
    pub cid: Cid,
    /// Read back from the immutable request's creation epoch, never replaced
    /// with the server's current epoch during a retry or recovery.
    pub epoch: Epoch,
    pub opportunity: Option<StrongRef>,
    pub inputs: Inputs,
    pub authority_revision: u64,
    /// Full destination URI captured from the audience at acceptance. The
    /// adapter supplies its canonical repo/collection/rkey, not a later lookup.
    pub result_uri: RecordUri,
}

/// The caller checks current cast, suspension and scene authority atomically.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Eligibility {
    Eligible,
    Unauthorized,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Failure {
    InvalidNotation,
    Unsupported,
    LimitsExceeded,
    ArithmeticOverflow,
    EntropyUnavailable,
    EntropyBudgetExhausted,
    Interrupted,
}

/// Exact bounded, encoded result fields, including faces, trace and timestamps.
/// The adapter validates the result schema; this module never evaluates faces.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Outcome {
    Rolled(Vec<u8>),
    Failed { code: Failure, details: Vec<u8> },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Publication {
    pub reference: StrongRef,
    pub signed_record: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Phase {
    Accepted,
    Drawing(Fence),
    Recorded(Outcome),
    Published { outcome: Outcome, publication: Publication },
    RecoveryUnresolved,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Receipt {
    pub request: Request,
    pub phase: Phase,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SlotState {
    Open,
    Reserved(RequestKey),
    RecoveryUnresolved,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Slot {
    opportunity: Opportunity,
    state: SlotState,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum Refusal {
    #[error("the caller is not authorized")]
    Unauthorized,
    #[error("the immutable request or operation payload changed")]
    Conflict,
    #[error("the opportunity does not match the request")]
    OpportunityMismatch,
    #[error("the restricted slot is already reserved")]
    SlotConsumed,
    #[error("recovery cannot establish whether this request or slot was used")]
    RecoveryUnresolved,
    #[error("the request or opportunity is unknown")]
    Unknown,
    #[error("the transition is not valid in this phase")]
    WrongPhase,
    #[error("the writer has not proved a newer fence")]
    StaleFence,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dice {
    world: Did,
    epoch: Epoch,
    fence: Fence,
    #[serde(with = "entries")]
    requests: BTreeMap<RequestKey, Receipt>,
    operations: BTreeMap<OperationId, RequestKey>,
    #[serde(with = "entries")]
    slots: BTreeMap<SlotKey, Slot>,
    #[serde(with = "entries")]
    opportunities: BTreeMap<StrongRef, SlotKey>,
    #[serde(with = "entries")]
    result_uris: BTreeMap<(Audience, RecordUri), RequestKey>,
}

impl Dice {
    #[must_use]
    pub const fn new(world: Did, epoch: Epoch, fence: Fence) -> Self {
        Self {
            world,
            epoch,
            fence,
            requests: BTreeMap::new(),
            operations: BTreeMap::new(),
            slots: BTreeMap::new(),
            opportunities: BTreeMap::new(),
            result_uris: BTreeMap::new(),
        }
    }

    /// Current epoch for newly created requests. Old requests retain their epoch.
    #[must_use]
    pub const fn epoch(&self) -> Epoch {
        self.epoch
    }

    /// Internal receipt lookup. The serving adapter must enforce audience access.
    #[must_use]
    pub fn receipt(&self, key: &RequestKey) -> Option<&Receipt> {
        self.requests.get(key)
    }

    #[must_use]
    pub fn slot(&self, key: &SlotKey) -> Option<&SlotState> {
        self.slots.get(key).map(|slot| &slot.state)
    }

    /// Opens one immutable slot after GM authorization and expression validation.
    /// Reopening an existing action, even in a new epoch, is a conflict.
    pub fn open(&mut self, opportunity: Opportunity) -> Result<(), Refusal> {
        if let Some(slot) = self.slots.get(&opportunity.slot) {
            return if slot.opportunity == opportunity {
                Ok(())
            } else {
                Err(Refusal::Conflict)
            };
        }
        if opportunity.epoch != self.epoch {
            return Err(Refusal::RecoveryUnresolved);
        }
        if opportunity.slot.world != self.world
            || opportunity.inputs.action.as_ref().map(|action| &action.uri) != Some(&opportunity.slot.action)
            || self.opportunities.contains_key(&opportunity.reference)
        {
            return Err(Refusal::OpportunityMismatch);
        }
        self.opportunities
            .insert(opportunity.reference.clone(), opportunity.slot.clone());
        self.slots.insert(
            opportunity.slot.clone(),
            Slot {
                opportunity,
                state: SlotState::Open,
            },
        );
        Ok(())
    }

    /// Reserves only eligible, read-back requests. A retry can use a fresh
    /// operation ID but cannot change any frozen input. Existing receipts survive
    /// membership removal; returning their content still requires audience access.
    pub fn reserve(
        &mut self,
        op: OperationId,
        request: Request,
        eligibility: Eligibility,
    ) -> Result<&Receipt, Refusal> {
        if self.operations.get(&op).is_some_and(|key| key != &request.key) {
            return Err(Refusal::Conflict);
        }
        let request_key = request.key.clone();
        if let Some(receipt) = self.requests.get(&request_key) {
            if receipt.request != request {
                return Err(Refusal::Conflict);
            }
        } else {
            if eligibility == Eligibility::Unauthorized {
                return Err(Refusal::Unauthorized);
            }
            if request.epoch != self.epoch {
                return Err(Refusal::RecoveryUnresolved);
            }
            if request.world != self.world || request.key.audience != request.inputs.audience {
                return Err(Refusal::OpportunityMismatch);
            }
            let destination = (request.inputs.audience.clone(), request.result_uri.clone());
            if self.result_uris.contains_key(&destination) {
                return Err(Refusal::Conflict);
            }
            if let Some(reference) = &request.opportunity {
                let slot_key = self.opportunities.get(reference).ok_or(Refusal::Unknown)?;
                let slot = self.slots.get_mut(slot_key).ok_or(Refusal::Unknown)?;
                if slot_key.actor != request.key.author
                    || slot.opportunity.inputs != request.inputs
                    || slot.opportunity.epoch != request.epoch
                {
                    return Err(Refusal::OpportunityMismatch);
                }
                match slot.state {
                    SlotState::Open => slot.state = SlotState::Reserved(request.key.clone()),
                    SlotState::Reserved(_) => return Err(Refusal::SlotConsumed),
                    SlotState::RecoveryUnresolved => return Err(Refusal::RecoveryUnresolved),
                }
            }
            self.result_uris.insert(destination, request_key.clone());
            self.requests.insert(
                request_key.clone(),
                Receipt {
                    request,
                    phase: Phase::Accepted,
                },
            );
        }
        // Keep aliases so reusing a retry's operation ID cannot mutate another request.
        self.operations.insert(op, request_key.clone());
        self.requests.get(&request_key).ok_or(Refusal::Unknown)
    }

    /// Claims entropy exactly once. A duplicate claim is refused, even with the
    /// same fence: a retried worker must not interpret success as a second draw.
    pub fn begin(&mut self, key: &RequestKey, fence: Fence) -> Result<(), Refusal> {
        if fence != self.fence {
            return Err(Refusal::StaleFence);
        }
        let receipt = self.requests.get_mut(key).ok_or(Refusal::Unknown)?;
        if receipt.phase != Phase::Accepted {
            return Err(Refusal::WrongPhase);
        }
        receipt.phase = Phase::Drawing(fence);
        Ok(())
    }

    /// Stores the first complete outcome. Retries cannot replace its bytes.
    pub fn record(&mut self, key: &RequestKey, fence: Fence, outcome: Outcome) -> Result<(), Refusal> {
        if fence != self.fence {
            return Err(Refusal::StaleFence);
        }
        let receipt = self.requests.get_mut(key).ok_or(Refusal::Unknown)?;
        match &receipt.phase {
            Phase::Drawing(claim) if *claim == fence => (),
            Phase::Recorded(first) | Phase::Published { outcome: first, .. } => {
                return if first == &outcome {
                    Ok(())
                } else {
                    Err(Refusal::Conflict)
                };
            }
            _ => return Err(Refusal::WrongPhase),
        }
        receipt.phase = Phase::Recorded(outcome);
        // ADR 0024 owner decision 2 keeps the slot reserved even for zero-face
        // failures. A corrected attempt needs an explicit new action.
        Ok(())
    }

    /// Stores exact signed bytes and reference after the publication transaction.
    /// Only Published receipts may be exposed as terminal API responses.
    pub fn publish(&mut self, key: &RequestKey, publication: Publication) -> Result<(), Refusal> {
        let receipt = self.requests.get_mut(key).ok_or(Refusal::Unknown)?;
        if publication.reference.uri != receipt.request.result_uri {
            return Err(Refusal::Conflict);
        }
        match &receipt.phase {
            Phase::Published { publication: first, .. } => {
                return if first == &publication {
                    Ok(())
                } else {
                    Err(Refusal::Conflict)
                };
            }
            Phase::Recorded(_) => (),
            _ => return Err(Refusal::WrongPhase),
        }
        match std::mem::replace(&mut receipt.phase, Phase::Accepted) {
            Phase::Recorded(outcome) => {
                receipt.phase = Phase::Published { outcome, publication };
                Ok(())
            }
            previous => {
                receipt.phase = previous;
                Err(Refusal::WrongPhase)
            }
        }
    }

    /// Requires external proof that the old writer cannot commit, and resolution
    /// of ambiguous log appends. It is not a response to a client timeout.
    pub fn takeover(&mut self, fence: Fence) -> Result<(), Refusal> {
        if fence <= self.fence {
            return Err(Refusal::StaleFence);
        }
        self.fence = fence;
        for receipt in self.requests.values_mut() {
            if matches!(receipt.phase, Phase::Drawing(_)) {
                receipt.phase = Phase::Recorded(Outcome::Failed {
                    code: Failure::Interrupted,
                    details: Vec::new(),
                });
            }
        }
        Ok(())
    }

    /// Declares an older-checkpoint recovery after fencing. Every uncertain old
    /// opportunity freezes, including those appearing open in the export.
    /// Known outcomes and publication receipts remain byte-identical. Missing
    /// old-epoch requests cannot enter through reserve as fresh free-form rolls.
    pub fn recover(&mut self, epoch: Epoch, fence: Fence) -> Result<(), Refusal> {
        if epoch <= self.epoch {
            return Err(Refusal::Conflict);
        }
        if fence <= self.fence {
            return Err(Refusal::StaleFence);
        }
        self.fence = fence;
        self.epoch = epoch;
        for receipt in self.requests.values_mut() {
            // An older Drawing marker may hide a post-checkpoint success.
            if matches!(receipt.phase, Phase::Accepted | Phase::Drawing(_)) {
                receipt.phase = Phase::RecoveryUnresolved;
            }
        }
        for slot in self.slots.values_mut() {
            let known = match &slot.state {
                SlotState::Reserved(key) => self
                    .requests
                    .get(key)
                    .is_some_and(|receipt| matches!(receipt.phase, Phase::Recorded(_) | Phase::Published { .. })),
                SlotState::Open | SlotState::RecoveryUnresolved => false,
            };
            if !known {
                slot.state = SlotState::RecoveryUnresolved;
            }
        }
        Ok(())
    }
}

// JSON object keys cannot encode compound identities. Ordered entry sequences
// preserve them without stringifying or dropping any identity fields.
mod entries {
    use std::collections::BTreeMap;
    use std::fmt;
    use std::marker::PhantomData;

    use serde::de::{Error, SeqAccess, Visitor};
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub(super) fn serialize<K: Serialize, V: Serialize, S: Serializer>(
        map: &BTreeMap<K, V>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        serializer.collect_seq(map.iter())
    }

    pub(super) fn deserialize<'de, K: Deserialize<'de> + Ord, V: Deserialize<'de>, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<BTreeMap<K, V>, D::Error> {
        struct Entries<K, V>(PhantomData<(K, V)>);

        impl<'de, K: Deserialize<'de> + Ord, V: Deserialize<'de>> Visitor<'de> for Entries<K, V> {
            type Value = BTreeMap<K, V>;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("ordered unique key/value entries")
            }

            fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<Self::Value, A::Error> {
                let mut map = BTreeMap::new();
                while let Some((key, value)) = sequence.next_element()? {
                    if map.insert(key, value).is_some() {
                        return Err(A::Error::custom("duplicate state key"));
                    }
                }
                Ok(map)
            }
        }

        deserializer.deserialize_seq(Entries(PhantomData))
    }
}
