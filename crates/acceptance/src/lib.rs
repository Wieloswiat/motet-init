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
    pub async fn start(world: &str) -> Self {
        let log = motet_log_on_memory();
        let mut shard = Shard::open(log).await.expect("open shard");
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
    pub fn repo(&self) -> Vec<RepoEvent> {
        self.published.clone()
    }
}

fn motet_log_on_memory() -> motet_log::Log {
    motet_log::Log::new(Arc::new(InMemory::new()), "shard")
}
