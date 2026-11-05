//! The shard's promises: an acknowledged outcome is exactly what a replay
//! reproduces, whatever was checkpointed or flushed before the takeover; an
//! operation that fails leaves nothing; and a second writer can't
//! acknowledge anything over the first.

use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use motet_log::Log;
use motet_rules::world::{Command, Outcome, ReadBack};
use motet_rules::{Cid, Did, OperationId, RecordUri, SceneId, StrongRef};
use motet_tenor::event::{Event, Sequenced};
use motet_tenor::keys::MasterKey;
use motet_tenor::shard::{Entry, Handle, Mutation, Op, Options, Rejected, Shard, ShardError, Txn, Write, spawn};
use motet_tenor::store::keys::EVENTS;
use object_store::ObjectStore;
use object_store::memory::InMemory;
use proptest::prelude::*;

const WORLDS: [&str; 2] = ["did:plc:world-a", "did:plc:world-b"];
const MEMBERS: [&str; 3] = ["did:plc:alice", "did:plc:bob", "did:plc:carol"];
const SCENES: [&str; 2] = ["s1", "s2"];

fn did(s: &str) -> Did {
    Did::new(s)
}

fn master() -> Arc<MasterKey> {
    Arc::new(MasterKey::from_bytes(&[7; 32]))
}

async fn open(store: &Arc<dyn ObjectStore>) -> Shard {
    Shard::open(Arc::clone(store), "shard", master()).await.expect("open")
}

fn command() -> impl Strategy<Value = Command> {
    let member = (0..MEMBERS.len()).prop_map(|i| did(MEMBERS.get(i).expect("generated index is below MEMBERS.len()")));
    let scene =
        (0..SCENES.len()).prop_map(|i| SceneId::new(*SCENES.get(i).expect("generated index is below SCENES.len()")));
    prop_oneof![
        1 => (scene.clone(), proptest::collection::btree_set(member.clone(), 0..3))
            .prop_map(|(scene, cast)| Command::OpenScene { scene, cast }),
        1 => scene.clone().prop_map(|scene| Command::CloseScene { scene }),
        1 => (scene.clone(), member.clone()).prop_map(|(scene, member)| Command::JoinCast { scene, member }),
        1 => (scene.clone(), member.clone()).prop_map(|(scene, member)| Command::LeaveCast { scene, member }),
        1 => member.clone().prop_map(|member| Command::Suspend { member }),
        1 => member.clone().prop_map(|member| Command::Reinstate { member }),
        4 => (member, scene, 0..3u8, 0..3u8, proptest::option::of(0..3u8)).prop_map(|(author, scene, post, version, replaces)| {
            let uri = RecordUri::new(format!("at://{author}/at.motet.pilot.post/{post}"));
            Command::Admit {
                post: ReadBack { post: StrongRef::new(uri, Cid::new(format!("v{version}"))), author, scene },
                replaces: replaces.map(|v| Cid::new(format!("v{v}"))),
            }
        }),
    ]
}

/// A host operation's effects: a few keys set or deleted, and an event.
fn write() -> impl Strategy<Value = Entry> {
    let key = (0..4u8).prop_map(|k| Bytes::from(format!("test/{k}")));
    let value = proptest::option::of((0..4u8).prop_map(|v| Bytes::from(format!("v{v}"))));
    proptest::collection::vec((key, value), 0..3).prop_map(|kvs| {
        Entry::Write(Write {
            mutations: kvs.into_iter().map(|(key, value)| Mutation { key, value }).collect(),
            events: vec![Event::Account {
                did: did(WORLDS[0]),
                active: true,
                status: None,
                time: "2026-10-26T00:00:00Z".into(),
            }],
        })
    })
}

/// Entries for both Worlds, including some for a World that never began and
/// some operation IDs reused, so rejections are replayed too, mixed with
/// host writes.
fn entries() -> impl Strategy<Value = Vec<Entry>> {
    let apply = (0..3usize, 0..12u8, command()).prop_map(|(w, op, command)| Entry::Apply {
        world: did(WORLDS.get(w).copied().unwrap_or("did:plc:nowhere")),
        op: OperationId::new(format!("op-{op}")),
        command,
    });
    let entry = prop_oneof![4 => apply, 1 => write()];
    proptest::collection::vec(entry, 0..60).prop_map(|mut entries| {
        let begins = WORLDS.iter().map(|w| Entry::Begin {
            world: did(w),
            epoch: 0,
        });
        entries.splice(0..0, begins);
        entries
    })
}

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime")
}

/// Everything a reader can see that the shard's writes produced.
async fn visible(shard: &Shard) -> Vec<(Bytes, Bytes)> {
    let reader = shard.reader();
    let mut out = reader.scan(b"test/", None, usize::MAX).await.expect("scan");
    out.extend(reader.scan(EVENTS, None, usize::MAX).await.expect("scan"));
    out
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]

    /// Commit in any batching, with checkpoints anywhere, then take the shard
    /// over without a clean close: every World is in exactly the state the
    /// acknowledged outcomes describe, readers see the same keys and the same
    /// event numbering, and the outcomes don't depend on how entries were
    /// batched. A bug in which segments the takeover replays, or in what it
    /// re-applies versus re-runs, shows up here as a difference.
    #[test]
    fn replay_reproduces_what_was_acknowledged(
        entries in entries(),
        cuts in proptest::collection::vec((1..8usize, any::<bool>()), 1..20),
    ) {
        runtime().block_on(async {
            let store: Arc<dyn ObjectStore> = Arc::new(InMemory::new());
            let mut batched = open(&store).await;
            let mut one_by_one = open(&(Arc::new(InMemory::new()) as Arc<dyn ObjectStore>)).await;

            let mut outcomes = Vec::new();
            let mut rest = entries.as_slice();
            // Back-to-back checkpoints pile up L0 tables faster than the
            // compactor clears them, and past the store's limit each flush
            // waits for a manifest poll. Sixteen per case still checkpoint at
            // arbitrary points without measuring that wait instead of replay.
            let mut checkpoints = 0;
            for (cut, checkpoint) in cuts.iter().cycle() {
                if rest.is_empty() { break; }
                let (batch, tail) = rest.split_at((*cut).min(rest.len()));
                outcomes.extend(batched.commit(batch).await.expect("commit"));
                if *checkpoint && checkpoints < 16 {
                    batched.checkpoint().await.expect("checkpoint");
                    checkpoints += 1;
                }
                rest = tail;
            }
            let mut sequential = Vec::new();
            for entry in &entries {
                sequential.extend(one_by_one.commit(std::slice::from_ref(entry)).await.expect("commit"));
            }
            prop_assert_eq!(&outcomes, &sequential);

            let before = visible(&batched).await;
            // No close: whatever wasn't flushed is lost with the process.
            let replayed = open(&store).await;
            for world in WORLDS {
                prop_assert_eq!(replayed.world(&did(world)), batched.world(&did(world)));
            }
            prop_assert_eq!(visible(&replayed).await, before);
            Ok(())
        })?;
    }
}

#[tokio::test]
async fn entries_for_a_world_this_shard_doesnt_host_are_rejected_and_kept() {
    let store: Arc<dyn ObjectStore> = Arc::new(InMemory::new());
    let mut shard = open(&store).await;
    let entry = Entry::Apply {
        world: did("did:plc:nowhere"),
        op: OperationId::new("op"),
        command: Command::Suspend {
            member: did(MEMBERS[0]),
        },
    };
    assert_eq!(
        shard.commit(&[entry]).await.expect("commit"),
        vec![Err(Rejected::UnknownWorld(did("did:plc:nowhere")))]
    );
}

/// An operation that writes, emits and changes rules, then fails.
#[derive(Debug)]
struct Fails;

impl Op for Fails {
    type Output = ();
    type Error = &'static str;

    fn run(self, txn: &mut Txn<'_>) -> impl Future<Output = Result<(), &'static str>> + Send {
        txn.put(b"test/failed".to_vec(), Bytes::from_static(b"x"));
        txn.emit(Event::Account {
            did: did(WORLDS[0]),
            active: false,
            status: None,
            time: "2026-10-26T00:00:00Z".into(),
        });
        let _ = txn.begin_world(did(WORLDS[1]), 0);
        std::future::ready(Err("refused"))
    }
}

/// An operation reads what the operations before it in the same batch wrote,
/// and writes on top of it.
#[derive(Debug)]
struct Append(&'static str);

impl Op for Append {
    type Output = ();
    type Error = std::convert::Infallible;

    async fn run(self, txn: &mut Txn<'_>) -> Result<(), std::convert::Infallible> {
        let mut value = txn
            .get(b"test/log")
            .await
            .expect("read")
            .map(|b| b.to_vec())
            .unwrap_or_default();
        value.extend_from_slice(self.0.as_bytes());
        txn.put(b"test/log".to_vec(), Bytes::from(value));
        Ok(())
    }
}

fn running(shard: Shard) -> Handle {
    spawn(
        shard,
        Options {
            max_batch: 8,
            checkpoint_every: Duration::from_secs(3600),
        },
    )
}

/// A failed operation leaves nothing, not even in the batch it shared with
/// operations that succeeded, and nothing of it survives a takeover.
#[tokio::test]
async fn a_failed_operation_leaves_nothing() {
    let store: Arc<dyn ObjectStore> = Arc::new(InMemory::new());
    let handle = running(open(&store).await);
    let mut events = handle.subscribe();

    let (failed, first, second) = tokio::join!(handle.run(Fails), handle.run(Append("1")), handle.run(Append("2")));
    assert_eq!(failed.expect("durable"), Err("refused"));
    first.expect("durable").expect("appends");
    second.expect("durable").expect("appends");

    let reader = handle.reader().clone();
    assert_eq!(reader.get(b"test/failed").await.expect("read"), None);
    let log = reader.get(b"test/log").await.expect("read").expect("written");
    assert!(log == "12" || log == "21", "each append saw the one before it: {log:?}");
    assert!(
        events.try_recv().is_err(),
        "a failed operation's event reached subscribers"
    );

    drop(handle);
    let replayed = open(&store).await;
    assert!(replayed.world(&did(WORLDS[1])).is_none());
    assert_eq!(replayed.reader().get(b"test/failed").await.expect("read"), None);
}

/// Live subscribers and stored events agree on numbering, so a consumer that
/// reconnects with a cursor resumes exactly where it left off.
#[tokio::test]
async fn live_events_are_numbered_as_stored() {
    let store: Arc<dyn ObjectStore> = Arc::new(InMemory::new());
    let mut shard = open(&store).await;
    let account = |active| Event::Account {
        did: did(WORLDS[0]),
        active,
        status: None,
        time: "2026-10-26T00:00:00Z".into(),
    };
    let write = |events| {
        Entry::Write(Write {
            mutations: Vec::new(),
            events,
        })
    };
    shard
        .commit(&[write(vec![account(true), account(false)])])
        .await
        .expect("commit");
    let handle = running(shard);
    let mut live = handle.subscribe();
    handle
        .submit(write(vec![account(true)]))
        .await
        .expect("durable")
        .expect("applies");
    let event = live.recv().await.expect("live event");
    assert_eq!(event.seq, 2);

    let stored = handle.reader().scan(EVENTS, None, usize::MAX).await.expect("scan");
    let stored: Vec<Sequenced> = stored
        .iter()
        .map(|(_, v)| serde_json::from_slice(v).expect("stored event"))
        .collect();
    assert_eq!(stored.iter().map(|e| e.seq).collect::<Vec<_>>(), vec![0, 1, 2]);
    assert_eq!(stored.last(), Some(&*event));
}

/// "Start a second writer against the same bucket": the older writer is
/// fenced at its next commit, and nothing it staged becomes visible.
#[tokio::test]
async fn a_second_writer_fences_the_first() {
    let store: Arc<dyn ObjectStore> = Arc::new(InMemory::new());
    let begin = |w: &str| Entry::Begin {
        world: did(w),
        epoch: 0,
    };
    let mut old = open(&store).await;
    old.commit(&[begin(WORLDS[0])]).await.expect("first commit");

    let mut new = open(&store).await;
    new.commit(&[begin(WORLDS[1])]).await.expect("new writer commits");

    let open_scene = Command::OpenScene {
        scene: SceneId::new("s1"),
        cast: BTreeSet::new(),
    };
    let late = Entry::Apply {
        world: did(WORLDS[0]),
        op: OperationId::new("late"),
        command: open_scene,
    };
    let fenced = old.commit(&[late]).await.expect_err("the old writer is fenced");
    assert!(matches!(*fenced, ShardError::Fenced(1)), "{fenced}");

    let replayed = open(&store).await;
    assert!(replayed.world(&did(WORLDS[1])).is_some());
    assert_eq!(
        replayed
            .world(&did(WORLDS[0]))
            .map(motet_rules::world::World::authority_rev),
        Some(0)
    );
}

/// Group commit hands every waiter its own entry's outcome. If it paired
/// outcomes with the wrong waiters, a caller could hear "admitted" for an
/// entry the World refused.
#[tokio::test]
async fn group_commit_answers_each_submitter() {
    let store: Arc<dyn ObjectStore> = Arc::new(InMemory::new());
    let handle = running(open(&store).await);
    let begin = |w: &str| Entry::Begin {
        world: did(w),
        epoch: 0,
    };
    let worlds: Vec<String> = (0..20).map(|i| format!("did:plc:w{i}")).collect();
    // Every other World began earlier. Each batch then mixes acceptances with
    // refusals that name their own World, so a waiter that gets the outcome
    // of another entry sees the wrong answer.
    let earlier: Vec<&String> = worlds.iter().step_by(2).collect();
    for w in &earlier {
        handle
            .submit(begin(w))
            .await
            .expect("durable")
            .expect("a new World begins");
    }
    let before = Log::new(Arc::clone(&store), "shard")
        .read_from(0)
        .await
        .expect("read")
        .len();

    let submissions = worlds.iter().map(|w| {
        let (handle, entry) = (handle.clone(), begin(w));
        async move { handle.submit(entry).await }
    });
    let results = futures::future::join_all(submissions).await;
    for (w, result) in worlds.iter().zip(results) {
        let expected = if earlier.contains(&w) {
            Err(Rejected::AlreadyBegun(did(w)))
        } else {
            Ok(Outcome::Unchanged)
        };
        assert_eq!(result.expect("durable"), expected, "{w}");
    }

    let segments = Log::new(store, "shard").read_from(0).await.expect("read").len() - before;
    assert!(
        segments < worlds.len(),
        "group commit wrote {segments} segments for {} entries",
        worlds.len()
    );
}

/// The same takeover against the real bucket (SeaweedFS under devenv), which
/// also has to honour SlateDB's conditional writes.
#[tokio::test]
#[ignore = "integration: needs S3 (devenv up)"]
async fn fault_takeover_against_real_bucket() {
    let store = motet_testkit::services::bucket();
    let mut old = open(&store).await;
    let mut new = open(&store).await;
    new.commit(&[Entry::Begin {
        world: did(WORLDS[0]),
        epoch: 0,
    }])
    .await
    .expect("new writer commits");
    let fenced = old
        .commit(&[Entry::Begin {
            world: did(WORLDS[1]),
            epoch: 0,
        }])
        .await
        .expect_err("the old writer is fenced");
    assert!(matches!(*fenced, ShardError::Fenced(0)), "{fenced}");
}

/// Checkpoint, flush and reopen on the real bucket: what a takeover reads
/// back from SeaweedFS is what was written.
#[tokio::test]
#[ignore = "integration: needs S3 (devenv up)"]
async fn checkpointed_state_reopens_from_the_real_bucket() {
    let store = motet_testkit::services::bucket();
    let mut shard = open(&store).await;
    shard
        .commit(&[
            Entry::Begin {
                world: did(WORLDS[0]),
                epoch: 0,
            },
            Entry::Write(Write {
                mutations: vec![Mutation {
                    key: Bytes::from_static(b"test/k"),
                    value: Some(Bytes::from_static(b"v")),
                }],
                events: Vec::new(),
            }),
        ])
        .await
        .expect("commit");
    shard.checkpoint().await.expect("checkpoint");
    shard.close().await.expect("close");

    let reopened = open(&store).await;
    assert!(reopened.world(&did(WORLDS[0])).is_some());
    assert_eq!(
        reopened.reader().get(b"test/k").await.expect("read"),
        Some(Bytes::from_static(b"v"))
    );
}
