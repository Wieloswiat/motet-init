//! Registered World Space methods and their bound-credential boundary.
//!
//! Wire shapes, pagination and MAC ordering follow atproto-pds
//! `http/space_handlers.rs`, `space/reader.rs` and `http/space_mac.rs` at
//! 09a868cd (MIT). Storage, authorization and mutations use Motet's shard.
//!
//! `listRecords` applies one limit across ascending collections; `reverse`
//! changes record order within each collection. Whole-Space cursors join the
//! last collection and rkey with U+0001. A collection alone resumes at its start.
//! Revocation notifications carry credential JTIs in the `credentials` array.

use std::sync::Arc;

use atproto_identity::traits::IdentityResolver;
use atproto_oauth::scopes::{SpaceAction, SpaceManageVerb};
use atproto_record::tid::Tid;
use atproto_space::commit::{Commit, CommitContext, create_commit};
use atproto_space::credential::{DELEGATION_TOKEN_TTL_SECS, create_delegation_token};
use atproto_space::kem::SharedSecret;
use atproto_space::mac::{MAC_HEADER, MacType, WANT_MAC_HEADER, format_mac_header, record_mac};
use atproto_space::set_hash::{LtHash, SetHash};
use atproto_space::space_repo::OpAction;
use atproto_space::storage::{OplogCursor, RepoState, SpaceRepoStorage};
use atproto_space::sync::RepoStarWriter;
use atproto_space::{RecordUri, SpaceUri};
use axum::Json;
use axum::body::Body;
use axum::extract::Request;
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use motet_lexicons::group_opensocial_access::Access as AccessRecord;
use motet_rules::Did;
use motet_xrpc::server::{Authenticate, Input, Service};
use motet_xrpc::{Access, Caller, Kind, Method, XrpcError};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

use super::credentials::{self, CredentialError, VerifiedCredential};
use super::storage::TxnStorage;
use super::{SpaceError, SpaceWrite, notify};
use crate::oauth::grants::{Policy as SessionPolicy, Session};
use crate::shard::{Handle, Op, Txn};
use crate::store::keys;

/// Shared by registered handlers; the identity resolver must use motet-net.
pub struct State {
    pub shard: Handle,
    pub resolver: Arc<dyn IdentityResolver>,
    pub fetcher: motet_net::Fetcher,
}

impl std::fmt::Debug for State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SpaceHttpState")
            .field("shard", &self.shard)
            .finish_non_exhaustive()
    }
}

const fn query(nsid: &'static str, access: Access) -> Method {
    Method {
        nsid,
        kind: Kind::Query,
        access,
        max_body: 0,
    }
}

const fn procedure(nsid: &'static str, access: Access, max_body: usize) -> Method {
    Method {
        nsid,
        kind: Kind::Procedure,
        access,
        max_body,
    }
}

// Simplespace create/update/putMember/removeMember are not declared: their
// imperative per-member ACL would compete with opensocial's role records.
// Group creation/deletion and conditional access/membership record writes
// remain the one management path; getSpace/listMembers derive their view.
const METHODS: &[Method] = &[
    procedure("com.atproto.space.getSpaceCredential", Access::Public, 64 * 1024),
    query("com.atproto.space.getDelegationToken", Access::Session),
    query("com.atproto.space.listRepos", Access::SpaceCredential),
    query("com.atproto.space.getRepo", Access::SpaceCredential),
    query("com.atproto.space.getLatestCommit", Access::SpaceCredential),
    query("com.atproto.space.listRepoOps", Access::SpaceCredential),
    query("com.atproto.space.getRecord", Access::SpaceCredential),
    query("com.atproto.space.listRecords", Access::SpaceCredential),
    query("com.atproto.space.getBlob", Access::SpaceCredential),
    query("com.atproto.space.listBlobs", Access::SpaceCredential),
    procedure("com.atproto.space.registerNotify", Access::SpaceCredential, 4096),
    procedure("com.atproto.space.unregisterNotify", Access::SpaceCredential, 4096),
    procedure("com.atproto.space.notifyWrite", Access::Service, 4096),
    procedure("com.atproto.space.notifySpaceDeleted", Access::Service, 4096),
    procedure("com.atproto.space.notifyCredentialRevoked", Access::Service, 64 * 1024),
    procedure(
        "com.atproto.space.createRecord",
        Access::ServiceOrSession,
        2 * 1024 * 1024,
    ),
    procedure("com.atproto.space.putRecord", Access::ServiceOrSession, 2 * 1024 * 1024),
    procedure("com.atproto.space.deleteRecord", Access::ServiceOrSession, 4096),
    procedure(
        "com.atproto.space.applyWrites",
        Access::ServiceOrSession,
        4 * 1024 * 1024,
    ),
    query("com.atproto.space.listSpaces", Access::Service),
    procedure("group.opensocial.createSpace", Access::ServiceOrSession, 64 * 1024),
    procedure("group.opensocial.deleteSpace", Access::ServiceOrSession, 4096),
    procedure("com.atproto.simplespace.deleteSpace", Access::ServiceOrSession, 4096),
    query("com.atproto.simplespace.getSpace", Access::SpaceCredential),
    query("com.atproto.simplespace.listMembers", Access::Service),
];

/// Adds every implemented Space method to the host's one fail-closed registry.
/// Each handler retains a shared reference to the borrowed state.
#[must_use]
pub fn register<S: Clone + Send + Sync + 'static>(mut service: Service<S>, state: &Arc<State>) -> Service<S> {
    for method in METHODS {
        let state = Arc::clone(state);
        let nsid = method.nsid;
        service = service.method(method.clone(), move |_, caller, input| {
            dispatch(Arc::clone(&state), nsid, caller, input)
        });
    }
    service
}

/// Wraps the host authenticator. A malformed or forged Space proof never
/// falls through to anonymous, account, or service-auth admission.
#[must_use]
pub fn authenticate(next: Authenticate, state: Arc<State>) -> Authenticate {
    Arc::new(move |request| {
        let is_space = request.headers.get_all(header::AUTHORIZATION).iter().any(|value| {
            value
                .to_str()
                .ok()
                .and_then(|value| value.split_whitespace().next())
                .is_some_and(|scheme| scheme.eq_ignore_ascii_case(atproto_space::http_signature::AUTH_SCHEME))
        });
        if !is_space {
            return next(request);
        }
        let state = Arc::clone(&state);
        Box::pin(async move {
            let verified = credentials::authenticate_http(state.shard.reader(), request.headers)
                .await
                .map_err(credential_error)?;
            Ok(Caller::SpaceCredential {
                space: verified.space().to_string(),
                audience: verified.audience().to_string(),
            })
        })
    })
}

/// Layer this outside the router so even authentication and query refusals
/// vary on the request's MAC key share, as required by the alpha.
pub async fn vary_mac(request: Request, next: Next) -> Response {
    let needs_mac = matches!(
        request.uri().path(),
        "/xrpc/com.atproto.space.getRepo"
            | "/xrpc/com.atproto.space.getLatestCommit"
            | "/xrpc/com.atproto.space.listRepoOps"
            | "/xrpc/com.atproto.space.getRecord"
    );
    let mut response = next.run(request).await;
    if needs_mac {
        response
            .headers_mut()
            .append(header::VARY, HeaderValue::from_static(WANT_MAC_HEADER));
        response.headers_mut().append(
            header::ACCESS_CONTROL_EXPOSE_HEADERS,
            HeaderValue::from_static(MAC_HEADER),
        );
    }
    response
}

async fn dispatch(state: Arc<State>, nsid: &str, caller: Caller, input: Input) -> Result<Response, XrpcError> {
    match nsid {
        "com.atproto.space.getSpaceCredential" => mint(&state, &input).await,
        "com.atproto.space.getDelegationToken" => delegation(&state, caller, &input).await,
        "com.atproto.space.listRepos" => list_repos(&state, &input).await,
        "com.atproto.space.getRepo"
        | "com.atproto.space.getLatestCommit"
        | "com.atproto.space.listRepoOps"
        | "com.atproto.space.getRecord"
        | "com.atproto.space.listRecords"
        | "com.atproto.space.getBlob"
        | "com.atproto.space.listBlobs" => read_repo(&state, nsid, input).await,
        "com.atproto.space.registerNotify" | "com.atproto.space.unregisterNotify" => {
            registration(&state, nsid, input).await
        }
        "com.atproto.space.notifyWrite"
        | "com.atproto.space.notifySpaceDeleted"
        | "com.atproto.space.notifyCredentialRevoked" => receive_notify(&state, nsid, caller, &input).await,
        "com.atproto.space.createRecord"
        | "com.atproto.space.putRecord"
        | "com.atproto.space.deleteRecord"
        | "com.atproto.space.applyWrites" => write_records(&state, nsid, caller, &input).await,
        "com.atproto.space.listSpaces" => list_spaces(&state, caller, &input).await,
        "group.opensocial.createSpace" => create_space(&state, caller, &input).await,
        "group.opensocial.deleteSpace" => delete_space(&state, caller, &input, DeleteMode::Group).await,
        "com.atproto.simplespace.deleteSpace" => delete_space(&state, caller, &input, DeleteMode::Simple).await,
        "com.atproto.simplespace.getSpace" => describe_space(&state, &input).await,
        "com.atproto.simplespace.listMembers" => list_members(&state, caller, &input).await,
        _ => Err(XrpcError::MethodNotImplemented),
    }
}

fn protocol(status: StatusCode, name: &'static str, message: impl Into<String>) -> XrpcError {
    XrpcError::Protocol {
        status,
        name,
        message: message.into(),
    }
}

fn invalid(message: impl Into<String>) -> XrpcError {
    XrpcError::InvalidRequest(message.into())
}

fn internal(error: impl std::fmt::Display) -> XrpcError {
    tracing::error!(error = %error, "Space operation failed");
    XrpcError::Internal
}

fn credential_error(error: CredentialError) -> XrpcError {
    let name = error.error_name();
    let status = match name {
        "SpaceNotFound"
        | "SpaceDeleted"
        | "InvalidDelegationToken"
        | "InvalidClientAttestation"
        | "InvalidRequest"
        | "BadSpaceMac"
        | "InvalidSwap" => StatusCode::BAD_REQUEST,
        "UserNotAuthorized" | "AppNotAuthorized" | "NotAuthorized" => StatusCode::FORBIDDEN,
        "KemKeyUnavailable" => StatusCode::INTERNAL_SERVER_ERROR,
        "InternalError" | "InternalServerError" => return internal(error),
        _ => StatusCode::UNAUTHORIZED,
    };
    protocol(status, name, error.to_string())
}

fn space_error(error: SpaceError) -> XrpcError {
    match error {
        SpaceError::NotFound | SpaceError::Deleted => {
            protocol(StatusCode::BAD_REQUEST, "SpaceNotFound", "Space not found")
        }
        SpaceError::NotAuthorized => XrpcError::AuthRequired,
        SpaceError::Conflict => protocol(StatusCode::BAD_REQUEST, "InvalidSwap", "the expected record changed"),
        SpaceError::FutureRev => protocol(
            StatusCode::BAD_REQUEST,
            "FutureRev",
            "notification revision is too far in the future",
        ),
        SpaceError::ServiceNotResolvable(message) => protocol(StatusCode::BAD_REQUEST, "ServiceNotResolvable", message),
        SpaceError::Protocol(error) => protocol_error(error),
        SpaceError::Invalid(message)
        | SpaceError::Repo(
            crate::repo::RepoError::InvalidRecord(message) | crate::repo::RepoError::UnknownCollection(message),
        ) => invalid(message),
        other => internal(other),
    }
}

fn protocol_error(error: atproto_space::SpaceError) -> XrpcError {
    match error {
        atproto_space::SpaceError::Storage { .. } => internal(error),
        atproto_space::SpaceError::OplogGap { .. } => protocol(StatusCode::BAD_REQUEST, "OplogGap", error.to_string()),
        _ => invalid(error.to_string()),
    }
}

fn body<T: DeserializeOwned>(input: &Input) -> Result<T, XrpcError> {
    serde_json::from_slice(&input.body).map_err(|error| invalid(error.to_string()))
}

fn parameter<'a>(input: &'a Input, name: &str) -> Result<Option<&'a str>, XrpcError> {
    let mut values = input.params.iter().filter(|(key, _)| key == name);
    let value = values.next().map(|(_, value)| value.as_str());
    if values.next().is_some() {
        return Err(invalid(format!("duplicate {name}")));
    }
    Ok(value)
}

fn required<'a>(input: &'a Input, name: &str) -> Result<&'a str, XrpcError> {
    parameter(input, name)?
        .filter(|value| !value.is_empty())
        .ok_or_else(|| invalid(format!("missing {name}")))
}

fn page_limit(input: &Input, default: u32, maximum: u32) -> Result<u32, XrpcError> {
    parameter(input, "limit")?.map_or(Ok(default), |value| {
        value
            .parse::<u32>()
            .map(|limit| limit.clamp(1, maximum))
            .map_err(|_| invalid("invalid limit"))
    })
}

fn flag(input: &Input, name: &str) -> Result<bool, XrpcError> {
    parameter(input, name)?.map_or(Ok(false), |value| {
        value.parse().map_err(|_| invalid(format!("invalid {name}")))
    })
}

fn parse_space(value: &str) -> Result<SpaceUri, XrpcError> {
    value.parse().map_err(protocol_error)
}

fn parse_did(value: &str) -> Result<Did, XrpcError> {
    atproto_lexicon::validation::syntax::validate_did(value).map_err(|_| invalid("invalid repo DID"))?;
    Ok(Did::new(value))
}

async fn verify(
    state: &State,
    input: &Input,
    space: &SpaceUri,
    audience: &Did,
) -> Result<VerifiedCredential, XrpcError> {
    credentials::verify_http(state.shard.reader(), &input.headers, space, audience)
        .await
        .map_err(credential_error)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MintInput {
    space: String,
    credential_kid: String,
    delegation_token: Option<String>,
    client_attestation: Option<String>,
}

async fn mint(state: &State, input: &Input) -> Result<Response, XrpcError> {
    let input: MintInput = body(input)?;
    let prepared = credentials::prepare_mint(
        state.shard.reader(),
        state.resolver.as_ref(),
        &state.fetcher,
        credentials::MintRequest {
            space: parse_space(&input.space)?,
            credential_kid: input.credential_kid,
            delegation_token: input.delegation_token.filter(|value| !value.is_empty()),
            client_attestation: input.client_attestation.filter(|value| !value.is_empty()),
        },
    )
    .await
    .map_err(credential_error)?;
    let minted = state
        .shard
        .run(prepared)
        .await
        .map_err(internal)?
        .map_err(credential_error)?;
    Ok(Json(json!({"credential": minted.token})).into_response())
}

async fn list_repos(state: &State, input: &Input) -> Result<Response, XrpcError> {
    let space = parse_space(required(input, "space")?)?;
    let credential = verify(state, input, &space, &Did::new(&space.space_did)).await?;
    let page = state
        .shard
        .run(ListRepos {
            space,
            credential,
            cursor: parameter(input, "cursor")?.map(str::to_owned),
            limit: page_limit(input, 100, 1000)? as usize,
        })
        .await
        .map_err(internal)??;
    Ok(Json(page).into_response())
}

struct ListRepos {
    space: SpaceUri,
    credential: VerifiedCredential,
    cursor: Option<String>,
    limit: usize,
}

impl Op for ListRepos {
    type Output = notify::ReposPage;
    type Error = XrpcError;
    async fn run(self, txn: &mut Txn<'_>) -> Result<Self::Output, XrpcError> {
        credentials::recheck(txn, &self.credential)
            .await
            .map_err(credential_error)?;
        notify::list_repos_txn(txn, &self.space, self.cursor.as_deref(), self.limit)
            .await
            .map_err(space_error)
    }
}

enum ReadKind {
    Repo,
    Commit,
    Record {
        collection: String,
        rkey: String,
    },
    Records {
        collection: Option<String>,
        cursor: Option<String>,
        limit: u32,
        reverse: bool,
        exclude_values: bool,
    },
    Ops {
        cursor: Option<OplogCursor>,
        limit: u32,
        exclude_values: bool,
    },
    Blob,
    Blobs,
}

struct ReadRepo {
    space: SpaceUri,
    repo: Did,
    credential: VerifiedCredential,
    headers: HeaderMap,
    kind: ReadKind,
}

async fn read_repo(state: &State, nsid: &str, input: Input) -> Result<Response, XrpcError> {
    let space = parse_space(required(&input, "space")?)?;
    let repo = parse_did(required(&input, "repo")?)?;
    let credential = verify(state, &input, &space, &repo).await?;
    let kind = match nsid {
        "com.atproto.space.getRepo" => ReadKind::Repo,
        "com.atproto.space.getLatestCommit" => ReadKind::Commit,
        "com.atproto.space.getRecord" => ReadKind::Record {
            collection: required(&input, "collection")?.into(),
            rkey: required(&input, "rkey")?.into(),
        },
        "com.atproto.space.listRecords" => ReadKind::Records {
            collection: parameter(&input, "collection")?.map(str::to_owned),
            cursor: parameter(&input, "cursor")?.map(str::to_owned),
            limit: page_limit(&input, 50, 1000)?,
            reverse: flag(&input, "reverse")?,
            exclude_values: flag(&input, "excludeValues")?,
        },
        "com.atproto.space.listRepoOps" => {
            let cursor = match (
                parameter(&input, "cursor")?.filter(|value| !value.is_empty()),
                parameter(&input, "since")?.filter(|value| !value.is_empty()),
            ) {
                (Some(cursor), _) => Some(
                    OplogCursor::from_cursor_token(cursor)
                        .map_err(|_| protocol(StatusCode::BAD_REQUEST, "MalformedCursor", "malformed cursor"))?,
                ),
                (None, Some(since)) => Some(OplogCursor::from_token(since).map_err(protocol_error)?),
                (None, None) => None,
            };
            ReadKind::Ops {
                cursor,
                limit: page_limit(&input, 100, 1000)?,
                exclude_values: flag(&input, "excludeValues")?,
            }
        }
        "com.atproto.space.getBlob" => {
            required(&input, "cid")?
                .parse::<atproto_dasl::Cid>()
                .map_err(|_| invalid("invalid blob CID"))?;
            ReadKind::Blob
        }
        "com.atproto.space.listBlobs" => {
            page_limit(&input, 500, 1000)?;
            ReadKind::Blobs
        }
        _ => return Err(XrpcError::MethodNotImplemented),
    };
    // A read operation holds one writer-serialized view. A commit, its STAR
    // records and a credential's revocation state cannot come from different states.
    state
        .shard
        .run(ReadRepo {
            space,
            repo,
            credential,
            headers: input.headers,
            kind,
        })
        .await
        .map_err(internal)?
}

impl Op for ReadRepo {
    type Output = Response;
    type Error = XrpcError;

    async fn run(self, txn: &mut Txn<'_>) -> Result<Response, XrpcError> {
        credentials::recheck(txn, &self.credential)
            .await
            .map_err(credential_error)?;
        let mac = match &self.kind {
            ReadKind::Repo | ReadKind::Commit | ReadKind::Ops { .. } => Some(
                credentials::response_secret(txn, &self.headers, &self.credential, MacType::Commit)
                    .await
                    .map_err(credential_error)?,
            ),
            ReadKind::Record { .. } => Some(
                credentials::response_secret(txn, &self.headers, &self.credential, MacType::Record)
                    .await
                    .map_err(credential_error)?,
            ),
            _ => None,
        };
        let storage = TxnStorage::new(txn, &self.repo);
        match self.kind {
            ReadKind::Repo => repo_response(&storage, &self.space, &self.repo, mac.as_ref()).await,
            ReadKind::Commit => {
                let state = storage.current_state(&self.space).await.map_err(protocol_error)?;
                let commit = current_commit(&self.space, &self.repo, state)?.ok_or_else(repo_not_found)?;
                let mut response = Json(json!({"commit": commit})).into_response();
                attach_commit_mac(&mut response, mac.as_ref(), &commit)?;
                Ok(response)
            }
            ReadKind::Record { collection, rkey } => {
                let record = storage
                    .get_record(&self.space, &collection, &rkey)
                    .await
                    .map_err(protocol_error)?
                    .ok_or_else(|| protocol(StatusCode::NOT_FOUND, "RecordNotFound", "record not found"))?;
                let uri = RecordUri::new(self.space, self.repo.to_string(), collection, rkey).to_string();
                let value = decode_record(&record.value)?;
                let mut response = Json(json!({"uri": uri, "cid": record.cid, "value": value})).into_response();
                if let Some(mac) = mac {
                    let cid = record.cid.parse().map_err(internal)?;
                    let bytes = record_mac(&mac, &uri, &cid).map_err(internal)?;
                    attach_mac(&mut response, MacType::Record, &bytes)?;
                }
                Ok(response)
            }
            ReadKind::Records {
                collection,
                cursor,
                limit,
                reverse,
                exclude_values,
            } => {
                records_response(
                    &storage,
                    &self.space,
                    collection,
                    cursor.as_deref(),
                    limit,
                    reverse,
                    exclude_values,
                )
                .await
            }
            ReadKind::Ops {
                cursor,
                limit,
                exclude_values,
            } => {
                ops_response(
                    &storage,
                    &self.space,
                    &self.repo,
                    cursor.as_ref(),
                    limit,
                    exclude_values,
                    mac.as_ref(),
                )
                .await
            }
            ReadKind::Blob => Err(protocol(StatusCode::NOT_FOUND, "BlobNotFound", "blob not found")),
            // write_as_world rejects blob references. This is the actual empty
            // blob set, not an unavailable storage service reported as success.
            ReadKind::Blobs => Ok(Json(json!({"cids": []})).into_response()),
        }
    }
}

async fn repo_response(
    storage: &TxnStorage<'_, '_>,
    space: &SpaceUri,
    repo: &Did,
    mac: Option<&SharedSecret>,
) -> Result<Response, XrpcError> {
    let state = storage.current_state(space).await.map_err(protocol_error)?;
    let commit = current_commit(space, repo, state)?.ok_or_else(repo_not_found)?;
    let mut writer = RepoStarWriter::new(Vec::new(), &commit).map_err(internal)?;
    let mut collections = storage.list_collections(space).await.map_err(protocol_error)?;
    // The slash is part of the path: `a.b.c.d/` precedes
    // `a.b.c/`, despite collection-only lexical ordering.
    collections.sort_by(|left, right| left.bytes().chain(*b"/").cmp(right.bytes().chain(*b"/")));
    for collection in collections {
        let mut cursor = None;
        loop {
            let page = storage
                .list_records(space, &collection, cursor.as_deref(), 256, true)
                .await
                .map_err(protocol_error)?;
            for record in page.records {
                writer
                    .write_record(&record.collection, &record.rkey, &record.value)
                    .map_err(internal)?;
            }
            cursor = page.cursor;
            if cursor.is_none() {
                break;
            }
        }
    }
    let mut response = Response::new(Body::from(writer.into_inner()));
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/octet-stream"),
    );
    attach_commit_mac(&mut response, mac, &commit)?;
    Ok(response)
}

async fn records_response(
    storage: &TxnStorage<'_, '_>,
    space: &SpaceUri,
    collection: Option<String>,
    cursor: Option<&str>,
    limit: u32,
    reverse: bool,
    exclude_values: bool,
) -> Result<Response, XrpcError> {
    let mut records = Vec::with_capacity(limit as usize);
    let mut next = None;
    let mut remaining = limit;
    let one_collection = collection.is_some();
    let (resume_collection, resume_rkey) = match (one_collection, cursor) {
        (false, Some(raw)) => {
            let (collection, rkey) = raw
                .split_once('\u{1}')
                .map_or((raw, None), |(collection, rkey)| (collection, Some(rkey)));
            (Some(collection), rkey)
        }
        _ => (None, cursor),
    };
    let collections = match collection {
        Some(collection) => vec![collection],
        None => storage.list_collections(space).await.map_err(protocol_error)?,
    };
    for collection in collections {
        if remaining == 0 {
            break;
        }
        if resume_collection.is_some_and(|from| collection.as_str() < from) {
            continue;
        }
        let within = if one_collection || resume_collection == Some(collection.as_str()) {
            resume_rkey
        } else {
            None
        };
        let page = storage
            .list_records(space, &collection, within, remaining, reverse)
            .await
            .map_err(protocol_error)?;
        remaining -= u32::try_from(page.records.len()).map_err(internal)?;
        if one_collection {
            next = page.cursor;
        } else if remaining == 0 {
            next = page
                .records
                .last()
                .map(|record| format!("{collection}\u{1}{}", record.rkey));
        }
        for record in page.records {
            let mut value = json!({"collection": record.collection, "rkey": record.rkey, "cid": record.cid});
            if !exclude_values {
                value
                    .as_object_mut()
                    .ok_or(XrpcError::Internal)?
                    .insert("value".into(), decode_record(&record.value)?);
            }
            records.push(value);
        }
    }
    let mut response = json!({"records": records});
    if let Some(cursor) = next {
        response
            .as_object_mut()
            .ok_or(XrpcError::Internal)?
            .insert("cursor".into(), cursor.into());
    }
    Ok(Json(response).into_response())
}

async fn ops_response(
    storage: &TxnStorage<'_, '_>,
    space: &SpaceUri,
    repo: &Did,
    cursor: Option<&OplogCursor>,
    limit: u32,
    exclude_values: bool,
    mac: Option<&SharedSecret>,
) -> Result<Response, XrpcError> {
    let page = storage.read_oplog(space, cursor, limit).await.map_err(protocol_error)?;
    let caught_up = page.ops.len() < limit as usize;
    let next = if caught_up {
        None
    } else {
        page.ops
            .last()
            .map(|entry| OplogCursor::new(entry.rev.clone(), entry.idx).to_token())
    };
    let mut ops = Vec::with_capacity(page.ops.len());
    for entry in page.ops {
        let collection = entry.collection.ok_or(XrpcError::Internal)?;
        let rkey = entry.rkey.ok_or(XrpcError::Internal)?;
        let mut value =
            json!({"rev": entry.rev, "collection": collection, "rkey": rkey, "cid": entry.cid, "prev": entry.prev});
        if !exclude_values && let Some(bytes) = entry.value {
            value
                .as_object_mut()
                .ok_or(XrpcError::Internal)?
                .insert("value".into(), decode_record(&bytes)?);
        }
        ops.push(value);
    }
    let commit = if caught_up {
        current_commit(space, repo, page.state)?
    } else {
        None
    };
    let mut value = json!({"ops": ops});
    if let Some(cursor) = next {
        value
            .as_object_mut()
            .ok_or(XrpcError::Internal)?
            .insert("cursor".into(), cursor.into());
    }
    if let Some(commit) = &commit {
        value
            .as_object_mut()
            .ok_or(XrpcError::Internal)?
            .insert("commit".into(), serde_json::to_value(commit).map_err(internal)?);
    }
    let mut response = Json(value).into_response();
    if let Some(commit) = commit {
        attach_commit_mac(&mut response, mac, &commit)?;
    }
    Ok(response)
}

fn decode_record(bytes: &[u8]) -> Result<Value, XrpcError> {
    atproto_dasl::atproto_json::from_slice(bytes).map_err(internal)
}

fn repo_not_found() -> XrpcError {
    protocol(
        StatusCode::BAD_REQUEST,
        "RepoNotFound",
        "repo has no commit in this Space",
    )
}

fn current_commit(space: &SpaceUri, repo: &Did, state: RepoState) -> Result<Option<Commit>, XrpcError> {
    match (state.set_hash, state.rev) {
        (None, None) => Ok(None),
        (Some(bytes), Some(rev)) => Ok(Some(create_commit(
            &LtHash::from_state_bytes(&bytes).map_err(internal)?,
            &CommitContext {
                space: space.to_string(),
                repo: repo.to_string(),
                rev,
            },
        ))),
        _ => Err(XrpcError::Internal),
    }
}

fn attach_mac(response: &mut Response, kind: MacType, mac: &[u8]) -> Result<(), XrpcError> {
    response.headers_mut().insert(
        HeaderName::from_static(MAC_HEADER),
        HeaderValue::from_str(&format_mac_header(kind, mac)).map_err(internal)?,
    );
    Ok(())
}

fn attach_commit_mac(response: &mut Response, mac: Option<&SharedSecret>, commit: &Commit) -> Result<(), XrpcError> {
    if let Some(mac) = mac {
        attach_mac(response, MacType::Commit, &commit.mac(mac).map_err(internal)?)?;
    }
    Ok(())
}

#[derive(Deserialize)]
struct RegistrationInput {
    space: String,
    service: String,
}

struct Register {
    space: SpaceUri,
    credential: VerifiedCredential,
    service: notify::ResolvedService,
}

impl Op for Register {
    type Output = i64;
    type Error = SpaceError;
    async fn run(self, txn: &mut Txn<'_>) -> Result<i64, SpaceError> {
        notify::register(txn, &self.space, &self.credential, self.service).await
    }
}

struct Unregister {
    space: SpaceUri,
    credential: VerifiedCredential,
    service: String,
}

impl Op for Unregister {
    type Output = ();
    type Error = SpaceError;
    async fn run(self, txn: &mut Txn<'_>) -> Result<(), SpaceError> {
        notify::unregister(txn, &self.space, &self.credential, &self.service).await
    }
}

async fn registration(state: &State, nsid: &str, input: Input) -> Result<Response, XrpcError> {
    let request: RegistrationInput = body(&input)?;
    let space = parse_space(&request.space)?;
    let credential = verify(state, &input, &space, &Did::new(&space.space_did)).await?;
    if nsid == "com.atproto.space.unregisterNotify" {
        state
            .shard
            .run(Unregister {
                space,
                credential,
                service: request.service,
            })
            .await
            .map_err(internal)?
            .map_err(space_error)?;
        return Ok(StatusCode::OK.into_response());
    }
    let service = notify::resolve_service(state.resolver.as_ref(), &request.service)
        .await
        .map_err(space_error)?;
    let expires = state
        .shard
        .run(Register {
            space,
            credential,
            service,
        })
        .await
        .map_err(internal)?
        .map_err(space_error)?;
    let expires = chrono::DateTime::from_timestamp(expires, 0).ok_or(XrpcError::Internal)?;
    Ok(Json(json!({"expiresAt": expires.to_rfc3339()})).into_response())
}

#[derive(Deserialize)]
struct SpaceInput {
    space: String,
}

#[derive(Deserialize)]
struct RevocationInput {
    space: String,
    credentials: Vec<String>,
}

enum Notification {
    Write(notify::NotifyWrite),
    Deleted,
    Revoked(Vec<String>),
}

struct Receive {
    space: SpaceUri,
    issuer: String,
    audience: String,
    notification: Notification,
}

impl Op for Receive {
    type Output = ();
    type Error = SpaceError;
    async fn run(self, txn: &mut Txn<'_>) -> Result<(), SpaceError> {
        // Notifications for a repo host name the hosted bare DID, not the
        // authority or an arbitrary service fragment.
        if !matches!(self.notification, Notification::Write(_))
            && (self.audience.contains('#') || txn.get(&keys::world_info(&Did::new(&self.audience))).await?.is_none())
        {
            return Err(SpaceError::NotAuthorized);
        }
        match self.notification {
            Notification::Write(payload) => {
                notify::receive_write(txn, &self.space, &self.issuer, &self.audience, payload).await
            }
            Notification::Deleted => notify::receive_deletion(txn, &self.space, &self.issuer).await,
            Notification::Revoked(jtis) => notify::receive_revocation(txn, &self.space, &self.issuer, &jtis).await,
        }
    }
}

async fn receive_notify(state: &State, nsid: &str, caller: Caller, input: &Input) -> Result<Response, XrpcError> {
    let Caller::Service {
        did: issuer, audience, ..
    } = caller
    else {
        return Err(XrpcError::AuthRequired);
    };
    let (space, notification) = match nsid {
        "com.atproto.space.notifyWrite" => {
            let value: notify::NotifyWrite = body(input)?;
            (parse_space(&value.space)?, Notification::Write(value))
        }
        "com.atproto.space.notifySpaceDeleted" => {
            let value: SpaceInput = body(input)?;
            (parse_space(&value.space)?, Notification::Deleted)
        }
        "com.atproto.space.notifyCredentialRevoked" => {
            let value: RevocationInput = body(input)?;
            (parse_space(&value.space)?, Notification::Revoked(value.credentials))
        }
        _ => return Err(XrpcError::MethodNotImplemented),
    };
    state
        .shard
        .run(Receive {
            space,
            issuer,
            audience,
            notification,
        })
        .await
        .map_err(internal)?
        .map_err(space_error)?;
    Ok(StatusCode::OK.into_response())
}

enum Principal {
    Service(String),
    Session(Session),
}

struct Authority {
    principal: Principal,
    world: Did,
}

impl Authority {
    fn from_caller(caller: Caller, world: Did) -> Result<Self, XrpcError> {
        let principal = match caller {
            Caller::Service { did, audience, .. } => {
                if !matches!(
                    audience.strip_prefix(world.as_str()),
                    Some("" | "#atproto_pds" | "#motet_tenor")
                ) {
                    return Err(XrpcError::AuthRequired);
                }
                Principal::Service(did)
            }
            caller @ Caller::Session { .. } => {
                let session = Session::from_caller(caller).map_err(space_error)?;
                if session.world != world {
                    return Err(XrpcError::AuthRequired);
                }
                Principal::Session(session)
            }
            _ => return Err(XrpcError::AuthRequired),
        };
        Ok(Self { principal, world })
    }

    async fn check(&self, txn: &Txn<'_>) -> Result<Option<SessionPolicy>, XrpcError> {
        if let Principal::Session(session) = &self.principal {
            return session.policy(txn).await.map(Some).map_err(space_error);
        }
        let raw = txn
            .get(&keys::world_info(&self.world))
            .await
            .map_err(internal)?
            .ok_or(XrpcError::AuthRequired)?;
        let world: crate::world::WorldInfo = serde_json::from_slice(&raw).map_err(internal)?;
        if matches!(&self.principal, Principal::Service(issuer) if issuer == self.world.as_str() || issuer == world.owner.as_str())
        {
            Ok(None)
        } else {
            Err(XrpcError::AuthRequired)
        }
    }

    async fn audit(&self, txn: &mut Txn<'_>, action: &str, subject: &str) -> Result<(), XrpcError> {
        if let Principal::Session(session) = &self.principal {
            session.audit(txn, action, subject).await.map_err(space_error)?;
        }
        Ok(())
    }
}

struct Delegation {
    session: Session,
    space: SpaceUri,
}

impl Op for Delegation {
    type Output = String;
    type Error = SpaceError;

    async fn run(self, txn: &mut Txn<'_>) -> Result<String, SpaceError> {
        self.session
            .policy(txn)
            .await?
            .space(&self.space, None, SpaceAction::Read)?;
        if self.space.space_did == self.session.world.as_str() {
            super::require_live(&super::info_txn(txn, &self.space).await?)?;
        }
        let key = txn
            .key(&self.session.world, crate::keys::Purpose::Signing)
            .await?
            .ok_or(SpaceError::NotAuthorized)?;
        Ok(create_delegation_token(
            self.session.world.as_str(),
            &self.space,
            &key,
            DELEGATION_TOKEN_TTL_SECS,
        )?)
    }
}

async fn delegation(state: &State, caller: Caller, input: &Input) -> Result<Response, XrpcError> {
    let session = Session::from_caller(caller).map_err(|_| {
        protocol(
            StatusCode::FORBIDDEN,
            "OAuthSessionRequired",
            "OAuth session with verified client identity required",
        )
    })?;
    let space = parse_space(required(input, "space")?)?;
    let token = state
        .shard
        .run(Delegation { session, space })
        .await
        .map_err(internal)?
        .map_err(space_error)?;
    Ok(Json(json!({"token": token})).into_response())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RecordInput {
    space: String,
    repo: String,
    collection: String,
    rkey: Option<String>,
    record: Option<Value>,
    #[serde(default)]
    swap_record: Swap,
    // World records always use the compiled schema, including when callers
    // opt out of optional validation. Deserialize the wire flag's real type.
    #[serde(rename = "validate")]
    _validate: Option<bool>,
}

#[derive(Deserialize)]
#[serde(tag = "$type")]
enum BatchWrite {
    #[serde(rename = "com.atproto.space.applyWrites#create")]
    Create {
        collection: String,
        rkey: Option<String>,
        value: Value,
    },
    #[serde(rename = "com.atproto.space.applyWrites#update")]
    Update {
        collection: String,
        rkey: String,
        value: Value,
    },
    #[serde(rename = "com.atproto.space.applyWrites#delete")]
    Delete { collection: String, rkey: String },
}

#[derive(Deserialize)]
struct BatchInput {
    space: String,
    repo: String,
    writes: Vec<BatchWrite>,
    #[serde(rename = "validate")]
    _validate: Option<bool>,
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

enum WriteKind {
    Create,
    Put,
    Update,
    Delete,
}

struct PendingWrite {
    kind: WriteKind,
    collection: String,
    rkey: String,
    value: Option<Vec<u8>>,
    swap: Swap,
}

struct WriteRecords {
    authority: Authority,
    space: SpaceUri,
    writes: Vec<PendingWrite>,
    batch: bool,
}

impl Op for WriteRecords {
    type Output = Value;
    type Error = XrpcError;
    async fn run(self, txn: &mut Txn<'_>) -> Result<Value, XrpcError> {
        let policy = self.authority.check(txn).await?;
        let mut writes = Vec::with_capacity(self.writes.len());
        let mut actions = Vec::with_capacity(self.writes.len());
        for pending in self.writes {
            let prior = if matches!(pending.kind, WriteKind::Put) || !matches!(pending.swap, Swap::Unchecked) {
                super::record_txn(txn, &self.space, &pending.collection, &pending.rkey)
                    .await
                    .map_err(space_error)?
            } else {
                None
            };
            let swap = match pending.swap {
                Swap::Unchecked => None,
                Swap::Absent if prior.is_none() => None,
                Swap::Cid(cid) if prior.as_ref().is_some_and(|record| record.cid == cid) => Some(cid),
                Swap::Absent | Swap::Cid(_) => return Err(space_error(SpaceError::Conflict)),
            };
            let action = match pending.kind {
                WriteKind::Create => OpAction::Create,
                WriteKind::Update => OpAction::Update,
                WriteKind::Delete => OpAction::Delete,
                WriteKind::Put => {
                    if prior.is_some() {
                        OpAction::Update
                    } else {
                        OpAction::Create
                    }
                }
            };
            actions.push(action);
            writes.push(SpaceWrite {
                action,
                collection: pending.collection,
                rkey: pending.rkey,
                value: pending.value,
                swap,
            });
        }
        if let Some(policy) = policy {
            policy.records(txn, &self.space, &writes).await.map_err(space_error)?;
        }
        let committed = super::write_as_world(txn, &self.space, writes)
            .await
            .map_err(space_error)?;
        let mut results = Vec::with_capacity(committed.results.len());
        for (result, action) in committed.results.into_iter().zip(actions) {
            let audit_action = match action {
                OpAction::Create => "create",
                OpAction::Update => "update",
                OpAction::Delete => "delete",
            };
            self.authority.audit(txn, audit_action, &result.uri).await?;
            let mut result = if action == OpAction::Delete {
                json!({})
            } else {
                json!({"uri": result.uri, "cid": result.cid.ok_or(XrpcError::Internal)?, "validationStatus": "valid"})
            };
            if self.batch {
                result.as_object_mut().ok_or(XrpcError::Internal)?.insert(
                    "$type".into(),
                    match action {
                        OpAction::Create => "com.atproto.space.applyWrites#createResult",
                        OpAction::Update => "com.atproto.space.applyWrites#updateResult",
                        OpAction::Delete => "com.atproto.space.applyWrites#deleteResult",
                    }
                    .into(),
                );
            }
            results.push(result);
        }
        if self.batch {
            Ok(json!({"results": results}))
        } else {
            results.into_iter().next().ok_or(XrpcError::Internal)
        }
    }
}

fn encode_record(value: &Value) -> Result<Vec<u8>, XrpcError> {
    atproto_dasl::to_vec_from_json(value).map_err(|error| invalid(error.to_string()))
}

fn generated_rkey(collection: &str) -> Result<String, XrpcError> {
    for (nsid, schema) in motet_lexicons::SCHEMAS {
        if *nsid == collection {
            let schema: Value = serde_json::from_str(schema).map_err(internal)?;
            if let Some(literal) = schema
                .get("defs")
                .and_then(|defs| defs.get("main"))
                .and_then(|main| main.get("key"))
                .and_then(Value::as_str)
                .and_then(|key| key.strip_prefix("literal:"))
            {
                return Ok(literal.into());
            }
            break;
        }
    }
    Ok(Tid::new().to_string())
}

async fn write_records(state: &State, nsid: &str, caller: Caller, input: &Input) -> Result<Response, XrpcError> {
    let batch = nsid == "com.atproto.space.applyWrites";
    let (space, repo, writes) = if batch {
        let request: BatchInput = body(input)?;
        if request.writes.is_empty() || request.writes.len() > 200 {
            return Err(invalid("writes must contain 1 to 200 entries"));
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
                    } => {
                        let rkey = rkey.map_or_else(|| generated_rkey(&collection), Ok)?;
                        (WriteKind::Create, collection, rkey, Some(encode_record(&value)?))
                    }
                    BatchWrite::Update {
                        collection,
                        rkey,
                        value,
                    } => (WriteKind::Update, collection, rkey, Some(encode_record(&value)?)),
                    BatchWrite::Delete { collection, rkey } => (WriteKind::Delete, collection, rkey, None),
                };
                Ok(PendingWrite {
                    kind,
                    collection,
                    rkey,
                    value,
                    swap: Swap::Unchecked,
                })
            })
            .collect::<Result<Vec<_>, XrpcError>>()?;
        (parse_space(&request.space)?, request.repo, writes)
    } else {
        let request: RecordInput = body(input)?;
        let kind = match nsid {
            "com.atproto.space.createRecord" => WriteKind::Create,
            "com.atproto.space.putRecord" => WriteKind::Put,
            "com.atproto.space.deleteRecord" => WriteKind::Delete,
            _ => return Err(XrpcError::MethodNotImplemented),
        };
        let rkey = match request.rkey {
            Some(rkey) => rkey,
            None if matches!(kind, WriteKind::Create) => generated_rkey(&request.collection)?,
            None => return Err(invalid("missing rkey")),
        };
        let value = if matches!(kind, WriteKind::Delete) {
            None
        } else {
            Some(encode_record(
                &request.record.ok_or_else(|| invalid("missing record"))?,
            )?)
        };
        (
            parse_space(&request.space)?,
            request.repo,
            vec![PendingWrite {
                kind,
                collection: request.collection,
                rkey,
                value,
                swap: request.swap_record,
            }],
        )
    };
    // A member's credential or service token never implicitly becomes the
    // World. Explicit World/owner authority is rechecked inside the operation.
    if repo != space.space_did {
        return Err(XrpcError::AuthRequired);
    }
    let authority = Authority::from_caller(caller, parse_did(&repo)?)?;
    let value = state
        .shard
        .run(WriteRecords {
            authority,
            space,
            writes,
            batch,
        })
        .await
        .map_err(internal)??;
    Ok(Json(value).into_response())
}

#[derive(Deserialize)]
struct CreateInput {
    #[serde(rename = "type")]
    space_type: String,
    skey: Option<String>,
    access: AccessRecord,
    index: Option<bool>,
}

struct Create {
    authority: Authority,
    space: SpaceUri,
    access: AccessRecord,
    indexed: bool,
}

impl Op for Create {
    type Output = Value;
    type Error = XrpcError;
    async fn run(self, txn: &mut Txn<'_>) -> Result<Value, XrpcError> {
        if let Some(policy) = self.authority.check(txn).await? {
            policy
                .manage(&self.space, SpaceManageVerb::Create)
                .map_err(space_error)?;
            policy
                .access_change(&self.space, None, Some(&self.access))
                .map_err(space_error)?;
        }
        let committed = super::create(txn, &self.space, self.access, self.indexed)
            .await
            .map_err(|error| match error {
                SpaceError::Conflict => protocol(StatusCode::BAD_REQUEST, "SpaceAlreadyExists", "Space already exists"),
                other => space_error(other),
            })?;
        self.authority
            .audit(txn, "createSpace", &self.space.to_string())
            .await?;
        let access = committed.results.into_iter().next().ok_or(XrpcError::Internal)?;
        let mut response = json!({"space": self.space.to_string(), "access": {"uri": access.uri, "cid": access.cid.ok_or(XrpcError::Internal)?}});
        if self.indexed {
            let info = super::info_txn(txn, &self.space).await.map_err(space_error)?;
            let members = super::world_space(&self.authority.world, super::MEMBERS).map_err(space_error)?;
            let row = super::record_txn(txn, &members, "group.opensocial.space", &info.generation)
                .await
                .map_err(space_error)?
                .ok_or(XrpcError::Internal)?;
            response.as_object_mut().ok_or(XrpcError::Internal)?.insert("index".into(), json!({"uri": RecordUri::new(members, self.authority.world.to_string(), "group.opensocial.space".into(), row.rkey).to_string(), "cid": row.cid}));
        }
        Ok(response)
    }
}

fn require_modality(space: &SpaceUri) -> Result<(), XrpcError> {
    if matches!(space.space_type.as_str(), super::META | super::MEMBERS) {
        Err(protocol(
            StatusCode::BAD_REQUEST,
            "ReservedSpace",
            "meta and members Spaces are reserved",
        ))
    } else {
        Ok(())
    }
}

fn caller_world(caller: &Caller) -> Result<Did, XrpcError> {
    match caller {
        Caller::Service { audience, .. } => {
            parse_did(audience.split_once('#').map_or(audience.as_str(), |(did, _)| did))
        }
        Caller::Session { world, .. } => parse_did(world),
        _ => Err(XrpcError::AuthRequired),
    }
}

async fn create_space(state: &State, caller: Caller, input: &Input) -> Result<Response, XrpcError> {
    let request: CreateInput = body(input)?;
    let world = caller_world(&caller)?;
    let authority = Authority::from_caller(caller, world.clone())?;
    let skey = request.skey.unwrap_or_else(|| Tid::new().to_string());
    let space = parse_space(&format!("at://{world}/space/{}/{skey}", request.space_type))?;
    require_modality(&space)?;
    let result = state
        .shard
        .run(Create {
            authority,
            space,
            access: request.access,
            indexed: request.index.unwrap_or(true),
        })
        .await
        .map_err(internal)??;
    Ok(Json(result).into_response())
}

enum DeleteMode {
    Group,
    Simple,
}

struct Delete {
    authority: Authority,
    space: SpaceUri,
    mode: DeleteMode,
}

impl Op for Delete {
    type Output = ();
    type Error = XrpcError;
    async fn run(self, txn: &mut Txn<'_>) -> Result<(), XrpcError> {
        if let Some(policy) = self.authority.check(txn).await? {
            policy
                .manage(&self.space, SpaceManageVerb::Delete)
                .map_err(space_error)?;
        }
        require_modality(&self.space)?;
        // The audit destination cannot be removed by a session which needs it.
        if self.space.space_type.as_str() == super::GMS && matches!(self.authority.principal, Principal::Session(_)) {
            return Err(XrpcError::AuthRequired);
        }
        match super::delete(txn, &self.space).await {
            Err(SpaceError::Deleted) if matches!(self.mode, DeleteMode::Simple) => return Ok(()),
            result => result.map_err(space_error)?,
        }
        self.authority.audit(txn, "deleteSpace", &self.space.to_string()).await
    }
}

async fn delete_space(state: &State, caller: Caller, input: &Input, mode: DeleteMode) -> Result<Response, XrpcError> {
    let request: SpaceInput = body(input)?;
    let space = parse_space(&request.space)?;
    let authority = Authority::from_caller(caller, Did::new(&space.space_did))?;
    state
        .shard
        .run(Delete { authority, space, mode })
        .await
        .map_err(internal)??;
    Ok(StatusCode::OK.into_response())
}

async fn list_spaces(state: &State, caller: Caller, input: &Input) -> Result<Response, XrpcError> {
    let world = caller_world(&caller)?;
    if parameter(input, "did")?.is_some_and(|did| did != world.as_str()) {
        return Err(XrpcError::AuthRequired);
    }
    let authority = Authority::from_caller(caller, world.clone())?;
    state.shard.run(CheckAuthority(authority)).await.map_err(internal)??;
    let cursor = parameter(input, "cursor")?;
    let kind = parameter(input, "spaceType")?;
    let limit = page_limit(input, 50, 100)? as usize;
    let mut entries = super::list_spaces(state.shard.reader(), &world)
        .await
        .map_err(space_error)?;
    entries.retain(|info| {
        !info.deleted
            && kind.is_none_or(|kind| kind == info.space.space_type.as_str())
            && cursor.is_none_or(|cursor| info.space.to_string().as_str() > cursor)
    });
    entries.sort_by_key(|info| info.space.to_string());
    let more = entries.len() > limit;
    entries.truncate(limit);
    let next = if more {
        entries.last().map(|info| info.space.to_string())
    } else {
        None
    };
    let mut spaces = Vec::with_capacity(entries.len());
    for info in entries {
        spaces.push(json!({"uri": info.space.to_string(), "isOwner": true, "isMember": true, "createdAt": revision_time(&info.generation)?}));
    }
    let mut value = json!({"spaces": spaces});
    if let Some(cursor) = next {
        value
            .as_object_mut()
            .ok_or(XrpcError::Internal)?
            .insert("cursor".into(), cursor.into());
    }
    Ok(Json(value).into_response())
}

struct CheckAuthority(Authority);

impl Op for CheckAuthority {
    type Output = ();
    type Error = XrpcError;
    async fn run(self, txn: &mut Txn<'_>) -> Result<(), XrpcError> {
        self.0.check(txn).await.map(|_| ())
    }
}

fn revision_time(rev: &str) -> Result<String, XrpcError> {
    let tid = Tid::decode(rev).map_err(internal)?;
    let micros = i64::try_from(tid.timestamp_micros()).map_err(internal)?;
    let datetime = chrono::DateTime::from_timestamp_micros(micros).ok_or(XrpcError::Internal)?;
    Ok(datetime.to_rfc3339())
}

struct Describe {
    space: SpaceUri,
    credential: VerifiedCredential,
}

impl Op for Describe {
    type Output = Value;
    type Error = XrpcError;
    async fn run(self, txn: &mut Txn<'_>) -> Result<Value, XrpcError> {
        credentials::recheck(txn, &self.credential)
            .await
            .map_err(credential_error)?;
        let info = super::info_txn(txn, &self.space).await.map_err(space_error)?;
        let public = super::access_txn(txn, &self.space, None)
            .await
            .map_err(space_error)?
            .read;
        // listMembers derives this member list's effective flags from the
        // same role grants used at mint and notify ingress. It stores no ACL.
        let read_policy = if public {
            "com.atproto.simplespace.defs#publicPolicy"
        } else {
            "com.atproto.simplespace.defs#memberListPolicy"
        };
        let app_access = info.allowed_clients.map_or_else(
            || json!({"$type": "com.atproto.simplespace.defs#open"}),
            |allowed| json!({"$type": "com.atproto.simplespace.defs#allowList", "allowed": allowed}),
        );
        Ok(json!({
            "uri": self.space.to_string(),
            "readPolicy": {"$type": read_policy},
            "writePolicy": {"$type": "com.atproto.simplespace.defs#memberListPolicy"},
            "appAccess": app_access,
        }))
    }
}

async fn describe_space(state: &State, input: &Input) -> Result<Response, XrpcError> {
    let space = parse_space(required(input, "space")?)?;
    let credential = verify(state, input, &space, &Did::new(&space.space_did)).await?;
    let result = state
        .shard
        .run(Describe { space, credential })
        .await
        .map_err(internal)??;
    Ok(Json(result).into_response())
}

struct Members {
    authority: Authority,
    space: SpaceUri,
    cursor: Option<String>,
    limit: u32,
}

impl Op for Members {
    type Output = Value;
    type Error = XrpcError;
    async fn run(self, txn: &mut Txn<'_>) -> Result<Value, XrpcError> {
        self.authority.check(txn).await?;
        let info = super::info_txn(txn, &self.space).await.map_err(space_error)?;
        if info.deleted {
            return Err(space_error(SpaceError::Deleted));
        }
        let roster = super::world_space(&self.authority.world, super::MEMBERS).map_err(space_error)?;
        let mut members = std::collections::BTreeMap::new();
        members.insert(
            self.authority.world.to_string(),
            json!({
                "did": self.authority.world.as_str(), "read": true, "write": true,
                "memberRev": info.generation, "addedAt": revision_time(&info.generation)?,
            }),
        );
        let mut cursor = None;
        loop {
            let page = {
                let storage = TxnStorage::new(txn, &self.authority.world);
                storage
                    .list_records(&roster, "group.opensocial.membership", cursor.as_deref(), 256, true)
                    .await
                    .map_err(protocol_error)?
            };
            for row in page.records {
                let membership: motet_lexicons::group_opensocial_membership::Membership =
                    super::record_value(&row).map_err(space_error)?;
                let access = super::access_txn(txn, &self.space, Some(&membership.member))
                    .await
                    .map_err(space_error)?;
                if access.read || access.write {
                    members.insert(
                        membership.member.clone(),
                        json!({
                            "did": membership.member, "read": access.read, "write": access.write,
                            "memberRev": row.repo_rev, "addedAt": membership.created_at,
                        }),
                    );
                }
            }
            cursor = page.cursor;
            if cursor.is_none() {
                break;
            }
        }
        let mut page: Vec<_> = members
            .into_iter()
            .filter(|(did, _)| self.cursor.as_ref().is_none_or(|cursor| did > cursor))
            .take(self.limit as usize + 1)
            .collect();
        let more = page.len() > self.limit as usize;
        page.truncate(self.limit as usize);
        let cursor = if more {
            page.last().map(|(did, _)| did.clone())
        } else {
            None
        };
        let members: Vec<_> = page.into_iter().map(|(_, member)| member).collect();
        let mut result = json!({"members": members});
        if let Some(cursor) = cursor {
            result
                .as_object_mut()
                .ok_or(XrpcError::Internal)?
                .insert("cursor".into(), cursor.into());
        }
        Ok(result)
    }
}

async fn list_members(state: &State, caller: Caller, input: &Input) -> Result<Response, XrpcError> {
    let space = parse_space(required(input, "space")?)?;
    let authority = Authority::from_caller(caller, Did::new(&space.space_did))?;
    let result = state
        .shard
        .run(Members {
            authority,
            space,
            cursor: parameter(input, "cursor")?.map(str::to_owned),
            limit: page_limit(input, 100, 1000)?,
        })
        .await
        .map_err(internal)??;
    Ok(Json(result).into_response())
}
