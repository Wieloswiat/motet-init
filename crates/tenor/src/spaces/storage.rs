//! Space records and membership over the shard's transaction and committed reader.
//!
//! The repository owner is explicit: a Space URI identifies its authority,
//! not the author whose record set is being stored. Both commitments retain
//! the complete set-hash state, including all 2048 bytes of `LtHash` state.
//!
//! Prepared commits are checked and encoded before any mutation reaches the
//! transaction. Their rows, oplog and commitment then enter the shard log
//! together. Oplogs are never trimmed, so an old cursor is not an `OplogGap`.
//! Record-op values are joined against the current CID when read; historical
//! operations never acquire the value of a different, newer record version.

use std::collections::{BTreeMap, BTreeSet};

use async_trait::async_trait;
use atproto_space::errors::SpaceResult;
use atproto_space::storage::{
    MemberAccess, MemberChange, MemberPage, MemberRow, MemberState, OplogCursor, OplogEntry, OplogPage,
    PreparedCommitMembers, PreparedCommitRecords, RecordChange, RecordPage, RecordRow, RepoState, SpaceMembersStorage,
    SpaceRepoStorage,
};
use atproto_space::{SpaceError, SpaceUri};
use bytes::Bytes;
use motet_rules::Did;
use serde::Serialize;
use serde::de::DeserializeOwned;
use tokio::sync::Mutex;

use crate::shard::Txn;
use crate::store::{Reader, keys};

/// A Space storage adapter borrowing the running shard operation.
///
/// Reads include the operation's overlay. Dropping this adapter (or the
/// `SpaceRepo`/`SpaceMembers` that owns it) releases the transaction borrow.
/// Format and apply each commit together inside that operation; a prepared
/// commitment must not outlive the state from which it was formatted.
#[derive(Debug)]
pub struct TxnStorage<'t, 's> {
    // Reads may await SlateDB, so this is an async lock, not a blocking lock.
    // The lock also keeps validation and mutation of one commit together.
    txn: Mutex<&'t mut Txn<'s>>,
    owner: Did,
}

impl<'t, 's> TxnStorage<'t, 's> {
    #[must_use]
    pub fn new(txn: &'t mut Txn<'s>, owner: &Did) -> Self {
        Self {
            txn: Mutex::new(txn),
            owner: owner.clone(),
        }
    }

    /// Releases the adapter without committing the enclosing shard operation.
    #[must_use]
    pub fn finish(self) -> &'t mut Txn<'s> {
        self.txn.into_inner()
    }
}

/// Committed Space storage. Every mutating trait method returns an error.
#[derive(Clone, Debug)]
pub struct ReadStorage {
    reader: Reader,
    owner: Did,
}

impl ReadStorage {
    #[must_use]
    pub fn new(reader: &Reader, owner: &Did) -> Self {
        Self {
            reader: reader.clone(),
            owner: owner.clone(),
        }
    }
}

fn storage_error(error: impl std::fmt::Display) -> SpaceError {
    SpaceError::Storage {
        reason: error.to_string(),
    }
}

fn read_only() -> SpaceError {
    storage_error("committed Space storage is read-only; write through the shard")
}

fn encode(value: &impl Serialize) -> SpaceResult<Bytes> {
    serde_json::to_vec(value).map(Bytes::from).map_err(storage_error)
}

fn decode<T: DeserializeOwned>(bytes: &[u8]) -> SpaceResult<T> {
    serde_json::from_slice(bytes).map_err(storage_error)
}

#[derive(Clone, Copy)]
enum View<'a, 's> {
    Txn(&'a Txn<'s>),
    Reader(&'a Reader),
}

impl View<'_, '_> {
    async fn get<T: DeserializeOwned>(self, key: &[u8]) -> SpaceResult<Option<T>> {
        let bytes = match self {
            Self::Txn(txn) => txn.get(key).await.map_err(storage_error)?,
            Self::Reader(reader) => reader.get(key).await.map_err(storage_error)?,
        };
        bytes.as_deref().map(decode).transpose()
    }

    async fn scan(self, prefix: &[u8], after: Option<&[u8]>, limit: usize) -> SpaceResult<Vec<(Bytes, Bytes)>> {
        match self {
            Self::Txn(txn) => {
                // Txn::scan merges the overlay but has no seek cursor.
                let mut rows = txn
                    .scan(prefix, if after.is_some() { usize::MAX } else { limit })
                    .await
                    .map_err(storage_error)?;
                if let Some(after) = after {
                    rows.retain(|(key, _)| key.as_ref() > after);
                    rows.truncate(limit);
                }
                Ok(rows)
            }
            Self::Reader(reader) => reader.scan(prefix, after, limit).await.map_err(storage_error),
        }
    }

    async fn state(self, key: &[u8]) -> SpaceResult<RepoState> {
        Ok(self.get(key).await?.unwrap_or_else(RepoState::empty))
    }

    async fn record(
        self,
        owner: &Did,
        space: &SpaceUri,
        collection: &str,
        rkey: &str,
    ) -> SpaceResult<Option<RecordRow>> {
        self.get(&keys::space_record(owner, space, collection, rkey)).await
    }

    async fn records(
        self,
        owner: &Did,
        space: &SpaceUri,
        collection: &str,
        cursor: Option<&str>,
        limit: u32,
        reverse: bool,
    ) -> SpaceResult<RecordPage> {
        let prefix = keys::space_records(owner, space, Some(collection));
        let cursor_key = cursor.map(|rkey| keys::space_record(owner, space, collection, rkey));
        let limit = limit as usize;
        if limit == 0 {
            return Ok(RecordPage {
                records: Vec::new(),
                cursor: None,
            });
        }
        let mut rows = if reverse {
            self.scan(&prefix, cursor_key.as_deref(), limit.saturating_add(1))
                .await?
        } else {
            // The store exposes forward scans only. Reverse the raw rows,
            // then decode only this page rather than every record value.
            let mut rows = self.scan(&prefix, None, usize::MAX).await?;
            if let Some(cursor) = cursor_key {
                rows.retain(|(key, _)| key.as_ref() < cursor.as_slice());
            }
            rows.reverse();
            rows.truncate(limit.saturating_add(1));
            rows
        };
        let more = rows.len() > limit;
        rows.truncate(limit);
        let records: Vec<RecordRow> = rows
            .into_iter()
            .map(|(_, value)| decode(&value))
            .collect::<SpaceResult<_>>()?;
        let cursor = if more {
            records.last().map(|row| row.rkey.clone())
        } else {
            None
        };
        Ok(RecordPage { records, cursor })
    }

    async fn collections(self, owner: &Did, space: &SpaceUri) -> SpaceResult<Vec<String>> {
        let prefix = keys::space_records(owner, space, None);
        let rows = self.scan(&prefix, None, usize::MAX).await?;
        let mut collections = BTreeSet::new();
        for (key, _) in &rows {
            let collection =
                keys::space_record_collection(&prefix, key).ok_or_else(|| storage_error("invalid Space record key"))?;
            collections.insert(collection);
        }
        Ok(collections.into_iter().map(str::to_owned).collect())
    }

    async fn members(self, owner: &Did, space: &SpaceUri, cursor: Option<&str>, limit: u32) -> SpaceResult<MemberPage> {
        let limit = limit as usize;
        if limit == 0 {
            return Ok(MemberPage {
                members: Vec::new(),
                cursor: None,
            });
        }
        let after = cursor.map(|did| keys::space_member(owner, space, did));
        let mut rows = self
            .scan(
                &keys::space_members(owner, space),
                after.as_deref(),
                limit.saturating_add(1),
            )
            .await?;
        let more = rows.len() > limit;
        rows.truncate(limit);
        let members: Vec<MemberRow> = rows
            .into_iter()
            .map(|(_, value)| decode(&value))
            .collect::<SpaceResult<_>>()?;
        let cursor = if more {
            members.last().map(|row| row.did.clone())
        } else {
            None
        };
        Ok(MemberPage { members, cursor })
    }

    async fn member_access(self, owner: &Did, space: &SpaceUri, did: &str) -> SpaceResult<Option<MemberAccess>> {
        Ok(self
            .get::<MemberRow>(&keys::space_member(owner, space, did))
            .await?
            .map(|row| row.access))
    }

    async fn oplog(
        self,
        owner: &Did,
        space: &SpaceUri,
        since: Option<&OplogCursor>,
        limit: u32,
    ) -> SpaceResult<OplogPage> {
        let after = since.map(|cursor| keys::space_repo_op(owner, space, &cursor.rev, cursor.idx));
        let rows = self
            .scan(&keys::space_repo_oplog(owner, space), after.as_deref(), limit as usize)
            .await?;
        let mut ops = Vec::with_capacity(rows.len());
        for (_, value) in rows {
            let mut entry: OplogEntry = decode(&value)?;
            entry.value = None;
            if matches!(entry.action.as_str(), "create" | "update")
                && let (Some(collection), Some(rkey), Some(cid)) = (&entry.collection, &entry.rkey, &entry.cid)
                && let Some(row) = self.record(owner, space, collection, rkey).await?
                && row.cid == *cid
            {
                entry.value = Some(row.value);
            }
            ops.push(entry);
        }
        let state = self.state(&keys::space_repo_state(owner, space)).await?;
        Ok(OplogPage { ops, state })
    }
}

/// Reads the full record-set state without taking a mutable transaction borrow.
pub async fn current_state_txn(txn: &Txn<'_>, owner: &Did, space: &SpaceUri) -> SpaceResult<RepoState> {
    View::Txn(txn).state(&keys::space_repo_state(owner, space)).await
}

/// Reads a record from the transaction overlay without a mutable borrow.
pub async fn get_record_txn(
    txn: &Txn<'_>,
    owner: &Did,
    space: &SpaceUri,
    collection: &str,
    rkey: &str,
) -> SpaceResult<Option<RecordRow>> {
    View::Txn(txn).record(owner, space, collection, rkey).await
}

type Mutations = BTreeMap<Vec<u8>, Option<Bytes>>;

fn enqueue(txn: &mut Txn<'_>, mutations: Mutations) {
    // No fallible work or suspension after the first mutation: a refused or
    // cancelled adapter call must leave the enclosing operation untouched.
    for (key, value) in mutations {
        match value {
            Some(value) => txn.put(key, value),
            None => txn.delete(key),
        }
    }
}

async fn check_revision(txn: &Txn<'_>, state_key: &[u8], rev: &str) -> SpaceResult<()> {
    let state = View::Txn(txn).state(state_key).await?;
    if state.rev.as_deref().is_some_and(|current| current >= rev) {
        return Err(SpaceError::InvalidOp {
            reason: "commit revision does not advance current state".into(),
        });
    }
    Ok(())
}

async fn append_oplog(
    txn: &Txn<'_>,
    mutations: &mut Mutations,
    owner: &Did,
    space: &SpaceUri,
    entries: Vec<OplogEntry>,
    key: fn(&Did, &SpaceUri, &str, u32) -> Vec<u8>,
) -> SpaceResult<()> {
    for mut entry in entries {
        let key = key(owner, space, &entry.rev, entry.idx);
        if mutations.contains_key(&key) || txn.get(&key).await.map_err(storage_error)?.is_some() {
            return Err(SpaceError::InvalidOp {
                reason: "oplog position already exists".into(),
            });
        }
        // Historical values are deliberately not retained in the oplog.
        entry.value = None;
        mutations.insert(key, Some(encode(&entry)?));
    }
    Ok(())
}

async fn apply_records(
    txn: &mut Txn<'_>,
    owner: &Did,
    space: &SpaceUri,
    commit: PreparedCommitRecords,
) -> SpaceResult<()> {
    let state_key = keys::space_repo_state(owner, space);
    check_revision(txn, &state_key, &commit.rev).await?;
    let mut mutations = Mutations::new();
    for change in &commit.record_changes {
        let (collection, rkey) = match change {
            RecordChange::Create(row) | RecordChange::Update { row, .. } => (&row.collection, &row.rkey),
            RecordChange::Delete { collection, rkey, .. } => (collection, rkey),
        };
        let key = keys::space_record(owner, space, collection, rkey);
        if mutations.contains_key(&key) {
            return Err(SpaceError::DuplicateKeyInBatch {
                collection: collection.clone(),
                rkey: rkey.clone(),
            });
        }
        let prior: Option<RecordRow> = View::Txn(txn).get(&key).await?;
        let value = match change {
            RecordChange::Create(row) => {
                if prior.is_some() {
                    return Err(SpaceError::RecordAlreadyExists {
                        collection: collection.clone(),
                        rkey: rkey.clone(),
                    });
                }
                Some(encode(row)?)
            }
            RecordChange::Update { row, prior_cid } => {
                check_prior(prior.as_ref(), collection, rkey, prior_cid)?;
                Some(encode(row)?)
            }
            RecordChange::Delete { prior_cid, .. } => {
                check_prior(prior.as_ref(), collection, rkey, prior_cid)?;
                None
            }
        };
        mutations.insert(key, value);
    }
    append_oplog(
        txn,
        &mut mutations,
        owner,
        space,
        commit.oplog_entries,
        keys::space_repo_op,
    )
    .await?;
    let state = RepoState {
        set_hash: Some(commit.new_set_hash),
        rev: Some(commit.rev),
    };
    mutations.insert(state_key, Some(encode(&state)?));
    enqueue(txn, mutations);
    Ok(())
}

fn check_prior(prior: Option<&RecordRow>, collection: &str, rkey: &str, prior_cid: &str) -> SpaceResult<()> {
    let prior = prior.ok_or_else(|| SpaceError::RecordNotFound {
        collection: collection.into(),
        rkey: rkey.into(),
    })?;
    if prior.cid != prior_cid {
        return Err(SpaceError::InvalidOp {
            reason: format!("prior CID changed for {collection}/{rkey}"),
        });
    }
    Ok(())
}

async fn apply_members(
    txn: &mut Txn<'_>,
    owner: &Did,
    space: &SpaceUri,
    commit: PreparedCommitMembers,
) -> SpaceResult<()> {
    let state_key = keys::space_member_state(owner, space);
    check_revision(txn, &state_key, &commit.rev).await?;
    let mut mutations = Mutations::new();
    for change in &commit.member_changes {
        let did = match change {
            MemberChange::Add(row) => &row.did,
            MemberChange::Remove(did) => did,
        };
        let key = keys::space_member(owner, space, did);
        if mutations.contains_key(&key) {
            return Err(SpaceError::DuplicateMemberInBatch { did: did.clone() });
        }
        let present = txn.get(&key).await.map_err(storage_error)?.is_some();
        let value = match change {
            MemberChange::Add(row) => {
                if present {
                    return Err(SpaceError::MemberAlreadyExists { did: did.clone() });
                }
                Some(encode(row)?)
            }
            MemberChange::Remove(did) => {
                if !present {
                    return Err(SpaceError::NotAMember { did: did.clone() });
                }
                None
            }
        };
        mutations.insert(key, value);
    }
    append_oplog(
        txn,
        &mut mutations,
        owner,
        space,
        commit.oplog_entries,
        keys::space_member_op,
    )
    .await?;
    let state = MemberState {
        set_hash: Some(commit.new_set_hash),
        rev: Some(commit.rev),
    };
    mutations.insert(state_key, Some(encode(&state)?));
    enqueue(txn, mutations);
    Ok(())
}

#[async_trait]
impl SpaceRepoStorage for TxnStorage<'_, '_> {
    async fn current_state(&self, space: &SpaceUri) -> SpaceResult<RepoState> {
        let txn = self.txn.lock().await;
        current_state_txn(&txn, &self.owner, space).await
    }

    async fn get_record(&self, space: &SpaceUri, collection: &str, rkey: &str) -> SpaceResult<Option<RecordRow>> {
        let txn = self.txn.lock().await;
        get_record_txn(&txn, &self.owner, space, collection, rkey).await
    }

    async fn list_records(
        &self,
        space: &SpaceUri,
        collection: &str,
        cursor: Option<&str>,
        limit: u32,
        reverse: bool,
    ) -> SpaceResult<RecordPage> {
        let txn = self.txn.lock().await;
        View::Txn(&txn)
            .records(&self.owner, space, collection, cursor, limit, reverse)
            .await
    }

    async fn list_collections(&self, space: &SpaceUri) -> SpaceResult<Vec<String>> {
        let txn = self.txn.lock().await;
        View::Txn(&txn).collections(&self.owner, space).await
    }

    async fn apply_commit(&self, space: &SpaceUri, commit: PreparedCommitRecords) -> SpaceResult<()> {
        let mut txn = self.txn.lock().await;
        apply_records(&mut txn, &self.owner, space, commit).await
    }

    async fn read_oplog(&self, space: &SpaceUri, since: Option<&OplogCursor>, limit: u32) -> SpaceResult<OplogPage> {
        let txn = self.txn.lock().await;
        View::Txn(&txn).oplog(&self.owner, space, since, limit).await
    }
}

#[async_trait]
impl SpaceMembersStorage for TxnStorage<'_, '_> {
    async fn current_state(&self, space: &SpaceUri) -> SpaceResult<MemberState> {
        let txn = self.txn.lock().await;
        View::Txn(&txn)
            .state(&keys::space_member_state(&self.owner, space))
            .await
    }

    async fn member_access(&self, space: &SpaceUri, did: &str) -> SpaceResult<Option<MemberAccess>> {
        let txn = self.txn.lock().await;
        View::Txn(&txn).member_access(&self.owner, space, did).await
    }

    async fn set_member_access(&self, space: &SpaceUri, did: &str, access: MemberAccess) -> SpaceResult<bool> {
        let mut txn = self.txn.lock().await;
        let key = keys::space_member(&self.owner, space, did);
        let Some(mut row) = View::Txn(&txn).get::<MemberRow>(&key).await? else {
            return Ok(false);
        };
        row.access = access;
        let value = encode(&row)?;
        txn.put(key, value);
        drop(txn);
        Ok(true)
    }

    async fn list_members(&self, space: &SpaceUri, cursor: Option<&str>, limit: u32) -> SpaceResult<MemberPage> {
        let txn = self.txn.lock().await;
        View::Txn(&txn).members(&self.owner, space, cursor, limit).await
    }

    async fn apply_commit(&self, space: &SpaceUri, commit: PreparedCommitMembers) -> SpaceResult<()> {
        let mut txn = self.txn.lock().await;
        apply_members(&mut txn, &self.owner, space, commit).await
    }
}

#[async_trait]
impl SpaceRepoStorage for ReadStorage {
    async fn current_state(&self, space: &SpaceUri) -> SpaceResult<RepoState> {
        View::Reader(&self.reader)
            .state(&keys::space_repo_state(&self.owner, space))
            .await
    }

    async fn get_record(&self, space: &SpaceUri, collection: &str, rkey: &str) -> SpaceResult<Option<RecordRow>> {
        View::Reader(&self.reader)
            .record(&self.owner, space, collection, rkey)
            .await
    }

    async fn list_records(
        &self,
        space: &SpaceUri,
        collection: &str,
        cursor: Option<&str>,
        limit: u32,
        reverse: bool,
    ) -> SpaceResult<RecordPage> {
        View::Reader(&self.reader)
            .records(&self.owner, space, collection, cursor, limit, reverse)
            .await
    }

    async fn list_collections(&self, space: &SpaceUri) -> SpaceResult<Vec<String>> {
        View::Reader(&self.reader).collections(&self.owner, space).await
    }

    async fn apply_commit(&self, _space: &SpaceUri, _commit: PreparedCommitRecords) -> SpaceResult<()> {
        Err(read_only())
    }

    async fn read_oplog(&self, space: &SpaceUri, since: Option<&OplogCursor>, limit: u32) -> SpaceResult<OplogPage> {
        View::Reader(&self.reader).oplog(&self.owner, space, since, limit).await
    }
}

#[async_trait]
impl SpaceMembersStorage for ReadStorage {
    async fn current_state(&self, space: &SpaceUri) -> SpaceResult<MemberState> {
        View::Reader(&self.reader)
            .state(&keys::space_member_state(&self.owner, space))
            .await
    }

    async fn member_access(&self, space: &SpaceUri, did: &str) -> SpaceResult<Option<MemberAccess>> {
        View::Reader(&self.reader).member_access(&self.owner, space, did).await
    }

    async fn set_member_access(&self, _space: &SpaceUri, _did: &str, _access: MemberAccess) -> SpaceResult<bool> {
        Err(read_only())
    }

    async fn list_members(&self, space: &SpaceUri, cursor: Option<&str>, limit: u32) -> SpaceResult<MemberPage> {
        View::Reader(&self.reader)
            .members(&self.owner, space, cursor, limit)
            .await
    }

    async fn apply_commit(&self, _space: &SpaceUri, _commit: PreparedCommitMembers) -> SpaceResult<()> {
        Err(read_only())
    }
}
