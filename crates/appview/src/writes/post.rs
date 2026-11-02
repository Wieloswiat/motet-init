//! Publish a post first, then durably retry admission of that exact version.

use std::sync::Arc;
use std::time::Duration;

use atproto_identity::resolve::{InputType, parse_input};
use atproto_record::datetime::Datetime;
use atproto_record::tid::Tid;
use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use motet_xrpc::server::Input;
use motet_xrpc::{Access, Caller, Kind, Method, XrpcError};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

use crate::api::State;
use crate::auth::{
    self,
    pds::PdsError,
    store::{SessionRef, StoreError},
};
use crate::outbox::{self, Job};
use crate::writes::client::{Client, ClientError};

const POST: &str = "at.motet.pilot.post";
const SUBMIT: &str = "at.motet.pilot.play.submitAdmission";
const WAIT: Duration = Duration::from_secs(30);
const POLL: Duration = Duration::from_millis(200);

/// The authenticated browser procedure for publishing and admitting a post.
pub const CREATE_POST: Method = Method {
    nsid: "at.motet.pilot.createPost",
    kind: Kind::Procedure,
    access: Access::AppviewSession,
    // JSON escapes can take six bytes per source character.
    max_body: 512 * 1024,
};

#[derive(Deserialize)]
struct CreatePost {
    op: String,
    world: String,
    scene: String,
    body: String,
    register: String,
}

#[derive(Deserialize)]
#[serde(tag = "stage", rename_all = "snake_case")]
enum Stage<'a> {
    Publish { world: &'a str, rkey: &'a str },
    Publishing { world: &'a str, rkey: &'a str },
    Admission { world: &'a str, uri: &'a str, cid: &'a str },
}

#[derive(Deserialize)]
#[serde(tag = "status", rename_all = "lowercase")]
enum Admission {
    Pending,
    Admitted { epoch: u32, seq: u64 },
    Refused { reason: String },
}

struct Published<'a> {
    world: &'a str,
    uri: &'a str,
    cid: &'a str,
}

fn upstream_failure() -> XrpcError {
    XrpcError::Protocol {
        status: StatusCode::BAD_GATEWAY,
        name: "UpstreamFailure",
        message: "The post could not be published to the author's PDS.".into(),
    }
}

fn forbidden() -> XrpcError {
    XrpcError::Protocol {
        status: StatusCode::FORBIDDEN,
        name: "Forbidden",
        message: "The signed-in author is not authorized to publish.".into(),
    }
}

#[expect(
    clippy::needless_pass_by_value,
    reason = "Result::map_err passes the database error by value"
)]
fn internal(error: sqlx::Error) -> XrpcError {
    tracing::error!(%error, "post outbox database error");
    XrpcError::Internal
}

/// Queues a create-only PDS write and returns its first admission outcome.
///
/// Posts reach their author's PDS in order, even while earlier admissions
/// wait for the tenor. A published post returns its exact URI and CID.
pub async fn create_post(state: Arc<State>, caller: Caller, input: Input) -> Result<Response, XrpcError> {
    let Caller::AppviewSession { did, .. } = caller else {
        return Err(XrpcError::AuthRequired);
    };
    let cookie = auth::session_cookie(&input.headers)?;
    let session = SessionRef::from_cookie(cookie).ok_or(XrpcError::AuthRequired)?;
    let request: CreatePost =
        serde_json::from_slice(&input.body).map_err(|_| XrpcError::InvalidRequest("invalid post request".into()))?;
    if Tid::decode(&request.op).is_err() {
        return Err(XrpcError::InvalidRequest("operation must be a TID".into()));
    }
    match parse_input(&request.world) {
        Ok(InputType::Plc(world) | InputType::Web(world)) if world == request.world => {}
        _ => return Err(XrpcError::InvalidRequest("invalid World DID".into())),
    }
    if !atproto_record::record_key::is_valid_record_key(&request.scene) {
        return Err(XrpcError::InvalidRequest("invalid scene record key".into()));
    }
    if request.body.chars().count() > 65_536 {
        return Err(XrpcError::InvalidRequest("post body is too long".into()));
    }
    let identity = json!({
        "world": request.world,
        "scene": request.scene,
        "body": request.body,
        "register": request.register,
    });
    let record = json!({
        "$type": POST,
        "world": request.world,
        "scene": request.scene,
        "body": request.body,
        "register": request.register,
        "createdAt": Datetime::now().to_string(),
    });
    let world = record.get("world").and_then(Value::as_str).ok_or(XrpcError::Internal)?;
    let payload = json!({ "stage": "publish", "world": world, "rkey": request.op, "record": record });
    let id = outbox::enqueue_post(&state.pool, &did, &request.op, &payload, &identity, &session)
        .await
        .map_err(internal)?
        .ok_or_else(|| XrpcError::InvalidRequest("operation was already used for a different post".into()))?;
    let deadline = tokio::time::Instant::now() + WAIT;
    loop {
        let owner = Uuid::now_v7().to_string();
        if let Some(job) = outbox::claim_post(&state.pool, &owner, Some(id))
            .await
            .map_err(internal)?
        {
            process(&state.pool, &state.auth, &state.client, &job, &owner)
                .await
                .map_err(internal)?;
        }
        let current: (String, Value, Option<Value>) =
            sqlx::query_as("SELECT state, payload, outcome FROM service.outbox WHERE id = $1")
                .bind(id)
                .fetch_one(&state.pool)
                .await
                .map_err(internal)?;
        if let Ok(Stage::Admission { uri, cid, .. }) = Stage::deserialize(&current.1) {
            if current.0 == "failed" {
                let reason = current
                    .2
                    .as_ref()
                    .and_then(|outcome| outcome.get("error"))
                    .and_then(Value::as_str)
                    .unwrap_or("Admission failed");
                return Ok(
                    Json(json!({ "uri": uri, "cid": cid, "status": "failed", "reason": reason })).into_response(),
                );
            }
            if current.0 == "done" {
                return Ok(Json(current.2.ok_or(XrpcError::Internal)?).into_response());
            }
            if current.2.is_some() || tokio::time::Instant::now() >= deadline {
                return Ok(Json(json!({ "uri": uri, "cid": cid, "status": "pending" })).into_response());
            }
        } else if let Some(outcome) = current.2 {
            return response(outcome);
        }
        if tokio::time::Instant::now() >= deadline {
            // A timed-out create may still commit; keep the job for read-back.
            return Err(upstream_failure());
        }
        tokio::time::sleep(POLL).await;
    }
}

fn response(outcome: Value) -> Result<Response, XrpcError> {
    if outcome.get("error").is_some() {
        return Err(match outcome.get("error").and_then(Value::as_str) {
            Some("forbidden") => forbidden(),
            Some("conflict") => XrpcError::InvalidRequest("post record key is already in use".into()),
            _ => upstream_failure(),
        });
    }
    Ok(Json(outcome).into_response())
}

/// One fenced attempt. A create-only key and read-back recover uncertain PDS
/// writes; tenor submissions reuse the same durable operation and strong ref.
pub(crate) async fn process(
    pool: &PgPool,
    auth: &auth::Server,
    client: &Client,
    job: &Job,
    owner: &str,
) -> sqlx::Result<()> {
    let Some(session) = job.session_id.as_deref().and_then(SessionRef::from_db) else {
        outbox::pause(pool, job, owner, &json!({"error": "forbidden"})).await?;
        return Ok(());
    };
    let Ok(stage) = Stage::deserialize(&job.payload) else {
        outbox::finish(pool, job, owner, true, &json!({"error": "upstream"})).await?;
        return Ok(());
    };
    let recovering = matches!(stage, Stage::Publishing { .. });
    match stage {
        Stage::Publish { world, rkey } | Stage::Publishing { world, rkey } => {
            let Some(record) = job.payload.get("record") else {
                outbox::finish(pool, job, owner, true, &json!({"error": "upstream"})).await?;
                return Ok(());
            };
            if !recovering && !outbox::mark_publishing(pool, job, owner).await? {
                return Ok(());
            }
            let existing = if recovering {
                match auth.pds().get_record_by_ref(&session, POST, rkey).await {
                    Ok(value) => value,
                    Err(
                        PdsError::Session
                        | PdsError::Refresh
                        | PdsError::Status(StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN),
                    ) => {
                        outbox::pause(pool, job, owner, &json!({"error": "forbidden"})).await?;
                        return Ok(());
                    }
                    Err(_) => {
                        outbox::reschedule(pool, job, owner, &json!({"error": "upstream"})).await?;
                        return Ok(());
                    }
                }
            } else {
                None
            };
            let (value, read_back) = if let Some(value) = existing {
                (value, true)
            } else {
                let written = auth
                    .pds()
                    .create_record_by_ref(&session, POST, Some(rkey), record.clone())
                    .await;
                match written {
                    Ok(value) => (value, false),
                    Err(PdsError::Store(StoreError::Busy)) => {
                        // Session locking failed before createRecord could be sent.
                        outbox::retry_publication(pool, job, owner).await?;
                        return Ok(());
                    }
                    Err(
                        PdsError::Session
                        | PdsError::Refresh
                        | PdsError::Status(StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN),
                    ) => {
                        outbox::pause(pool, job, owner, &json!({"error": "forbidden"})).await?;
                        return Ok(());
                    }
                    Err(_) => {
                        if let Ok(Some(value)) = auth.pds().get_record_by_ref(&session, POST, rkey).await {
                            (value, true)
                        } else {
                            // The write may still complete. Read it back before
                            // trying the same create-only key again.
                            outbox::reschedule(pool, job, owner, &json!({"error": "upstream"})).await?;
                            return Ok(());
                        }
                    }
                }
            };
            if read_back && value.get("value") != Some(record) {
                outbox::finish(pool, job, owner, true, &json!({"error": "conflict"})).await?;
                return Ok(());
            }
            let expected = format!("at://{}/{POST}/{rkey}", job.account);
            let uri = value.get("uri").and_then(Value::as_str);
            let cid = value.get("cid").and_then(Value::as_str);
            let Some(cid) = cid.filter(|cid| cid.parse::<atproto_dasl::cid::Cid>().is_ok()) else {
                outbox::finish(pool, job, owner, true, &json!({"error": "upstream"})).await?;
                return Ok(());
            };
            if uri != Some(expected.as_str()) {
                outbox::finish(pool, job, owner, true, &json!({"error": "upstream"})).await?;
                return Ok(());
            }
            let admission = json!({ "stage": "admission", "world": world, "uri": expected, "cid": cid });
            if !outbox::advance(pool, job, owner, &admission).await? {
                return Ok(());
            }
            let post = Published {
                world,
                uri: &expected,
                cid,
            };
            submit(pool, auth, client, job, owner, &session, post).await
        }
        Stage::Admission { world, uri, cid } => {
            submit(pool, auth, client, job, owner, &session, Published { world, uri, cid }).await
        }
    }
}

async fn submit(
    pool: &PgPool,
    auth: &auth::Server,
    client: &Client,
    job: &Job,
    owner: &str,
    session: &SessionRef,
    post: Published<'_>,
) -> sqlx::Result<()> {
    let Published { world, uri, cid } = post;
    let pending = json!({"uri": uri, "cid": cid, "status": "pending"});
    if !outbox::may_submit(pool, job, owner).await? {
        outbox::reschedule(pool, job, owner, &pending).await?;
        return Ok(());
    }
    let target = match client.world_target(world).await {
        Ok(target) => target,
        Err(ClientError::InvalidRequest(_)) => {
            outbox::finish(pool, job, owner, true, &json!({"error": "upstream"})).await?;
            return Ok(());
        }
        Err(_) => {
            outbox::retry_admission(pool, job, owner, &pending, "World tenor could not be resolved").await?;
            return Ok(());
        }
    };
    let token = match auth
        .pds()
        .service_token_by_ref(session, target.audience(), SUBMIT)
        .await
    {
        Ok(token) => token,
        Err(
            PdsError::Session | PdsError::Refresh | PdsError::Status(StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN),
        ) => {
            let blocked = json!({"uri": uri, "cid": cid, "status": "pending", "error": "forbidden"});
            outbox::pause(pool, job, owner, &blocked).await?;
            return Ok(());
        }
        Err(_) => {
            outbox::retry_admission(pool, job, owner, &pending, "PDS service token could not be obtained").await?;
            return Ok(());
        }
    };
    let request = json!({"op": job.op_id, "world": world, "post": {"uri": uri, "cid": cid}});
    let answer = client.post(&target, &token, SUBMIT, &request).await;
    let answer = match answer {
        Ok(value) => serde_json::from_value::<Admission>(value).ok(),
        Err(ClientError::Forbidden) => {
            let refused = json!({"uri": uri, "cid": cid, "status": "refused", "reason": "Forbidden"});
            outbox::finish(pool, job, owner, false, &refused).await?;
            return Ok(());
        }
        Err(ClientError::Unauthorized) => {
            outbox::retry_admission(
                pool,
                job,
                owner,
                &pending,
                "Tenor repeatedly rejected the service token",
            )
            .await?;
            return Ok(());
        }
        Err(ClientError::InvalidRequest(_)) => {
            outbox::finish(pool, job, owner, true, &json!({"error": "upstream"})).await?;
            return Ok(());
        }
        Err(ClientError::NotFound | ClientError::Upstream) => {
            outbox::retry_admission(
                pool,
                job,
                owner,
                &pending,
                "Tenor admission service remained unavailable",
            )
            .await?;
            return Ok(());
        }
    };
    let outcome = match answer {
        Some(Admission::Admitted { epoch, seq }) => {
            json!({ "uri": uri, "cid": cid, "status": "admitted", "epoch": epoch, "seq": seq })
        }
        Some(Admission::Refused { reason }) if !reason.is_empty() => {
            json!({ "uri": uri, "cid": cid, "status": "refused", "reason": reason })
        }
        _ => {
            outbox::retry_admission(
                pool,
                job,
                owner,
                &pending,
                "Tenor admission did not reach a final outcome",
            )
            .await?;
            return Ok(());
        }
    };
    outbox::finish(pool, job, owner, false, &outcome).await?;
    Ok(())
}
