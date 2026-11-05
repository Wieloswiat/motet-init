//! The AppView's XRPC methods. The SvelteKit app is one client of these;
//! anyone else may call them too. Every method is declared here with who may
//! call it (see `motet-xrpc`).
//!
//! Reads come from archive-derived public projections. Private Spaces are not
//! indexed or served by these methods.

use std::sync::Arc;

use axum::Json;
use axum::response::{IntoResponse, Response};
use motet_xrpc::server::{Authenticate, Input, Service};
use motet_xrpc::{Access, Caller, Kind, Method, RegistryError, XrpcError};
use serde::Serialize;
use sqlx::PgPool;

use crate::{auth, writes};

#[derive(Debug)]
pub struct State {
    pub pool: PgPool,
    /// Confidential OAuth and authenticated PDS operations for write handlers.
    pub auth: Arc<auth::Server>,
    /// Resolves tenor endpoints and sends member-authenticated writes.
    pub client: Arc<writes::client::Client>,
}

/// One exact version of a record, as replies and admissions pin them.
pub const GET_RECORD_VERSION: Method = Method {
    nsid: "at.motet.pilot.getRecordVersion",
    kind: Kind::Query,
    access: Access::Public,
    max_body: 0,
};

pub const GET_WORLD: Method = Method {
    nsid: "at.motet.pilot.getWorld",
    kind: Kind::Query,
    access: Access::Public,
    max_body: 0,
};

pub const LIST_SCENES: Method = Method {
    nsid: "at.motet.pilot.listScenes",
    kind: Kind::Query,
    access: Access::Public,
    max_body: 0,
};

pub const GET_SCENE: Method = Method {
    nsid: "at.motet.pilot.getScene",
    kind: Kind::Query,
    access: Access::Public,
    max_body: 0,
};

pub const GET_SESSION: Method = Method {
    nsid: "at.motet.pilot.auth.getSession",
    kind: Kind::Query,
    access: Access::AppviewSession,
    max_body: 0,
};

pub const GET_POST_STATUS: Method = Method {
    nsid: "at.motet.pilot.getPostStatus",
    kind: Kind::Query,
    access: Access::AppviewSession,
    max_body: 0,
};

/// Compose the public server. Health and OAuth share the API's admission budget and deadline.
pub fn served_router(
    state: Arc<State>,
    authenticate: Authenticate,
    limits: motet_xrpc::limits::Limits,
) -> Result<axum::Router, RegistryError> {
    let oauth = state.auth.router();
    Ok(router(state, authenticate)?
        .merge(oauth)
        .route("/_health", axum::routing::get(|| async { "ok" }))
        .layer(axum::middleware::from_fn_with_state(
            limits,
            motet_xrpc::limits::enforce,
        )))
}

pub fn router(state: Arc<State>, authenticate: Authenticate) -> Result<axum::Router, RegistryError> {
    let service = Service::new(authenticate)
        .method(GET_RECORD_VERSION, get_record_version)
        .method(GET_WORLD, get_world)
        .method(LIST_SCENES, list_scenes)
        .method(GET_SCENE, get_scene)
        .method(GET_SESSION, get_session)
        .method(GET_POST_STATUS, get_post_status);
    writes::world::register(service)
        .method(writes::post::CREATE_POST, writes::post::create_post)
        .into_router(state)
}

async fn get_session(_state: Arc<State>, caller: Caller, _input: Input) -> Result<Response, XrpcError> {
    let Caller::AppviewSession { did, handle } = caller else {
        return Err(XrpcError::AuthRequired);
    };
    Ok(Json(serde_json::json!({ "did": did, "handle": handle })).into_response())
}

async fn get_post_status(state: Arc<State>, caller: Caller, input: Input) -> Result<Response, XrpcError> {
    let Caller::AppviewSession { did, .. } = caller else {
        return Err(XrpcError::AuthRequired);
    };
    validate_params(&input, &["uri"])?;
    let uri = param(&input, "uri")?;
    let row: Option<(String, serde_json::Value, Option<serde_json::Value>)> = sqlx::query_as(
        "SELECT state, payload, outcome FROM service.outbox \
         WHERE account = $1 AND kind = 'create-post' AND payload->>'stage' = 'admission' \
           AND payload->>'uri' = $2 ORDER BY sequence DESC LIMIT 1",
    )
    .bind(did)
    .bind(uri)
    .fetch_optional(&state.pool)
    .await
    .map_err(|error| {
        tracing::error!(%error, "getPostStatus");
        XrpcError::Internal
    })?;
    let (status, payload, outcome) = row.ok_or(XrpcError::NotFound)?;
    if status == "queued" {
        let cid = payload
            .get("cid")
            .and_then(serde_json::Value::as_str)
            .ok_or(XrpcError::Internal)?;
        return Ok(Json(serde_json::json!({ "uri": uri, "cid": cid, "status": "pending" })).into_response());
    }
    if status == "done" {
        return Ok(Json(outcome.ok_or(XrpcError::Internal)?).into_response());
    }
    let cid = payload
        .get("cid")
        .and_then(serde_json::Value::as_str)
        .ok_or(XrpcError::Internal)?;
    let failed = outcome
        .as_ref()
        .and_then(|value| value.get("error"))
        .and_then(serde_json::Value::as_str)
        .map_or_else(
            || serde_json::json!({ "uri": uri, "cid": cid, "status": "failed" }),
            |reason| serde_json::json!({ "uri": uri, "cid": cid, "status": "failed", "reason": reason }),
        );
    Ok(Json(failed).into_response())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RecordVersion {
    uri: String,
    cid: String,
    /// Whether this is the version the author's repo currently holds.
    current: bool,
    value: serde_json::Value,
}

fn param<'a>(input: &'a Input, name: &str) -> Result<&'a str, XrpcError> {
    input
        .params
        .iter()
        .find(|(k, _)| k == name)
        .map(|(_, v)| v.as_str())
        .ok_or_else(|| XrpcError::InvalidRequest(format!("{name} is required")))
}

async fn get_record_version(state: Arc<State>, _caller: Caller, input: Input) -> Result<Response, XrpcError> {
    let (uri, cid) = (param(&input, "uri")?, param(&input, "cid")?);
    let row: Option<(serde_json::Value, bool)> = sqlx::query_as(
        "SELECT v.record, r.cid IS NOT DISTINCT FROM v.cid \
         FROM projection.record_versions v JOIN projection.records r USING (uri) \
         WHERE v.uri = $1 AND v.cid = $2",
    )
    .bind(uri)
    .bind(cid)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| {
        tracing::error!(error = %e, "getRecordVersion");
        XrpcError::Internal
    })?;
    // Never seen is "this version isn't available", never the current text
    // in its place.
    let (value, current) = row.ok_or(XrpcError::NotFound)?;
    Ok(Json(RecordVersion {
        uri: uri.to_owned(),
        cid: cid.to_owned(),
        current,
        value,
    })
    .into_response())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct World {
    did: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    handle: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    display_name: Option<String>,
}

#[derive(Serialize)]
struct Record {
    uri: String,
    cid: String,
    value: serde_json::Value,
}

#[derive(Serialize)]
struct StrongRef {
    uri: String,
    cid: String,
}

#[derive(Serialize)]
struct Place {
    epoch: i64,
    seq: i64,
}

#[derive(Serialize)]
struct AdmittedPost {
    admission: StrongRef,
    uri: String,
    cid: String,
    author: String,
    place: Place,
    value: serde_json::Value,
}

#[derive(sqlx::FromRow)]
struct PostRow {
    admission_uri: String,
    admission_cid: String,
    post_uri: String,
    post_cid: String,
    author: String,
    epoch: i64,
    place_seq: i64,
    record: serde_json::Value,
}

#[derive(Serialize)]
struct Scenes {
    scenes: Vec<Record>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cursor: Option<String>,
}

#[derive(Serialize)]
struct ScenePage {
    world: World,
    scene: Record,
    posts: Vec<AdmittedPost>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cursor: Option<String>,
}

fn invalid(name: &str) -> XrpcError {
    XrpcError::InvalidRequest(format!("invalid {name}"))
}

fn validate_params(input: &Input, allowed: &[&str]) -> Result<(), XrpcError> {
    for (index, (name, _)) in input.params.iter().enumerate() {
        if !allowed.contains(&name.as_str()) || input.params.iter().take(index).any(|(key, _)| key == name) {
            return Err(invalid(name));
        }
    }
    Ok(())
}

fn optional_param<'a>(input: &'a Input, name: &str) -> Option<&'a str> {
    input
        .params
        .iter()
        .find(|(key, _)| key == name)
        .map(|(_, value)| value.as_str())
}

fn limit(input: &Input) -> Result<usize, XrpcError> {
    match optional_param(input, "limit") {
        None => Ok(50),
        Some(value) if !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit()) => value
            .parse::<usize>()
            .ok()
            .filter(|n| (1..=100).contains(n))
            .ok_or_else(|| invalid("limit")),
        Some(_) => Err(invalid("limit")),
    }
}

fn valid_did(value: &str) -> bool {
    let Some((method, id)) = value.strip_prefix("did:").and_then(|v| v.split_once(':')) else {
        return false;
    };
    if value.len() > 2048
        || method.is_empty()
        || !method.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        || id.is_empty()
        || id.ends_with(':')
    {
        return false;
    }
    let mut bytes = id.bytes();
    while let Some(byte) = bytes.next() {
        if byte == b'%' {
            if !bytes.next().is_some_and(|b| b.is_ascii_hexdigit())
                || !bytes.next().is_some_and(|b| b.is_ascii_hexdigit())
            {
                return false;
            }
        } else if !(byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_' | b':')) {
            return false;
        }
    }
    true
}

fn valid_handle(value: &str) -> bool {
    if value.len() > 253 || !value.contains('.') {
        return false;
    }
    let mut labels = value.split('.');
    if !labels.all(|label| {
        !label.is_empty()
            && label.len() <= 63
            && label.bytes().next().is_some_and(|b| b.is_ascii_alphanumeric())
            && label.bytes().last().is_some_and(|b| b.is_ascii_alphanumeric())
            && label.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
    }) {
        return false;
    }
    value
        .rsplit('.')
        .next()
        .and_then(|label| label.bytes().next())
        .is_some_and(|b| b.is_ascii_alphabetic())
}

fn valid_rkey(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 512
        && value != "."
        && value != ".."
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'~' | b'-' | b':'))
}

fn page_cursor(value: &str) -> Result<(i64, i64), XrpcError> {
    let parse = |part: &str| {
        if part.is_empty() || !part.bytes().all(|b| b.is_ascii_digit()) {
            return Err(invalid("cursor"));
        }
        part.parse::<i64>().map_err(|_| invalid("cursor"))
    };
    let (epoch, seq) = value.split_once(':').ok_or_else(|| invalid("cursor"))?;
    Ok((parse(epoch)?, parse(seq)?))
}

fn database_error(error: &sqlx::Error) -> XrpcError {
    tracing::error!(%error, "public projection query");
    XrpcError::Internal
}

async fn world(pool: &PgPool, actor: &str) -> Result<World, XrpcError> {
    if valid_did(actor) {
        let row: Option<(String, Option<String>, Option<String>)> =
            sqlx::query_as("SELECT did, handle, display_name FROM projection.worlds WHERE did = $1")
                .bind(actor)
                .fetch_optional(pool)
                .await
                .map_err(|error| database_error(&error))?;
        let (did, handle, display_name) = row.ok_or(XrpcError::NotFound)?;
        return Ok(World {
            did,
            handle,
            display_name,
        });
    }
    if !valid_handle(actor) {
        return Err(invalid("actor"));
    }
    // A conflicting handle must not select an arbitrary World.
    let rows: Vec<(String, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT did, handle, display_name FROM projection.worlds \
         WHERE lower(handle COLLATE \"C\") = lower($1 COLLATE \"C\") ORDER BY did COLLATE \"C\" LIMIT 2",
    )
    .bind(actor)
    .fetch_all(pool)
    .await
    .map_err(|error| database_error(&error))?;
    let [(did, handle, display_name)] = <Vec<_> as TryInto<[_; 1]>>::try_into(rows).map_err(|_| XrpcError::NotFound)?;
    Ok(World {
        did,
        handle,
        display_name,
    })
}

async fn get_world(state: Arc<State>, _caller: Caller, input: Input) -> Result<Response, XrpcError> {
    validate_params(&input, &["actor"])?;
    Ok(Json(world(&state.pool, param(&input, "actor")?).await?).into_response())
}

async fn list_scenes(state: Arc<State>, _caller: Caller, input: Input) -> Result<Response, XrpcError> {
    validate_params(&input, &["actor", "limit", "cursor"])?;
    let limit = limit(&input)?;
    let cursor = optional_param(&input, "cursor");
    if cursor.is_some_and(|value| !valid_rkey(value)) {
        return Err(invalid("cursor"));
    }
    let world = world(&state.pool, param(&input, "actor")?).await?;
    let mut rows: Vec<(String, String, String, serde_json::Value)> = sqlx::query_as(
        "SELECT rkey, uri, cid, record FROM projection.scenes \
         WHERE world_did = $1 AND ($2::text IS NULL OR rkey COLLATE \"C\" > $2 COLLATE \"C\") \
         ORDER BY rkey COLLATE \"C\" LIMIT $3",
    )
    .bind(&world.did)
    .bind(cursor)
    .bind(i64::try_from(limit + 1).map_err(|_| XrpcError::Internal)?)
    .fetch_all(&state.pool)
    .await
    .map_err(|error| database_error(&error))?;
    let more = rows.len() > limit;
    if more {
        rows.pop();
    }
    let cursor = if more {
        rows.last().map(|row| row.0.clone())
    } else {
        None
    };
    let scenes = rows
        .into_iter()
        .map(|(_, uri, cid, value)| Record { uri, cid, value })
        .collect();
    Ok(Json(Scenes { scenes, cursor }).into_response())
}

async fn get_scene(state: Arc<State>, _caller: Caller, input: Input) -> Result<Response, XrpcError> {
    validate_params(&input, &["actor", "scene", "limit", "cursor"])?;
    let limit = limit(&input)?;
    let scene = param(&input, "scene")?;
    if !valid_rkey(scene) {
        return Err(invalid("scene"));
    }
    let cursor = optional_param(&input, "cursor").map(page_cursor).transpose()?;
    let world = world(&state.pool, param(&input, "actor")?).await?;
    let row: Option<(String, String, serde_json::Value)> =
        sqlx::query_as("SELECT uri, cid, record FROM projection.scenes WHERE world_did = $1 AND rkey = $2")
            .bind(&world.did)
            .bind(scene)
            .fetch_optional(&state.pool)
            .await
            .map_err(|error| database_error(&error))?;
    let (uri, cid, value) = row.ok_or(XrpcError::NotFound)?;
    let scene_record = Record { uri, cid, value };
    let mut rows: Vec<PostRow> = sqlx::query_as(
        "SELECT admission_uri, admission_cid, post_uri, post_cid, author, epoch, place_seq, record \
         FROM projection.admitted_posts WHERE world_did = $1 AND scene = $2 \
         AND ($3::bigint IS NULL OR (epoch, place_seq) > ($3, $4)) \
         ORDER BY epoch, place_seq LIMIT $5",
    )
    .bind(&world.did)
    .bind(scene)
    .bind(cursor.map(|c| c.0))
    .bind(cursor.map(|c| c.1))
    .bind(i64::try_from(limit + 1).map_err(|_| XrpcError::Internal)?)
    .fetch_all(&state.pool)
    .await
    .map_err(|error| database_error(&error))?;
    let more = rows.len() > limit;
    if more {
        rows.pop();
    }
    let cursor = if more {
        rows.last().map(|row| format!("{}:{}", row.epoch, row.place_seq))
    } else {
        None
    };
    let posts = rows
        .into_iter()
        .map(|row| AdmittedPost {
            admission: StrongRef {
                uri: row.admission_uri,
                cid: row.admission_cid,
            },
            uri: row.post_uri,
            cid: row.post_cid,
            author: row.author,
            place: Place {
                epoch: row.epoch,
                seq: row.place_seq,
            },
            value: row.record,
        })
        .collect();
    Ok(Json(ScenePage {
        world,
        scene: scene_record,
        posts,
        cursor,
    })
    .into_response())
}
