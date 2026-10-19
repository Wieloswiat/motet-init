//! The AppView's XRPC methods. The SvelteKit app is one client of these;
//! anyone else may call them too. Every method is declared here with who may
//! call it (see `motet-xrpc`).
//!
//! Reads come from the `projection` schema and are filtered by what the
//! reader may see before anything is counted or ranked. Only public records
//! are indexed so far, so the filter is trivial until Spaces arrive.

use std::sync::Arc;

use axum::Json;
use axum::response::{IntoResponse, Response};
use motet_xrpc::server::{Authenticate, Input, Service};
use motet_xrpc::{Access, Caller, Kind, Method, RegistryError, XrpcError};
use serde::Serialize;
use sqlx::PgPool;

#[derive(Debug)]
pub struct State {
    pub pool: PgPool,
}

/// One exact version of a record, as replies and admissions pin them.
pub const GET_RECORD_VERSION: Method = Method {
    nsid: "at.motet.pilot.getRecordVersion",
    kind: Kind::Query,
    access: Access::Public,
    max_body: 0,
};

pub fn router(state: Arc<State>, authenticate: Authenticate) -> Result<axum::Router, RegistryError> {
    Service::new(authenticate)
        .method(GET_RECORD_VERSION, get_record_version)
        .into_router(state)
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
