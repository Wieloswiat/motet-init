//! Resolve opensocial grants from the records the shard currently sees.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::LazyLock;

use atproto_oauth::scopes::{
    RpcAudience, RpcLexicon, RpcScope, Scope, SpaceAction, SpaceCollection, SpaceCollections, SpacePermission,
    SpaceTarget, TransitionScope,
};
use atproto_space::SpaceUri;
use atproto_space::storage::MemberAccess;
use motet_lexicons::Record;
use motet_lexicons::group_opensocial_membership::Membership;
use motet_lexicons::group_opensocial_permissions::Permissions;
use motet_lexicons::group_opensocial_role::Role;
use motet_rules::Did;

use super::{
    AccessRecord, MEMBERS, RoleId, SpaceError, info, info_txn, record, record_txn, record_value, require_live,
    world_space,
};
use crate::shard::Txn;
use crate::store::Reader;

/// Existing roles of this member. A deleted role confers no permission even
/// while an older membership still names it. Acceptance affects roster only.
pub async fn roles(reader: &Reader, world: &Did, member: &Did) -> Result<Vec<RoleId>, SpaceError> {
    let members = world_space(world, MEMBERS)?;
    let Some(row) = record(reader, &members, Membership::NSID, member.as_str()).await? else {
        return Ok(Vec::new());
    };
    let membership: Membership = record_value(&row)?;
    if membership.member != member.as_str() {
        return Err(SpaceError::Invalid("membership key and member disagree".into()));
    }
    let mut roles = Vec::with_capacity(membership.roles.len());
    for role in membership.roles {
        if record(reader, &members, Role::NSID, &role).await?.is_some() {
            roles.push(role);
        }
    }
    Ok(roles)
}

pub(crate) async fn roles_txn(txn: &Txn<'_>, world: &Did, member: &str) -> Result<Vec<RoleId>, SpaceError> {
    let members = world_space(world, MEMBERS)?;
    let Some(row) = record_txn(txn, &members, Membership::NSID, member).await? else {
        return Ok(Vec::new());
    };
    let membership: Membership = record_value(&row)?;
    if membership.member != member {
        return Err(SpaceError::Invalid("membership key and member disagree".into()));
    }
    let mut roles = Vec::with_capacity(membership.roles.len());
    for role in membership.roles {
        if record_txn(txn, &members, Role::NSID, &role).await?.is_some() {
            roles.push(role);
        }
    }
    Ok(roles)
}

pub async fn access(reader: &Reader, space: &SpaceUri, member: Option<&str>) -> Result<MemberAccess, SpaceError> {
    require_live(&info(reader, space).await?)?;
    if member == Some(space.space_did.as_str()) {
        return Ok(MemberAccess::FULL);
    }
    let access: AccessRecord = record_value(
        &record(reader, space, AccessRecord::NSID, "self")
            .await?
            .ok_or(SpaceError::NotFound)?,
    )?;
    let world = Did::new(&space.space_did);
    let roles = match member {
        Some(member) => roles(reader, &world, &Did::new(member)).await?,
        None => Vec::new(),
    };
    let members = world_space(&world, MEMBERS)?;
    let permissions = record(reader, &members, Permissions::NSID, "self")
        .await?
        .as_ref()
        .map(record_value)
        .transpose()?;
    evaluate(&access, permissions.as_ref(), &roles, space)
}

/// Commit-time equivalent of [`access`]. A prepared delegation never bypasses
/// a membership removal which reached the shard first.
pub async fn access_txn(txn: &Txn<'_>, space: &SpaceUri, member: Option<&str>) -> Result<MemberAccess, SpaceError> {
    require_live(&info_txn(txn, space).await?)?;
    if member == Some(space.space_did.as_str()) {
        return Ok(MemberAccess::FULL);
    }
    let access: AccessRecord = record_value(
        &record_txn(txn, space, AccessRecord::NSID, "self")
            .await?
            .ok_or(SpaceError::NotFound)?,
    )?;
    let world = Did::new(&space.space_did);
    let roles = match member {
        Some(member) => roles_txn(txn, &world, member).await?,
        None => Vec::new(),
    };
    let members = world_space(&world, MEMBERS)?;
    let permissions = record_txn(txn, &members, Permissions::NSID, "self")
        .await?
        .as_ref()
        .map(record_value)
        .transpose()?;
    evaluate(&access, permissions.as_ref(), &roles, space)
}

fn evaluate(
    access: &AccessRecord,
    permissions: Option<&Permissions>,
    roles: &[RoleId],
    space: &SpaceUri,
) -> Result<MemberAccess, SpaceError> {
    let mut result = MemberAccess {
        read: access.public.unwrap_or(false),
        write: false,
    };
    for grant in &access.grants {
        if roles.contains(&grant.role) {
            add_scopes(&mut result, &grant.scopes, space)?;
        }
    }
    if let Some(permissions) = permissions {
        for grant in &permissions.grants {
            if roles.contains(&grant.role) {
                add_scopes(&mut result, &grant.scopes, space)?;
            }
        }
    }
    Ok(result)
}

fn add_scopes(result: &mut MemberAccess, scopes: &[String], space: &SpaceUri) -> Result<(), SpaceError> {
    let mut target = SpaceTarget::new(
        space.space_type.as_str(),
        &space.space_did,
        space.space_key.as_str(),
        SpaceAction::Read,
    );
    let declared = declared_collections(space.space_type.as_str())?;
    for scope in scopes {
        with_space_permissions(scope, |permission| {
            target.action = SpaceAction::Read;
            target.collection = None;
            result.read |= permission.matches_with(&target, declared, Some(&space.space_did));
            for action in [SpaceAction::Create, SpaceAction::Update, SpaceAction::Delete] {
                if !permission.action.contains(&action) {
                    continue;
                }
                target.action = action;
                match &permission.collection {
                    SpaceCollections::Explicit(collections) => {
                        for collection in collections {
                            target.collection = match collection {
                                SpaceCollection::All => None,
                                SpaceCollection::Nsid(nsid) => Some(nsid.clone()),
                            };
                            result.write |= permission.matches_with(&target, declared, Some(&space.space_did));
                        }
                    }
                    SpaceCollections::Default => {
                        for collection in declared {
                            target.collection = Some(collection.clone());
                            result.write |= permission.matches_with(&target, declared, Some(&space.space_did));
                        }
                    }
                }
            }
        })?;
    }
    Ok(())
}

pub(super) fn validate_scopes(scopes: &[String]) -> Result<(), SpaceError> {
    for scope in scopes {
        with_space_permissions(scope, |_| {})?;
    }
    Ok(())
}

#[derive(Default)]
struct Catalog {
    collections: BTreeMap<String, Vec<String>>,
    includes: BTreeMap<String, Vec<Scope>>,
    legacy: Vec<Scope>,
}

fn catalog() -> Result<&'static Catalog, SpaceError> {
    static CATALOG: LazyLock<Result<Catalog, String>> = LazyLock::new(build_catalog);
    CATALOG.as_ref().map_err(|error| SpaceError::Invalid(error.clone()))
}

pub(crate) fn declared_collections(kind: &str) -> Result<&'static [String], SpaceError> {
    Ok(catalog()?.collections.get(kind).map_or(&[], Vec::as_slice))
}

fn build_catalog() -> Result<Catalog, String> {
    let mut catalog = Catalog::default();
    let mut legacy_rpc = BTreeSet::new();
    for &(nsid, source) in motet_lexicons::SCHEMAS {
        let schema: serde_json::Value = serde_json::from_str(source).map_err(|error| error.to_string())?;
        let Some(main) = schema.get("defs").and_then(|defs| defs.get("main")) else {
            continue;
        };
        match main.get("type").and_then(serde_json::Value::as_str) {
            Some("space") => {
                let values = main.get("collections").and_then(serde_json::Value::as_array);
                let collections = values
                    .into_iter()
                    .flatten()
                    .filter_map(serde_json::Value::as_str)
                    .map(str::to_owned)
                    .collect();
                catalog.collections.insert(nsid.into(), collections);
            }
            Some("permission-set") => {
                let mut scopes = Vec::new();
                for permission in main
                    .get("permissions")
                    .and_then(serde_json::Value::as_array)
                    .into_iter()
                    .flatten()
                {
                    let resource = permission
                        .get("resource")
                        .and_then(serde_json::Value::as_str)
                        .ok_or("permission needs resource")?;
                    let prefix = match resource {
                        "space" => format!(
                            "space:{}",
                            permission
                                .get("spaceType")
                                .and_then(serde_json::Value::as_str)
                                .ok_or("Space permission needs spaceType")?
                        ),
                        "repo" | "rpc" => resource.to_owned(),
                        _ => return Err(format!("unsupported permission resource: {resource}")),
                    };
                    let mut query = url::form_urlencoded::Serializer::new(String::new());
                    for name in ["authority", "skey", "collection", "action", "manage", "aud", "lxm"] {
                        match permission.get(name) {
                            Some(serde_json::Value::String(value)) => {
                                query.append_pair(name, value);
                            }
                            Some(serde_json::Value::Array(values)) => {
                                for value in values {
                                    query.append_pair(
                                        name,
                                        value.as_str().ok_or("permission parameter must be a string")?,
                                    );
                                }
                            }
                            None => {}
                            Some(_) => return Err("invalid permission parameter".into()),
                        }
                    }
                    scopes.push(
                        Scope::parse(&format!("{prefix}?{}", query.finish())).map_err(|error| error.to_string())?,
                    );
                }
                catalog.includes.insert(nsid.into(), scopes);
            }
            Some("query" | "procedure") if legacy_rpc_method(nsid) => {
                legacy_rpc.insert(RpcLexicon::Nsid(nsid.into()));
            }
            _ => {}
        }
    }
    for scope in ["repo:*", "blob:*/*"] {
        catalog
            .legacy
            .push(Scope::parse(scope).map_err(|error| error.to_string())?);
    }
    if !legacy_rpc.is_empty() {
        catalog.legacy.push(Scope::Rpc(RpcScope {
            lxm: legacy_rpc,
            aud: BTreeSet::from([RpcAudience::All]),
        }));
    }
    Ok(catalog)
}

// A wildcard RPC scope would include chat and any future sensitive method.
// Freeze the legacy grant to the compiled non-Space, non-identity methods.
fn legacy_rpc_method(nsid: &str) -> bool {
    !nsid.starts_with("chat.bsky.")
        && !nsid.starts_with("com.atproto.space.")
        && !nsid.starts_with("com.atproto.simplespace.")
        && !nsid.starts_with("com.atproto.identity.")
        && !matches!(
            nsid,
            "group.opensocial.createSpace"
                | "group.opensocial.deleteSpace"
                | "at.motet.pilot.tenor.reserveWorld"
                | "at.motet.pilot.tenor.createWorld"
        )
}

/// Expand only compiled permission sets; authorization never fetches lexicons.
pub(crate) fn expand(scope: &str) -> Result<Vec<Scope>, SpaceError> {
    match Scope::parse(scope).map_err(|error| SpaceError::Invalid(error.to_string()))? {
        Scope::Include(include) => {
            let mut permissions = catalog()?
                .includes
                .get(&include.nsid)
                .ok_or_else(|| SpaceError::Invalid(format!("unknown permission set: {}", include.nsid)))?
                .clone();
            if let Some(aud) = include.aud {
                for permission in &mut permissions {
                    if let Scope::Rpc(rpc) = permission {
                        // An explicit audience in the declaration takes precedence.
                        if rpc.aud.is_empty() {
                            rpc.aud.insert(atproto_oauth::scopes::RpcAudience::Did(aud.clone()));
                        }
                    }
                }
            }
            Ok(permissions)
        }
        Scope::Transition(TransitionScope::Generic) => Ok(catalog()?.legacy.clone()),
        scope => Ok(vec![scope]),
    }
}

fn with_space_permissions(scope: &str, mut visit: impl FnMut(&SpacePermission)) -> Result<(), SpaceError> {
    for permission in expand(scope)? {
        if let Scope::Space(permission) = permission {
            visit(&permission);
        }
    }
    Ok(())
}
