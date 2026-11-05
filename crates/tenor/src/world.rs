//! A hosted World's account metadata and the atomic creation transition.

use atproto_identity::model::Document;
use atproto_identity::plc;
use atproto_record::datetime::Datetime;
use bytes::Bytes;
use motet_rules::Did;
use serde::{Deserialize, Serialize};

use crate::event::Event;
use crate::identity::{self, IdentityError};
use crate::keys::Purpose;
use crate::shard::{Op, Txn};
use crate::store::{Reader, keys};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Status {
    Active,
}

/// Durable identity facts. The DID document is the accepted PLC genesis,
/// kept alongside the handle so a restart does not need PLC to serve it.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorldInfo {
    pub did: Did,
    pub handle: String,
    pub owner: Did,
    pub epoch: u32,
    pub status: Status,
    pub created: String,
    pub document: Document,
}

pub async fn get(reader: &Reader, did: &Did) -> Result<Option<WorldInfo>, IdentityError> {
    reader
        .get(&keys::world_info(did))
        .await?
        .map(|raw| serde_json::from_slice(&raw).map_err(Into::into))
        .transpose()
}

/// Only the identity orchestrator constructs this after reconciling PLC.
#[derive(Debug)]
pub(crate) struct CreateWorld {
    pub(crate) reservation: String,
    pub(crate) did: Did,
    pub(crate) document: Document,
}

impl Op for CreateWorld {
    type Output = WorldInfo;
    type Error = IdentityError;

    async fn run(self, txn: &mut Txn<'_>) -> Result<WorldInfo, IdentityError> {
        let mut reserved = identity::load(txn, &self.reservation).await?;
        let operation = reserved.signed.as_ref().ok_or(IdentityError::Inconsistent)?;
        if plc::Did::from_genesis(operation)?.to_string() != self.did.as_str()
            || self.document.id != self.did.as_str()
            || !self.document.confirms_handle(&reserved.handle)
        {
            return Err(IdentityError::Inconsistent);
        }
        if reserved.created {
            let raw = txn
                .get(&keys::world_info(&self.did))
                .await?
                .ok_or(IdentityError::Inconsistent)?;
            return Ok(serde_json::from_slice(&raw)?);
        }
        if txn.get(&keys::world_info(&self.did)).await?.is_some()
            || txn.get(&keys::handle(&reserved.handle)).await?.is_some()
        {
            return Err(IdentityError::Inconsistent);
        }
        for purpose in [Purpose::Signing, Purpose::Kem] {
            if !txn.adopt_reserved_key(&self.reservation, &self.did, purpose).await? {
                return Err(IdentityError::Inconsistent);
            }
        }
        txn.adopt_reserved_key(&self.reservation, &self.did, Purpose::Rotation)
            .await?;
        let info = WorldInfo {
            did: self.did,
            handle: reserved.handle.clone(),
            owner: reserved.owner.clone(),
            epoch: 1,
            status: Status::Active,
            created: Datetime::now().to_string(),
            document: self.document,
        };
        txn.put(keys::world_info(&info.did), serde_json::to_vec(&info)?.into());
        txn.put(
            keys::handle(&info.handle),
            Bytes::copy_from_slice(info.did.as_str().as_bytes()),
        );
        txn.delete(keys::reserved_handle(&info.handle));
        txn.begin_world(info.did.clone(), info.epoch)
            .map_err(|_| IdentityError::Inconsistent)?;
        crate::repo::genesis(txn, &info.did).await?;
        crate::spaces::bootstrap(
            txn,
            &info.did,
            &info.owner,
            crate::spaces::ProfileInput {
                display_name: Some(reserved.name.clone().unwrap_or_else(|| info.handle.clone())),
                public: matches!(reserved.visibility, identity::Visibility::Public),
                ..crate::spaces::ProfileInput::default()
            },
        )
        .await?;
        reserved.created = true;
        txn.put(
            keys::reservation(&self.reservation),
            serde_json::to_vec(&reserved)?.into(),
        );
        txn.emit(Event::Identity {
            did: info.did.clone(),
            handle: Some(info.handle.clone()),
            time: info.created.clone(),
        });
        txn.emit(Event::Account {
            did: info.did.clone(),
            active: true,
            status: None,
            time: info.created.clone(),
        });
        Ok(info)
    }
}
