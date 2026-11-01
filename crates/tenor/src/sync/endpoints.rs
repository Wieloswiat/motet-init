//! Atproto's JSON reads and CAR responses over committed shard state.

use std::collections::{BTreeSet, VecDeque};

use atproto_dasl::{BlockStorage, Cid};
use atproto_record::tid::Tid;
use atproto_repo::{Mst, RepoConfig};
use axum::Json;
use axum::http::header;
use axum::response::{IntoResponse, Response};
use bytes::Bytes;
use motet_rules::Did;
use serde::Serialize;
use serde_json::{Value, json};

use crate::identity::{self, IdentityError};
use crate::repo::{self, Head};
use crate::store::{Reader, keys};
use crate::world;

use super::{Error, car, internal, optional, required};

const PAGE: usize = 256;

type Params = [(String, String)];

#[derive(Clone, Copy)]
pub(super) enum Query {
    Record,
    Records,
    Describe,
    Repo,
    Proof,
    Latest,
    Status,
    Repos,
    Blocks,
}

pub(super) async fn serve(reader: &Reader, query: Query, params: &Params) -> Result<Response, Error> {
    let result = match query {
        Query::Record => record(reader, params).await,
        Query::Records => records(reader, params).await,
        Query::Describe => describe(reader, params).await,
        Query::Repo => export(reader, params).await,
        Query::Proof => proof(reader, params).await,
        Query::Latest => latest(reader, params).await,
        Query::Status => status(reader, params).await,
        Query::Repos => repos(reader, params).await,
        Query::Blocks => blocks(reader, params).await,
    };
    // These repo lexicons do not declare RepoNotFound. A missing account is
    // distinct from a missing record, which getRecord does declare.
    match result {
        Err(Error::RepoNotFound) if matches!(query, Query::Record | Query::Records | Query::Describe) => {
            Err(Error::InvalidRequest("repository is not hosted here".into()))
        }
        other => other,
    }
}

async fn resolve(reader: &Reader, identifier: &str) -> Result<Did, Error> {
    if identifier.starts_with("did:") {
        return Ok(Did::new(identifier));
    }
    identity::resolve_handle(reader, identifier)
        .await
        .map_err(|error| match error {
            IdentityError::HandleNotFound => Error::RepoNotFound,
            IdentityError::InvalidHandle => Error::InvalidRequest(error.to_string()),
            other => internal(other),
        })
}

async fn repository(reader: &Reader, identifier: &str) -> Result<(Did, Head), Error> {
    let did = resolve(reader, identifier).await?;
    let head = repo::head(reader, &did)
        .await
        .map_err(internal)?
        .ok_or(Error::RepoNotFound)?;
    Ok((did, head))
}

fn collection(params: &Params) -> Result<&str, Error> {
    let collection = required(params, "collection")?;
    atproto_record::nsid::check_nsid(collection).map_err(|error| Error::InvalidRequest(error.into()))?;
    Ok(collection)
}

fn rkey(params: &Params) -> Result<&str, Error> {
    let rkey = required(params, "rkey")?;
    atproto_record::record_key::check_record_key(rkey).map_err(|error| Error::InvalidRequest(error.into()))?;
    Ok(rkey)
}

fn parse_cid(value: &str) -> Result<Cid, Error> {
    value.parse().map_err(|_| Error::InvalidRequest("invalid CID".into()))
}

fn limit(params: &Params, default: usize, maximum: usize) -> Result<usize, Error> {
    let Some(value) = optional(params, "limit")? else {
        return Ok(default);
    };
    let value = value
        .parse::<usize>()
        .map_err(|_| Error::InvalidRequest("limit must be an integer".into()))?;
    if !(1..=maximum).contains(&value) {
        return Err(Error::InvalidRequest(format!("limit must be between 1 and {maximum}")));
    }
    Ok(value)
}

#[derive(Serialize)]
struct Record {
    uri: String,
    cid: String,
    value: Value,
}

async fn record_value(reader: &Reader, did: &Did, collection: &str, rkey: &str, cid: Cid) -> Result<Record, Error> {
    let bytes = reader
        .blocks(did)
        .get(&cid)
        .await
        .map_err(internal)?
        .ok_or_else(|| internal("record index points to a missing block"))?;
    Ok(Record {
        uri: format!("at://{did}/{collection}/{rkey}"),
        cid: cid.to_string(),
        value: atproto_dasl::atproto_json::from_slice(&bytes).map_err(internal)?,
    })
}

async fn record(reader: &Reader, params: &Params) -> Result<Response, Error> {
    let collection = collection(params)?;
    let rkey = rkey(params)?;
    let requested = optional(params, "cid")?.map(parse_cid).transpose()?;
    let (did, head) = repository(reader, required(params, "repo")?).await?;
    let mst = Mst::from_root(head.data, reader.blocks(&did), RepoConfig::default());
    let cid = mst
        .get(&format!("{collection}/{rkey}"))
        .await
        .map_err(internal)?
        .ok_or(Error::RecordNotFound)?;
    if requested.is_some_and(|requested| requested != cid) {
        return Err(Error::RecordNotFound);
    }
    Ok(Json(record_value(reader, &did, collection, rkey, cid).await?).into_response())
}

/// SlateDB 0.17 exposes ascending scans only. Keep one bounded page for a
/// descending request, rather than allocating every record in a collection.
async fn record_page(
    reader: &Reader,
    prefix: &[u8],
    cursor: Option<&[u8]>,
    ascending: bool,
    count: usize,
) -> Result<Vec<(Bytes, Bytes)>, Error> {
    if ascending {
        return reader.scan(prefix, cursor, count).await.map_err(internal);
    }
    let mut selected = VecDeque::with_capacity(count);
    let mut after = None;
    loop {
        let page = reader.scan(prefix, after.as_deref(), PAGE).await.map_err(internal)?;
        let complete = page.len() < PAGE;
        after = page.last().map(|(key, _)| key.clone());
        for (key, value) in page {
            if cursor.is_some_and(|cursor| key.as_ref() >= cursor) {
                selected.make_contiguous().reverse();
                return Ok(selected.into());
            }
            if selected.len() == count {
                selected.pop_front();
            }
            selected.push_back((key, value));
        }
        if complete {
            selected.make_contiguous().reverse();
            return Ok(selected.into());
        }
    }
}

async fn records(reader: &Reader, params: &Params) -> Result<Response, Error> {
    #[derive(Serialize)]
    struct Page {
        records: Vec<Record>,
        #[serde(skip_serializing_if = "Option::is_none")]
        cursor: Option<String>,
    }
    let collection = collection(params)?;
    let limit = limit(params, 50, 100)?;
    let ascending = match optional(params, "reverse")? {
        None | Some("false") => false,
        Some("true") => true,
        Some(_) => return Err(Error::InvalidRequest("reverse must be a boolean".into())),
    };
    let (did, _) = repository(reader, required(params, "repo")?).await?;
    let prefix = keys::repo_records(&did, Some(collection));
    let cursor = optional(params, "cursor")?;
    if let Some(cursor) = cursor {
        atproto_record::record_key::check_record_key(cursor).map_err(|error| Error::InvalidRequest(error.into()))?;
    }
    let after = cursor.map(|cursor| keys::repo_record(&did, collection, cursor));
    let rows = record_page(reader, &prefix, after.as_deref(), ascending, limit + 1).await?;
    let more = rows.len() > limit;
    let mut records = Vec::with_capacity(rows.len().min(limit));
    let mut cursor = None;
    for (key, value) in rows.into_iter().take(limit) {
        let rkey = key.strip_prefix(prefix.as_slice()).ok_or(Error::Internal)?;
        let rkey = std::str::from_utf8(rkey).map_err(internal)?;
        let cid = Cid::from_bytes(&value).map_err(internal)?;
        records.push(record_value(reader, &did, collection, rkey, cid).await?);
        if more && records.len() == limit {
            cursor = Some(rkey.to_owned());
        }
    }
    Ok(Json(Page { records, cursor }).into_response())
}

async fn describe(reader: &Reader, params: &Params) -> Result<Response, Error> {
    let (did, _) = repository(reader, required(params, "repo")?).await?;
    let info = world::get(reader, &did)
        .await
        .map_err(internal)?
        .ok_or(Error::RepoNotFound)?;
    let correct = match identity::resolve_handle(reader, &info.handle).await {
        Ok(resolved) => resolved == did,
        Err(IdentityError::HandleNotFound | IdentityError::InvalidHandle) => false,
        Err(error) => return Err(internal(error)),
    };
    let prefix = keys::repo_records(&did, None);
    let mut after = None;
    let mut collections = BTreeSet::new();
    loop {
        let page = reader.scan(&prefix, after.as_deref(), PAGE).await.map_err(internal)?;
        let complete = page.len() < PAGE;
        after = page.last().map(|(key, _)| key.clone());
        for (key, _) in page {
            let path = key.strip_prefix(prefix.as_slice()).ok_or(Error::Internal)?;
            let path = std::str::from_utf8(path).map_err(internal)?;
            let (collection, _) = path.split_once('/').ok_or(Error::Internal)?;
            if !collections.contains(collection) {
                collections.insert(collection.to_owned());
            }
        }
        if complete {
            break;
        }
    }
    Ok(Json(json!({
        "did": did.as_str(), "handle": info.handle, "didDoc": info.document,
        "collections": collections, "handleIsCorrect": correct,
    }))
    .into_response())
}

fn car_response(bytes: Vec<u8>) -> Response {
    ([(header::CONTENT_TYPE, "application/vnd.ipld.car")], bytes).into_response()
}

async fn export(reader: &Reader, params: &Params) -> Result<Response, Error> {
    let since = optional(params, "since")?;
    if let Some(since) = since {
        since
            .parse::<Tid>()
            .map_err(|_| Error::InvalidRequest("since must be a repository revision".into()))?;
    }
    let (did, head) = repository(reader, required(params, "did")?).await?;
    Ok(car_response(car::repo(reader, &did, &head, since).await?))
}

async fn proof(reader: &Reader, params: &Params) -> Result<Response, Error> {
    let path = format!("{}/{}", collection(params)?, rkey(params)?);
    let (did, head) = repository(reader, required(params, "did")?).await?;
    Ok(car_response(car::proof(reader, &did, &head, &path).await?))
}

async fn latest(reader: &Reader, params: &Params) -> Result<Response, Error> {
    let (_, head) = repository(reader, required(params, "did")?).await?;
    Ok(Json(json!({ "cid": head.cid.to_string(), "rev": head.rev.to_string() })).into_response())
}

async fn status(reader: &Reader, params: &Params) -> Result<Response, Error> {
    let (did, head) = repository(reader, required(params, "did")?).await?;
    let info = world::get(reader, &did)
        .await
        .map_err(internal)?
        .ok_or(Error::RepoNotFound)?;
    let world::Status::Active = info.status;
    Ok(Json(json!({ "did": did.as_str(), "active": true, "rev": head.rev.to_string() })).into_response())
}

async fn repos(reader: &Reader, params: &Params) -> Result<Response, Error> {
    #[derive(Serialize)]
    struct Repo {
        did: String,
        head: String,
        rev: String,
        active: bool,
    }
    #[derive(Serialize)]
    struct Page {
        repos: Vec<Repo>,
        #[serde(skip_serializing_if = "Option::is_none")]
        cursor: Option<String>,
    }
    let limit = limit(params, 500, 1000)?;
    let cursor = optional(params, "cursor")?;
    if cursor.is_some_and(|cursor| !cursor.starts_with("did:") || cursor.contains('/')) {
        return Err(Error::InvalidRequest("invalid repository cursor".into()));
    }
    let mut after = cursor.map(|cursor| Bytes::from(keys::repo_head(&Did::new(cursor))));
    let mut repos = Vec::with_capacity(limit + 1);
    'pages: loop {
        let rows = reader
            .scan(keys::REPO_HEADS, after.as_deref(), PAGE)
            .await
            .map_err(internal)?;
        let complete = rows.len() < PAGE;
        after = rows.last().map(|(key, _)| key.clone());
        for (key, value) in rows {
            let suffix = key.strip_prefix(keys::REPO_HEADS).ok_or(Error::Internal)?;
            if suffix.contains(&b'/') {
                continue;
            }
            let did = std::str::from_utf8(suffix).map_err(internal)?;
            let head: Head = serde_json::from_slice(&value).map_err(internal)?;
            let info = world::get(reader, &Did::new(did))
                .await
                .map_err(internal)?
                .ok_or(Error::Internal)?;
            let world::Status::Active = info.status;
            repos.push(Repo {
                did: did.into(),
                head: head.cid.to_string(),
                rev: head.rev.to_string(),
                active: true,
            });
            if repos.len() > limit {
                break 'pages;
            }
        }
        if complete {
            break;
        }
    }
    let more = repos.len() > limit;
    repos.truncate(limit);
    let cursor = if more {
        repos.last().map(|repo| repo.did.clone())
    } else {
        None
    };
    Ok(Json(Page { repos, cursor }).into_response())
}

async fn blocks(reader: &Reader, params: &Params) -> Result<Response, Error> {
    let mut cids = params
        .iter()
        .filter(|(name, _)| name == "cids")
        .map(|(_, value)| parse_cid(value))
        .collect::<Result<Vec<_>, _>>()?;
    if cids.is_empty() {
        return Err(Error::InvalidRequest(
            "cids is required; repeat it for each block".into(),
        ));
    }
    cids.sort_unstable();
    cids.dedup();
    let (did, _) = repository(reader, required(params, "did")?).await?;
    Ok(car_response(car::blocks(reader, &did, cids).await?))
}
