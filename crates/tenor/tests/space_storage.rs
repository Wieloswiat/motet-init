//! Space persistence protects the published commitment, not just the latest
//! row: replay, multi-write overlays, pagination and refused commits must all
//! describe the same record and member sets.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::time::Duration;

use atproto_dasl::DaslCid;
use atproto_space::errors::SpaceResult;
use atproto_space::set_hash::record_element_bytes;
use atproto_space::storage::{MemberAccess, MemberChange, OplogCursor, OplogEntry, RecordChange, RecordRow};
use atproto_space::{
    LtHash, MemberOp, Op as RecordOp, OpAction, RepoOp, RepoState, SetHash, SpaceError, SpaceMembers,
    SpaceMembersStorage, SpaceRepo, SpaceRepoStorage, SpaceUri,
};
use motet_rules::Did;
use motet_tenor::keys::MasterKey;
use motet_tenor::shard::{Handle, Op, Options, Shard, Txn, spawn};
use motet_tenor::spaces::storage::{ReadStorage, TxnStorage};
use object_store::ObjectStore;
use object_store::memory::InMemory;
use proptest::prelude::*;

const COLLECTION: &str = "at.motet.pilot.post";

fn owner() -> Did {
    Did::new("did:plc:world")
}

fn space() -> SpaceUri {
    // The authority deliberately differs from the repository owner.
    SpaceUri::parse("at://did:plc:authority/space/at.motet.pilot.space.scene/storage").expect("Space URI")
}

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime")
}

async fn open(store: &Arc<dyn ObjectStore>) -> Shard {
    Shard::open(
        Arc::clone(store),
        "space-storage",
        Arc::new(MasterKey::from_bytes(&[9; 32])),
    )
    .await
    .expect("open shard")
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

fn record(action: OpAction, rkey: &str, value: u8) -> RecordOp {
    let bytes = atproto_dasl::to_vec(&serde_json::json!({ "$type": COLLECTION, "value": value })).expect("DRISL");
    let cid = DaslCid::from_dag_cbor_sha256(&bytes).to_string();
    RecordOp {
        action,
        collection: COLLECTION.into(),
        rkey: rkey.into(),
        cid: (action != OpAction::Delete).then_some(cid),
        value: (action != OpAction::Delete).then_some(bytes),
    }
}

#[derive(Debug)]
struct Records {
    owner: Did,
    space: SpaceUri,
    batches: Vec<Vec<RecordOp>>,
    abort: bool,
}

impl Records {
    fn new(batches: Vec<Vec<RecordOp>>) -> Self {
        Self {
            owner: owner(),
            space: space(),
            batches,
            abort: false,
        }
    }
}

impl Op for Records {
    type Output = RepoState;
    type Error = SpaceError;

    async fn run(self, txn: &mut Txn<'_>) -> SpaceResult<RepoState> {
        let repo = SpaceRepo::<_, LtHash>::new(self.space, TxnStorage::new(txn, &self.owner));
        for batch in self.batches {
            let prepared = repo.format_commit(&batch).await?;
            repo.apply_commit(prepared).await?;
        }
        if self.abort {
            return Err(SpaceError::InvalidOp {
                reason: "refuse the enclosing operation".into(),
            });
        }
        repo.current_state().await
    }
}

async fn all_records(repo: &SpaceRepo<ReadStorage, LtHash>, limit: u32, reverse: bool) -> Vec<RecordRow> {
    let mut out = Vec::new();
    let mut cursor = None;
    let mut seen = BTreeSet::new();
    loop {
        let page = repo
            .list_records(COLLECTION, cursor.as_deref(), limit, reverse)
            .await
            .expect("record page");
        assert!(page.records.len() <= limit as usize);
        for row in &page.records {
            assert!(seen.insert(row.rkey.clone()), "pagination repeated a record");
        }
        out.extend(page.records);
        if page.cursor.is_none() {
            break;
        }
        assert_ne!(page.cursor, cursor, "a record cursor must advance");
        cursor = page.cursor;
    }
    out
}

async fn all_ops(repo: &SpaceRepo<ReadStorage, LtHash>, limit: u32) -> Vec<OplogEntry> {
    // An old cursor has not missed anything when the store never trimmed.
    let mut cursor = Some(OplogCursor::new("2222222222222".into(), 0));
    let mut out = Vec::new();
    loop {
        let page = repo.read_oplog(cursor.as_ref(), limit).await.expect("retained oplog");
        assert!(page.ops.len() <= limit as usize);
        if page.ops.is_empty() {
            break;
        }
        for op in &page.ops {
            if let Some(previous) = &cursor {
                assert!((op.rev.as_str(), op.idx) > (previous.rev.as_str(), previous.idx));
            }
            cursor = Some(OplogCursor::new(op.rev.clone(), op.idx));
        }
        out.extend(page.ops);
    }
    out
}

fn fold_records(records: &BTreeMap<String, RecordOp>) -> LtHash {
    let mut hash = LtHash::empty();
    for (rkey, op) in records {
        hash.add(&record_element_bytes(
            COLLECTION,
            rkey,
            op.cid.as_deref().expect("live record CID"),
        ));
    }
    hash
}

fn model_batches(changes: &[BTreeMap<u8, Option<u8>>]) -> (Vec<Vec<RecordOp>>, BTreeMap<String, RecordOp>) {
    let mut model = BTreeMap::new();
    let mut batches = Vec::with_capacity(changes.len());
    for changes in changes {
        let mut batch = Vec::with_capacity(changes.len());
        for (key, value) in changes {
            let rkey = format!("{key:03}");
            match value {
                Some(value) => {
                    let action = if model.contains_key(&rkey) {
                        OpAction::Update
                    } else {
                        OpAction::Create
                    };
                    let op = record(action, &rkey, *value);
                    model.insert(rkey, op.clone());
                    batch.push(op);
                }
                None if model.remove(&rkey).is_some() => batch.push(record(OpAction::Delete, &rkey, 0)),
                None => {}
            }
        }
        batches.push(batch);
    }
    (batches, model)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(24))]

    /// Every acknowledged version survives takeover and can be changed again.
    /// Persisting only the digest, reading past the Txn overlay, or omitting
    /// Space mutations from the log breaks the independent set fold here.
    #[test]
    fn replay_preserves_the_full_commitment_across_multiple_writes(
        changes in proptest::collection::vec(
            proptest::collection::btree_map(0..12u8, proptest::option::of(any::<u8>()), 1..7),
            1..9,
        ),
        page_size in 1..7u32,
        checkpoint in any::<bool>(),
    ) {
        runtime().block_on(async {
            let bucket: Arc<dyn ObjectStore> = Arc::new(InMemory::new());
            let handle = running(open(&bucket).await);
            let (batches, mut model) = model_batches(&changes);
            let committed = handle.run(Records::new(batches)).await.expect("durable").expect("record commits");
            let expected = fold_records(&model).state_bytes();
            prop_assert_eq!(committed.set_hash.as_deref(), Some(expected.as_slice()));

            // Take over while the first writer is still alive: nothing relies
            // on its shutdown flushing an uncheckpointed memtable.
            let mut replayed = open(&bucket).await;
            drop(handle);
            if checkpoint {
                replayed.checkpoint().await.expect("checkpoint");
                replayed.close().await.expect("close checkpoint");
                replayed = open(&bucket).await;
            }
            let handle = running(replayed);
            let repo = SpaceRepo::<_, LtHash>::new(space(), ReadStorage::new(handle.reader(), &owner()));
            prop_assert_eq!(repo.current_state().await.expect("replayed state"), committed);
            let collections = if model.is_empty() { vec![] } else { vec![COLLECTION.to_string()] };
            prop_assert_eq!(repo.list_collections().await.expect("nonempty collections"), collections);
            for reverse in [false, true] {
                let rows = all_records(&repo, page_size, reverse).await;
                let mut expected_keys: Vec<_> = model.keys().cloned().collect();
                if !reverse { expected_keys.reverse(); }
                prop_assert_eq!(rows.iter().map(|row| row.rkey.clone()).collect::<Vec<_>>(), expected_keys);
                for row in rows {
                    let expected = model.get(&row.rkey).expect("model record");
                    prop_assert_eq!(Some(row.cid), expected.cid.clone());
                    prop_assert_eq!(Some(row.value), expected.value.clone());
                }
            }
            let another_author = SpaceRepo::<_, LtHash>::new(
                space(), ReadStorage::new(handle.reader(), &Did::new("did:plc:other-author")),
            );
            prop_assert_eq!(another_author.current_state().await.expect("other author"), RepoState::empty());

            let next = record(OpAction::Create, "after-reopen", 73);
            model.insert(next.rkey.clone(), next.clone());
            let next_state = handle.run(Records::new(vec![vec![next]])).await.expect("durable").expect("write after replay");
            prop_assert_eq!(next_state.set_hash, Some(fold_records(&model).state_bytes()));
            Ok(())
        })?;
    }

    /// An atomic batch may span many pages, including indices above nine.
    /// A rev-only cursor, decimal index ordering, or joining historical values
    /// by key without checking their CID loses operations or mislabels bytes.
    #[test]
    fn batch_pagination_delivers_each_operation_once_and_only_current_values(
        count in 12..40u8,
        page_size in 1..12u32,
        first_value in any::<u8>(),
    ) {
        runtime().block_on(async {
            let bucket: Arc<dyn ObjectStore> = Arc::new(InMemory::new());
            let handle = running(open(&bucket).await);
            let initial: BTreeMap<_, _> = (0..count).map(|key| (key, Some(first_value))).collect();
            let updates = (0..count).filter(|key| key % 2 == 0).map(|key| (key, Some(first_value.wrapping_add(1)))).collect();
            let deletes = (0..count).filter(|key| key % 3 == 0).map(|key| (key, None)).collect();
            let (batches, model) = model_batches(&[initial, updates, deletes]);
            let expected_ops: usize = batches.iter().map(Vec::len).sum();
            let state = handle.run(Records::new(batches)).await.expect("durable").expect("three commits");
            let repo = SpaceRepo::<_, LtHash>::new(space(), ReadStorage::new(handle.reader(), &owner()));
            let ops = all_ops(&repo, page_size).await;
            prop_assert_eq!(ops.len(), expected_ops);
            let mut synced = LtHash::empty();
            for op in ops {
                let rkey = op.rkey.expect("record key");
                let current = model.get(&rkey).filter(|current| current.cid == op.cid);
                prop_assert_eq!(op.value, current.and_then(|current| current.value.clone()));
                RepoOp {
                    rev: op.rev,
                    collection: op.collection.expect("collection"),
                    rkey,
                    cid: op.cid,
                    prev: op.prev,
                }.apply(&mut synced).expect("consumer applies exactly once");
            }
            prop_assert_eq!(synced.state_bytes(), state.set_hash.expect("commitment"));
            prop_assert_eq!(synced.digest(), fold_records(&model).digest());
            prop_assert!(repo.read_oplog(None, 0).await.expect("empty page").ops.is_empty());
            Ok(())
        })?;
    }
}

#[derive(Debug)]
struct RefusedRecords {
    prefix: u8,
    delete: bool,
}

impl Op for RefusedRecords {
    type Output = ();
    type Error = SpaceError;

    async fn run(self, txn: &mut Txn<'_>) -> SpaceResult<()> {
        let mut ops: Vec<_> = (0..self.prefix)
            .map(|key| record(OpAction::Create, &format!("new-{key}"), key))
            .collect();
        ops.push(record(
            if self.delete {
                OpAction::Delete
            } else {
                OpAction::Update
            },
            "existing",
            8,
        ));
        let mut prepared = {
            let repo = SpaceRepo::<_, LtHash>::new(space(), TxnStorage::new(txn, &owner()));
            repo.format_commit(&ops).await?.storage_commit
        };
        match prepared.record_changes.last_mut().expect("final conflicting change") {
            RecordChange::Update { prior_cid, .. } | RecordChange::Delete { prior_cid, .. } => {
                *prior_cid = record(OpAction::Create, "existing", 99).cid.expect("different CID");
            }
            RecordChange::Create(_) => panic!("last operation updates or deletes"),
        }
        let storage = TxnStorage::new(txn, &owner());
        let refused = SpaceRepoStorage::apply_commit(&storage, &space(), prepared).await;
        assert!(
            matches!(refused, Err(SpaceError::InvalidOp { .. })),
            "prior CID must be checked"
        );
        // A caller is allowed to catch this refusal and keep its operation.
        // The rejected adapter call must not have staged the earlier creates.
        let txn = storage.finish();
        assert!(
            motet_tenor::spaces::storage::get_record_txn(txn, &owner(), &space(), COLLECTION, "new-0")
                .await?
                .is_none()
        );
        Ok(())
    }
}

#[derive(Debug)]
struct Members {
    adds: Vec<String>,
    removes: Vec<String>,
    access: MemberAccess,
}

impl Op for Members {
    type Output = RepoState;
    type Error = SpaceError;

    async fn run(self, txn: &mut Txn<'_>) -> SpaceResult<RepoState> {
        let members = SpaceMembers::<_, LtHash>::new(space(), TxnStorage::new(txn, &owner()));
        let adds: Vec<_> = self
            .adds
            .iter()
            .map(|did| MemberOp::add(did, MemberAccess::FULL))
            .collect();
        members.apply_commit(members.format_commit(&adds).await?).await?;
        let before_access = members.current_state().await?;
        for did in &self.adds {
            members.put_member(did, self.access).await?;
        }
        assert_eq!(
            members.current_state().await?,
            before_access,
            "access flags are not member-set changes"
        );
        if !self.removes.is_empty() {
            let removes: Vec<_> = self.removes.iter().map(MemberOp::remove).collect();
            members.apply_commit(members.format_commit(&removes).await?).await?;
        }
        members.current_state().await
    }
}

#[derive(Debug)]
struct RefusedMembers(u8);

impl Op for RefusedMembers {
    type Output = ();
    type Error = SpaceError;

    async fn run(self, txn: &mut Txn<'_>) -> SpaceResult<()> {
        let ops: Vec<_> = (0..self.0)
            .map(|key| MemberOp::add(format!("did:plc:new-{key}"), MemberAccess::FULL))
            .collect();
        let mut prepared = {
            let members = SpaceMembers::<_, LtHash>::new(space(), TxnStorage::new(txn, &owner()));
            members.format_commit(&ops).await?.storage_commit
        };
        prepared
            .member_changes
            .push(MemberChange::Remove("did:plc:absent".into()));
        let storage = TxnStorage::new(txn, &owner());
        assert!(matches!(
            SpaceMembersStorage::apply_commit(&storage, &space(), prepared).await,
            Err(SpaceError::NotAMember { .. }),
        ));
        Ok(())
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(24))]

    /// Refusing a late change must undo every earlier row and oplog mutation,
    /// even if the operation catches the error. A successful adapter commit
    /// must also disappear when its enclosing shard operation is refused.
    #[test]
    fn errors_leave_neither_partial_commits_nor_unlogged_effects(
        valid_prefix in 1..10u8,
        delete in any::<bool>(),
    ) {
        runtime().block_on(async {
            let bucket: Arc<dyn ObjectStore> = Arc::new(InMemory::new());
            let handle = running(open(&bucket).await);
            handle.run(Records::new(vec![vec![record(OpAction::Create, "existing", 1)]]))
                .await.expect("durable").expect("initial record");
            handle.run(Members { adds: vec!["did:plc:existing".into()], removes: vec![], access: MemberAccess::FULL })
                .await.expect("durable").expect("initial member");
            let repo = SpaceRepo::<_, LtHash>::new(space(), ReadStorage::new(handle.reader(), &owner()));
            let members = SpaceMembers::<_, LtHash>::new(space(), ReadStorage::new(handle.reader(), &owner()));
            let before_state = repo.current_state().await.expect("before state");
            let before_rows = all_records(&repo, 3, true).await;
            let before_ops = all_ops(&repo, 2).await;
            let before_members = members.list_members(None, 100).await.expect("before members").members;
            let before_member_state = members.current_state().await.expect("before member state");

            handle.run(RefusedRecords { prefix: valid_prefix, delete }).await.expect("durable").expect("caught refusal");
            handle.run(RefusedMembers(valid_prefix)).await.expect("durable").expect("caught member refusal");
            let mut abort = Records::new(vec![vec![record(OpAction::Create, "rolled-back", 41)]]);
            abort.abort = true;
            prop_assert!(handle.run(abort).await.expect("operation ran").is_err());

            let replayed = open(&bucket).await;
            drop(handle);
            let repo = SpaceRepo::<_, LtHash>::new(space(), ReadStorage::new(&replayed.reader(), &owner()));
            let members = SpaceMembers::<_, LtHash>::new(space(), ReadStorage::new(&replayed.reader(), &owner()));
            prop_assert_eq!(repo.current_state().await.expect("after state"), before_state);
            prop_assert_eq!(all_records(&repo, 3, true).await, before_rows);
            prop_assert_eq!(all_ops(&repo, 2).await, before_ops);
            prop_assert_eq!(members.current_state().await.expect("after member state"), before_member_state);
            prop_assert_eq!(members.list_members(None, 100).await.expect("after members").members, before_members);
            Ok(())
        })?;
    }

    /// Member sets survive replay and remain writable, while changing access
    /// leaves the DID commitment unchanged. Reusing record-state keys, storing
    /// a digest instead of the lattice, or hashing access flags breaks this.
    #[test]
    fn membership_and_access_survive_replay_without_changing_each_others_commitment(
        ids in proptest::collection::btree_set(0..20u8, 1..16),
        read in any::<bool>(),
        write in any::<bool>(),
        page_size in 1..6u32,
    ) {
        runtime().block_on(async {
            let bucket: Arc<dyn ObjectStore> = Arc::new(InMemory::new());
            let handle = running(open(&bucket).await);
            let adds: Vec<_> = ids.iter().map(|id| format!("did:plc:member-{id:03}")).collect();
            let removes: Vec<_> = ids.iter().filter(|id| **id % 2 == 0).map(|id| format!("did:plc:member-{id:03}")).collect();
            let expected: Vec<_> = adds.iter().filter(|did| !removes.contains(did)).cloned().collect();
            let access = MemberAccess { read, write };
            let state = handle.run(Members { adds, removes, access }).await.expect("durable").expect("member commits");
            let mut expected_hash = LtHash::empty();
            for did in &expected { expected_hash.add(did.as_bytes()); }
            prop_assert_eq!(state.set_hash, Some(expected_hash.state_bytes()));

            let replayed = open(&bucket).await;
            drop(handle);
            let handle = running(replayed);
            let members = SpaceMembers::<_, LtHash>::new(space(), ReadStorage::new(handle.reader(), &owner()));
            let mut actual = Vec::new();
            let mut cursor = None;
            loop {
                let page = members.list_members(cursor.as_deref(), page_size).await.expect("member page");
                for row in page.members {
                    prop_assert_eq!(row.access, access);
                    actual.push(row.did);
                }
                if page.cursor.is_none() { break; }
                prop_assert_ne!(&page.cursor, &cursor);
                cursor = page.cursor;
            }
            prop_assert_eq!(actual, expected);
            let repo = SpaceRepo::<_, LtHash>::new(space(), ReadStorage::new(handle.reader(), &owner()));
            prop_assert_eq!(repo.current_state().await.expect("separate record set"), RepoState::empty());
            let next = "did:plc:after-reopen";
            let state = handle.run(Members { adds: vec![next.into()], removes: vec![], access })
                .await.expect("durable").expect("member write after replay");
            expected_hash.add(next.as_bytes());
            prop_assert_eq!(state.set_hash, Some(expected_hash.state_bytes()));
            Ok(())
        })?;
    }
}

/// A Reader cannot bypass the shard log through any mutating trait method.
/// Returning success for a member-access change or writing a prepared commit
/// directly to SlateDB would violate this property.
#[tokio::test]
async fn committed_adapters_refuse_record_member_and_access_mutations() {
    let bucket: Arc<dyn ObjectStore> = Arc::new(InMemory::new());
    let handle = running(open(&bucket).await);
    handle
        .run(Records::new(vec![vec![record(OpAction::Create, "existing", 1)]]))
        .await
        .expect("durable")
        .expect("record");
    handle
        .run(Members {
            adds: vec!["did:plc:existing".into()],
            removes: vec![],
            access: MemberAccess::FULL,
        })
        .await
        .expect("durable")
        .expect("member");
    let reader = ReadStorage::new(handle.reader(), &owner());
    let repo = SpaceRepo::<_, LtHash>::new(space(), reader.clone());
    let members = SpaceMembers::<_, LtHash>::new(space(), reader.clone());
    let record_state = repo.current_state().await.expect("record state");
    let member_state = members.current_state().await.expect("member state");
    let commit = repo
        .format_commit(&[record(OpAction::Update, "existing", 2)])
        .await
        .expect("prepared record");
    assert!(matches!(
        repo.apply_commit(commit).await,
        Err(SpaceError::Storage { .. })
    ));
    let commit = members
        .format_commit(&[MemberOp::remove("did:plc:existing")])
        .await
        .expect("prepared member");
    assert!(matches!(
        members.apply_commit(commit).await,
        Err(SpaceError::Storage { .. })
    ));
    assert!(matches!(
        SpaceMembersStorage::set_member_access(
            &reader,
            &space(),
            "did:plc:existing",
            MemberAccess {
                read: false,
                write: false
            }
        )
        .await,
        Err(SpaceError::Storage { .. }),
    ));
    assert_eq!(
        repo.current_state().await.expect("unchanged record state"),
        record_state
    );
    assert_eq!(
        members.current_state().await.expect("unchanged member state"),
        member_state
    );
    assert_eq!(
        members
            .member_access("did:plc:existing")
            .await
            .expect("unchanged access"),
        Some(MemberAccess::FULL)
    );
}
