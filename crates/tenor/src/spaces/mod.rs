//! A World's Spaces: authority records and its own repositories.
//!
//! Membership and access records are the permission truth. Credentials,
//! writer tracking and delivery queues are derived state in the same shard
//! transaction, so a role removal takes effect before its acknowledgement.

pub(crate) mod access;
pub mod credentials;
pub mod http;
pub mod notify;
pub mod storage;

use std::collections::BTreeSet;

use atproto_record::{datetime::Datetime, tid::Tid};
use atproto_space::commit::{Commit, CommitContext, create_commit};
use atproto_space::space_repo::{Op as RecordOp, OpAction, SpaceRepo};
use atproto_space::storage::{RecordRow, SpaceRepoStorage};
use atproto_space::{LtHash, SpaceUri};
use bytes::Bytes;
use motet_lexicons::Record;
pub use motet_lexicons::group_opensocial_access::Access as AccessRecord;
pub use motet_lexicons::group_opensocial_defs::RoleId;
use motet_lexicons::group_opensocial_defs::{GroupGrant, SpaceGrant};
use motet_lexicons::group_opensocial_membership::Membership;
use motet_lexicons::group_opensocial_permissions::Permissions;
use motet_lexicons::group_opensocial_profile::Profile;
use motet_lexicons::group_opensocial_role::Role;
use motet_lexicons::group_opensocial_space::Space as SpaceIndex;
use motet_rules::Did;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::repo;
use crate::shard::{Txn, TxnError};
use crate::store::{ReadError, Reader, keys};

pub use access::{access, access_txn, roles};

pub const META: &str = "group.opensocial.meta";
pub const MEMBERS: &str = "group.opensocial.members";
pub const GMS: &str = "at.motet.pilot.space.gms";
pub const OWNER_ROLE: &str = "owner";
pub const GM_ROLE: &str = "gm";

/// An authority refusal is distinct from a failed storage operation.
#[derive(Debug, thiserror::Error)]
pub enum SpaceError {
    #[error(transparent)]
    Protocol(#[from] atproto_space::SpaceError),
    #[error(transparent)]
    Txn(#[from] TxnError),
    #[error(transparent)]
    Read(#[from] ReadError),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Repo(#[from] repo::RepoError),
    #[error("Space not found")]
    NotFound,
    #[error("Space has been deleted")]
    Deleted,
    #[error("not authorized for this Space")]
    NotAuthorized,
    #[error("the expected record or commit changed")]
    Conflict,
    #[error("notification service cannot be resolved: {0}")]
    ServiceNotResolvable(String),
    #[error("notification revision is too far in the future")]
    FutureRev,
    #[error("invalid Space operation: {0}")]
    Invalid(String),
}

/// Creation inputs which do not require uploading blobs first.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileInput {
    pub display_name: Option<String>,
    pub description: Option<String>,
    /// Private by default; only a public meta Space has a public declaration.
    #[serde(default)]
    pub public: bool,
    pub join_policy: Option<String>,
}

/// Host-private inventory includes secret Spaces and deletion generations.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SpaceInfo {
    pub space: SpaceUri,
    pub generation: String,
    pub deleted: bool,
    pub indexed: bool,
    /// `None` means the alpha's open application policy.
    pub allowed_clients: Option<Vec<String>>,
}

/// One conditional record mutation, performed as the World.
#[derive(Clone, Debug)]
pub struct SpaceWrite {
    pub action: OpAction,
    pub collection: String,
    pub rkey: String,
    pub value: Option<Vec<u8>>,
    /// An expected CID. Creates always require absence, even without a swap.
    pub swap: Option<String>,
}

impl SpaceWrite {
    /// Encode a typed record with its collection discriminator.
    pub fn create<T: Serialize>(collection: &str, rkey: &str, value: &T) -> Result<Self, SpaceError> {
        let mut value = serde_json::to_value(value)?;
        let object = value
            .as_object_mut()
            .ok_or_else(|| SpaceError::Invalid("record must be an object".into()))?;
        object.insert("$type".into(), collection.into());
        Ok(Self {
            action: OpAction::Create,
            collection: collection.into(),
            rkey: rkey.into(),
            value: Some(repo::record_from_json(&value)?.to_vec()),
            swap: None,
        })
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct WriteResult {
    pub uri: String,
    pub cid: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct SpaceCommitted {
    pub commit: Commit,
    pub results: Vec<WriteResult>,
}

pub(crate) fn encode<T: Serialize>(value: &T) -> Result<Bytes, SpaceError> {
    Ok(Bytes::from(serde_json::to_vec(value)?))
}

pub(crate) fn decode<T: DeserializeOwned>(value: &[u8]) -> Result<T, SpaceError> {
    Ok(serde_json::from_slice(value)?)
}

pub(crate) fn record_value<T: DeserializeOwned>(record: &RecordRow) -> Result<T, SpaceError> {
    atproto_dasl::from_slice(&record.value).map_err(|error| SpaceError::Invalid(error.to_string()))
}

/// A well-known Space of this World, at the proposal's literal `self` key.
pub fn world_space(world: &Did, kind: &str) -> Result<SpaceUri, SpaceError> {
    format!("at://{world}/space/{kind}/self")
        .parse()
        .map_err(SpaceError::from)
}

pub async fn info(reader: &Reader, space: &SpaceUri) -> Result<SpaceInfo, SpaceError> {
    let value = reader
        .get(&keys::space_info(space))
        .await?
        .ok_or(SpaceError::NotFound)?;
    decode(&value)
}

pub async fn info_txn(txn: &Txn<'_>, space: &SpaceUri) -> Result<SpaceInfo, SpaceError> {
    let value = txn.get(&keys::space_info(space)).await?.ok_or(SpaceError::NotFound)?;
    decode(&value)
}

pub async fn list_spaces(reader: &Reader, world: &Did) -> Result<Vec<SpaceInfo>, SpaceError> {
    reader
        .scan(&keys::space_inventory(world), None, usize::MAX)
        .await?
        .into_iter()
        .map(|(_, value)| decode(&value))
        .collect()
}

pub async fn record(
    reader: &Reader,
    space: &SpaceUri,
    collection: &str,
    rkey: &str,
) -> Result<Option<RecordRow>, SpaceError> {
    Ok(storage::ReadStorage::new(reader, &Did::new(&space.space_did))
        .get_record(space, collection, rkey)
        .await?)
}

pub async fn record_txn(
    txn: &Txn<'_>,
    space: &SpaceUri,
    collection: &str,
    rkey: &str,
) -> Result<Option<RecordRow>, SpaceError> {
    Ok(storage::get_record_txn(txn, &Did::new(&space.space_did), space, collection, rkey).await?)
}

pub(crate) fn require_live(info: &SpaceInfo) -> Result<(), SpaceError> {
    if info.deleted { Err(SpaceError::Deleted) } else { Ok(()) }
}

/// The World writes only validated records. State, oplog, credential
/// revocations and notification intents commit together with the caller's op.
pub async fn write_as_world(
    txn: &mut Txn<'_>,
    space: &SpaceUri,
    writes: Vec<SpaceWrite>,
) -> Result<SpaceCommitted, SpaceError> {
    require_live(&info_txn(txn, space).await?)?;
    validate_authority_records(txn, space, &writes).await?;
    let affects_access = writes.iter().any(|write| {
        matches!(
            write.collection.as_str(),
            "group.opensocial.access"
                | "group.opensocial.membership"
                | "group.opensocial.role"
                | "group.opensocial.permissions"
        )
    });
    let committed = commit_records(txn, space, writes).await?;
    if affects_access {
        credentials::reconcile(txn, &Did::new(&space.space_did)).await?;
        if space.space_type.as_str() == META {
            sync_declaration(txn, space).await?;
        }
    }
    notify::record_write(txn, space, &space.space_did, &committed.commit.rev).await?;
    Ok(committed)
}

async fn sync_declaration(txn: &mut Txn<'_>, meta: &SpaceUri) -> Result<(), SpaceError> {
    let world = Did::new(&meta.space_did);
    let access: AccessRecord = record_value(
        &record_txn(txn, meta, AccessRecord::NSID, "self")
            .await?
            .ok_or(SpaceError::NotFound)?,
    )?;
    let existing = txn
        .get(&keys::repo_record(&world, "group.opensocial.declaration", "self"))
        .await?;
    let write = match (access.public.unwrap_or(false), existing) {
        (true, None) => {
            let declaration = serde_json::json!({"$type":"group.opensocial.declaration", "meta":meta.to_string()});
            repo::Write::Create {
                collection: repo::Nsid::new("group.opensocial.declaration")?,
                rkey: "self".into(),
                record: repo::record_from_json(&declaration)?,
            }
        }
        (false, Some(_)) => repo::Write::Delete {
            collection: repo::Nsid::new("group.opensocial.declaration")?,
            rkey: "self".into(),
            swap: None,
        },
        _ => return Ok(()),
    };
    repo::commit(txn, &world, vec![write], None).await?;
    Ok(())
}

async fn commit_records(
    txn: &mut Txn<'_>,
    space: &SpaceUri,
    writes: Vec<SpaceWrite>,
) -> Result<SpaceCommitted, SpaceError> {
    let mut ops = Vec::with_capacity(writes.len());
    let mut results = Vec::with_capacity(writes.len());
    for write in writes {
        if let Some(expected) = &write.swap {
            let current = record_txn(txn, space, &write.collection, &write.rkey).await?;
            if current.as_ref().map(|row| &row.cid) != Some(expected) {
                return Err(SpaceError::Conflict);
            }
        }
        let cid = if let Some(value) = &write.value {
            if value.len() > atproto_space::MAX_RECORD_SIZE {
                return Err(SpaceError::Invalid("record exceeds the Space record size limit".into()));
            }
            repo::validate_record(&write.collection, &write.rkey, value)?;
            reject_blobs(
                &atproto_dasl::from_slice_to_json(value).map_err(|error| SpaceError::Invalid(error.to_string()))?,
            )?;
            Some(atproto_dasl::cid::compute_cid(value).to_string())
        } else {
            if write.action != OpAction::Delete {
                return Err(SpaceError::Invalid("create/update needs a record".into()));
            }
            None
        };
        results.push(WriteResult {
            uri: format!("{space}/{}/{}/{}", space.space_did, write.collection, write.rkey),
            cid: cid.clone(),
        });
        ops.push(RecordOp {
            action: write.action,
            collection: write.collection,
            rkey: write.rkey,
            cid,
            value: write.value,
        });
    }
    let owner = Did::new(&space.space_did);
    let repo = SpaceRepo::<_, LtHash>::new(space.clone(), storage::TxnStorage::new(txn, &owner));
    let prepared = repo.format_commit(&ops).await?;
    let commit = create_commit(
        &prepared.set_hash,
        &CommitContext {
            repo: space.space_did.clone(),
            space: space.to_string(),
            rev: prepared.rev.clone(),
        },
    );
    repo.apply_commit(prepared).await?;
    Ok(SpaceCommitted { commit, results })
}

fn reject_blobs(value: &serde_json::Value) -> Result<(), SpaceError> {
    match value {
        serde_json::Value::Object(object) => {
            // No blob upload boundary exists yet. Accepting a reference would
            // promise readable bytes this host cannot supply.
            if object.get("$type").and_then(serde_json::Value::as_str) == Some("blob") {
                return Err(SpaceError::Invalid(
                    "blob must be uploaded before it can be referenced".into(),
                ));
            }
            for value in object.values() {
                reject_blobs(value)?;
            }
        }
        serde_json::Value::Array(values) => {
            for value in values {
                reject_blobs(value)?;
            }
        }
        _ => {}
    }
    Ok(())
}

async fn validate_authority_records(txn: &Txn<'_>, space: &SpaceUri, writes: &[SpaceWrite]) -> Result<(), SpaceError> {
    let world = Did::new(&space.space_did);
    let members = world_space(&world, MEMBERS)?;
    let mut added_roles = BTreeSet::new();
    for write in writes {
        if write.collection == Role::NSID && write.action != OpAction::Delete {
            added_roles.insert(write.rkey.as_str());
        }
    }
    for write in writes {
        if matches!(
            write.collection.as_str(),
            "group.opensocial.role"
                | "group.opensocial.membership"
                | "group.opensocial.permissions"
                | "group.opensocial.space"
        ) && *space != members
        {
            return Err(SpaceError::Invalid(
                "group membership records belong in the members Space".into(),
            ));
        }
        if write.collection == AccessRecord::NSID && (write.rkey != "self" || write.action == OpAction::Delete) {
            return Err(SpaceError::Invalid("each Space must keep access/self".into()));
        }
        let Some(value) = &write.value else { continue };
        if write.collection == AccessRecord::NSID {
            let access: AccessRecord =
                atproto_dasl::from_slice(value).map_err(|e| SpaceError::Invalid(e.to_string()))?;
            let mut seen = BTreeSet::new();
            for grant in &access.grants {
                if !seen.insert(grant.role.as_str()) {
                    return Err(SpaceError::Invalid("duplicate role grant".into()));
                }
                check_role(txn, &members, &added_roles, &grant.role).await?;
                access::validate_scopes(&grant.scopes)?;
            }
        } else if write.collection == Membership::NSID {
            let membership: Membership =
                atproto_dasl::from_slice(value).map_err(|e| SpaceError::Invalid(e.to_string()))?;
            if membership.member != write.rkey {
                return Err(SpaceError::Invalid("membership key must be its member DID".into()));
            }
            let mut seen = BTreeSet::new();
            for role in &membership.roles {
                if !seen.insert(role.as_str()) {
                    return Err(SpaceError::Invalid("duplicate membership role".into()));
                }
                check_role(txn, &members, &added_roles, role).await?;
            }
        } else if write.collection == Permissions::NSID {
            let permissions: Permissions =
                atproto_dasl::from_slice(value).map_err(|e| SpaceError::Invalid(e.to_string()))?;
            let mut seen = BTreeSet::new();
            for grant in &permissions.grants {
                if !seen.insert(grant.role.as_str()) {
                    return Err(SpaceError::Invalid("duplicate role grant".into()));
                }
                check_role(txn, &members, &added_roles, &grant.role).await?;
                for role in &grant.assignable {
                    check_role(txn, &members, &added_roles, role).await?;
                }
                access::validate_scopes(&grant.scopes)?;
            }
        }
    }
    Ok(())
}

async fn check_role(txn: &Txn<'_>, members: &SpaceUri, added: &BTreeSet<&str>, role: &str) -> Result<(), SpaceError> {
    if added.contains(role) || record_txn(txn, members, Role::NSID, role).await?.is_some() {
        Ok(())
    } else {
        Err(SpaceError::Invalid(format!("unknown role: {role}")))
    }
}

async fn insert_info(txn: &mut Txn<'_>, space: &SpaceUri, indexed: bool) -> Result<SpaceInfo, SpaceError> {
    let prior = txn
        .get(&keys::space_info(space))
        .await?
        .map(|value| decode::<SpaceInfo>(&value))
        .transpose()?;
    if prior.as_ref().is_some_and(|info| !info.deleted) {
        return Err(SpaceError::Conflict);
    }
    let generation = prior
        .as_ref()
        .and_then(|info| Tid::decode(&info.generation).ok())
        .map_or_else(Tid::new, |prior| Tid::next_after(&prior))
        .to_string();
    let info = SpaceInfo {
        space: space.clone(),
        generation,
        deleted: false,
        indexed,
        allowed_clients: None,
    };
    txn.put(keys::space_info(space), encode(&info)?);
    Ok(info)
}

fn index_write(info: &SpaceInfo) -> Result<SpaceWrite, SpaceError> {
    SpaceWrite::create(
        SpaceIndex::NSID,
        &info.generation,
        &SpaceIndex {
            space: info.space.to_string(),
            created_at: Datetime::now().into_string(),
            extra: serde_json::Map::new(),
        },
    )
}

/// Create a modality Space and its discovery entry atomically.
/// The proposal reserves meta and members for the creation workflow.
pub async fn create(
    txn: &mut Txn<'_>,
    space: &SpaceUri,
    access: AccessRecord,
    indexed: bool,
) -> Result<SpaceCommitted, SpaceError> {
    if matches!(space.space_type.as_str(), META | MEMBERS) {
        return Err(SpaceError::Invalid("reserved Space".into()));
    }
    let info = insert_info(txn, space, indexed).await?;
    let written = write_as_world(
        txn,
        space,
        vec![SpaceWrite::create(AccessRecord::NSID, "self", &access)?],
    )
    .await?;
    if indexed {
        let members = world_space(&Did::new(&space.space_did), MEMBERS)?;
        write_as_world(txn, &members, vec![index_write(&info)?]).await?;
    }
    Ok(written)
}

/// Delete a modality Space, retaining a tombstone and durable notifications.
/// Recreating it gets a different generation, invalidating old credentials.
pub async fn delete(txn: &mut Txn<'_>, space: &SpaceUri) -> Result<(), SpaceError> {
    if matches!(space.space_type.as_str(), META | MEMBERS) {
        return Err(SpaceError::Invalid("reserved Space".into()));
    }
    let mut info = info_txn(txn, space).await?;
    require_live(&info)?;
    info.deleted = true;
    txn.put(keys::space_info(space), encode(&info)?);
    notify::queue_deletion(txn, space, &info.generation).await?;
    credentials::reconcile(txn, &Did::new(&space.space_did)).await?;
    // Keep the full history, including deletion ops. A recreated URI then
    // resumes after the old revision without silently dropping an oplog tail.
    let mut deletes = Vec::new();
    for (_, value) in txn
        .scan(
            &keys::space_records(&Did::new(&space.space_did), space, None),
            usize::MAX,
        )
        .await?
    {
        let row: RecordRow = decode(&value)?;
        deletes.push(SpaceWrite {
            action: OpAction::Delete,
            collection: row.collection,
            rkey: row.rkey,
            value: None,
            swap: Some(row.cid),
        });
    }
    if !deletes.is_empty() {
        commit_records(txn, space, deletes).await?;
    }
    if info.indexed {
        let members = world_space(&Did::new(&space.space_did), MEMBERS)?;
        write_as_world(
            txn,
            &members,
            vec![SpaceWrite {
                action: OpAction::Delete,
                collection: SpaceIndex::NSID.into(),
                rkey: info.generation,
                value: None,
                swap: None,
            }],
        )
        .await?;
    }
    Ok(())
}

fn owner_access(public: bool) -> AccessRecord {
    AccessRecord {
        public: Some(public),
        grants: vec![SpaceGrant {
            role: OWNER_ROLE.into(),
            scopes: vec!["space:*?collection=*".into()],
            extra: serde_json::Map::new(),
        }],
        extra: serde_json::Map::new(),
    }
}

/// Create the opensocial creation set and Motet's GM Space inside the World
/// creation operation. Call after keys and the public repo genesis exist.
pub async fn bootstrap(txn: &mut Txn<'_>, world: &Did, owner: &Did, profile: ProfileInput) -> Result<(), SpaceError> {
    let meta = world_space(world, META)?;
    let members = world_space(world, MEMBERS)?;
    let gms = world_space(world, GMS)?;
    let meta_info = insert_info(txn, &meta, true).await?;
    let members_info = insert_info(txn, &members, true).await?;
    let gms_info = insert_info(txn, &gms, true).await?;
    let role = Role {
        default: Some(false),
        description: None,
        display_name: "Owner".into(),
        extra: serde_json::Map::new(),
    };
    let gm_role = Role {
        default: Some(false),
        description: None,
        display_name: "GM".into(),
        extra: serde_json::Map::new(),
    };
    let permissions = Permissions {
        grants: vec![
            GroupGrant {
                role: OWNER_ROLE.into(),
                assignable: vec![OWNER_ROLE.into(), GM_ROLE.into()],
                scopes: vec![
                    "space:*?authority=*&collection=*&manage=create&manage=update&manage=delete".into(),
                    "repo:*".into(),
                    "rpc:*".into(),
                    "blob:*/*".into(),
                    "identity:*".into(),
                    "account:email?action=manage".into(),
                    "account:repo?action=manage".into(),
                    "account:status?action=manage".into(),
                ],
                extra: serde_json::Map::new(),
            },
            GroupGrant {
                role: GM_ROLE.into(),
                assignable: Vec::new(),
                scopes: vec![
                    "rpc:at.motet.pilot.play.openScene".into(),
                    "rpc:at.motet.pilot.play.setSceneCast".into(),
                    "rpc:at.motet.pilot.play.closeScene".into(),
                ],
                extra: serde_json::Map::new(),
            },
        ],
        extra: serde_json::Map::new(),
    };
    let membership = Membership {
        created_at: Datetime::now().into_string(),
        member: owner.to_string(),
        roles: vec![OWNER_ROLE.into()],
        extra: serde_json::Map::new(),
    };
    write_as_world(
        txn,
        &members,
        vec![
            SpaceWrite::create(Role::NSID, OWNER_ROLE, &role)?,
            SpaceWrite::create(Role::NSID, GM_ROLE, &gm_role)?,
            SpaceWrite::create(Permissions::NSID, "self", &permissions)?,
            SpaceWrite::create(Membership::NSID, owner.as_str(), &membership)?,
            SpaceWrite::create(AccessRecord::NSID, "self", &owner_access(false))?,
            index_write(&meta_info)?,
            index_write(&members_info)?,
            index_write(&gms_info)?,
        ],
    )
    .await?;
    let public = profile.public;
    let profile = Profile {
        avatar: None,
        banner: None,
        description: profile.description,
        display_name: profile.display_name.unwrap_or_else(|| world.to_string()),
        join_policy: profile.join_policy.unwrap_or_else(|| "invite".into()),
        url: None,
        extra: serde_json::Map::new(),
    };
    write_as_world(
        txn,
        &meta,
        vec![
            SpaceWrite::create(AccessRecord::NSID, "self", &owner_access(public))?,
            SpaceWrite::create(Profile::NSID, "self", &profile)?,
        ],
    )
    .await?;
    write_as_world(
        txn,
        &gms,
        vec![SpaceWrite::create(AccessRecord::NSID, "self", &owner_access(false))?],
    )
    .await?;
    Ok(())
}
