//! Publication properties across durable decisions, restart and signed repos.

use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::Duration;

use atproto_identity::key::to_public;
use atproto_repo::{MemoryRepository, RecordPath, RepoConfig};
use atproto_space::space_repo::OpAction;
use atproto_space::storage::SpaceRepoStorage;
use motet_lexicons::Record;
use motet_lexicons::at_motet_pilot_admission::Admission as AdmissionRecord;
use motet_lexicons::at_motet_pilot_closing::Closing;
use motet_rules::world::{Admission, Command, Outcome, ReadBack, Refusal};
use motet_rules::{Cid, Did, OperationId, RecordUri, SceneId, StrongRef};
use motet_tenor::admission::{self, Decision, FetchError, Fetched, Records, Request};
use motet_tenor::event::{Event, Sequenced};
use motet_tenor::keys::{MasterKey, Purpose};
use motet_tenor::publication::{self, PublicationError};
use motet_tenor::repo::{self, ApplyWrites, Write};
use motet_tenor::shard::{Entry, Handle, Op, Options, Shard, Txn};
use motet_tenor::spaces::{self, GMS, ProfileInput, SpaceWrite};
use motet_tenor::store::keys;
use motet_tenor::sync;
use object_store::ObjectStore;
use object_store::memory::InMemory;
use serde_json::{Value, json};

const WORLD: &str = "did:plc:ewvi7nxzyoun6zhxrhs64oiz";
const AUTHOR: &str = "did:web:alice.example";
const SCENE: &str = "at.motet.pilot.scene";

#[derive(Clone, Copy)]
enum Audience {
    Public,
    Private,
    Missing,
}

struct Setup(Audience);

fn scene() -> Value {
    json!({ "$type": SCENE, "title": "The Reach", "state": "open", "createdAt": "2026-10-26T00:00:00Z" })
}

fn scene_write(rkey: &str) -> Write {
    Write::Create {
        collection: SCENE.parse().expect("the scene collection is a valid NSID"),
        rkey: rkey.into(),
        record: repo::record_from_json(&scene()).expect("the scene fixture encodes as DRISL"),
    }
}

impl Op for Setup {
    type Output = ();
    type Error = PublicationError;

    async fn run(self, txn: &mut Txn<'_>) -> Result<(), Self::Error> {
        let world = Did::new(WORLD);
        txn.begin_world(world.clone(), 7)
            .expect("the fixture starts a new World");
        txn.generate_key(&world, Purpose::Signing)
            .expect("the fixture generates a signing key");
        repo::genesis(txn, &world).await?;
        match self.0 {
            Audience::Public => {
                repo::commit(txn, &world, vec![scene_write("scene")], None).await?;
            }
            Audience::Private => {
                txn.generate_key(&world, Purpose::Kem)
                    .expect("the fixture generates a KEM key");
                spaces::bootstrap(txn, &world, &Did::new(AUTHOR), ProfileInput::default()).await?;
                spaces::write_as_world(
                    txn,
                    &spaces::world_space(&world, GMS)?,
                    vec![SpaceWrite::create(SCENE, "scene", &scene())?],
                )
                .await?;
            }
            Audience::Missing => {}
        }
        txn.command(
            world,
            OperationId::new("open"),
            Command::OpenScene {
                scene: SceneId::new("scene"),
                cast: BTreeSet::from([Did::new(AUTHOR)]),
            },
        )
        .await?
        .expect("the fixture opens its World's scene");
        Ok(())
    }
}

async fn open(store: Arc<dyn ObjectStore>) -> Handle {
    let shard = Shard::open(store, "publication", Arc::new(MasterKey::from_bytes(&[23; 32])))
        .await
        .expect("the publication shard opens");
    motet_tenor::shard::spawn(
        shard,
        Options {
            max_batch: 8,
            checkpoint_every: Duration::from_secs(3600),
        },
    )
}

async fn fixture(audience: Audience) -> (Arc<dyn ObjectStore>, Handle) {
    let store: Arc<dyn ObjectStore> = Arc::new(InMemory::new());
    let handle = open(Arc::clone(&store)).await;
    handle
        .run(Setup(audience))
        .await
        .expect("the writer accepts setup")
        .expect("setup creates the World");
    (store, handle)
}

fn post(bytes: &[u8]) -> StrongRef {
    StrongRef::new(
        RecordUri::new(format!("at://{AUTHOR}/at.motet.pilot.post/post")),
        Cid::new(atproto_dasl::cid::compute_cid(bytes).to_string()),
    )
}

struct AuthorRepo;

impl Records for AuthorRepo {
    fn fetch(&self, post: &StrongRef) -> impl Future<Output = Result<Option<Fetched>, FetchError>> + Send {
        std::future::ready(Ok(Some(Fetched {
            cid: post.cid.clone(),
            author: Did::new(AUTHOR),
            world: Did::new(WORLD),
            scene: SceneId::new("scene"),
        })))
    }
}

async fn admit(handle: &Handle, op: &str, version: &StrongRef, replaces: Option<Cid>) -> Decision {
    admission::admit(
        handle,
        &AuthorRepo,
        Request {
            op: OperationId::new(op),
            world: Did::new(WORLD),
            post: version.clone(),
            replaces,
        },
    )
    .await
    .expect("the writer decides admission")
}

fn admitted(decision: Decision) -> Admission {
    let Decision::Decided(Ok(Outcome::Admitted(admission))) = decision else {
        panic!("not admitted: {decision:?}")
    };
    admission
}

async fn apply(handle: &Handle, op: &str, command: Command) -> Outcome {
    handle
        .submit(Entry::Apply {
            world: Did::new(WORLD),
            op: OperationId::new(op),
            command,
        })
        .await
        .expect("the writer accepts the rules command")
        .expect("the World applies the rules command")
}

/// Read indexed values from an independently verified signed repository, not
/// just the publication receipts. A missing MST write cannot satisfy this.
async fn records(handle: &Handle, collection: &str) -> Vec<Value> {
    let world = Did::new(WORLD);
    let key = handle
        .reader()
        .key(&world, Purpose::Signing)
        .await
        .expect("the fixture signing key can be read")
        .expect("the World has a signing key");
    let public = to_public(&key).expect("the signing key has a public key");
    let car = sync::repo_car(handle.reader(), &world, None)
        .await
        .expect("the World repo exports as CAR");
    let repo = MemoryRepository::from_car_verified(car.as_slice(), RepoConfig::default(), &public, WORLD)
        .await
        .expect("the consumer verifies the signed World repository");
    let all = handle
        .reader()
        .scan(&keys::repo_records(&world, None), None, usize::MAX)
        .await
        .expect("the World's record index can be read");
    assert_eq!(
        repo.record_count()
            .await
            .expect("the consumer counts repository records"),
        all.len()
    );
    let prefix = keys::repo_records(&world, Some(collection));
    let mut records = Vec::new();
    for (key, _) in all {
        if let Some(rkey) = key.strip_prefix(prefix.as_slice()) {
            records.push(
                repo.get_record(&RecordPath::new(
                    collection,
                    std::str::from_utf8(rkey).expect("record keys are UTF-8"),
                ))
                .await
                .expect("the consumer reads the repository record")
                .expect("the indexed record exists in the signed repository"),
            );
        }
    }
    records
}

/// An acknowledged admission survives process loss and retries without a
/// second signed record. Catches an in-memory queue or non-atomic receipt.
#[tokio::test]
async fn restart_and_repeated_publishing_create_exactly_one_admission() {
    let (store, handle) = fixture(Audience::Public).await;
    let version = post(b"first");
    let first = admitted(admit(&handle, "admit", &version, None).await);
    assert_eq!(records(&handle, AdmissionRecord::NSID).await, [] as [Value; 0]);
    drop(handle);
    let handle = open(Arc::clone(&store)).await;
    let result = publication::drain(&handle).await.unwrap();
    assert_eq!((result.published, result.failed), (1, 0));
    let before = repo::head(handle.reader(), &Did::new(WORLD)).await.unwrap();
    assert_eq!(admitted(admit(&handle, "admit", &version, None).await), first);
    assert_eq!(publication::drain(&handle).await.unwrap().published, 0);
    assert_eq!(publication::drain(&handle).await.unwrap().published, 0);
    assert_eq!(repo::head(handle.reader(), &Did::new(WORLD)).await.unwrap(), before);
    let values = records(&handle, AdmissionRecord::NSID).await;
    assert_eq!(values.len(), 1);
    assert_eq!(
        values[0]["post"],
        json!({ "uri": version.uri.as_str(), "cid": version.cid.as_str() })
    );
    assert_eq!(values[0]["place"], json!({ "epoch": 7, "seq": 0 }));
    assert_eq!(values[0]["authorityRev"], first.authority_rev);
    drop(handle);
    let handle = open(store).await;
    assert_eq!(publication::drain(&handle).await.unwrap().published, 0);
    assert_eq!(records(&handle, AdmissionRecord::NSID).await, values);
}

/// Refusal never makes a post visible as admitted. Catches enqueueing every
/// requested post rather than only successful rules outcomes.
#[tokio::test]
async fn refused_posts_publish_nothing() {
    let (_, handle) = fixture(Audience::Public).await;
    apply(
        &handle,
        "suspend",
        Command::Suspend {
            member: Did::new(AUTHOR),
        },
    )
    .await;
    let before = repo::head(handle.reader(), &Did::new(WORLD)).await.unwrap();
    assert_eq!(
        admit(&handle, "admit", &post(b"refused"), None).await,
        Decision::Decided(Ok(Outcome::Refused(Refusal::Suspended)))
    );
    let result = publication::drain(&handle).await.unwrap();
    assert_eq!((result.published, result.failed), (0, 0));
    assert_eq!(records(&handle, AdmissionRecord::NSID).await, [] as [Value; 0]);
    assert_eq!(records(&handle, Closing::NSID).await, [] as [Value; 0]);
    assert_eq!(repo::head(handle.reader(), &Did::new(WORLD)).await.unwrap(), before);
}

/// An edit retains scene order and names the superseded bytes, even when
/// both versions awaited publication. Catches deriving from only open state.
#[tokio::test]
async fn edits_publish_both_versions_at_the_same_place_with_replaces() {
    let (store, handle) = fixture(Audience::Public).await;
    let original = post(b"original");
    let edited = post(b"edited");
    admitted(admit(&handle, "first", &original, None).await);
    admitted(admit(&handle, "edit", &edited, Some(original.cid.clone())).await);
    drop(handle);
    let handle = open(store).await;
    assert_eq!(publication::drain(&handle).await.unwrap().published, 2);
    let values = records(&handle, AdmissionRecord::NSID).await;
    assert_eq!(values.len(), 2);
    let old = values
        .iter()
        .find(|value| value["post"]["cid"] == original.cid.as_str())
        .unwrap();
    let new = values
        .iter()
        .find(|value| value["post"]["cid"] == edited.cid.as_str())
        .unwrap();
    assert_eq!(new["place"], old["place"]);
    assert_eq!(new["replaces"], original.cid.as_str());
    assert!(old.get("replaces").is_none());
}

/// Deletion closes a specific admission, not all future uses of the bytes.
/// Catches a pair-only dedup index that suppresses re-creation's admission,
/// or a worker that loses closed admissions before ever publishing them.
#[tokio::test]
async fn deletion_and_byte_identical_recreation_publish_distinct_places() {
    let (store, handle) = fixture(Audience::Public).await;
    let version = post(b"unchanged bytes");
    let first = admitted(admit(&handle, "first", &version, None).await);
    assert_eq!(
        apply(
            &handle,
            "delete",
            Command::ObserveDeletion {
                post: version.uri.clone()
            }
        )
        .await,
        Outcome::Closed(first.clone())
    );
    let recreated = admitted(admit(&handle, "recreate", &version, None).await);
    assert_ne!(recreated.place, first.place);
    drop(handle);
    let handle = open(store).await;
    let result = publication::drain(&handle).await.unwrap();
    assert_eq!(result.failed, 0);
    let values = records(&handle, AdmissionRecord::NSID).await;
    assert_eq!(values.len(), 2);
    assert!(values.iter().all(|value| value["post"]["cid"] == version.cid.as_str()));
    let closings = records(&handle, Closing::NSID).await;
    assert_eq!(closings.len(), 1);
    assert_eq!(closings[0]["place"], json!({ "epoch": 7, "seq": 0 }));
    assert_eq!(closings[0]["post"]["cid"], version.cid.as_str());
    let closed = values.iter().find(|value| value["place"]["seq"] == 0).unwrap();
    let closed_cid = atproto_dasl::cid::compute_cid(&repo::record_from_json(closed).unwrap());
    assert_eq!(closings[0]["admission"]["cid"], closed_cid.to_string());
    assert!(values.iter().any(|value| value["place"]["seq"] == 1));
    admitted(admit(&handle, "first", &version, None).await);
    apply(
        &handle,
        "delete",
        Command::ObserveDeletion {
            post: version.uri.clone(),
        },
    )
    .await;
    assert_eq!(publication::drain(&handle).await.unwrap().published, 0);
    assert_eq!(records(&handle, AdmissionRecord::NSID).await, values);
    assert_eq!(records(&handle, Closing::NSID).await, closings);
}

struct RemovePrivateScene;

impl Op for RemovePrivateScene {
    type Output = ();
    type Error = PublicationError;

    async fn run(self, txn: &mut Txn<'_>) -> Result<(), Self::Error> {
        let world = Did::new(WORLD);
        spaces::write_as_world(
            txn,
            &spaces::world_space(&world, GMS)?,
            vec![SpaceWrite {
                action: OpAction::Delete,
                collection: SCENE.into(),
                rkey: "scene".into(),
                value: None,
                swap: None,
            }],
        )
        .await?;
        repo::commit(txn, &world, vec![scene_write("scene")], None).await?;
        Ok(())
    }
}

/// Private facts keep their admission-time audience even if the scene moves
/// before the worker runs. Catches resolving a queued decision's audience late.
#[tokio::test]
async fn private_admissions_and_closings_never_enter_the_public_repo() {
    let (store, handle) = fixture(Audience::Private).await;
    let version = post(b"private");
    admitted(admit(&handle, "admit", &version, None).await);
    handle.run(RemovePrivateScene).await.unwrap().unwrap();
    apply(
        &handle,
        "delete",
        Command::ObserveDeletion {
            post: version.uri.clone(),
        },
    )
    .await;
    drop(handle);
    let handle = open(store).await;
    assert_eq!(publication::drain(&handle).await.unwrap().published, 2);
    assert_eq!(records(&handle, AdmissionRecord::NSID).await, [] as [Value; 0]);
    assert_eq!(records(&handle, Closing::NSID).await, [] as [Value; 0]);
    let world = Did::new(WORLD);
    let space = spaces::world_space(&world, GMS).unwrap();
    let storage = spaces::storage::ReadStorage::new(handle.reader(), &world);
    for collection in [AdmissionRecord::NSID, Closing::NSID] {
        let records = storage.list_records(&space, collection, None, 100, true).await.unwrap();
        assert_eq!(records.records.len(), 1);
        let record = atproto_dasl::from_slice_to_json(&records.records[0].value).unwrap();
        assert_eq!(record["post"]["cid"], version.cid.as_str());
        assert_eq!(record["place"], json!({ "epoch": 7, "seq": 0 }));
    }
    assert_eq!(publication::drain(&handle).await.unwrap().published, 0);
}

/// An absent audience cannot be guessed public, and one blocked scene must
/// not starve another. Catches drop-on-error and stop-at-first-error workers.
#[tokio::test]
async fn unknown_audiences_stay_queued_without_blocking_other_publications() {
    let (_, handle) = fixture(Audience::Missing).await;
    admitted(admit(&handle, "unknown", &post(b"unknown audience"), None).await);
    handle
        .run(ApplyWrites {
            world: Did::new(WORLD),
            writes: vec![scene_write("known")],
            swap_commit: None,
        })
        .await
        .unwrap()
        .unwrap();
    apply(
        &handle,
        "open-known",
        Command::OpenScene {
            scene: SceneId::new("known"),
            cast: BTreeSet::from([Did::new(AUTHOR)]),
        },
    )
    .await;
    let other = StrongRef::new(
        RecordUri::new(format!("at://{AUTHOR}/at.motet.pilot.post/other")),
        post(b"other").cid,
    );
    assert!(matches!(
        apply(
            &handle,
            "known",
            Command::Admit {
                post: ReadBack {
                    post: other,
                    author: Did::new(AUTHOR),
                    scene: SceneId::new("known")
                },
                replaces: None,
            }
        )
        .await,
        Outcome::Admitted(_)
    ));
    let result = publication::drain(&handle).await.unwrap();
    assert_eq!((result.published, result.failed), (1, 1));
    assert_eq!(records(&handle, AdmissionRecord::NSID).await.len(), 1);
    handle
        .run(ApplyWrites {
            world: Did::new(WORLD),
            writes: vec![scene_write("scene")],
            swap_commit: None,
        })
        .await
        .unwrap()
        .unwrap();
    let result = publication::drain(&handle).await.unwrap();
    assert_eq!((result.published, result.failed), (0, 1));
    assert_eq!(records(&handle, AdmissionRecord::NSID).await.len(), 1);
}

/// A duplicate scene key cannot make private posts public. Catches resolving
/// the public repository first and overlooking a conflicting Space record.
#[tokio::test]
async fn ambiguous_scene_locations_never_fall_back_to_publication() {
    let (_, handle) = fixture(Audience::Private).await;
    handle
        .run(ApplyWrites {
            world: Did::new(WORLD),
            writes: vec![scene_write("scene")],
            swap_commit: None,
        })
        .await
        .unwrap()
        .unwrap();
    admitted(admit(&handle, "ambiguous", &post(b"private bytes"), None).await);
    let result = publication::drain(&handle).await.unwrap();
    assert_eq!((result.published, result.failed), (0, 1));
    assert_eq!(records(&handle, AdmissionRecord::NSID).await, [] as [Value; 0]);
    let world = Did::new(WORLD);
    let space = spaces::world_space(&world, GMS).unwrap();
    let storage = spaces::storage::ReadStorage::new(handle.reader(), &world);
    assert_eq!(
        storage
            .list_records(&space, AdmissionRecord::NSID, None, 100, true)
            .await
            .unwrap()
            .records,
        [] as [atproto_space::storage::RecordRow; 0]
    );
}

struct AbortedAdmission;

impl Op for AbortedAdmission {
    type Output = ();
    type Error = &'static str;

    async fn run(self, txn: &mut Txn<'_>) -> Result<(), Self::Error> {
        txn.command(
            Did::new(WORLD),
            OperationId::new("abort"),
            Command::Admit {
                post: ReadBack {
                    post: post(b"aborted"),
                    author: Did::new(AUTHOR),
                    scene: SceneId::new("scene"),
                },
                replaces: None,
            },
        )
        .await
        .expect("the admission and publication intent are staged")
        .expect("the admission is staged before the enclosing operation fails");
        Err("the enclosing operation failed")
    }
}

/// Publication cannot outlive a rolled-back rules decision. Catches moving
/// queue insertion outside the transaction's rollback scope.
#[tokio::test]
async fn an_aborted_operation_cannot_publish_its_admission() {
    let (_, handle) = fixture(Audience::Public).await;
    assert!(handle.run(AbortedAdmission).await.unwrap().is_err());
    let result = publication::drain(&handle).await.unwrap();
    assert_eq!((result.published, result.failed), (0, 0));
    assert_eq!(records(&handle, AdmissionRecord::NSID).await, [] as [Value; 0]);
    let admission = admitted(admit(&handle, "accepted", &post(b"aborted"), None).await);
    assert_eq!(admission.place.seq, 0);
}

/// Decode what a firehose consumer sees, in event order rather than CID order.
async fn publication_history(handle: &Handle) -> Vec<Value> {
    let events = handle
        .reader()
        .scan(keys::EVENTS, None, usize::MAX)
        .await
        .expect("read firehose history");
    let mut records = Vec::new();
    for (_, bytes) in events {
        let event: Sequenced = serde_json::from_slice(&bytes).expect("decode firehose event");
        let Event::Commit(commit) = event.event else { continue };
        for op in commit.ops {
            let collection = op
                .path
                .split_once('/')
                .expect("record operation names collection/rkey")
                .0;
            if collection != AdmissionRecord::NSID && collection != Closing::NSID {
                continue;
            }
            let cid: atproto_dasl::Cid = op
                .cid
                .expect("publication creates a record")
                .parse()
                .expect("record CID is valid");
            let value = handle
                .reader()
                .get(&keys::block(&commit.repo, &cid.to_bytes()))
                .await
                .expect("read published record block")
                .expect("published block exists");
            records.push(atproto_dasl::from_slice_to_json(&value).expect("published record is DRISL"));
        }
    }
    records
}

/// An edit's smaller CID must not move it ahead of its original in the
/// firehose. Catches URI/CID-ordered queues that roll consumers back.
#[tokio::test]
async fn publication_follows_decision_order_not_post_cid_order() {
    let (store, handle) = fixture(Audience::Public).await;
    let mut versions = [post(b"one"), post(b"two")];
    versions.sort_by(|left, right| right.cid.cmp(&left.cid));
    let [original, edited] = versions;
    assert!(edited.cid < original.cid);
    admitted(admit(&handle, "original", &original, None).await);
    admitted(admit(&handle, "edit", &edited, Some(original.cid.clone())).await);
    apply(
        &handle,
        "delete",
        Command::ObserveDeletion {
            post: edited.uri.clone(),
        },
    )
    .await;
    admitted(admit(&handle, "recreate", &original, None).await);
    drop(handle);
    let handle = open(store).await;
    assert_eq!(publication::drain(&handle).await.unwrap().published, 4);
    let history = publication_history(&handle).await;
    assert_eq!(history.len(), 4);
    assert_eq!(history[0]["post"]["cid"], original.cid.as_str());
    assert_eq!(history[1]["post"]["cid"], edited.cid.as_str());
    assert_eq!(history[1]["replaces"], original.cid.as_str());
    assert_eq!(history[2]["$type"], Closing::NSID);
    assert_eq!(history[2]["post"]["cid"], edited.cid.as_str());
    assert_eq!(history[3]["$type"], AdmissionRecord::NSID);
    assert_eq!(history[3]["post"]["cid"], original.cid.as_str());
    assert_eq!(history[3]["place"]["seq"], 1);
}

/// A -> B -> A is three accepted decisions at one place, not a retry of A.
/// Catches both overwritten pending intents and reused old receipts.
#[tokio::test]
async fn byte_identical_reversions_publish_new_admissions_but_retries_do_not() {
    for eager in [false, true] {
        let (store, handle) = fixture(Audience::Public).await;
        let a = post(b"version A");
        let b = post(b"version B");
        admitted(admit(&handle, "first", &a, None).await);
        if eager {
            publication::drain(&handle).await.unwrap();
        }
        admitted(admit(&handle, "edit", &b, Some(a.cid.clone())).await);
        if eager {
            publication::drain(&handle).await.unwrap();
        }
        admitted(admit(&handle, "revert", &a, Some(b.cid.clone())).await);
        drop(handle);
        let handle = open(store).await;
        assert_eq!(publication::drain(&handle).await.unwrap().failed, 0);
        let history = publication_history(&handle).await;
        assert_eq!(history.len(), 3);
        assert_eq!(history[0]["post"]["cid"], a.cid.as_str());
        assert_eq!(history[1]["post"]["cid"], b.cid.as_str());
        assert_eq!(history[2]["post"]["cid"], a.cid.as_str());
        assert_eq!(history[2]["replaces"], b.cid.as_str());
        assert!(history.iter().all(|record| record["place"] == history[0]["place"]));
        admitted(admit(&handle, "revert", &a, Some(b.cid.clone())).await);
        admitted(admit(&handle, "first", &a, None).await);
        assert_eq!(publication::drain(&handle).await.unwrap().published, 0);
        assert_eq!(publication_history(&handle).await, history);
    }
}

struct RecreatePrivateSpace;

impl Op for RecreatePrivateSpace {
    type Output = ();
    type Error = PublicationError;

    async fn run(self, txn: &mut Txn<'_>) -> Result<(), Self::Error> {
        let space = spaces::world_space(&Did::new(WORLD), GMS)?;
        spaces::delete(txn, &space).await?;
        spaces::create(
            txn,
            &space,
            spaces::AccessRecord {
                public: Some(false),
                grants: vec![],
                extra: serde_json::Map::new(),
            },
            false,
        )
        .await?;
        Ok(())
    }
}

/// A recreated Space may have different readers. Catches pinning only its
/// URI and disclosing an old queued admission into the new generation.
#[tokio::test]
async fn queued_private_admissions_cannot_enter_a_recreated_space() {
    let (store, handle) = fixture(Audience::Private).await;
    admitted(admit(&handle, "private", &post(b"old audience"), None).await);
    handle.run(RecreatePrivateSpace).await.unwrap().unwrap();
    drop(handle);
    let handle = open(store).await;
    let drained = publication::drain(&handle).await.unwrap();
    assert_eq!((drained.published, drained.failed), (0, 1));
    let world = Did::new(WORLD);
    let space = spaces::world_space(&world, GMS).unwrap();
    let storage = spaces::storage::ReadStorage::new(handle.reader(), &world);
    assert_eq!(
        storage
            .list_records(&space, AdmissionRecord::NSID, None, 100, true)
            .await
            .unwrap()
            .records,
        [] as [atproto_space::storage::RecordRow; 0]
    );
    assert_eq!(records(&handle, AdmissionRecord::NSID).await, [] as [Value; 0]);
}
