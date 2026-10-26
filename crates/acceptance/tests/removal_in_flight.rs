//! Acceptance: "Remove a player while their post is in flight". There is one
//! outcome, admitted before the removal or refused after it, and every
//! consumer agrees on it whatever order they receive the records in.

use std::collections::BTreeSet;

use motet_acceptance::Tenor;
use motet_rules::view::{Op, RepoEvent, WorldView};
use motet_rules::world::{Command, Outcome, ReadBack, Refusal};
use motet_rules::{Cid, Did, RecordUri, SceneId, StrongRef};
use proptest::prelude::*;
use proptest::strategy::ValueTree;

const WORLD: &str = "did:plc:world";
const ALICE: &str = "did:plc:alice";

proptest! {
    #[test]
    fn every_consumer_agrees_on_the_one_outcome(
        removal_first: bool,
        orders in (any::<u64>(), any::<u64>()),
    ) {
        // The tenor's state store runs timers, so the runtime needs its drivers.
        let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().expect("runtime");
        let (outcome, streams) = runtime.block_on(async {
            let scene = SceneId::new("s1");
            let alice = Did::new(ALICE);
            let post = StrongRef::new(RecordUri::new(format!("at://{ALICE}/at.motet.pilot.post/p1")), Cid::new("v1"));
            let mut tenor = Tenor::start(WORLD).await;
            tenor.command(Command::OpenScene { scene: scene.clone(), cast: BTreeSet::from([alice.clone()]) }).await;

            let admit = Command::Admit {
                post: ReadBack { post: post.clone(), author: alice.clone(), scene: scene.clone() },
                replaces: None,
            };
            let remove = Command::LeaveCast { scene: scene.clone(), member: alice.clone() };
            let outcome = if removal_first {
                tenor.command(remove).await;
                tenor.command(admit).await
            } else {
                let outcome = tenor.command(admit).await;
                tenor.command(remove).await;
                outcome
            };
            let alices_repo = vec![RepoEvent {
                repo: alice,
                rev: 1,
                op: Op::PutPost { post, scene, body: "I slip through the gate.".into() },
            }];
            (outcome, vec![alices_repo, tenor.repo()])
        });

        // The tenor's order is the only one that decides.
        if removal_first {
            prop_assert_eq!(&outcome, &Outcome::Refused(Refusal::NotInCast));
        } else {
            prop_assert!(matches!(outcome, Outcome::Admitted(_)));
        }

        // Two consumers, two arrival orders, one answer.
        let view = |seed: u64| {
            let mut runner = proptest::test_runner::TestRunner::new_with_rng(
                proptest::test_runner::Config::default(),
                proptest::test_runner::TestRng::from_seed(proptest::test_runner::RngAlgorithm::ChaCha, &seed_bytes(seed)),
            );
            let delivery = motet_testkit::delivery::deliveries(streams.clone(), 2)
                .new_tree(&mut runner)
                .expect("a delivery")
                .current();
            let mut view = WorldView::new(Did::new(WORLD));
            for event in &delivery {
                view.apply(event);
            }
            view.scene(&SceneId::new("s1"))
        };
        let (a, b) = (view(orders.0), view(orders.1));
        prop_assert_eq!(&a, &b);
        prop_assert_eq!(a.len(), usize::from(!removal_first));
    }
}

fn seed_bytes(seed: u64) -> [u8; 32] {
    let mut bytes = [0; 32];
    bytes[..8].copy_from_slice(&seed.to_le_bytes());
    bytes
}
