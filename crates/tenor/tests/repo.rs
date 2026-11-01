//! Repository properties through the real single writer and independent
//! atproto consumer: signed state, incremental events and conditional writes.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use atproto_dasl::{BlockStorage, CarBlock, CarReader, CarWriter, Cid, MemoryStorage};
use atproto_identity::key::{KeyData, to_public};
use atproto_record::tid::Tid;
use atproto_repo::{Commit, MemoryRepository, Mst, MstNode, RecordPath, RepoConfig};
use bytes::Bytes;
use motet_rules::Did;
use motet_tenor::event::{Action, Event, Sequenced};
use motet_tenor::keys::{MasterKey, Purpose};
use motet_tenor::repo::{self, ApplyWrites, Committed, Nsid, RepoError, Write};
use motet_tenor::shard::{Handle, Op, Options, Shard, Txn, TxnError, spawn};
use motet_tenor::store::{Reader, keys};
use motet_tenor::sync;
use object_store::memory::InMemory;
use proptest::prelude::*;
use serde_json::{Value, json};

const COLLECTION: &str = "group.opensocial.rule";
const WORLD: &str = "did:plc:abcdefghijklmnopqrstuvwx";

struct Begin;

impl Op for Begin {
    type Output = Committed;
    type Error = RepoError;

    async fn run(self, txn: &mut Txn<'_>) -> Result<Committed, RepoError> {
        let world = Did::new(WORLD);
        txn.begin_world(world.clone(), 1)
            .expect("fixture starts a new World command stream");
        txn.generate_key(&world, Purpose::Signing).map_err(TxnError::from)?;
        repo::genesis(txn, &world).await
    }
}

async fn fixture() -> (Handle, KeyData) {
    let shard = Shard::open(
        Arc::new(InMemory::new()),
        "repo",
        Arc::new(MasterKey::from_bytes(&[9; 32])),
    )
    .await
    .expect("fresh in-memory repository shard opens");
    let handle = spawn(
        shard,
        Options {
            max_batch: 8,
            checkpoint_every: Duration::from_secs(3600),
        },
    );
    handle
        .run(Begin)
        .await
        .expect("shard accepts fixture setup")
        .expect("fixture creates the World repository");
    let private = handle
        .reader()
        .key(&Did::new(WORLD), Purpose::Signing)
        .await
        .expect("fixture signing key can be read")
        .expect("fixture generated a signing key");
    (
        handle,
        to_public(&private).expect("generated signing key has a public key"),
    )
}

fn rkey(slot: u8) -> String {
    Tid::from_parts(1_700_000_000_000_000 + u64::from(slot), 0).to_string()
}

fn value(number: u16) -> Value {
    let foreign = atproto_dasl::cid::compute_cid(b"a foreign block this repo does not hold");
    json!({
        "$type": COLLECTION,
        "title": format!("Rule {number}"),
        "text": "Be kind",
        "order": number,
        "bytes": { "$bytes": "AQID" },
        // Records may have unknown fields that resemble an MST node. Their
        // links still must not be treated as repository structural links.
        "e": [],
        "l": { "$link": foreign.to_string() }
    })
}

fn create(slot: u8, number: u16) -> Write {
    Write::Create {
        collection: Nsid::new(COLLECTION).expect("fixture collection is a valid NSID"),
        rkey: rkey(slot),
        record: repo::record_from_json(&value(number)).expect("fixture record is valid DAG-CBOR JSON"),
    }
}

async fn decode_car(bytes: &[u8]) -> (Cid, Vec<CarBlock>) {
    let mut reader = CarReader::new(bytes)
        .await
        .expect("repository output has a valid CAR header");
    let root = *reader.root().expect("repository CAR declares a root");
    let mut blocks = Vec::new();
    while let Some(block) = reader.next_block().await.expect("repository CAR blocks decode") {
        blocks.push(block);
    }
    (root, blocks)
}

async fn consumer_car(root: Cid, blocks: &BTreeMap<Cid, Vec<u8>>) -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut writer = CarWriter::new(&mut bytes, vec![root])
        .await
        .expect("consumer CAR header writes to memory");
    for (cid, data) in blocks {
        writer
            .write_block(&CarBlock {
                cid: *cid,
                data: data.clone(),
            })
            .await
            .expect("consumer CAR block writes to memory");
    }
    writer.finish().await.expect("consumer CAR finalizes in memory");
    bytes
}

async fn assert_repo(car: &[u8], public: &KeyData, expected: &BTreeMap<u8, u16>) {
    let repo = MemoryRepository::from_car_verified(car, RepoConfig::default(), public, WORLD)
        .await
        .expect("independent consumer verifies the World repository");
    assert_eq!(
        repo.record_count()
            .await
            .expect("verified repository records can be counted"),
        expected.len()
    );
    for (slot, number) in expected {
        let path = RecordPath::new(COLLECTION, rkey(*slot));
        assert_eq!(
            repo.get_record(&path)
                .await
                .expect("verified repository record can be read"),
            Some(value(*number))
        );
    }
}

/// A consumer can invert just the event's operations using its covering MST
/// nodes. Missing unchanged sibling nodes make a deletion/insertion fail here.
async fn assert_covering_proof(event: &motet_tenor::event::Commit, blocks: &[CarBlock]) {
    let root: Cid = event.commit.parse().expect("commit event names a valid CID");
    atproto_repo::verify_slice_integrity(root, blocks).expect("event blocks match their CIDs and contain the root");
    let signed = Commit::from_bytes(
        &blocks
            .iter()
            .find(|b| b.cid == root)
            .expect("event contains its commit block")
            .data,
    )
    .expect("event root decodes as a signed commit");
    let ops: Vec<_> = event
        .ops
        .iter()
        .map(|op| atproto_repo::RepoOp {
            action: match op.action {
                Action::Create => atproto_repo::RepoOpAction::Create,
                Action::Update => atproto_repo::RepoOpAction::Update,
                Action::Delete => atproto_repo::RepoOpAction::Delete,
            },
            path: op.path.clone(),
            cid: op
                .cid
                .as_deref()
                .map(str::parse)
                .transpose()
                .expect("operation record CID is valid"),
            prev: op
                .prev
                .as_deref()
                .map(str::parse)
                .transpose()
                .expect("operation previous record CID is valid"),
        })
        .collect();
    let included = atproto_repo::verify_op_inclusion(signed.data, &ops, blocks, &[])
        .await
        .expect("event MST proof supports operation inclusion checks");
    assert!(included.iter().all(|op| op.verified));
    if ops.is_empty() {
        return;
    }
    let mut storage = MemoryStorage::new();
    for block in blocks {
        storage
            .put(&block.cid, block.data.clone())
            .await
            .expect("proof block stores in consumer memory");
    }
    let mut inverted = Mst::from_root(signed.data, storage, RepoConfig::default());
    for op in ops.iter().rev() {
        match op.action {
            atproto_repo::RepoOpAction::Create => {
                inverted
                    .delete(&op.path)
                    .await
                    .expect("covering proof supports reversing a create");
            }
            atproto_repo::RepoOpAction::Update | atproto_repo::RepoOpAction::Delete => {
                inverted
                    .insert(&op.path, op.prev.expect("update or delete names the previous record"))
                    .await
                    .expect("covering proof supports restoring the previous record");
            }
            atproto_repo::RepoOpAction::Other(_) => panic!("the producer emitted an unknown repo action"),
        }
    }
    let empty = atproto_dasl::cid::compute_cid(
        &atproto_dasl::to_vec(&MstNode::empty()).expect("empty MST node encodes as DAG-CBOR"),
    );
    let previous = inverted.root().copied().unwrap_or(empty);
    assert_eq!(Some(previous.to_string()), event.prev_data);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(16))]

    /// Random creates, updates and deletions yield exactly the model's records
    /// and a valid World signature. Replaying only commit CARs and ops reaches
    /// the same state as getRepo; omitted proof blocks, wrong prev CIDs, broken
    /// MST updates or JSON sentinel encoding all make this fail.
    #[test]
    fn random_writes_verify_and_replay_from_genesis(
        batches in proptest::collection::vec(proptest::collection::btree_map(0u8..24, proptest::option::of(any::<u16>()), 1..8), 1..15)
    ) {
        tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(async {
            let (handle, public) = fixture().await;
            let world = Did::new(WORLD);
            let initial = sync::repo_car(handle.reader(), &world, None).await.unwrap();
            let (_, initial_blocks) = decode_car(&initial).await;
            let mut retained: BTreeMap<_, _> = initial_blocks.into_iter().map(|b| (b.cid, b.data)).collect();
            let mut model = BTreeMap::new();
            assert_repo(&initial, &public, &model).await;
            let mut replayed: BTreeMap<String, (Cid, Value)> = BTreeMap::new();
            let mut prior = repo::head(handle.reader(), &world).await.unwrap().unwrap();
            let mut after = None;
            for (key, bytes) in handle.reader().scan(keys::EVENTS, None, usize::MAX).await.unwrap() {
                let event: Sequenced = serde_json::from_slice(&bytes).unwrap();
                let Event::Sync(sync) = event.event else { panic!("genesis must re-anchor a consumer") };
                let (cid, blocks) = decode_car(&sync.blocks).await;
                assert_eq!(cid, prior.cid);
                assert_eq!(blocks.len(), 1);
                after = Some(key);
            }
            for batch in batches {
                let mut writes = Vec::new();
                for (slot, next) in batch {
                    match (model.get(&slot), next) {
                        (None, None) => {}
                        (None, Some(number)) => { writes.push(create(slot, number)); model.insert(slot, number); }
                        (Some(_), Some(number)) => {
                            writes.push(Write::Update { collection: Nsid::new(COLLECTION).unwrap(), rkey: rkey(slot), record: repo::record_from_json(&value(number)).unwrap(), swap: None });
                            model.insert(slot, number);
                        }
                        (Some(_), None) => {
                            writes.push(Write::Delete { collection: Nsid::new(COLLECTION).unwrap(), rkey: rkey(slot), swap: None });
                            model.remove(&slot);
                        }
                    }
                }
                let committed = handle.run(ApplyWrites { world: world.clone(), writes, swap_commit: Some(prior.cid) }).await.unwrap().unwrap();
                assert!(committed.rev > prior.rev, "a relay never ignores a new commit as stale");
                let events = handle.reader().scan(keys::EVENTS, after.as_deref(), usize::MAX).await.unwrap();
                assert_eq!(events.len(), 1);
                for (key, encoded) in events {
                    let sequenced: Sequenced = serde_json::from_slice(&encoded).unwrap();
                    let Event::Commit(event) = sequenced.event else { panic!("a write must publish a commit") };
                    assert_eq!(event.since, Some(prior.rev.to_string()));
                    assert_eq!(event.prev_data, Some(prior.data.to_string()));
                    let (root, blocks) = decode_car(&event.blocks).await;
                    assert_eq!(root, committed.commit);
                    assert_covering_proof(&event, &blocks).await;
                    for block in blocks { retained.insert(block.cid, block.data); }
                    for op in event.ops {
                        let previous = replayed.get(&op.path).map(|(cid, _)| cid.to_string());
                        assert_eq!(op.prev, previous);
                        match op.action {
                            Action::Create | Action::Update => {
                                let cid: Cid = op.cid.unwrap().parse().unwrap();
                                let record = atproto_dasl::from_slice_to_json(&retained[&cid]).unwrap();
                                replayed.insert(op.path, (cid, record));
                            }
                            Action::Delete => { assert!(op.cid.is_none()); replayed.remove(&op.path); }
                        }
                    }
                    assert_eq!(replayed.len(), model.len());
                    for (slot, number) in &model {
                        assert_eq!(replayed[&format!("{COLLECTION}/{}", rkey(*slot))].1, value(*number));
                    }
                    assert_repo(&consumer_car(root, &retained).await, &public, &model).await;
                    after = Some(key);
                }
                assert_repo(&sync::repo_car(handle.reader(), &world, None).await.unwrap(), &public, &model).await;
                prior = repo::head(handle.reader(), &world).await.unwrap().unwrap();
            }
        });
    }
}

async fn visible(reader: &Reader) -> Vec<(Bytes, Bytes)> {
    let mut all = Vec::new();
    for prefix in [b"h/".as_slice(), b"r/", b"b/", keys::EVENTS] {
        all.extend(
            reader
                .scan(prefix, None, usize::MAX)
                .await
                .expect("repository state snapshot can be read"),
        );
    }
    all
}

struct CatchInvalidSwap {
    writes: Vec<Write>,
}

impl Op for CatchInvalidSwap {
    type Output = ();
    type Error = RepoError;

    async fn run(self, txn: &mut Txn<'_>) -> Result<(), RepoError> {
        let error = repo::commit(txn, &Did::new(WORLD), self.writes, None)
            .await
            .expect_err("invalid record swap must reject the commit");
        assert!(matches!(error, RepoError::InvalidSwap));
        Ok(())
    }
}

/// A failed swap changes neither the repo nor its events, even if a caller
/// catches the error and commits the enclosing operation. Catches staging
/// index writes before checking the last record's swap.
#[tokio::test]
async fn failed_swaps_leave_no_effects_even_when_caught() {
    let (handle, _) = fixture().await;
    let world = Did::new(WORLD);
    let first = handle
        .run(ApplyWrites {
            world: world.clone(),
            writes: vec![create(1, 10)],
            swap_commit: None,
        })
        .await
        .unwrap()
        .unwrap();
    let before = visible(handle.reader()).await;
    let wrong = atproto_dasl::cid::compute_cid(b"not the current commit or record");
    let result = handle
        .run(ApplyWrites {
            world: world.clone(),
            writes: vec![create(2, 20)],
            swap_commit: Some(wrong),
        })
        .await
        .unwrap();
    assert!(matches!(result, Err(RepoError::InvalidSwap)));
    for deletion in [false, true] {
        let bad = if deletion {
            Write::Delete {
                collection: Nsid::new(COLLECTION).unwrap(),
                rkey: rkey(1),
                swap: Some(wrong),
            }
        } else {
            Write::Update {
                collection: Nsid::new(COLLECTION).unwrap(),
                rkey: rkey(1),
                record: repo::record_from_json(&value(30)).unwrap(),
                swap: Some(wrong),
            }
        };
        handle
            .run(CatchInvalidSwap {
                writes: vec![create(2, 20), bad],
            })
            .await
            .unwrap()
            .unwrap();
        assert_eq!(visible(handle.reader()).await, before);
    }
    assert_eq!(
        repo::head(handle.reader(), &world).await.unwrap().unwrap().cid,
        first.commit
    );
}

/// Rejecting an unknown collection or invalid known schema leaves an earlier
/// valid write in the same batch unpublished. Catches validation after staging
/// and a catalog lookup that silently accepts unknown lexicons.
#[tokio::test]
async fn schema_refusals_are_atomic_and_fail_closed() {
    let (handle, _) = fixture().await;
    let before = visible(handle.reader()).await;
    for (collection, record) in [
        ("example.unknown.record", json!({ "$type": "example.unknown.record" })),
        (
            COLLECTION,
            json!({ "$type": COLLECTION, "title": "Missing required text" }),
        ),
        (
            COLLECTION,
            json!({ "$type": "at.motet.pilot.profile", "title": "Wrong type", "text": "x" }),
        ),
    ] {
        let writes = vec![
            create(0, 10),
            Write::Create {
                collection: Nsid::new(collection).unwrap(),
                rkey: rkey(1),
                record: repo::record_from_json(&record).unwrap(),
            },
        ];
        let result = handle
            .run(ApplyWrites {
                world: Did::new(WORLD),
                writes,
                swap_commit: None,
            })
            .await
            .unwrap();
        assert!(matches!(
            result,
            Err(RepoError::UnknownCollection(_) | RepoError::InvalidRecord(_))
        ));
        assert_eq!(visible(handle.reader()).await, before);
    }
}

/// Raw byte callers cannot bypass the atproto data model through an open
/// lexicon extension field. Catches DRISL's broader default numeric domain.
#[tokio::test]
async fn non_atproto_numbers_are_refused_without_effects() {
    let (handle, _) = fixture().await;
    let before = visible(handle.reader()).await;
    for extension in [json!(1.5), json!(u64::MAX)] {
        let raw = atproto_dasl::to_vec(&json!({
            "$type": COLLECTION, "title": "Rule", "text": "Valid known fields", "extension": extension,
        }))
        .unwrap();
        let writes = vec![
            create(0, 1),
            Write::Create {
                collection: Nsid::new(COLLECTION).unwrap(),
                rkey: rkey(1),
                record: Bytes::from(raw),
            },
        ];
        let result = handle
            .run(ApplyWrites {
                world: Did::new(WORLD),
                writes,
                swap_commit: None,
            })
            .await
            .unwrap();
        assert!(matches!(result, Err(RepoError::Decode(_))));
        assert_eq!(visible(handle.reader()).await, before);
    }
}

/// The writer must not acknowledge a state that cannot be delivered as a
/// lexicon-valid commit event. Catches checking each record but not the CAR.
#[tokio::test]
async fn an_oversized_commit_is_refused_without_effects() {
    let (handle, _) = fixture().await;
    let before = visible(handle.reader()).await;
    let writes = (0..3)
        .map(|slot| {
            let record = json!({
                "$type": COLLECTION, "title": format!("Rule {slot}"), "text": "Valid known fields",
                "extension": "a".repeat(800_000),
            });
            Write::Create {
                collection: Nsid::new(COLLECTION).unwrap(),
                rkey: rkey(slot),
                record: repo::record_from_json(&record).unwrap(),
            }
        })
        .collect();
    let result = handle
        .run(ApplyWrites {
            world: Did::new(WORLD),
            writes,
            swap_commit: None,
        })
        .await
        .unwrap();
    assert!(matches!(result, Err(RepoError::CommitTooLarge)));
    assert_eq!(visible(handle.reader()).await, before);
}
