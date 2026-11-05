//! Public scene mutations share a shard transaction with rules and attribution.

use std::collections::BTreeSet;
use std::fmt::Display;

use atproto_dasl::{BlockStorage, Cid};
use atproto_record::{datetime::Datetime, tid::Tid};
use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use motet_rules::world::{Command, Outcome};
use motet_rules::{Did, OperationId, SceneId};
use motet_xrpc::server::{Input, Service};
use motet_xrpc::{Access, Caller, Kind, Method, XrpcError};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::authorization::Actor;
use crate::repo::{self, RepoError, Write};
use crate::shard::{Handle, Op, Rejected, Txn};
use crate::spaces::SpaceError;
use crate::store::keys;

const SCENE: &str = "at.motet.pilot.scene";
const OPEN: &str = "at.motet.pilot.play.openScene";
const SET_CAST: &str = "at.motet.pilot.play.setSceneCast";
const CLOSE: &str = "at.motet.pilot.play.closeScene";
const MAX_CAST: usize = 100;

const METHODS: [Method; 3] = [
    Method {
        nsid: OPEN,
        kind: Kind::Procedure,
        access: Access::Service,
        max_body: 256 * 1024,
    },
    Method {
        nsid: SET_CAST,
        kind: Kind::Procedure,
        access: Access::Service,
        max_body: 256 * 1024,
    },
    Method {
        nsid: CLOSE,
        kind: Kind::Procedure,
        access: Access::Service,
        max_body: 4096,
    },
];

/// Add the typed public-scene procedures to the service-auth registry.
#[must_use]
pub(crate) fn register<S: Clone + Send + Sync + 'static>(mut service: Service<S>, shard: &Handle) -> Service<S> {
    for method in METHODS {
        let shard = shard.clone();
        let nsid = method.nsid;
        service = service.method(method, move |_, caller, input| {
            dispatch(shard.clone(), nsid, caller, input)
        });
    }
    service
}

#[derive(Deserialize, Serialize)]
struct OpenInput {
    op: String,
    world: String,
    title: String,
    location: Option<String>,
    arc: Option<String>,
    #[serde(default)]
    cast: Vec<String>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct CastInput {
    op: String,
    world: String,
    scene: String,
    cast: Vec<String>,
    swap_record: String,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct CloseInput {
    op: String,
    world: String,
    scene: String,
    swap_record: String,
}

#[derive(Deserialize, Serialize)]
struct Changed {
    scene: String,
    uri: String,
    cid: String,
}

#[derive(Deserialize, Serialize)]
struct Receipt {
    method: String,
    request: Value,
    result: Changed,
}

enum Change {
    Open {
        title: String,
        location: Option<String>,
        arc: Option<String>,
        cast: BTreeSet<Did>,
    },
    Cast {
        scene: String,
        cast: BTreeSet<Did>,
        swap: Cid,
    },
    Close {
        scene: String,
        swap: Cid,
    },
}

struct Mutate {
    actor: Actor,
    method: &'static str,
    op: String,
    request: Value,
    change: Change,
}

fn invalid(error: impl Display) -> XrpcError {
    XrpcError::InvalidRequest(error.to_string())
}

fn internal(error: impl Display) -> XrpcError {
    tracing::error!(%error, "public scene operation failed");
    XrpcError::Internal
}

fn space_error(error: SpaceError) -> XrpcError {
    match error {
        SpaceError::NotAuthorized => XrpcError::Protocol {
            status: StatusCode::FORBIDDEN,
            name: "Forbidden",
            message: "the actor lacks the current World RPC grant".into(),
        },
        SpaceError::NotFound | SpaceError::Deleted => XrpcError::NotFound,
        SpaceError::Invalid(message) | SpaceError::Repo(RepoError::InvalidRecord(message)) => invalid(message),
        other => internal(other),
    }
}

fn repo_error(error: RepoError) -> XrpcError {
    match error {
        RepoError::InvalidRecord(message) => invalid(message),
        RepoError::RecordNotFound(_) | RepoError::UnknownWorld(_) => XrpcError::NotFound,
        RepoError::InvalidSwap => invalid_swap(),
        other => internal(other),
    }
}

fn invalid_swap() -> XrpcError {
    XrpcError::Protocol {
        status: StatusCode::BAD_REQUEST,
        name: "InvalidSwap",
        message: "the scene changed since the supplied swapRecord".into(),
    }
}

fn world(value: String) -> Result<Did, XrpcError> {
    atproto_lexicon::validation::syntax::validate_did(&value).map_err(invalid)?;
    Ok(Did::new(value))
}

fn key(value: &str) -> Result<(), XrpcError> {
    atproto_record::record_key::check_record_key(value).map_err(invalid)
}

fn cast(values: Vec<String>) -> Result<BTreeSet<Did>, XrpcError> {
    if values.len() > MAX_CAST {
        return Err(invalid("cast exceeds 100 members"));
    }
    values
        .into_iter()
        .map(|value| {
            atproto_lexicon::validation::syntax::validate_did(&value).map_err(invalid)?;
            Ok(Did::new(value))
        })
        .collect()
}

async fn dispatch(shard: Handle, method: &'static str, caller: Caller, input: Input) -> Result<Response, XrpcError> {
    let request: Value = serde_json::from_slice(&input.body).map_err(invalid)?;
    let (world, op, change) = match method {
        OPEN => {
            let body: OpenInput = serde_json::from_slice(&input.body).map_err(invalid)?;
            key(&body.op)?;
            if let Some(value) = &body.location {
                key(value)?;
            }
            if let Some(value) = &body.arc {
                key(value)?;
            }
            let world = world(body.world)?;
            (
                world,
                body.op,
                Change::Open {
                    title: body.title,
                    location: body.location,
                    arc: body.arc,
                    cast: cast(body.cast)?,
                },
            )
        }
        SET_CAST => {
            let body: CastInput = serde_json::from_slice(&input.body).map_err(invalid)?;
            key(&body.op)?;
            key(&body.scene)?;
            let swap = body.swap_record.parse().map_err(invalid)?;
            (
                world(body.world)?,
                body.op,
                Change::Cast {
                    scene: body.scene,
                    cast: cast(body.cast)?,
                    swap,
                },
            )
        }
        CLOSE => {
            let body: CloseInput = serde_json::from_slice(&input.body).map_err(invalid)?;
            key(&body.op)?;
            key(&body.scene)?;
            let swap = body.swap_record.parse().map_err(invalid)?;
            (
                world(body.world)?,
                body.op,
                Change::Close {
                    scene: body.scene,
                    swap,
                },
            )
        }
        _ => return Err(XrpcError::MethodNotImplemented),
    };
    let actor = Actor::from_caller(caller, world).map_err(space_error)?;
    let changed = shard
        .run(Mutate {
            actor,
            method,
            op,
            request,
            change,
        })
        .await
        .map_err(internal)??;
    Ok(Json(changed).into_response())
}

/// Load the current signed public record through the transaction overlay.
async fn existing(txn: &Txn<'_>, world: &Did, scene: &str, swap: Cid) -> Result<Value, XrpcError> {
    let index = txn
        .get(&keys::repo_record(world, SCENE, scene))
        .await
        .map_err(internal)?
        .ok_or(XrpcError::NotFound)?;
    let cid = Cid::from_bytes(&index).map_err(internal)?;
    if cid != swap {
        return Err(invalid_swap());
    }
    let block = txn
        .blocks(world)
        .get(&cid)
        .await
        .map_err(internal)?
        .ok_or_else(|| internal("scene index points to a missing block"))?;
    atproto_dasl::from_slice_to_json(&block).map_err(internal)
}

fn require_open(record: &Value) -> Result<(), XrpcError> {
    match record.get("state").and_then(Value::as_str) {
        Some("open") => Ok(()),
        Some("closed") => Err(XrpcError::Protocol {
            status: StatusCode::BAD_REQUEST,
            name: "SceneClosed",
            message: "the scene is already closed".into(),
        }),
        _ => Err(invalid("the scene is not open")),
    }
}

async fn prepare(
    change: Change,
    actor: &Actor,
    txn: &Txn<'_>,
) -> Result<(String, Value, Command, Option<Cid>), XrpcError> {
    let world = &actor.world;
    Ok(match change {
        Change::Open {
            title,
            location,
            arc,
            mut cast,
        } => {
            cast.insert(actor.member.clone());
            if cast.len() > MAX_CAST {
                return Err(invalid("cast including the opener exceeds 100 members"));
            }
            let scene = Tid::new().to_string();
            let mut record = json!({
                "$type": SCENE, "title": title, "state": "open", "cast": cast,
                "openedBy": actor.member, "createdAt": Datetime::now().to_string(),
            });
            let fields = record
                .as_object_mut()
                .ok_or_else(|| internal("scene record must be an object"))?;
            if let Some(location) = location {
                fields.insert("location".into(), location.into());
            }
            if let Some(arc) = arc {
                fields.insert("arc".into(), arc.into());
            }
            let command = Command::OpenScene {
                scene: SceneId::new(&scene),
                cast,
            };
            (scene, record, command, None)
        }
        Change::Cast { scene, mut cast, swap } => {
            let mut record = existing(txn, world, &scene, swap).await?;
            require_open(&record)?;
            let opener = record
                .get("openedBy")
                .and_then(Value::as_str)
                .ok_or_else(|| internal("scene has no opener"))?;
            cast.insert(Did::new(opener));
            if cast.len() > MAX_CAST {
                return Err(invalid("cast including the opener exceeds 100 members"));
            }
            record
                .as_object_mut()
                .ok_or_else(|| internal("scene record must be an object"))?
                .insert("cast".into(), json!(cast));
            let command = Command::SetSceneCast {
                scene: SceneId::new(&scene),
                cast,
            };
            (scene, record, command, Some(swap))
        }
        Change::Close { scene, swap } => {
            let mut record = existing(txn, world, &scene, swap).await?;
            require_open(&record)?;
            record
                .as_object_mut()
                .ok_or_else(|| internal("scene record must be an object"))?
                .insert("state".into(), "closed".into());
            let command = Command::CloseScene {
                scene: SceneId::new(&scene),
            };
            (scene, record, command, Some(swap))
        }
    })
}

impl Op for Mutate {
    type Output = Changed;
    type Error = XrpcError;

    async fn run(self, txn: &mut Txn<'_>) -> Result<Changed, XrpcError> {
        let world = &self.actor.world;
        self.actor
            .policy(txn)
            .await
            .map_err(space_error)?
            .rpc(self.method)
            .map_err(space_error)?;
        self.actor.consume(txn).await.map_err(space_error)?;
        if txn.world(world).is_none() {
            return Err(internal("hosted World has no rules state"));
        }
        let receipt_key = keys::scene_operation(world, &self.actor.member, &self.op);
        if let Some(raw) = txn.get(&receipt_key).await.map_err(internal)? {
            let receipt: Receipt = serde_json::from_slice(&raw).map_err(internal)?;
            if receipt.method != self.method || receipt.request != self.request {
                return Err(invalid("operation ID was already used for a different request"));
            }
            return Ok(receipt.result);
        }
        let (scene, record, command, swap) = prepare(self.change, &self.actor, txn).await?;
        let collection = repo::Nsid::new(SCENE).map_err(repo_error)?;
        let encoded = repo::record_from_json(&record).map_err(repo_error)?;
        let write = if let Some(swap) = swap {
            Write::Update {
                collection,
                rkey: scene.clone(),
                record: encoded,
                swap: Some(swap),
            }
        } else {
            Write::Create {
                collection,
                rkey: scene.clone(),
                record: encoded,
            }
        };
        let operation = OperationId::new(format!("scene/{}/{}", self.actor.member, self.op));
        match txn.command(world.clone(), operation, command).await.map_err(internal)? {
            Ok(Outcome::AuthorityChanged { .. }) => {}
            Err(Rejected::UnknownWorld(_)) | Ok(Outcome::Refused(_)) => return Err(XrpcError::NotFound),
            other => return Err(internal(format!("unexpected rules outcome: {other:?}"))),
        }
        let committed = repo::commit(txn, world, vec![write], None).await.map_err(repo_error)?;
        let (uri, cid) = committed
            .results
            .into_iter()
            .next()
            .ok_or_else(|| internal("missing scene result"))?;
        let cid = cid.ok_or_else(|| internal("scene write has no CID"))?.to_string();
        self.actor.audit(txn, self.method, &uri).await.map_err(space_error)?;
        let receipt = Receipt {
            method: self.method.into(),
            request: self.request,
            result: Changed { scene, uri, cid },
        };
        txn.put(receipt_key, serde_json::to_vec(&receipt).map_err(internal)?.into());
        Ok(receipt.result)
    }
}
