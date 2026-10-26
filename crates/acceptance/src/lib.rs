//! A small in-process deployment for acceptance tests: a tenor's shard on an
//! in-memory bucket, authors' repos, and any number of consumers.
//!
//! Acceptance tests (the table in the backend plan, tracked in
//! `docs/testing.md`) live here when they need more than one part of Motet.
//! Tests of one part's guarantee live with that part.
//!
//! The harness grows with the system. Next: fake PDSes and a fake PLC
//! directory (atproto-crates' testkit is the first thing to try), a Space
//! host, the AppView's indexer, and fault injection on the bucket.

use std::sync::Arc;

use motet_rules::view::{Op, RepoEvent};
use motet_rules::world::Outcome;
use motet_rules::{Did, OperationId};
use motet_tenor::keys::MasterKey;
use motet_tenor::shard::{Entry, Shard};
use object_store::memory::InMemory;

/// A tenor hosting one World, with the records its decisions publish.
#[derive(Debug)]
pub struct Tenor {
    pub world: Did,
    shard: Shard,
    published: Vec<RepoEvent>,
    ops: u64,
}

impl Tenor {
    /// # Panics
    ///
    /// Panics if opening or committing to the fresh in-memory log fails, or
    /// if its single begin entry does not produce one accepted outcome.
    /// The log starts empty and has no competing writer or existing World.
    pub async fn start(world: &str) -> Self {
        let mut shard = Shard::open(
            Arc::new(InMemory::new()),
            "shard",
            Arc::new(MasterKey::from_bytes(&[0; 32])),
        )
        .await
        .expect("open shard");
        let world = Did::new(world);
        shard
            .commit(&[Entry::Begin {
                world: world.clone(),
                epoch: 0,
            }])
            .await
            .expect("begin world")
            .pop()
            .expect("one result")
            .expect("world begins");
        Self {
            world,
            shard,
            published: Vec::new(),
            ops: 0,
        }
    }

    /// Commits one command and publishes what it decided to the World's
    /// repo, as the tenor does after an outcome is durable.
    ///
    /// # Panics
    ///
    /// Panics if the commit fails or does not return one accepted outcome,
    /// including when the public `world` field was changed to an unknown
    /// World. Also panics if the operation or published-event counters exhaust
    /// their integer range; the harness is intended for bounded test histories.
    pub async fn command(&mut self, command: motet_rules::world::Command) -> Outcome {
        self.ops += 1;
        let entry = Entry::Apply {
            world: self.world.clone(),
            op: OperationId::new(format!("op-{}", self.ops)),
            command,
        };
        let outcome = self
            .shard
            .commit(&[entry])
            .await
            .expect("durable")
            .pop()
            .expect("one result")
            .expect("world exists");
        let op = match &outcome {
            Outcome::Admitted(a) => Some(Op::PutAdmission(a.clone())),
            Outcome::Closed(a) => Some(Op::CloseAdmission(a.clone())),
            _ => None,
        };
        if let Some(op) = op {
            let rev = u64::try_from(self.published.len()).expect("small") + 1;
            self.published.push(RepoEvent {
                repo: self.world.clone(),
                rev,
                op,
            });
        }
        outcome
    }

    /// The World's repo, in commit order.
    #[must_use]
    pub fn repo(&self) -> Vec<RepoEvent> {
        self.published.clone()
    }
}
