//! Signed public World repositories, written only on the shard's writer.
//!
//! All validation and swap checks precede staging. Blocks, head, record index
//! and firehose event become durable together through the enclosing operation.

mod commit_car;

use std::collections::{BTreeMap, BTreeSet};
use std::str::FromStr;
use std::sync::LazyLock;

use atproto_dasl::{BlockStorage, Cid};
use atproto_lexicon::validation::{BaseCatalog, SchemaDef, ValidateFlags};
use atproto_record::datetime::Datetime;
use atproto_record::tid::Tid;
use atproto_repo::{Mst, MstNode, RepoConfig, UnsignedCommit};
use bytes::Bytes;
use motet_rules::Did;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use crate::event::{self, Action, Event, RepoOp};
use crate::keys::Purpose;
use crate::shard::{Op, Txn, TxnError};
use crate::store::{ReadError, Reader, keys};

/// A syntactically valid collection name. The writer additionally requires a
/// record schema in Motet's offline lexicon catalog.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Nsid(String);

impl Nsid {
    pub fn new(value: impl Into<String>) -> Result<Self, RepoError> {
        let value = value.into();
        atproto_record::nsid::check_nsid(&value).map_err(|e| RepoError::InvalidRecord(e.to_owned()))?;
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for Nsid {
    type Err = RepoError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value)
    }
}

/// One conditional record mutation. Records are canonical DRISL; JSON callers
/// use [`record_from_json`] so `$link` and `$bytes` retain their data-model types.
#[derive(Debug)]
pub enum Write {
    Create {
        collection: Nsid,
        rkey: String,
        record: Bytes,
    },
    Update {
        collection: Nsid,
        rkey: String,
        record: Bytes,
        swap: Option<Cid>,
    },
    Delete {
        collection: Nsid,
        rkey: String,
        swap: Option<Cid>,
    },
}

impl Write {
    fn address(&self) -> (&str, &str) {
        match self {
            Self::Create { collection, rkey, .. }
            | Self::Update { collection, rkey, .. }
            | Self::Delete { collection, rkey, .. } => (collection.as_str(), rkey),
        }
    }
}

#[derive(Debug)]
pub struct Committed {
    pub commit: Cid,
    pub rev: Tid,
    /// In request order; deletions return no CID.
    pub results: Vec<(String, Option<Cid>)>,
}

/// The immutable commit selected by a current-head or revision index.
///
/// Both indexes persist this as JSON with CID strings, like the shard events.
/// Native CID serde expects CBOR tag 42 and cannot read those JSON indexes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Head {
    #[serde(with = "cid_string")]
    pub cid: Cid,
    #[serde(with = "cid_string")]
    pub data: Cid,
    pub rev: Tid,
}

mod cid_string {
    use atproto_dasl::Cid;
    use serde::{Deserialize, Deserializer, Serializer};

    pub(super) fn serialize<S: Serializer>(cid: &Cid, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(cid)
    }

    pub(super) fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Cid, D::Error> {
        let text = <&str>::deserialize(deserializer)?;
        text.parse().map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Error)]
pub enum RepoError {
    #[error("the repo or record changed since the supplied swap")]
    InvalidSwap,
    #[error("World {0} is not hosted")]
    UnknownWorld(Did),
    #[error("World {0} already has a repository")]
    AlreadyExists(Did),
    #[error("World {0} has no signing key")]
    MissingSigningKey(Did),
    #[error("record {0} already exists")]
    RecordAlreadyExists(String),
    #[error("record {0} does not exist")]
    RecordNotFound(String),
    #[error("collection {0} is not a known record lexicon")]
    UnknownCollection(String),
    #[error("invalid record: {0}")]
    InvalidRecord(String),
    #[error("commit exceeds the firehose CAR limit of two million bytes")]
    CommitTooLarge,
    #[error("the compiled lexicon catalog is invalid: {0}")]
    Catalog(String),
    #[error(transparent)]
    Transaction(#[from] TxnError),
    #[error(transparent)]
    Read(#[from] ReadError),
    #[error(transparent)]
    Storage(#[from] atproto_dasl::StorageError),
    #[error(transparent)]
    Mst(#[from] atproto_repo::MstError),
    #[error(transparent)]
    Protocol(#[from] atproto_repo::RepoError),
    #[error(transparent)]
    Encode(#[from] atproto_dasl::EncodeError),
    #[error(transparent)]
    Decode(#[from] atproto_dasl::DecodeError),
    #[error(transparent)]
    Car(#[from] atproto_dasl::CarError),
    #[error(transparent)]
    Signing(#[from] atproto_identity::errors::KeyError),
    #[error(transparent)]
    State(#[from] serde_json::Error),
}

/// A shard operation for an already-authorized caller. Session/role checks
/// belong in the calling operation, not in this storage primitive.
#[derive(Debug)]
pub struct ApplyWrites {
    pub world: Did,
    pub writes: Vec<Write>,
    pub swap_commit: Option<Cid>,
}

impl Op for ApplyWrites {
    type Output = Committed;
    type Error = RepoError;

    async fn run(self, txn: &mut Txn<'_>) -> Result<Self::Output, Self::Error> {
        commit(txn, &self.world, self.writes, self.swap_commit).await
    }
}

/// Encodes JSON's link and byte sentinels as DRISL links and byte strings.
/// Schema validation happens in [`commit`] or [`validate_record`].
pub fn record_from_json(value: &Value) -> Result<Bytes, RepoError> {
    Ok(Bytes::from(atproto_dasl::to_vec_from_json(value)?))
}

fn catalog() -> Result<&'static BaseCatalog, RepoError> {
    static CATALOG: LazyLock<Result<BaseCatalog, String>> = LazyLock::new(|| {
        let mut catalog = BaseCatalog::new();
        for (_, json) in motet_lexicons::SCHEMAS {
            catalog.add_schema_json(json).map_err(|e| e.to_string())?;
        }
        Ok(catalog)
    });
    CATALOG.as_ref().map_err(|e| RepoError::Catalog(e.clone()))
}

pub(crate) fn validate_address(collection: &str, rkey: &str) -> Result<(), RepoError> {
    atproto_record::record_key::check_record_key(rkey).map_err(|e| RepoError::InvalidRecord(e.to_owned()))?;
    let schema = catalog()?.get_schema(collection).and_then(|s| s.main());
    let Some(SchemaDef::Record(record)) = schema else {
        return Err(RepoError::UnknownCollection(collection.to_owned()));
    };
    match record.key.as_deref() {
        Some("tid") if rkey.parse::<Tid>().is_err() => {
            Err(RepoError::InvalidRecord("the record key must be a TID".to_owned()))
        }
        Some(key) if key.strip_prefix("literal:").is_some_and(|literal| literal != rkey) => {
            Err(RepoError::InvalidRecord(format!("the record key must match {key}")))
        }
        _ => Ok(()),
    }
}

/// Validates canonical DRISL and the complete known lexicon, including references.
///
/// Every supported schema is compiled into `SCHEMAS`; validation never resolves
/// a schema over the network while holding a writer.
/// Callers enforce their transport's record-size limit separately: public
/// repositories and Spaces have different limits.
pub fn validate_record(collection: &str, rkey: &str, record: &[u8]) -> Result<(), RepoError> {
    validate_address(collection, rkey)?;
    let config = atproto_dasl::DecodeConfig::default()
        .with_no_floats(true)
        .with_int64_range_only(true);
    let value = atproto_dasl::from_slice_to_json_with_config(record, config)?;
    atproto_lexicon::validation::validate_record(collection, &value, catalog()?, ValidateFlags::empty())
        .map_err(|e| RepoError::InvalidRecord(e.to_string()))
}

pub async fn head(reader: &Reader, world: &Did) -> Result<Option<Head>, RepoError> {
    reader
        .get(&keys::repo_head(world))
        .await?
        .map(|v| serde_json::from_slice(&v))
        .transpose()
        .map_err(Into::into)
}

async fn sign(txn: &Txn<'_>, world: &Did, data: Cid, rev: Tid) -> Result<(Head, Vec<u8>), RepoError> {
    let key = txn
        .key(world, Purpose::Signing)
        .await?
        .ok_or_else(|| RepoError::MissingSigningKey(world.clone()))?;
    // Sync 1.1 chains revisions and prevData in events, not commit.prev.
    let unsigned = UnsignedCommit::new(world.as_str().to_owned(), data, rev.to_string(), None);
    let signature = atproto_identity::key::sign(&key, &unsigned.to_bytes()?)?;
    let bytes = unsigned.sign(signature).to_bytes()?;
    let cid = atproto_dasl::cid::compute_cid(&bytes);
    Ok((Head { cid, data, rev }, bytes))
}

fn store_head(txn: &mut Txn<'_>, world: &Did, head: &Head, encoded: Bytes) {
    txn.put(keys::repo_head(world), encoded.clone());
    txn.put(keys::repo_revision(world, &head.rev.to_string()), encoded);
}

/// Creates the empty signed repo after the World's keys and rules exist.
/// An existing head is refused; this is not a reset or recovery operation.
pub async fn genesis(txn: &mut Txn<'_>, world: &Did) -> Result<Committed, RepoError> {
    if txn.world(world).is_none() {
        return Err(RepoError::UnknownWorld(world.clone()));
    }
    if txn.get(&keys::repo_head(world)).await?.is_some() {
        return Err(RepoError::AlreadyExists(world.clone()));
    }
    let empty = atproto_dasl::to_vec(&MstNode::empty())?;
    let root = atproto_dasl::cid::compute_cid(&empty);
    let (head, commit) = sign(txn, world, root, Tid::new()).await?;
    let car = commit_car::build_sync_car(head.cid, &commit).await?;
    let encoded = Bytes::from(serde_json::to_vec(&head)?);
    txn.put_blocks(world, [(root, empty), (head.cid, commit)]);
    store_head(txn, world, &head, encoded);
    txn.emit(Event::Sync(event::Sync {
        repo: world.clone(),
        rev: head.rev.to_string(),
        blocks: Bytes::from(car),
        time: Datetime::now().to_string(),
    }));
    Ok(Committed {
        commit: head.cid,
        rev: head.rev,
        results: Vec::new(),
    })
}

struct Prepared {
    path: String,
    index: Vec<u8>,
    action: Action,
    prev: Option<Cid>,
    next: Option<(Cid, Bytes)>,
}

async fn prepare(
    mst: &Mst<crate::shard::TxnBlocks<'_>>,
    world: &Did,
    writes: Vec<Write>,
) -> Result<Vec<Prepared>, RepoError> {
    if writes.len() > 200 {
        return Err(RepoError::InvalidRecord(
            "a commit accepts at most 200 writes".to_owned(),
        ));
    }
    let mut paths = BTreeSet::new();
    let mut prepared = Vec::with_capacity(writes.len());
    for write in writes {
        let (collection, rkey) = write.address();
        match &write {
            Write::Create { record, .. } | Write::Update { record, .. } => {
                if record.len() > 1_000_000 {
                    return Err(RepoError::InvalidRecord("record exceeds one million bytes".to_owned()));
                }
                validate_record(collection, rkey, record)?;
            }
            Write::Delete { .. } => validate_address(collection, rkey)?,
        }
        let path = format!("{collection}/{rkey}");
        if !paths.insert(path.clone()) {
            return Err(RepoError::InvalidRecord(format!("duplicate write to {path}")));
        }
        let index = keys::repo_record(world, collection, rkey);
        let prev = mst.get(&path).await?;
        let (action, record) = match write {
            Write::Create { record, .. } => {
                if prev.is_some() {
                    return Err(RepoError::RecordAlreadyExists(path));
                }
                (Action::Create, Some(record))
            }
            Write::Update { record, swap, .. } => {
                if swap.is_some() && swap != prev {
                    return Err(RepoError::InvalidSwap);
                }
                if prev.is_none() {
                    return Err(RepoError::RecordNotFound(path));
                }
                (Action::Update, Some(record))
            }
            Write::Delete { swap, .. } => {
                if swap.is_some() && swap != prev {
                    return Err(RepoError::InvalidSwap);
                }
                if prev.is_none() {
                    return Err(RepoError::RecordNotFound(path));
                }
                (Action::Delete, None)
            }
        };
        let next = record.map(|record| (atproto_dasl::cid::compute_cid(&record), record));
        prepared.push(Prepared {
            path,
            index,
            action,
            prev,
            next,
        });
    }
    Ok(prepared)
}

/// Validates and applies a batch against the current signed head. A failed
/// swap, invalid record or storage error stages no effects, even if the caller
/// catches the error inside a larger operation.
pub async fn commit(
    txn: &mut Txn<'_>,
    world: &Did,
    writes: Vec<Write>,
    swap_commit: Option<Cid>,
) -> Result<Committed, RepoError> {
    let prior: Head = txn
        .get(&keys::repo_head(world))
        .await?
        .ok_or_else(|| RepoError::UnknownWorld(world.clone()))
        .and_then(|v| serde_json::from_slice(&v).map_err(Into::into))?;
    if swap_commit.is_some_and(|swap| swap != prior.cid) {
        return Err(RepoError::InvalidSwap);
    }
    let mut mst = Mst::from_root(prior.data, txn.blocks(world), RepoConfig::default());
    let prepared = prepare(&mst, world, writes).await?;
    for write in &prepared {
        if let Some((cid, record)) = &write.next {
            mst.storage_mut().put(cid, record.to_vec()).await?;
            mst.insert(&write.path, *cid).await?;
        } else {
            mst.delete(&write.path).await?;
        }
    }
    let data = if let Some(root) = mst.root() {
        *root
    } else {
        let empty = atproto_dasl::to_vec(&MstNode::empty())?;
        let cid = atproto_dasl::cid::compute_cid(&empty);
        mst.storage_mut().put(&cid, empty).await?;
        cid
    };
    let mut proof = BTreeMap::new();
    for write in &prepared {
        proof.extend(mst.covering_proof(&write.path).await?);
    }
    // The transaction already records new blocks; proofs only affect the CAR,
    // not the durable batch, which need not rewrite unchanged MST nodes.
    let new: BTreeMap<_, _> = mst.into_storage().into_new().into_iter().collect();
    let (head, commit_block) = sign(txn, world, data, Tid::next_after(&prior.rev)).await?;
    let car = commit_car::build_commit_car(head.cid, &commit_block, &new, &proof).await?;
    if car.len() > 2_000_000 {
        return Err(RepoError::CommitTooLarge);
    }
    let encoded = Bytes::from(serde_json::to_vec(&head)?);
    let mut ops = Vec::with_capacity(prepared.len());
    let mut results = Vec::with_capacity(prepared.len());
    for write in prepared {
        let cid = write.next.map(|(cid, _)| cid);
        match cid {
            Some(cid) => txn.put(write.index, Bytes::from(cid.to_bytes())),
            None => txn.delete(write.index),
        }
        results.push((format!("at://{}/{}", world.as_str(), write.path), cid));
        ops.push(RepoOp {
            action: write.action,
            path: write.path,
            cid: cid.map(|c| c.to_string()),
            prev: write.prev.map(|c| c.to_string()),
        });
    }
    txn.put_blocks(world, new);
    txn.put_blocks(world, [(head.cid, commit_block)]);
    store_head(txn, world, &head, encoded);
    txn.emit(Event::Commit(event::Commit {
        repo: world.clone(),
        rev: head.rev.to_string(),
        since: Some(prior.rev.to_string()),
        commit: head.cid.to_string(),
        blocks: Bytes::from(car),
        ops,
        prev_data: Some(prior.data.to_string()),
        time: Datetime::now().to_string(),
    }));
    Ok(Committed {
        commit: head.cid,
        rev: head.rev,
        results,
    })
}
