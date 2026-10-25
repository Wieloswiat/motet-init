//! Admission reads the post back before deciding, and decides on what it
//! read, never on what the request claimed.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use motet_log::Log;
use motet_rules::world::{Command, Outcome, Refusal};
use motet_rules::{Cid, Did, OperationId, RecordUri, SceneId, StrongRef};
use motet_tenor::admission::{Decision, FetchError, Fetched, Records, Request, admit};
use motet_tenor::shard::{Entry, Handle, Shard, spawn};
use object_store::ObjectStore;
use object_store::memory::InMemory;

const WORLD: &str = "did:plc:world";
const ALICE: &str = "did:plc:alice";

/// Authors' repos as the tenor would read them.
#[derive(Default)]
struct Repos {
    posts: BTreeMap<String, Fetched>,
    down: bool,
}

impl Records for Repos {
    fn fetch(&self, post: &StrongRef) -> impl Future<Output = Result<Option<Fetched>, FetchError>> + Send {
        std::future::ready(if self.down {
            Err(FetchError::Unavailable("PDS down".into()))
        } else {
            Ok(self.posts.get(post.uri.as_str()).cloned())
        })
    }
}

fn post(rkey: &str, cid: &str) -> StrongRef {
    StrongRef::new(
        RecordUri::new(format!("at://{ALICE}/at.motet.pilot.post/{rkey}")),
        Cid::new(cid),
    )
}

fn fetched(cid: &str, scene: &str) -> Fetched {
    Fetched {
        cid: Cid::new(cid),
        author: Did::new(ALICE),
        world: Did::new(WORLD),
        scene: SceneId::new(scene),
    }
}

fn request(op: &str, post: StrongRef) -> Request {
    Request {
        op: OperationId::new(op),
        world: Did::new(WORLD),
        post,
        replaces: None,
    }
}

/// A World with Alice in the cast of `open` and not of `elsewhere`.
async fn world(store: Arc<dyn ObjectStore>) -> Handle {
    let handle = spawn(Shard::open(Log::new(store, "shard")).await.expect("open"), 16);
    let setup = [
        Entry::Begin {
            world: Did::new(WORLD),
            epoch: 0,
        },
        Entry::Apply {
            world: Did::new(WORLD),
            op: OperationId::new("setup-1"),
            command: Command::OpenScene {
                scene: SceneId::new("open"),
                cast: BTreeSet::from([Did::new(ALICE)]),
            },
        },
        Entry::Apply {
            world: Did::new(WORLD),
            op: OperationId::new("setup-2"),
            command: Command::OpenScene {
                scene: SceneId::new("elsewhere"),
                cast: BTreeSet::new(),
            },
        },
    ];
    for entry in setup {
        handle.submit(entry).await.expect("durable").expect("applied");
    }
    handle
}

async fn segments(store: &Arc<dyn ObjectStore>) -> usize {
    Log::new(Arc::clone(store), "shard")
        .read_from(0)
        .await
        .expect("read")
        .len()
}

/// Acceptance: "Request admission for a post whose PDS write fails". No
/// admission is ever written; the request stays pending.
#[tokio::test]
async fn a_post_that_never_reached_its_pds_is_never_admitted() {
    let store: Arc<dyn ObjectStore> = Arc::new(InMemory::new());
    let shard = world(Arc::clone(&store)).await;
    let before = segments(&store).await;

    let decision = admit(&shard, &Repos::default(), request("op", post("p1", "v1")))
        .await
        .expect("no shard error");
    assert_eq!(decision, Decision::NotFound);

    let unreachable = Repos {
        down: true,
        ..Repos::default()
    };
    let decision = admit(&shard, &unreachable, request("op", post("p1", "v1")))
        .await
        .expect("no shard error");
    assert_eq!(decision, Decision::Unreachable);

    assert_eq!(segments(&store).await, before, "nothing reached the World's log");
}

#[tokio::test]
async fn a_version_the_repo_no_longer_holds_is_not_admitted() {
    let store: Arc<dyn ObjectStore> = Arc::new(InMemory::new());
    let shard = world(store).await;
    let repos = Repos {
        posts: BTreeMap::from([(post("p1", "v2").uri.to_string(), fetched("v2", "open"))]),
        down: false,
    };
    let decision = admit(&shard, &repos, request("op", post("p1", "v1")))
        .await
        .expect("no shard error");
    assert_eq!(decision, Decision::NotFound);
}

/// The scene is whatever the record says. A request can't move a post into
/// a scene its author is in by claiming so.
#[tokio::test]
async fn the_record_decides_the_scene() {
    let store: Arc<dyn ObjectStore> = Arc::new(InMemory::new());
    let shard = world(store).await;
    let repos = Repos {
        posts: BTreeMap::from([(post("p1", "v1").uri.to_string(), fetched("v1", "elsewhere"))]),
        down: false,
    };
    let decision = admit(&shard, &repos, request("op", post("p1", "v1")))
        .await
        .expect("no shard error");
    assert_eq!(decision, Decision::Decided(Ok(Outcome::Refused(Refusal::NotInCast))));
}

/// A retry with the same operation ID gets the same outcome, and the post is
/// admitted once.
#[tokio::test]
async fn retries_return_the_first_outcome() {
    let store: Arc<dyn ObjectStore> = Arc::new(InMemory::new());
    let shard = world(store).await;
    let repos = Repos {
        posts: BTreeMap::from([(post("p1", "v1").uri.to_string(), fetched("v1", "open"))]),
        down: false,
    };
    let first = admit(&shard, &repos, request("op", post("p1", "v1")))
        .await
        .expect("no shard error");
    let retry = admit(&shard, &repos, request("op", post("p1", "v1")))
        .await
        .expect("no shard error");
    assert!(
        matches!(first, Decision::Decided(Ok(Outcome::Admitted(_)))),
        "{first:?}"
    );
    assert_eq!(first, retry);
}
