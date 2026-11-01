//! Public repository reads, CAR proofs and the durable repository firehose.
//!
//! All methods pass through the XRPC registry, including WebSocket upgrades.
//! CAR exports pin one immutable head; the firehose joins stored history to
//! live delivery by sequence number, not by notification timing.

mod car;
mod endpoints;
mod feed;
mod frame;
mod payload;

#[cfg(test)]
mod tests;

use std::sync::Arc;

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use motet_xrpc::server::Service;
use motet_xrpc::{Access, Kind, Method};
use serde::Serialize;

use crate::admission::Records;
use crate::api::State;

/// Registers the public atproto repository and sync methods on the tenor's
/// existing transport. It does not create an unauthenticated side router.
#[must_use]
pub fn methods<R: Records + 'static>(mut service: Service<Arc<State<R>>>) -> Service<Arc<State<R>>> {
    for (nsid, query) in [
        ("com.atproto.repo.getRecord", endpoints::Query::Record),
        ("com.atproto.repo.listRecords", endpoints::Query::Records),
        ("com.atproto.repo.describeRepo", endpoints::Query::Describe),
        ("com.atproto.sync.getRepo", endpoints::Query::Repo),
        ("com.atproto.sync.getRecord", endpoints::Query::Proof),
        ("com.atproto.sync.getLatestCommit", endpoints::Query::Latest),
        ("com.atproto.sync.getRepoStatus", endpoints::Query::Status),
        ("com.atproto.sync.listRepos", endpoints::Query::Repos),
        ("com.atproto.sync.getBlocks", endpoints::Query::Blocks),
    ] {
        service = service.method(public(nsid), move |state: Arc<State<R>>, _, input| async move {
            Ok(endpoints::serve(state.shard.reader(), query, &input.params)
                .await
                .unwrap_or_else(IntoResponse::into_response))
        });
    }
    service.method(
        public("com.atproto.sync.subscribeRepos"),
        |state: Arc<State<R>>, _, input| async move {
            Ok(feed::subscribe(&state.shard, input)
                .await
                .unwrap_or_else(IntoResponse::into_response))
        },
    )
}

fn public(nsid: &'static str) -> Method {
    Method {
        nsid,
        kind: Kind::Query,
        access: Access::Public,
        max_body: 0,
    }
}

/// Exports one committed repository snapshot, or its reachable-block delta
/// from a stored revision. The current signed commit is always included.
pub async fn repo_car(
    reader: &crate::store::Reader,
    world: &motet_rules::Did,
    since: Option<&str>,
) -> Result<Vec<u8>, Error> {
    let head = crate::repo::head(reader, world)
        .await
        .map_err(internal)?
        .ok_or(Error::RepoNotFound)?;
    car::repo(reader, world, &head, since).await
}

/// A repository read refusal or an unavailable storage dependency.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    InvalidRequest(String),
    #[error("repository is not hosted here")]
    RepoNotFound,
    #[error("record not found")]
    RecordNotFound,
    #[error("block not found")]
    BlockNotFound,
    #[error("internal error")]
    Internal,
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        #[derive(Serialize)]
        struct Body {
            error: &'static str,
            message: String,
        }
        let (status, error) = match &self {
            Self::InvalidRequest(_) => (StatusCode::BAD_REQUEST, "InvalidRequest"),
            Self::RepoNotFound => (StatusCode::NOT_FOUND, "RepoNotFound"),
            Self::RecordNotFound => (StatusCode::BAD_REQUEST, "RecordNotFound"),
            Self::BlockNotFound => (StatusCode::BAD_REQUEST, "BlockNotFound"),
            Self::Internal => (StatusCode::INTERNAL_SERVER_ERROR, "InternalServerError"),
        };
        (
            status,
            Json(Body {
                error,
                message: self.to_string(),
            }),
        )
            .into_response()
    }
}

fn internal(error: impl std::fmt::Display) -> Error {
    tracing::error!(error = %error, "repository read failed");
    Error::Internal
}

fn optional<'a>(params: &'a [(String, String)], name: &str) -> Result<Option<&'a str>, Error> {
    let mut matches = params.iter().filter(|(key, _)| key == name);
    let value = matches.next().map(|(_, value)| value.as_str());
    if matches.next().is_some() {
        return Err(Error::InvalidRequest(format!("{name} must not be repeated")));
    }
    Ok(value)
}

fn required<'a>(params: &'a [(String, String)], name: &str) -> Result<&'a str, Error> {
    optional(params, name)?.ok_or_else(|| Error::InvalidRequest(format!("{name} is required")))
}
