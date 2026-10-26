//! World creation and access properties, through the real shard transaction.

use std::sync::Arc;
use std::time::Duration;

use atproto_space::SpaceUri;
use atproto_space::space_repo::OpAction;
use atproto_space::storage::{MemberAccess, SpaceRepoStorage};
use motet_rules::Did;
use motet_tenor::keys::{MasterKey, Purpose};
use motet_tenor::repo;
use motet_tenor::shard::{Handle, Op, Options, Shard, Txn, spawn};
use motet_tenor::spaces::{self, AccessRecord, GMS, MEMBERS, META, ProfileInput, SpaceError, SpaceWrite};
use motet_tenor::store::{Reader, keys};
use object_store::ObjectStore;
use object_store::memory::InMemory;
use serde_json::json;

const WORLD: &str = "did:plc:ewvi7nxzyoun6zhxrhs64oiz";
const OWNER: &str = "did:web:alice.example";
const STRANGER: &str = "did:web:mallory.example";

#[derive(Debug)]
struct Bootstrap {
    public: bool,
}

impl Op for Bootstrap {
    type Output = ();
    type Error = SpaceError;

    async fn run(self, txn: &mut Txn<'_>) -> Result<(), Self::Error> {
        let world = Did::new(WORLD);
        txn.begin_world(world.clone(), 1).expect("new World");
        txn.generate_key(&world, Purpose::Signing).expect("fixture Signing key");
        txn.generate_key(&world, Purpose::Kem).expect("fixture Kem key");
        repo::genesis(txn, &world).await?;
        spaces::bootstrap(
            txn,
            &world,
            &Did::new(OWNER),
            ProfileInput {
                display_name: Some("The Reach".into()),
                public: self.public,
                ..ProfileInput::default()
            },
        )
        .await
    }
}

#[derive(Debug)]
struct WriteRecords {
    space: SpaceUri,
    writes: Vec<SpaceWrite>,
}

impl Op for WriteRecords {
    type Output = ();
    type Error = SpaceError;

    async fn run(self, txn: &mut Txn<'_>) -> Result<(), Self::Error> {
        spaces::write_as_world(txn, &self.space, self.writes).await?;
        Ok(())
    }
}

async fn start(store: Arc<dyn ObjectStore>, public: bool) -> Handle {
    let shard = Shard::open(store, "spaces", Arc::new(MasterKey::from_bytes(&[29; 32])))
        .await
        .expect("open fixture shard");
    let handle = spawn(
        shard,
        Options {
            max_batch: 8,
            checkpoint_every: Duration::from_secs(3600),
        },
    );
    handle
        .run(Bootstrap { public })
        .await
        .expect("submit bootstrap")
        .expect("bootstrap World");
    handle
}

async fn json_record(reader: &Reader, space: &SpaceUri, collection: &str, rkey: &str) -> serde_json::Value {
    let row = spaces::record(reader, space, collection, rkey)
        .await
        .expect("read fixture record")
        .expect("fixture record exists");
    atproto_dasl::from_slice_to_json(&row.value).expect("fixture record is DRISL")
}

/// Catches a partially created group, lost role/access rows on replay, or a
/// declaration which makes a private World's meta discoverable publicly.
#[tokio::test]
async fn bootstrap_creates_a_readable_owner_group_and_reopen_preserves_it() {
    for public in [false, true] {
        let store: Arc<dyn ObjectStore> = Arc::new(InMemory::new());
        let handle = start(Arc::clone(&store), public).await;
        let world = Did::new(WORLD);
        let members = spaces::world_space(&world, MEMBERS).unwrap();
        let meta = spaces::world_space(&world, META).unwrap();
        let gms = spaces::world_space(&world, GMS).unwrap();
        let before = handle.reader().scan(b"s/", None, usize::MAX).await.unwrap();
        assert_eq!(
            spaces::roles(handle.reader(), &world, &Did::new(OWNER)).await.unwrap(),
            vec!["owner"]
        );
        for space in [&meta, &members, &gms] {
            assert_eq!(
                spaces::access(handle.reader(), space, Some(OWNER)).await.unwrap(),
                MemberAccess::FULL
            );
            assert_eq!(
                spaces::access(handle.reader(), space, Some(WORLD)).await.unwrap(),
                MemberAccess::FULL
            );
            let stranger = spaces::access(handle.reader(), space, Some(STRANGER)).await.unwrap();
            assert_eq!(stranger.read, public && *space == meta);
            assert!(!stranger.write);
            assert!(
                spaces::record(handle.reader(), space, "group.opensocial.access", "self")
                    .await
                    .unwrap()
                    .is_some()
            );
        }
        assert_eq!(
            json_record(handle.reader(), &meta, "group.opensocial.profile", "self").await["displayName"],
            "The Reach"
        );
        assert_eq!(
            json_record(handle.reader(), &members, "group.opensocial.membership", OWNER).await["roles"],
            json!(["owner"])
        );
        let permissions = json_record(handle.reader(), &members, "group.opensocial.permissions", "self").await;
        assert_eq!(permissions["grants"][0]["assignable"], json!(["owner"]));
        let indexes = spaces::storage::ReadStorage::new(handle.reader(), &world)
            .list_records(&members, "group.opensocial.space", None, 100, true)
            .await
            .unwrap();
        let listed_spaces: std::collections::BTreeSet<_> = indexes
            .records
            .iter()
            .map(|row| {
                atproto_dasl::from_slice_to_json(&row.value).unwrap()["space"]
                    .as_str()
                    .unwrap()
                    .to_owned()
            })
            .collect();
        assert_eq!(
            listed_spaces,
            [meta.to_string(), members.to_string(), gms.to_string()]
                .into_iter()
                .collect()
        );
        assert_eq!(
            handle
                .reader()
                .get(&keys::repo_record(&world, "group.opensocial.declaration", "self"))
                .await
                .unwrap()
                .is_some(),
            public
        );
        drop(handle);
        let reopened = Shard::open(Arc::clone(&store), "spaces", Arc::new(MasterKey::from_bytes(&[29; 32])))
            .await
            .unwrap();
        assert_eq!(reopened.reader().scan(b"s/", None, usize::MAX).await.unwrap(), before);
        assert_eq!(
            spaces::access(&reopened.reader(), &gms, Some(OWNER)).await.unwrap(),
            MemberAccess::FULL
        );
    }
}

/// Catches subtractive role evaluation and using acceptance as permission:
/// either read-granting role is sufficient, without a member-authored record.
#[tokio::test]
async fn removing_one_role_preserves_access_granted_by_another() {
    let store: Arc<dyn ObjectStore> = Arc::new(InMemory::new());
    let handle = start(store, false).await;
    let world = Did::new(WORLD);
    let members = spaces::world_space(&world, MEMBERS).unwrap();
    let gms = spaces::world_space(&world, GMS).unwrap();
    let grants = json!({"public":false,"grants":[
        {"role":"red","scopes":["space:*?action=read"]},
        {"role":"blue","scopes":["space:*?action=read"]}
    ]});
    handle
        .run(WriteRecords {
            space: members.clone(),
            writes: vec![
                SpaceWrite::create("group.opensocial.role", "red", &json!({"displayName":"Red"})).unwrap(),
                SpaceWrite::create("group.opensocial.role", "blue", &json!({"displayName":"Blue"})).unwrap(),
                SpaceWrite::create(
                    "group.opensocial.membership",
                    STRANGER,
                    &json!({"member":STRANGER,"roles":["red","blue"],"createdAt":"2026-10-26T00:00:00Z"}),
                )
                .unwrap(),
            ],
        })
        .await
        .unwrap()
        .unwrap();
    let mut access = SpaceWrite::create("group.opensocial.access", "self", &grants).unwrap();
    access.action = OpAction::Update;
    handle
        .run(WriteRecords {
            space: gms.clone(),
            writes: vec![access],
        })
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        spaces::access(handle.reader(), &gms, Some(STRANGER)).await.unwrap(),
        MemberAccess {
            read: true,
            write: false
        }
    );
    let mut membership = SpaceWrite::create(
        "group.opensocial.membership",
        STRANGER,
        &json!({"member":STRANGER,"roles":["blue"],"createdAt":"2026-10-26T00:00:00Z"}),
    )
    .unwrap();
    membership.action = OpAction::Update;
    handle
        .run(WriteRecords {
            space: members,
            writes: vec![membership],
        })
        .await
        .unwrap()
        .unwrap();
    assert!(
        spaces::access(handle.reader(), &gms, Some(STRANGER))
            .await
            .unwrap()
            .read
    );
}

/// Catches a lost update at the actual shard write boundary and a stale
/// public declaration after meta becomes private.
#[tokio::test]
async fn conditional_writes_refuse_stale_versions_and_hide_private_meta() {
    let store: Arc<dyn ObjectStore> = Arc::new(InMemory::new());
    let handle = start(store, true).await;
    let world = Did::new(WORLD);
    let meta = spaces::world_space(&world, META).unwrap();
    let old = spaces::record(handle.reader(), &meta, "group.opensocial.profile", "self")
        .await
        .unwrap()
        .unwrap();
    let mut write = SpaceWrite::create(
        "group.opensocial.profile",
        "self",
        &json!({"displayName":"Changed", "joinPolicy":"invite"}),
    )
    .unwrap();
    write.action = OpAction::Update;
    write.swap = Some(old.cid);
    handle
        .run(WriteRecords {
            space: meta.clone(),
            writes: vec![write.clone()],
        })
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(
        handle
            .run(WriteRecords {
                space: meta.clone(),
                writes: vec![write]
            })
            .await
            .unwrap(),
        Err(SpaceError::Conflict)
    ));
    let access: AccessRecord = serde_json::from_value(json!({"public":false,"grants":[]})).unwrap();
    let mut write = SpaceWrite::create("group.opensocial.access", "self", &access).unwrap();
    write.action = OpAction::Update;
    handle
        .run(WriteRecords {
            space: meta,
            writes: vec![write],
        })
        .await
        .unwrap()
        .unwrap();
    assert!(
        handle
            .reader()
            .get(&keys::repo_record(&world, "group.opensocial.declaration", "self"))
            .await
            .unwrap()
            .is_none()
    );
}

#[derive(Debug)]
struct Recreate(SpaceUri);

impl Op for Recreate {
    type Output = ();
    type Error = SpaceError;

    async fn run(self, txn: &mut Txn<'_>) -> Result<(), Self::Error> {
        spaces::delete(txn, &self.0).await?;
        spaces::create(txn, &self.0, serde_json::from_value(json!({"grants":[]}))?, false).await?;
        Ok(())
    }
}

/// Catches deleting the oplog with a Space: a syncer resuming an older
/// cursor must see the removals before the recreated URI's new records.
#[tokio::test]
async fn recreating_a_space_retains_a_complete_delta_from_its_previous_generation() {
    let store: Arc<dyn ObjectStore> = Arc::new(InMemory::new());
    let handle = start(store, false).await;
    let world = Did::new(WORLD);
    let gms = spaces::world_space(&world, GMS).unwrap();
    let storage = spaces::storage::ReadStorage::new(handle.reader(), &world);
    let before = storage.read_oplog(&gms, None, 100).await.unwrap();
    let last = before.ops.last().unwrap();
    let cursor = atproto_space::storage::OplogCursor::new(last.rev.clone(), last.idx);
    let generation = spaces::info(handle.reader(), &gms).await.unwrap().generation;
    handle.run(Recreate(gms.clone())).await.unwrap().unwrap();
    let delta = storage.read_oplog(&gms, Some(&cursor), 100).await.unwrap();
    assert_eq!(
        delta.ops.iter().map(|op| op.action.as_str()).collect::<Vec<_>>(),
        ["delete", "create"]
    );
    assert_ne!(
        spaces::info(handle.reader(), &gms).await.unwrap().generation,
        generation
    );
    let records = storage
        .list_records(&gms, "group.opensocial.access", None, 100, true)
        .await
        .unwrap();
    assert_eq!(records.records.len(), 1);
    assert_eq!(records.records[0].repo_rev, delta.state.rev.unwrap());
}
