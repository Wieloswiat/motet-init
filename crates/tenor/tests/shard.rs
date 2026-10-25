//! The shard's promises: an acknowledged outcome is exactly what a replay
//! reproduces, and a second writer can't acknowledge anything over the first.

use std::collections::BTreeSet;
use std::sync::Arc;

use motet_log::Log;
use motet_rules::world::{Command, Outcome, ReadBack};
use motet_rules::{Cid, Did, OperationId, RecordUri, SceneId, StrongRef};
use motet_tenor::shard::{Entry, Rejected, Shard, ShardError, spawn};
use object_store::ObjectStore;
use object_store::memory::InMemory;
use proptest::prelude::*;

const WORLDS: [&str; 2] = ["did:plc:world-a", "did:plc:world-b"];
const MEMBERS: [&str; 3] = ["did:plc:alice", "did:plc:bob", "did:plc:carol"];
const SCENES: [&str; 2] = ["s1", "s2"];

fn did(s: &str) -> Did {
    Did::new(s)
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

/// Entries for both Worlds, including some for a World that never began and
/// some operation IDs reused, so rejections are replayed too.
fn entries() -> impl Strategy<Value = Vec<Entry>> {
    let apply = (0..3usize, 0..12u8, command()).prop_map(|(w, op, command)| Entry::Apply {
        world: did(WORLDS.get(w).copied().unwrap_or("did:plc:nowhere")),
        op: OperationId::new(format!("op-{op}")),
        command,
    });
    proptest::collection::vec(apply, 0..60).prop_map(|mut entries| {
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

proptest! {
    /// Commit in any batching, then open the shard elsewhere: every World is
    /// in exactly the state the acknowledged outcomes describe, and the
    /// outcomes don't depend on how entries were batched.
    #[test]
    fn replay_reproduces_what_was_acknowledged(entries in entries(), cuts in proptest::collection::vec(1..8usize, 1..20)) {
        runtime().block_on(async {
            let store: Arc<dyn ObjectStore> = Arc::new(InMemory::new());
            let mut batched = Shard::open(Log::new(Arc::clone(&store), "shard")).await.expect("open");
            let mut one_by_one = Shard::open(Log::new(Arc::new(InMemory::new()), "shard")).await.expect("open");

            let mut outcomes = Vec::new();
            let mut rest = entries.as_slice();
            for cut in cuts.iter().cycle() {
                if rest.is_empty() { break; }
                let (batch, tail) = rest.split_at((*cut).min(rest.len()));
                outcomes.extend(batched.commit(batch).await.expect("commit"));
                rest = tail;
            }
            let mut sequential = Vec::new();
            for entry in &entries {
                sequential.extend(one_by_one.commit(std::slice::from_ref(entry)).await.expect("commit"));
            }
            prop_assert_eq!(&outcomes, &sequential);

            let replayed = Shard::open(Log::new(store, "shard")).await.expect("reopen");
            for world in WORLDS {
                prop_assert_eq!(replayed.world(&did(world)), batched.world(&did(world)));
            }
            Ok(())
        })?;
    }
}

#[tokio::test]
async fn entries_for_a_world_this_shard_doesnt_host_are_rejected_and_kept() {
    let store: Arc<dyn ObjectStore> = Arc::new(InMemory::new());
    let mut shard = Shard::open(Log::new(store, "shard")).await.expect("open");
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

/// "Start a second writer against the same bucket": the older writer is
/// fenced at its next commit, and nothing it staged becomes visible.
#[tokio::test]
async fn a_second_writer_fences_the_first() {
    let store: Arc<dyn ObjectStore> = Arc::new(InMemory::new());
    let begin = |w: &str| Entry::Begin {
        world: did(w),
        epoch: 0,
    };
    let mut old = Shard::open(Log::new(Arc::clone(&store), "shard")).await.expect("open");
    old.commit(&[begin(WORLDS[0])]).await.expect("first commit");

    let mut new = Shard::open(Log::new(Arc::clone(&store), "shard"))
        .await
        .expect("takeover");
    new.commit(&[begin(WORLDS[1])]).await.expect("new writer commits");

    let open = Command::OpenScene {
        scene: SceneId::new("s1"),
        cast: BTreeSet::new(),
    };
    let late = Entry::Apply {
        world: did(WORLDS[0]),
        op: OperationId::new("late"),
        command: open,
    };
    assert!(matches!(old.commit(&[late]).await, Err(ShardError::Fenced(1))));

    let replayed = Shard::open(Log::new(store, "shard")).await.expect("replay");
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
    let handle = spawn(
        Shard::open(Log::new(Arc::clone(&store), "shard")).await.expect("open"),
        8,
    );
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

/// The same takeover against the real bucket (SeaweedFS under devenv).
#[tokio::test]
#[ignore = "integration: needs S3 (devenv up)"]
async fn fault_takeover_against_real_bucket() {
    let store = motet_testkit::services::bucket();
    let mut old = Shard::open(Log::new(Arc::clone(&store), "shard")).await.expect("open");
    let mut new = Shard::open(Log::new(Arc::clone(&store), "shard"))
        .await
        .expect("takeover");
    new.commit(&[Entry::Begin {
        world: did(WORLDS[0]),
        epoch: 0,
    }])
    .await
    .expect("new writer commits");
    assert!(matches!(
        old.commit(&[Entry::Begin {
            world: did(WORLDS[1]),
            epoch: 0
        }])
        .await,
        Err(ShardError::Fenced(0))
    ));
}
