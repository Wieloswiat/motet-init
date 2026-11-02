//! Current member authorization and audit for World operations.
//! An access record is confined to its own Space; permissions grants are global.

use std::collections::BTreeSet;

use atproto_oauth::scopes::{
    RepoAction, RepoCollection, RpcAudience, RpcLexicon, Scope, SpaceAction, SpaceCollection, SpaceCollections,
    SpaceDid, SpaceManageTarget, SpaceManageVerb, SpacePermission, SpaceSkey, SpaceTarget, SpaceType,
};
use atproto_record::{datetime::Datetime, tid::Tid};
use atproto_space::{SpaceUri, space_repo::OpAction};
use motet_lexicons::Record;
use motet_lexicons::at_motet_pilot_audit::Audit;
use motet_lexicons::group_opensocial_defs::GroupGrant;
use motet_lexicons::group_opensocial_membership::Membership;
use motet_lexicons::group_opensocial_permissions::Permissions;
use motet_lexicons::group_opensocial_role::Role;
use motet_rules::Did;
use motet_xrpc::Caller;

use crate::shard::Txn;
use crate::spaces::{self, AccessRecord, SpaceError, SpaceInfo, SpaceWrite};
use crate::store::keys;

pub(crate) const AUDIT: &str = Audit::NSID;

async fn current(txn: &Txn<'_>, world: &Did, actor: &Did) -> Result<Policy, SpaceError> {
    let raw = txn
        .get(&keys::world_info(world))
        .await?
        .ok_or(SpaceError::NotAuthorized)?;
    let info: crate::world::WorldInfo = serde_json::from_slice(&raw)?;
    if info.did != *world || info.status != crate::world::Status::Active {
        return Err(SpaceError::NotAuthorized);
    }
    let roles = spaces::access::roles_txn(txn, world, actor.as_str()).await?;
    let mut policy = Policy::member(world, &roles)?;
    let members = spaces::world_space(world, spaces::MEMBERS)?;
    if let Some(row) = spaces::record_txn(txn, &members, Permissions::NSID, "self").await? {
        policy.permissions(&spaces::record_value::<Permissions>(&row)?, &roles)?;
    }
    for (_, value) in txn.scan(&keys::space_inventory(world), usize::MAX).await? {
        let info: SpaceInfo = spaces::decode(&value)?;
        if !info.deleted
            && let Some(row) = spaces::record_txn(txn, &info.space, AccessRecord::NSID, "self").await?
        {
            policy.access(&spaces::record_value::<AccessRecord>(&row)?, &roles, &info.space)?;
        }
    }
    Ok(policy)
}

/// The World is the request target; the actor is the verified token issuer.
#[derive(Clone, Debug)]
pub(crate) struct Actor {
    pub world: Did,
    pub actor: Did,
}

impl Actor {
    fn audience_world(caller: &Caller) -> Result<&str, SpaceError> {
        let Caller::Service { did, audience, .. } = caller else {
            return Err(SpaceError::NotAuthorized);
        };
        atproto_lexicon::validation::syntax::validate_did(did).map_err(|_| SpaceError::NotAuthorized)?;
        let world = match audience.split_once('#') {
            None => audience.as_str(),
            Some((world, crate::api::SERVICE_FRAGMENT)) => world,
            Some(_) => return Err(SpaceError::NotAuthorized),
        };
        atproto_lexicon::validation::syntax::validate_did(world).map_err(|_| SpaceError::NotAuthorized)?;
        Ok(world)
    }

    pub(crate) fn world(caller: &Caller) -> Result<Did, SpaceError> {
        Ok(Did::new(Self::audience_world(caller)?))
    }

    pub(crate) fn from_caller(caller: Caller, world: Did) -> Result<Self, SpaceError> {
        if Self::audience_world(&caller)? != world.as_str() {
            return Err(SpaceError::NotAuthorized);
        }
        let Caller::Service { did, .. } = caller else {
            return Err(SpaceError::NotAuthorized);
        };
        Ok(Self {
            world,
            actor: Did::new(did),
        })
    }

    /// Load policy inside the mutation's operation, after earlier role removals.
    pub(crate) async fn policy(&self, txn: &Txn<'_>) -> Result<Policy, SpaceError> {
        current(txn, &self.world, &self.actor).await
    }

    /// A failed audit append aborts the enclosing mutation, including its events.
    pub(crate) async fn audit(&self, txn: &mut Txn<'_>, action: &str, subject: &str) -> Result<(), SpaceError> {
        let space = spaces::world_space(&self.world, spaces::GMS)?;
        let record = Audit {
            actor: self.actor.to_string(),
            action: action.into(),
            subject: subject.into(),
            at: Datetime::now().into_string(),
            extra: serde_json::Map::new(),
        };
        spaces::write_as_world(
            txn,
            &space,
            vec![SpaceWrite::create(AUDIT, &Tid::new().to_string(), &record)?],
        )
        .await?;
        Ok(())
    }
}

pub(crate) struct Policy {
    world: Did,
    scopes: Vec<Scope>,
    assignable: BTreeSet<String>,
}

impl Policy {
    fn member(world: &Did, roles: &[String]) -> Result<Self, SpaceError> {
        if roles.is_empty() {
            return Err(SpaceError::NotAuthorized);
        }
        Ok(Self {
            world: world.clone(),
            scopes: vec![Scope::Atproto],
            assignable: BTreeSet::new(),
        })
    }

    fn permissions(&mut self, permissions: &Permissions, roles: &[String]) -> Result<(), SpaceError> {
        for grant in &permissions.grants {
            if roles.contains(&grant.role) {
                self.scopes.extend(expand_all(&grant.scopes)?);
                self.assignable.extend(grant.assignable.iter().cloned());
            }
        }
        Ok(())
    }

    fn access(&mut self, access: &AccessRecord, roles: &[String], space: &SpaceUri) -> Result<(), SpaceError> {
        for grant in &access.grants {
            if roles.contains(&grant.role) {
                self.scopes.extend(confined(&grant.scopes, space, &self.world)?);
            }
        }
        Ok(())
    }

    fn covers(&self, requested: &[Scope]) -> Result<bool, SpaceError> {
        covers(&self.scopes, requested, &self.world)
    }

    fn require(&self, requested: Scope) -> Result<(), SpaceError> {
        if self.covers(&[requested])? {
            Ok(())
        } else {
            Err(SpaceError::NotAuthorized)
        }
    }

    pub(crate) fn rpc(&self, nsid: &str) -> Result<(), SpaceError> {
        let method = RpcLexicon::Nsid(nsid.into());
        let audience = RpcAudience::Did(format!("{}#{}", self.world, crate::api::SERVICE_FRAGMENT));
        self.require(Scope::Rpc(atproto_oauth::scopes::RpcScope {
            lxm: BTreeSet::from([method]),
            aud: BTreeSet::from([audience]),
        }))
    }

    pub(crate) fn repo(&self, collection: &str, action: RepoAction) -> Result<(), SpaceError> {
        if protected(collection) {
            return Err(SpaceError::NotAuthorized);
        }
        self.require(Scope::Repo(atproto_oauth::scopes::RepoScope {
            collections: BTreeSet::from([RepoCollection::Nsid(collection.into())]),
            actions: BTreeSet::from([action]),
        }))
    }

    pub(crate) fn space(
        &self,
        space: &SpaceUri,
        collection: Option<&str>,
        action: SpaceAction,
    ) -> Result<(), SpaceError> {
        if collection.is_some_and(protected)
            && matches!(action, SpaceAction::Create | SpaceAction::Update | SpaceAction::Delete)
        {
            return Err(SpaceError::NotAuthorized);
        }
        self.require(Scope::Space(SpacePermission {
            space_type: SpaceType::Nsid(space.space_type.to_string()),
            did: SpaceDid::Did(space.space_did.clone()),
            skey: SpaceSkey::Key(space.space_key.to_string()),
            collection: SpaceCollections::Explicit(
                collection
                    .into_iter()
                    .map(|name| SpaceCollection::Nsid(name.into()))
                    .collect(),
            ),
            action: BTreeSet::from([action]),
            manage: BTreeSet::new(),
        }))
    }

    pub(crate) fn manage(&self, space: &SpaceUri, verb: SpaceManageVerb) -> Result<(), SpaceError> {
        self.require(Scope::Space(SpacePermission {
            space_type: SpaceType::Nsid(space.space_type.to_string()),
            did: SpaceDid::Did(space.space_did.clone()),
            skey: SpaceSkey::Key(space.space_key.to_string()),
            collection: SpaceCollections::Explicit(BTreeSet::new()),
            action: BTreeSet::new(),
            manage: BTreeSet::from([verb]),
        }))
    }

    /// Scope checks alone cannot constrain which roles a membership may change.
    pub(crate) async fn records(
        &self,
        txn: &Txn<'_>,
        space: &SpaceUri,
        writes: &[SpaceWrite],
    ) -> Result<(), SpaceError> {
        for write in writes {
            let action = match write.action {
                OpAction::Create => SpaceAction::Create,
                OpAction::Update => SpaceAction::Update,
                OpAction::Delete => SpaceAction::Delete,
            };
            self.space(space, Some(&write.collection), action)?;
            let previous = spaces::record_txn(txn, space, &write.collection, &write.rkey).await?;
            match write.collection.as_str() {
                Membership::NSID => {
                    let old: Option<Membership> = previous.as_ref().map(spaces::record_value).transpose()?;
                    let new: Option<Membership> = write.value.as_deref().map(decode).transpose()?;
                    let old: BTreeSet<_> = old.as_ref().into_iter().flat_map(|m| &m.roles).collect();
                    let new: BTreeSet<_> = new.as_ref().into_iter().flat_map(|m| &m.roles).collect();
                    for role in old.symmetric_difference(&new) {
                        self.assign(role)?;
                    }
                }
                AccessRecord::NSID => {
                    let old = previous.as_ref().map(spaces::record_value).transpose()?;
                    let new = write.value.as_deref().map(decode).transpose()?;
                    self.access_change(space, old.as_ref(), new.as_ref())?;
                }
                Permissions::NSID => {
                    let old: Option<Permissions> = previous.as_ref().map(spaces::record_value).transpose()?;
                    let new: Option<Permissions> = write.value.as_deref().map(decode).transpose()?;
                    self.permission_change(old.as_ref(), new.as_ref())?;
                }
                Role::NSID if write.action == OpAction::Delete => self.assign(&write.rkey)?,
                // A fresh role has no privileges. Reusing a referenced ID
                // restores membership and grants, so it requires assignment.
                Role::NSID
                    if write.action == OpAction::Create && role_referenced(txn, &self.world, &write.rkey).await? =>
                {
                    self.assign(&write.rkey)?;
                }
                _ => {}
            }
        }
        Ok(())
    }

    pub(crate) fn access_change(
        &self,
        space: &SpaceUri,
        old: Option<&AccessRecord>,
        new: Option<&AccessRecord>,
    ) -> Result<(), SpaceError> {
        if new.is_some_and(|access| access.public.unwrap_or(false))
            && !old.is_some_and(|access| access.public.unwrap_or(false))
        {
            // Anonymous access is a whole-Space grant, not an ordinary edit.
            // The actor must be able to read it before making it public.
            self.space(space, None, SpaceAction::Read)?;
        }
        for after in new.into_iter().flat_map(|access| &access.grants) {
            let before = old.and_then(|access| access.grants.iter().find(|grant| grant.role == after.role));
            if before != Some(after) {
                self.assign(&after.role)?;
                self.added_scopes(before.map_or(&[], |grant| &grant.scopes), &after.scopes, Some(space))?;
            }
        }
        for before in old.into_iter().flat_map(|access| &access.grants) {
            if new.is_none_or(|access| access.grants.iter().all(|grant| grant.role != before.role)) {
                self.assign(&before.role)?;
            }
        }
        Ok(())
    }

    fn permission_change(&self, old: Option<&Permissions>, new: Option<&Permissions>) -> Result<(), SpaceError> {
        for after in new.into_iter().flat_map(|permissions| &permissions.grants) {
            let before = old.and_then(|permissions| permissions.grants.iter().find(|grant| grant.role == after.role));
            if before != Some(after) {
                self.assign(&after.role)?;
                self.added_scopes(before.map_or(&[], |grant| &grant.scopes), &after.scopes, None)?;
                self.added_assignable(before, after)?;
            }
        }
        for before in old.into_iter().flat_map(|permissions| &permissions.grants) {
            if new.is_none_or(|permissions| permissions.grants.iter().all(|grant| grant.role != before.role)) {
                self.assign(&before.role)?;
            }
        }
        Ok(())
    }

    fn added_assignable(&self, before: Option<&GroupGrant>, after: &GroupGrant) -> Result<(), SpaceError> {
        for role in &after.assignable {
            if before.is_none_or(|old| !old.assignable.contains(role)) {
                self.assign(role)?;
            }
        }
        Ok(())
    }

    fn added_scopes(&self, before: &[String], after: &[String], space: Option<&SpaceUri>) -> Result<(), SpaceError> {
        let mut available = self.scopes.clone();
        available.extend(match space {
            Some(space) => confined(before, space, &self.world)?,
            None => expand_all(before)?,
        });
        let requested = match space {
            Some(space) => confined(after, space, &self.world)?,
            None => expand_all(after)?,
        };
        if covers(&available, &requested, &self.world)? {
            Ok(())
        } else {
            Err(SpaceError::NotAuthorized)
        }
    }

    fn assign(&self, role: &str) -> Result<(), SpaceError> {
        if self.assignable.contains(role) {
            Ok(())
        } else {
            Err(SpaceError::NotAuthorized)
        }
    }
}

async fn role_referenced(txn: &Txn<'_>, world: &Did, role: &str) -> Result<bool, SpaceError> {
    let members = spaces::world_space(world, spaces::MEMBERS)?;
    for (_, value) in txn
        .scan(
            &keys::space_records(world, &members, Some(Membership::NSID)),
            usize::MAX,
        )
        .await?
    {
        let row = spaces::decode(&value)?;
        let membership: Membership = spaces::record_value(&row)?;
        if membership.roles.iter().any(|assigned| assigned == role) {
            return Ok(true);
        }
    }
    if let Some(row) = spaces::record_txn(txn, &members, Permissions::NSID, "self").await? {
        let permissions: Permissions = spaces::record_value(&row)?;
        if permissions
            .grants
            .iter()
            .any(|grant| grant.role == role || grant.assignable.iter().any(|assigned| assigned == role))
        {
            return Ok(true);
        }
    }
    for (_, value) in txn.scan(&keys::space_inventory(world), usize::MAX).await? {
        let info: SpaceInfo = spaces::decode(&value)?;
        if !info.deleted
            && let Some(row) = spaces::record_txn(txn, &info.space, AccessRecord::NSID, "self").await?
        {
            let access: AccessRecord = spaces::record_value(&row)?;
            if access.grants.iter().any(|grant| grant.role == role) {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

fn decode<T: serde::de::DeserializeOwned>(value: &[u8]) -> Result<T, SpaceError> {
    atproto_dasl::from_slice(value).map_err(|error| SpaceError::Invalid(error.to_string()))
}

/// Generic writes cannot forge records owned by the rules state machine or audit.
fn protected(collection: &str) -> bool {
    matches!(
        collection,
        AUDIT | "at.motet.pilot.scene" | "at.motet.pilot.admission" | "at.motet.pilot.closing"
    )
}

fn expand_all(scopes: &[String]) -> Result<Vec<Scope>, SpaceError> {
    let mut result = Vec::new();
    for scope in scopes {
        result.extend(expand(scope)?);
    }
    Ok(result)
}

fn expand(scope: &str) -> Result<Vec<Scope>, SpaceError> {
    let mut scopes = spaces::access::expand(scope)?;
    for scope in &mut scopes {
        if let Scope::Space(permission) = scope {
            let declared = match &permission.space_type {
                SpaceType::Nsid(kind) => spaces::access::declared_collections(kind)?,
                SpaceType::All => &[],
            };
            permission.collection = SpaceCollections::Explicit(permission.collections_resolved_with(declared));
        }
    }
    Ok(scopes)
}

fn confined(scopes: &[String], space: &SpaceUri, world: &Did) -> Result<Vec<Scope>, SpaceError> {
    let mut result = Vec::new();
    for scope in expand_all(scopes)? {
        if let Scope::Space(mut permission) = scope {
            if !permission.covers_space_type(space.space_type.as_str())
                || !matches!(&permission.did, SpaceDid::All | SpaceDid::SelfDid)
                    && permission.did != SpaceDid::Did(space.space_did.clone())
                || matches!(&permission.did, SpaceDid::SelfDid) && space.space_did != world.as_str()
                || matches!(&permission.skey, SpaceSkey::Key(key) if key != space.space_key.as_str())
            {
                continue;
            }
            permission.space_type = SpaceType::Nsid(space.space_type.to_string());
            permission.did = SpaceDid::Did(space.space_did.clone());
            permission.skey = SpaceSkey::Key(space.space_key.to_string());
            result.push(Scope::Space(permission));
        }
    }
    Ok(result)
}

/// Check each atomic permission against the union, not just one role's grant.
fn covers(grants: &[Scope], requested: &[Scope], world: &Did) -> Result<bool, SpaceError> {
    for request in requested {
        let allowed = match request {
            Scope::Space(space) => covers_space(grants, space, world)?,
            Scope::Repo(repo) => repo.collections.iter().all(|collection| repo.actions.iter().all(|action| {
                grants.iter().any(|grant| matches!(grant, Scope::Repo(grant)
                    if grant.actions.contains(action) && (grant.collections.contains(&RepoCollection::All) || grant.collections.contains(collection))))
            })),
            Scope::Rpc(rpc) => rpc.lxm.iter().all(|method| rpc.aud.iter().all(|audience| {
                grants.iter().any(|grant| matches!(grant, Scope::Rpc(grant)
                    if (grant.lxm.contains(&RpcLexicon::All) || grant.lxm.contains(method))
                        && grant.aud.iter().any(|allowed| rpc_audience_covers(allowed, audience))))
            })),
            _ => grants.iter().any(|grant| grant.grants(request)),
        };
        if !allowed {
            return Ok(false);
        }
    }
    Ok(true)
}

fn rpc_audience_covers(grant: &RpcAudience, target: &RpcAudience) -> bool {
    match (grant, target) {
        (RpcAudience::All, _) => true,
        (RpcAudience::Did(grant), RpcAudience::Did(target)) => {
            grant == target || (!grant.contains('#') && target.split_once('#').is_some_and(|(did, _)| did == grant))
        }
        (RpcAudience::Did(_), RpcAudience::All) => false,
    }
}

fn covers_space(grants: &[Scope], request: &SpacePermission, world: &Did) -> Result<bool, SpaceError> {
    let kind = match &request.space_type {
        SpaceType::All => "*",
        SpaceType::Nsid(kind) => kind,
    };
    let did = match &request.did {
        SpaceDid::All => "*",
        SpaceDid::SelfDid => world.as_str(),
        SpaceDid::Did(did) => did,
    };
    let skey = match &request.skey {
        SpaceSkey::All => "*",
        SpaceSkey::Key(key) => key,
    };
    let mut target = SpaceTarget::new(kind, did, skey, SpaceAction::Read);
    let declared = spaces::access::declared_collections(kind)?;
    let collections = request.collections_resolved_with(declared);
    for action in &request.action {
        target.action = *action;
        if matches!(action, SpaceAction::Read | SpaceAction::ReadSelf) {
            if !allows_space(grants, &target, declared, world) {
                return Ok(false);
            }
        } else {
            for collection in &collections {
                target.collection = match collection {
                    SpaceCollection::All => None,
                    SpaceCollection::Nsid(nsid) => Some(nsid.clone()),
                };
                if !allows_space(grants, &target, declared, world) {
                    return Ok(false);
                }
            }
        }
    }
    let mut target = SpaceManageTarget::new(kind, did, skey, SpaceManageVerb::Create);
    for verb in &request.manage {
        target.verb = *verb;
        if !grants
            .iter()
            .any(|grant| matches!(grant, Scope::Space(grant) if grant.matches_manage(&target, Some(world.as_str()))))
        {
            return Ok(false);
        }
    }
    Ok(true)
}

fn allows_space(grants: &[Scope], target: &SpaceTarget, declared: &[String], world: &Did) -> bool {
    grants
        .iter()
        .any(|grant| matches!(grant, Scope::Space(grant) if grant.matches_with(target, declared, Some(world.as_str()))))
}
