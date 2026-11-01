//! Publish durable rules decisions as World records.
//!
//! Each decision captures its audience and a shard sequence in the same log
//! operation. The audience never follows later scene moves or Space recreation.
//! Publication, its receipt and queue removal form another atomic operation.
//! Operation identities deduplicate retries without suppressing accepted edits
//! that return to earlier bytes. Per-post predecessor receipts preserve history
//! order even when an earlier publication fails.

use std::time::Duration;

use atproto_record::{datetime::Datetime, tid::Tid};
use atproto_space::SpaceUri;
use bytes::Bytes;
use motet_lexicons::Record;
use motet_lexicons::at_motet_pilot_admission::{Admission as AdmissionRecord, Place};
use motet_lexicons::at_motet_pilot_closing::Closing;
use motet_lexicons::at_motet_pilot_scene::Scene;
use motet_lexicons::com_atproto_repo_strongref::StrongRef;
use motet_rules::world::{Admission, Outcome};
use motet_rules::{Did, OperationId};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::task::JoinHandle;

use crate::repo::{self, RepoError};
use crate::shard::{Handle, Op, SubmitError, Txn, TxnError};
use crate::spaces::{self, SpaceError, SpaceInfo, SpaceWrite};
use crate::store::{ReadError, keys};

/// Queue failure aborts the enclosing rules operation before acknowledgement.
#[derive(Debug, Error)]
pub enum QueueError {
    #[error(transparent)]
    Transaction(#[from] TxnError),
    #[error(transparent)]
    Space(#[from] SpaceError),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("invalid publication queue state: {0}")]
    State(&'static str),
}

#[derive(Debug, Error)]
pub enum PublicationError {
    #[error("the scene had no unique audience when the post was admitted")]
    UnknownAudience,
    #[error("the admission's Space generation is no longer live")]
    StaleAudience,
    #[error("the post's preceding decision has not been published")]
    PredecessorUnpublished,
    #[error("the rules position or authority revision exceeds the lexicon integer range")]
    IntegerRange,
    #[error(transparent)]
    Transaction(#[from] TxnError),
    #[error(transparent)]
    Read(#[from] ReadError),
    #[error(transparent)]
    Submit(#[from] SubmitError),
    #[error(transparent)]
    Repo(#[from] RepoError),
    #[error(transparent)]
    Space(#[from] SpaceError),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Queue(#[from] QueueError),
}

/// The audience at admission time; unknown locations never default to public.
#[derive(Debug, Deserialize, Serialize)]
pub enum Destination {
    Public,
    Space { uri: SpaceUri, generation: String },
    Unknown,
}

#[derive(Deserialize)]
struct Intent {
    sequence: u64,
    world: Did,
    admission: Admission,
    closing: bool,
    previous: Option<u64>,
    destination: Destination,
}

/// The immutable record address and decision-time destination.
#[derive(Debug, Deserialize, Serialize)]
pub struct Receipt {
    pub rkey: String,
    pub record: StrongRef,
    pub destination: Destination,
}

fn sequence(value: &[u8]) -> Result<u64, QueueError> {
    Ok(u64::from_be_bytes(
        value
            .try_into()
            .map_err(|_| QueueError::State("sequence is not a u64"))?,
    ))
}

/// Capture the decision identity and audience while the command owns the writer.
/// The operation's effects are logged, so replay never re-resolves the audience.
pub(crate) async fn enqueue(
    txn: &mut Txn<'_>,
    world: &Did,
    operation: &OperationId,
    outcome: &Outcome,
) -> Result<(), QueueError> {
    #[derive(Serialize)]
    struct Pending<'a> {
        sequence: u64,
        world: &'a Did,
        admission: &'a Admission,
        closing: bool,
        previous: Option<u64>,
        destination: &'a Destination,
    }
    let (admission, closing) = match outcome {
        Outcome::Admitted(admission) => (admission, false),
        Outcome::Closed(admission) => (admission, true),
        _ => return Ok(()),
    };
    let operation_key = keys::publication_operation(world, operation);
    if txn.get(&operation_key).await?.is_some() {
        return Ok(());
    }
    let latest = keys::publication_latest(world, &admission.post.uri);
    let previous = txn.get(&latest).await?.map(|raw| sequence(&raw)).transpose()?;
    let destination = if closing {
        prior_destination(
            txn,
            previous.ok_or(QueueError::State("closing has no admission decision"))?,
        )
        .await?
    } else {
        destination(txn, world, admission.scene.as_str()).await?
    };
    let next = txn
        .get(keys::PUBLICATION_SEQ)
        .await?
        .map_or(Ok(0), |raw| sequence(&raw))?;
    let following = next
        .checked_add(1)
        .ok_or(QueueError::State("decision sequence exhausted"))?;
    let value = serde_json::to_vec(&Pending {
        sequence: next,
        world,
        admission,
        closing,
        previous,
        destination: &destination,
    })?;
    let encoded = Bytes::copy_from_slice(&next.to_be_bytes());
    txn.put(keys::publication_queue(next), value.into());
    txn.put(operation_key, encoded.clone());
    txn.put(latest, encoded);
    txn.put(
        keys::PUBLICATION_SEQ.to_vec(),
        Bytes::copy_from_slice(&following.to_be_bytes()),
    );
    Ok(())
}

async fn prior_destination(txn: &Txn<'_>, previous: u64) -> Result<Destination, QueueError> {
    if let Some(value) = txn.get(&keys::publication(previous)).await? {
        return Ok(serde_json::from_slice::<Receipt>(&value)?.destination);
    }
    let value = txn
        .get(&keys::publication_queue(previous))
        .await?
        .ok_or(QueueError::State("preceding admission has neither intent nor receipt"))?;
    Ok(serde_json::from_slice::<Intent>(&value)?.destination)
}

async fn destination(txn: &Txn<'_>, world: &Did, scene: &str) -> Result<Destination, QueueError> {
    let public = txn.get(&keys::repo_record(world, Scene::NSID, scene)).await?.is_some();
    let mut private = None;
    for (_, value) in txn.scan(&keys::space_inventory(world), usize::MAX).await? {
        let info: SpaceInfo = serde_json::from_slice(&value)?;
        if !info.deleted
            && spaces::record_txn(txn, &info.space, Scene::NSID, scene)
                .await?
                .is_some()
        {
            if public || private.is_some() {
                return Ok(Destination::Unknown);
            }
            private = Some(Destination::Space {
                uri: info.space,
                generation: info.generation,
            });
        }
    }
    Ok(private.unwrap_or(if public {
        Destination::Public
    } else {
        Destination::Unknown
    }))
}

/// Publish one queued decision, only after its post's predecessor is published.
/// A missing queue item or an existing decision receipt is a successful no-op.
#[derive(Debug)]
pub struct Publish {
    pub key: Bytes,
}

impl Op for Publish {
    type Output = bool;
    type Error = PublicationError;

    async fn run(self, txn: &mut Txn<'_>) -> Result<bool, PublicationError> {
        if !self.key.starts_with(keys::PUBLICATIONS) {
            return Ok(false);
        }
        let Some(value) = txn.get(&self.key).await? else {
            return Ok(false);
        };
        let intent: Intent = serde_json::from_slice(&value)?;
        let index = keys::publication(intent.sequence);
        if txn.get(&index).await?.is_some() {
            txn.delete(self.key.to_vec());
            return Ok(false);
        }
        let previous = if let Some(previous) = intent.previous {
            let value = txn
                .get(&keys::publication(previous))
                .await?
                .ok_or(PublicationError::PredecessorUnpublished)?;
            Some(serde_json::from_slice::<Receipt>(&value)?)
        } else {
            None
        };
        if let Destination::Space { uri, generation } = &intent.destination {
            let info = spaces::info_txn(txn, uri).await?;
            if info.deleted || info.generation != *generation {
                return Err(PublicationError::StaleAudience);
            }
        }
        let receipt = if intent.closing {
            let closing = Closing {
                admission: previous
                    .ok_or(QueueError::State("closing has no admission receipt"))?
                    .record,
                post: post_ref(&intent.admission),
                scene: intent.admission.scene.to_string(),
                place: place(&intent.admission)?,
                created_at: Datetime::now().into_string(),
                extra: serde_json::Map::new(),
            };
            write_record(txn, &intent.world, intent.destination, &closing).await?
        } else {
            let record = admission_record(&intent.admission)?;
            write_record(txn, &intent.world, intent.destination, &record).await?
        };
        txn.put(index, serde_json::to_vec(&receipt)?.into());
        txn.delete(self.key.to_vec());
        Ok(true)
    }
}

fn post_ref(admission: &Admission) -> StrongRef {
    StrongRef {
        uri: admission.post.uri.to_string(),
        cid: admission.post.cid.to_string(),
        extra: serde_json::Map::new(),
    }
}

fn place(admission: &Admission) -> Result<Place, PublicationError> {
    Ok(Place {
        epoch: i64::from(admission.place.epoch),
        seq: i64::try_from(admission.place.seq).map_err(|_| PublicationError::IntegerRange)?,
        extra: serde_json::Map::new(),
    })
}

fn admission_record(admission: &Admission) -> Result<AdmissionRecord, PublicationError> {
    Ok(AdmissionRecord {
        post: post_ref(admission),
        author: admission.author.to_string(),
        scene: admission.scene.to_string(),
        place: place(admission)?,
        authority_rev: i64::try_from(admission.authority_rev).map_err(|_| PublicationError::IntegerRange)?,
        replaces: admission.replaces.as_ref().map(ToString::to_string),
        created_at: Datetime::now().into_string(),
        extra: serde_json::Map::new(),
    })
}

async fn write_record<T: Record + Serialize + Sync>(
    txn: &mut Txn<'_>,
    world: &Did,
    destination: Destination,
    record: &T,
) -> Result<Receipt, PublicationError> {
    let rkey = Tid::new().to_string();
    let record = match &destination {
        Destination::Space { uri, .. } => {
            let write = SpaceWrite::create(T::NSID, &rkey, record)?;
            let committed = spaces::write_as_world(txn, uri, vec![write]).await?;
            let result = committed
                .results
                .into_iter()
                .next()
                .expect("one create returns one result");
            StrongRef {
                uri: result.uri,
                cid: result.cid.expect("a successful create has a CID"),
                extra: serde_json::Map::new(),
            }
        }
        Destination::Public => {
            let mut value = serde_json::to_value(record)?;
            value
                .as_object_mut()
                .ok_or_else(|| RepoError::InvalidRecord("publication must be a record object".into()))?
                .insert("$type".into(), T::NSID.into());
            let committed = repo::commit(
                txn,
                world,
                vec![repo::Write::Create {
                    collection: repo::Nsid::new(T::NSID)?,
                    rkey: rkey.clone(),
                    record: repo::record_from_json(&value)?,
                }],
                None,
            )
            .await?;
            let (uri, cid) = committed
                .results
                .into_iter()
                .next()
                .expect("one create returns one result");
            StrongRef {
                uri,
                cid: cid.expect("a successful create has a CID").to_string(),
                extra: serde_json::Map::new(),
            }
        }
        Destination::Unknown => return Err(PublicationError::UnknownAudience),
    };
    Ok(Receipt {
        rkey,
        record,
        destination,
    })
}

/// Results of one bounded-page pass. Failed records retain their queue position.
#[derive(Debug, Default)]
pub struct Drained {
    pub published: usize,
    pub failed: usize,
}

/// Drain committed decisions in shard sequence order, including prior processes.
///
/// Per-record failures retain the intent and block its post's later decisions,
/// not independent posts. Storage or writer loss returns an error to supervision.
pub async fn drain(handle: &Handle) -> Result<Drained, PublicationError> {
    let mut drained = Drained::default();
    let mut after: Option<Bytes> = None;
    loop {
        let page = handle.reader().scan(keys::PUBLICATIONS, after.as_deref(), 128).await?;
        if page.is_empty() {
            return Ok(drained);
        }
        for (key, _) in page {
            after = Some(key.clone());
            match handle.run(Publish { key }).await? {
                Ok(true) => drained.published += 1,
                Ok(false) => {}
                Err(error) => {
                    tracing::warn!(%error, "publication remains queued");
                    drained.failed += 1;
                }
            }
        }
    }
}

/// Start the recovering publisher, with failed passes backing off to one minute.
///
/// Poll for new decisions immediately, then at one-second intervals.
/// Aborting is safe: the writer makes publication and its receipt durable
/// together, even if the caller leaves.
#[must_use]
pub fn spawn(handle: Handle) -> JoinHandle<Result<(), PublicationError>> {
    tokio::spawn(async move {
        let mut delay = Duration::from_secs(1);
        loop {
            let result = drain(&handle).await?;
            tokio::time::sleep(delay).await;
            delay = if result.failed == 0 {
                Duration::from_secs(1)
            } else {
                (delay * 2).min(Duration::from_secs(60))
            };
        }
    })
}
