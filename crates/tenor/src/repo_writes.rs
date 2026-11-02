//! Service-authenticated public-repo mutations, checked and audited on the shard.
//!
//! Wire shapes follow the com.atproto.repo lexicons, not the PDS handler's
//! extra delete-response fields.

use atproto_dasl::Cid;
use atproto_oauth::scopes::RepoAction;
use atproto_record::tid::Tid;
use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use bytes::Bytes;
use motet_rules::Did;
use motet_xrpc::server::{Input, Service};
use motet_xrpc::{Access, Caller, Kind, Method, XrpcError};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::authorization::{Actor, Policy};
use crate::repo::{self, RepoError};
use crate::shard::{Handle, Op, Txn};
use crate::spaces::SpaceError;
use crate::store::keys;

/// Register World-repo mutations authenticated by the member's PDS.
#[must_use]
pub(crate) fn register<S: Clone + Send + Sync + 'static>(mut service: Service<S>, shard: &Handle) -> Service<S> {
    for nsid in [
        "com.atproto.repo.createRecord",
        "com.atproto.repo.putRecord",
        "com.atproto.repo.deleteRecord",
        "com.atproto.repo.applyWrites",
    ] {
        let shard = shard.clone();
        service = service.method(
            Method {
                nsid,
                kind: Kind::Procedure,
                access: Access::Service,
                max_body: 4 * 1024 * 1024,
            },
            move |_, caller, input| dispatch(shard.clone(), nsid, caller, input),
        );
    }
    service
}

#[derive(Default)]
enum Swap {
    #[default]
    Unchecked,
    Absent,
    Cid(String),
}

impl<'de> Deserialize<'de> for Swap {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(Option::<String>::deserialize(deserializer)?.map_or(Self::Absent, Self::Cid))
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RecordInput {
    repo: String,
    collection: String,
    rkey: Option<String>,
    record: Option<Value>,
    #[serde(default)]
    swap_record: Swap,
    swap_commit: Option<String>,
    // A World always validates known lexicons, even when validation is optional
    // for an ordinary PDS. Unknown collections cannot become World records.
    #[serde(rename = "validate")]
    _validate: Option<bool>,
}

#[derive(Deserialize)]
#[serde(tag = "$type")]
enum BatchWrite {
    #[serde(rename = "com.atproto.repo.applyWrites#create")]
    Create {
        collection: String,
        rkey: Option<String>,
        value: Value,
    },
    #[serde(rename = "com.atproto.repo.applyWrites#update")]
    Update {
        collection: String,
        rkey: String,
        value: Value,
    },
    #[serde(rename = "com.atproto.repo.applyWrites#delete")]
    Delete { collection: String, rkey: String },
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BatchInput {
    repo: String,
    writes: Vec<BatchWrite>,
    swap_commit: Option<String>,
    #[serde(rename = "validate")]
    _validate: Option<bool>,
}

#[derive(Clone, Copy)]
enum WriteKind {
    Create,
    Put,
    Update,
    Delete,
}

struct Pending {
    kind: WriteKind,
    collection: repo::Nsid,
    rkey: String,
    record: Option<Bytes>,
    swap: Swap,
}

enum Prepared {
    Mutation(repo::Write, RepoAction),
    AbsentDelete(String),
}

impl Pending {
    async fn prepare(self, txn: &Txn<'_>, world: &Did, policy: &Policy, batch: bool) -> Result<Prepared, XrpcError> {
        repo::validate_address(self.collection.as_str(), &self.rkey).map_err(repo_error)?;
        let prior = txn
            .get(&keys::repo_record(world, self.collection.as_str(), &self.rkey))
            .await
            .map_err(internal)?
            .map(|value| Cid::from_bytes(&value))
            .transpose()
            .map_err(internal)?;
        let action = match self.kind {
            WriteKind::Create => RepoAction::Create,
            WriteKind::Put if prior.is_none() => RepoAction::Create,
            WriteKind::Put | WriteKind::Update => RepoAction::Update,
            WriteKind::Delete => RepoAction::Delete,
        };
        policy
            .repo(self.collection.as_str(), action.clone())
            .map_err(space_error)?;
        let swap = match self.swap {
            Swap::Unchecked => None,
            Swap::Absent if prior.is_none() => None,
            Swap::Cid(value) => {
                let expected = parse_cid(&value)?;
                if prior != Some(expected) {
                    return Err(repo_error(RepoError::InvalidSwap));
                }
                Some(expected)
            }
            Swap::Absent => return Err(repo_error(RepoError::InvalidSwap)),
        };
        // deleteRecord means "ensure absent". A missing batch delete is
        // still a conflict; applyWrites specifies an existing record.
        if !batch && matches!(self.kind, WriteKind::Delete) && prior.is_none() {
            return Ok(Prepared::AbsentDelete(format!(
                "at://{world}/{}/{}",
                self.collection.as_str(),
                self.rkey
            )));
        }
        let write = match action {
            RepoAction::Create => repo::Write::Create {
                collection: self.collection,
                rkey: self.rkey,
                record: self.record.ok_or_else(|| invalid("missing record"))?,
            },
            RepoAction::Update => repo::Write::Update {
                collection: self.collection,
                rkey: self.rkey,
                record: self.record.ok_or_else(|| invalid("missing record"))?,
                swap,
            },
            RepoAction::Delete => repo::Write::Delete {
                collection: self.collection,
                rkey: self.rkey,
                swap,
            },
        };
        Ok(Prepared::Mutation(write, action))
    }
}

struct Apply {
    caller: Caller,
    repo: String,
    writes: Vec<Pending>,
    swap_commit: Option<Cid>,
    batch: bool,
}

impl Op for Apply {
    type Output = Value;
    type Error = XrpcError;

    async fn run(self, txn: &mut Txn<'_>) -> Result<Value, XrpcError> {
        let world = if self.repo.starts_with("did:") {
            Did::new(&self.repo)
        } else {
            let handle = self.repo.to_ascii_lowercase();
            let raw = txn
                .get(&keys::handle(&handle))
                .await
                .map_err(internal)?
                .ok_or(XrpcError::AuthRequired)?;
            let world = Did::new(std::str::from_utf8(&raw).map_err(internal)?);
            let value = txn
                .get(&keys::world_info(&world))
                .await
                .map_err(internal)?
                .ok_or(XrpcError::AuthRequired)?;
            let info: crate::world::WorldInfo = serde_json::from_slice(&value).map_err(internal)?;
            if info.did != world || info.handle != handle || !info.document.confirms_handle(&handle) {
                return Err(XrpcError::AuthRequired);
            }
            world
        };
        let actor = Actor::from_caller(self.caller, world).map_err(space_error)?;
        let policy = actor.policy(txn).await.map_err(space_error)?;
        actor.consume(txn).await.map_err(space_error)?;
        let raw = txn
            .get(&keys::repo_head(&actor.world))
            .await
            .map_err(internal)?
            .ok_or_else(|| repo_error(RepoError::UnknownWorld(actor.world.clone())))?;
        let head: repo::Head = serde_json::from_slice(&raw).map_err(internal)?;
        if self.swap_commit.is_some_and(|expected| expected != head.cid) {
            return Err(repo_error(RepoError::InvalidSwap));
        }
        if self.writes.is_empty() {
            return Ok(json!({"results": []}));
        }
        let mut writes = Vec::with_capacity(self.writes.len());
        let mut actions = Vec::with_capacity(self.writes.len());
        for pending in self.writes {
            match pending.prepare(txn, &actor.world, &policy, self.batch).await? {
                Prepared::Mutation(write, action) => {
                    writes.push(write);
                    actions.push(action);
                }
                Prepared::AbsentDelete(uri) => {
                    actor.audit(txn, "delete", &uri).await.map_err(space_error)?;
                    return Ok(json!({}));
                }
            }
        }
        let committed = repo::commit(txn, &actor.world, writes, self.swap_commit)
            .await
            .map_err(repo_error)?;
        let commit = json!({"cid": committed.commit.to_string(), "rev": committed.rev.to_string()});
        let mut results = Vec::with_capacity(committed.results.len());
        for ((uri, cid), action) in committed.results.into_iter().zip(actions) {
            actor.audit(txn, action.as_str(), &uri).await.map_err(space_error)?;
            let mut result = match action {
                RepoAction::Delete => json!({}),
                RepoAction::Create | RepoAction::Update => json!({
                    "uri": uri, "cid": cid.ok_or(XrpcError::Internal)?.to_string(), "validationStatus": "valid",
                }),
            };
            let object = result.as_object_mut().ok_or(XrpcError::Internal)?;
            if self.batch {
                object.insert(
                    "$type".into(),
                    match action {
                        RepoAction::Create => "com.atproto.repo.applyWrites#createResult",
                        RepoAction::Update => "com.atproto.repo.applyWrites#updateResult",
                        RepoAction::Delete => "com.atproto.repo.applyWrites#deleteResult",
                    }
                    .into(),
                );
            }
            results.push(result);
        }
        if self.batch {
            Ok(json!({"commit": commit, "results": results}))
        } else {
            let mut result = results.into_iter().next().ok_or(XrpcError::Internal)?;
            result
                .as_object_mut()
                .ok_or(XrpcError::Internal)?
                .insert("commit".into(), commit);
            Ok(result)
        }
    }
}

async fn dispatch(shard: Handle, nsid: &str, caller: Caller, input: Input) -> Result<Response, XrpcError> {
    let batch = nsid == "com.atproto.repo.applyWrites";
    let (repo, writes, swap_commit) = if batch {
        let request: BatchInput = serde_json::from_slice(&input.body).map_err(|error| invalid(error.to_string()))?;
        if request.writes.len() > 200 {
            return Err(invalid("writes must contain at most 200 entries"));
        }
        let writes = request
            .writes
            .into_iter()
            .map(|write| {
                let (kind, collection, rkey, value) = match write {
                    BatchWrite::Create {
                        collection,
                        rkey,
                        value,
                    } => (
                        WriteKind::Create,
                        collection,
                        rkey.unwrap_or_else(|| Tid::new().to_string()),
                        Some(value),
                    ),
                    BatchWrite::Update {
                        collection,
                        rkey,
                        value,
                    } => (WriteKind::Update, collection, rkey, Some(value)),
                    BatchWrite::Delete { collection, rkey } => (WriteKind::Delete, collection, rkey, None),
                };
                Ok(Pending {
                    kind,
                    collection: repo::Nsid::new(collection).map_err(repo_error)?,
                    rkey,
                    record: value
                        .as_ref()
                        .map(repo::record_from_json)
                        .transpose()
                        .map_err(repo_error)?,
                    swap: Swap::Unchecked,
                })
            })
            .collect::<Result<Vec<_>, XrpcError>>()?;
        (request.repo, writes, request.swap_commit)
    } else {
        let request: RecordInput = serde_json::from_slice(&input.body).map_err(|error| invalid(error.to_string()))?;
        let kind = match nsid {
            "com.atproto.repo.createRecord" => WriteKind::Create,
            "com.atproto.repo.putRecord" => WriteKind::Put,
            "com.atproto.repo.deleteRecord" => WriteKind::Delete,
            _ => return Err(XrpcError::MethodNotImplemented),
        };
        let rkey = match request.rkey {
            Some(rkey) => rkey,
            None if matches!(kind, WriteKind::Create) => Tid::new().to_string(),
            None => return Err(invalid("missing rkey")),
        };
        let swap = match (kind, request.swap_record) {
            (WriteKind::Create, _) | (WriteKind::Delete, Swap::Absent) => Swap::Unchecked,
            (_, swap) => swap,
        };
        let record = if matches!(kind, WriteKind::Delete) {
            None
        } else {
            Some(
                repo::record_from_json(&request.record.ok_or_else(|| invalid("missing record"))?)
                    .map_err(repo_error)?,
            )
        };
        (
            request.repo,
            vec![Pending {
                kind,
                collection: repo::Nsid::new(request.collection).map_err(repo_error)?,
                rkey,
                record,
                swap,
            }],
            request.swap_commit,
        )
    };
    let result = shard
        .run(Apply {
            caller,
            repo,
            writes,
            swap_commit: swap_commit.as_deref().map(parse_cid).transpose()?,
            batch,
        })
        .await
        .map_err(internal)??;
    Ok(Json(result).into_response())
}

fn parse_cid(value: &str) -> Result<Cid, XrpcError> {
    value.parse().map_err(|_| invalid("invalid CID"))
}

fn invalid(message: impl Into<String>) -> XrpcError {
    XrpcError::InvalidRequest(message.into())
}

fn internal(error: impl std::fmt::Display) -> XrpcError {
    tracing::error!(error = %error, "World repo mutation failed");
    XrpcError::Internal
}

fn space_error(error: SpaceError) -> XrpcError {
    match error {
        SpaceError::NotAuthorized => XrpcError::Protocol {
            status: StatusCode::FORBIDDEN,
            name: "InsufficientScope",
            message: "current membership and role grants do not permit this action".into(),
        },
        SpaceError::Repo(error) => repo_error(error),
        other => internal(other),
    }
}

fn repo_error(error: RepoError) -> XrpcError {
    match error {
        RepoError::InvalidSwap => XrpcError::Protocol {
            status: StatusCode::BAD_REQUEST,
            name: "InvalidSwap",
            message: "the expected record or commit changed".into(),
        },
        RepoError::InvalidRecord(message) | RepoError::UnknownCollection(message) => invalid(message),
        RepoError::RecordAlreadyExists(path) => invalid(format!("record already exists: {path}")),
        RepoError::RecordNotFound(path) => invalid(format!("record not found: {path}")),
        RepoError::UnknownWorld(_) => XrpcError::AuthRequired,
        other => internal(other),
    }
}
