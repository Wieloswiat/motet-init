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
use crate::auth::{self, pds::PdsError, store::SessionRef};
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
    world: String,
    scene: String,
    body: String,
    register: String,
}

#[derive(Deserialize)]
#[serde(tag = "stage", rename_all = "snake_case")]
enum Stage<'a> {
    Publish { world: &'a str, rkey: &'a str },
    Publishing {},
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
    let rkey = Tid::new().to_string();
    let record = json!({
        "$type": POST,
        "world": request.world,
        "scene": request.scene,
        "body": request.body,
        "register": request.register,
        "createdAt": Datetime::now().to_string(),
    });
    let world = record.get("world").and_then(Value::as_str).ok_or(XrpcError::Internal)?;
    let payload = json!({ "stage": "publish", "world": world, "rkey": rkey, "record": record });
    let operation = Uuid::now_v7().to_string();
    let id = outbox::enqueue_post(&state.pool, &did, &operation, &payload, &session)
        .await
        .map_err(internal)?;
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
        if let Some(outcome) = current.2 {
            return response(current.0.as_str(), outcome);
        }
        if tokio::time::Instant::now() >= deadline {
            if let Ok(Stage::Admission { uri, cid, .. }) = Stage::deserialize(&current.1) {
                return Ok(Json(json!({ "uri": uri, "cid": cid, "status": "pending" })).into_response());
            }
            // Fence even an in-flight PDS call before reporting failure. If
            // publication won the race, reread its exact URI and CID instead.
            let cancelled = sqlx::query(
                "UPDATE service.outbox SET state = 'failed', outcome = $2, lease_owner = NULL, lease_until = NULL \
                 WHERE id = $1 AND state = 'queued' AND payload->>'stage' IN ('publish', 'publishing')",
            )
            .bind(id)
            .bind(json!({"error": "upstream"}))
            .execute(&state.pool)
            .await
            .map_err(internal)?;
            if cancelled.rows_affected() == 1 {
                return Err(upstream_failure());
            }
            continue;
        }
        tokio::time::sleep(POLL).await;
    }
}

fn response(state: &str, outcome: Value) -> Result<Response, XrpcError> {
    if state == "failed" || outcome.get("error").is_some() {
        return Err(match outcome.get("error").and_then(Value::as_str) {
            Some("forbidden") => forbidden(),
            _ => upstream_failure(),
        });
    }
    Ok(Json(outcome).into_response())
}

/// One fenced attempt. Once a PDS call starts it is never repeated: a lost
/// response cannot establish which CID was committed. Tenor submissions are
/// safe to repeat because the tenor durably deduplicates the same operation.
pub(crate) async fn process(
    pool: &PgPool,
    auth: &auth::Server,
    client: &Client,
    job: &Job,
    owner: &str,
) -> sqlx::Result<()> {
    let Some(session) = job.session_id.as_deref().and_then(SessionRef::from_db) else {
        outbox::finish(pool, job, owner, true, &json!({"error": "forbidden"})).await?;
        return Ok(());
    };
    let Ok(stage) = Stage::deserialize(&job.payload) else {
        outbox::finish(pool, job, owner, true, &json!({"error": "upstream"})).await?;
        return Ok(());
    };
    match stage {
        Stage::Publish { world, rkey } => {
            let Some(record) = job.payload.get("record") else {
                outbox::finish(pool, job, owner, true, &json!({"error": "upstream"})).await?;
                return Ok(());
            };
            if !outbox::mark_publishing(pool, job, owner).await? {
                return Ok(());
            }
            let written = auth
                .pds()
                .create_record_by_ref(&session, POST, Some(rkey), record.clone())
                .await;
            let value = match written {
                Ok(value) => value,
                Err(error) => {
                    let code = if matches!(
                        error,
                        PdsError::Session | PdsError::Status(StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN)
                    ) {
                        "forbidden"
                    } else {
                        "upstream"
                    };
                    outbox::finish(pool, job, owner, true, &json!({"error": code})).await?;
                    return Ok(());
                }
            };
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
        Stage::Publishing {} => {
            // A prior process might have committed the record but lost its
            // response. Without its CID, admission must never be guessed.
            outbox::finish(pool, job, owner, true, &json!({"error": "upstream"})).await?;
            Ok(())
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
            outbox::reschedule(pool, job, owner, &pending).await?;
            return Ok(());
        }
    };
    let token = match auth
        .pds()
        .service_token_by_ref(session, target.audience(), SUBMIT)
        .await
    {
        Ok(token) => token,
        Err(PdsError::Session | PdsError::Status(StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN)) => {
            let blocked = json!({"uri": uri, "cid": cid, "status": "pending", "error": "forbidden"});
            outbox::pause(pool, job, owner, &blocked).await?;
            return Ok(());
        }
        Err(_) => {
            outbox::reschedule(pool, job, owner, &pending).await?;
            return Ok(());
        }
    };
    let request = json!({"op": job.op_id, "world": world, "post": {"uri": uri, "cid": cid}});
    let answer = client.post(&target, &token, SUBMIT, &request).await;
    let answer = match answer {
        Ok(value) => serde_json::from_value::<Admission>(value).ok(),
        Err(ClientError::Forbidden) => {
            outbox::finish(
                pool,
                job,
                owner,
                false,
                &json!({
                    "uri": uri, "cid": cid, "status": "refused", "reason": "Forbidden"
                }),
            )
            .await?;
            return Ok(());
        }
        Err(ClientError::Unauthorized) => {
            let blocked = json!({"uri": uri, "cid": cid, "status": "pending", "error": "forbidden"});
            outbox::pause(pool, job, owner, &blocked).await?;
            return Ok(());
        }
        Err(ClientError::InvalidRequest(_)) => {
            outbox::finish(pool, job, owner, true, &json!({"error": "upstream"})).await?;
            return Ok(());
        }
        Err(ClientError::NotFound | ClientError::Upstream) => None,
    };
    match answer {
        Some(Admission::Admitted { epoch, seq }) => {
            outbox::finish(
                pool,
                job,
                owner,
                false,
                &json!({
                    "uri": uri, "cid": cid, "status": "admitted", "epoch": epoch, "seq": seq
                }),
            )
            .await?;
        }
        Some(Admission::Refused { reason }) if !reason.is_empty() => {
            outbox::finish(
                pool,
                job,
                owner,
                false,
                &json!({
                    "uri": uri, "cid": cid, "status": "refused", "reason": reason
                }),
            )
            .await?;
        }
        _ => {
            outbox::reschedule(pool, job, owner, &pending).await?;
        }
    }
    Ok(())
}
