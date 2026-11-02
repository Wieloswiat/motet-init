//! Confidential OAuth sign-in and browser sessions for the AppView.
//!
//! The browser holds only opaque, host-only cookies. OAuth grants and pending
//! PKCE/DPoP state are sealed outside PostgreSQL's trust boundary.

pub mod login;
pub mod pds;
pub mod store;

use std::path::Path;
use std::sync::Arc;

use anyhow::{Context as _, ensure};
use atproto_identity::key::{KeyData, KeyType};
use atproto_identity::traits::IdentityResolver;
use atproto_oauth::workflow::{OAuthClient, generate_state};
use axum::Json;
use axum::extract::{Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use base64::Engine as _;
use chrono::{Duration, Utc};
use motet_net::Fetcher;
use motet_xrpc::server::Authenticate;
use motet_xrpc::{Caller, XrpcError};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::PgPool;
use url::Url;

use self::pds::{Grant, PdsClient};
use self::store::SessionStore;

const SESSION_COOKIE: &str = "__Host-motet_session";
const LOGIN_COOKIE_PREFIX: &str = "__Host-motet_login_";
const LOGIN_SECONDS: i64 = 600;
const SESSION_DAYS: i64 = 30;

/// OAuth endpoints and server-side browser session authentication.
pub struct Server {
    origin: String,
    default_pds: String,
    client: login::Client,
    store: SessionStore,
    pds: PdsClient,
    jwks: Value,
}

impl std::fmt::Debug for Server {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppviewOAuth")
            .field("origin", &self.origin)
            .finish_non_exhaustive()
    }
}

impl Server {
    /// Opens the independently provisioned signing and session-sealing keys.
    pub fn new(
        pool: PgPool,
        origin: String,
        default_pds: String,
        signing_key_file: &Path,
        session_key_file: &Path,
        fetcher: Fetcher,
        resolver: Arc<dyn IdentityResolver>,
    ) -> anyhow::Result<Arc<Self>> {
        let parsed = Url::parse(&origin).context("invalid AppView public origin")?;
        ensure!(
            parsed.scheme() == "https"
                && parsed.has_host()
                && parsed.port().is_none()
                && parsed.username().is_empty()
                && parsed.password().is_none()
                && parsed.path() == "/"
                && parsed.query().is_none()
                && parsed.fragment().is_none()
                && parsed.origin().ascii_serialization() == origin,
            "AppView public origin must be a canonical HTTPS origin without a port"
        );
        let key_bytes = read_key(signing_key_file).context("OAuth signing key")?;
        let signing_key = Arc::new(KeyData::new(KeyType::P256Private, key_bytes.to_vec()));
        // Deriving a public JWK also validates the P-256 private scalar.
        let public_key = atproto_oauth::jwk::generate(&signing_key).context("invalid OAuth P-256 signing key")?;
        let client_id = format!("{origin}/oauth/client-metadata.json");
        let redirect_uri = format!("{origin}/oauth/callback");
        let client = login::Client::new(
            fetcher.clone(),
            resolver,
            client_id.clone(),
            redirect_uri.clone(),
            Arc::clone(&signing_key),
        );
        let store = SessionStore::from_file(pool, session_key_file).context("session sealing key")?;
        let pds = PdsClient::new(
            fetcher,
            store.clone(),
            OAuthClient {
                redirect_uri,
                client_id,
                private_signing_key_data: Arc::unwrap_or_clone(signing_key),
            },
        );
        Ok(Arc::new(Self {
            origin,
            default_pds,
            client,
            store,
            pds,
            jwks: json!({"keys": [public_key]}),
        }))
    }

    /// DPoP-authenticated PDS operations for AppView write procedures.
    #[must_use]
    pub fn pds(&self) -> &PdsClient {
        &self.pds
    }

    /// Combines cookie sessions with the existing service authenticator.
    /// Present but invalid or ambiguous credentials never become anonymous.
    #[must_use]
    pub fn authenticate(&self, service_auth: Authenticate) -> Authenticate {
        let store = self.store.clone();
        let origin: Arc<str> = Arc::from(self.origin.as_str());
        Arc::new(move |context| {
            let store = store.clone();
            let service_auth = Arc::clone(&service_auth);
            let origin = Arc::clone(&origin);
            Box::pin(async move {
                let cookie = unique_cookie(context.headers, SESSION_COOKIE).map_err(|()| XrpcError::AuthRequired)?;
                let Some(cookie) = cookie else {
                    return service_auth(context).await;
                };
                if context.headers.contains_key(header::AUTHORIZATION)
                    || (*context.method != axum::http::Method::GET && !trusted_origin(context.headers, &origin))
                {
                    return Err(XrpcError::AuthRequired);
                }
                let session = store
                    .get_session::<Grant>(cookie)
                    .await
                    .map_err(|_| XrpcError::Internal)?
                    .ok_or(XrpcError::AuthRequired)?;
                Ok(Caller::AppviewSession {
                    did: session.grant.did,
                    handle: session.grant.handle,
                })
            })
        })
    }

    /// Browser endpoints and public OAuth client documents.
    #[must_use]
    pub fn router(self: &Arc<Self>) -> axum::Router {
        axum::Router::new()
            .route("/oauth/client-metadata.json", get(metadata))
            .route("/oauth/jwks.json", get(jwks))
            .route("/oauth/sign-in", get(sign_in))
            .route("/oauth/sign-up", get(sign_up))
            .route("/oauth/callback", get(callback))
            .route("/oauth/sign-out", post(sign_out))
            .layer(axum::middleware::map_response(browser_response))
            .with_state(Arc::clone(self))
    }
}

fn read_key(path: &Path) -> anyhow::Result<[u8; 32]> {
    let contents = std::fs::read(path)?;
    if let Ok(bytes) = <[u8; 32]>::try_from(contents.as_slice()) {
        return Ok(bytes);
    }
    let text = std::str::from_utf8(&contents)?;
    let decoded = base64::engine::general_purpose::STANDARD.decode(text.trim())?;
    <[u8; 32]>::try_from(decoded.as_slice()).map_err(|_| anyhow::anyhow!("key must be 32 bytes"))
}

#[derive(Debug)]
enum Error {
    Invalid,
    Forbidden,
    Internal,
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            Self::Invalid => (StatusCode::BAD_REQUEST, "The login request is invalid or expired."),
            Self::Forbidden => (StatusCode::FORBIDDEN, "The request origin is not permitted."),
            Self::Internal => (StatusCode::INTERNAL_SERVER_ERROR, "The sign-in service is unavailable."),
        };
        no_store((status, Json(json!({"error": message}))).into_response())
    }
}

fn no_store(mut response: Response) -> Response {
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
        .headers_mut()
        .insert(header::REFERRER_POLICY, HeaderValue::from_static("no-referrer"));
    response
}

async fn browser_response(response: Response) -> Response {
    no_store(response)
}

async fn metadata(State(server): State<Arc<Server>>) -> Json<Value> {
    Json(json!({
        "client_id": format!("{}/oauth/client-metadata.json", server.origin),
        "client_name": "Motet",
        "client_uri": server.origin,
        "application_type": "web",
        "redirect_uris": [format!("{}/oauth/callback", server.origin)],
        "grant_types": ["authorization_code", "refresh_token"],
        "response_types": ["code"],
        "scope": login::SCOPE,
        "token_endpoint_auth_method": "private_key_jwt",
        "token_endpoint_auth_signing_alg": "ES256",
        "dpop_bound_access_tokens": true,
        "jwks_uri": format!("{}/oauth/jwks.json", server.origin)
    }))
}

async fn jwks(State(server): State<Arc<Server>>) -> Json<Value> {
    Json(server.jwks.clone())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StartQuery {
    identifier: Option<String>,
    return_to: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct PendingLogin {
    pending: login::Pending,
    return_to: String,
}

async fn sign_in(State(server): State<Arc<Server>>, Query(query): Query<StartQuery>) -> Result<Response, Error> {
    let identifier = query
        .identifier
        .as_deref()
        .filter(|value| !value.is_empty())
        .ok_or(Error::Invalid)?;
    start(&server, Some(identifier), query.return_to.as_deref(), false).await
}

async fn sign_up(State(server): State<Arc<Server>>, Query(query): Query<StartQuery>) -> Result<Response, Error> {
    if query.identifier.is_some() {
        return Err(Error::Invalid);
    }
    start(&server, None, query.return_to.as_deref(), true).await
}

async fn start(
    server: &Server,
    identifier: Option<&str>,
    return_to: Option<&str>,
    signup: bool,
) -> Result<Response, Error> {
    let return_to = safe_return_path(&server.origin, return_to.unwrap_or("/"))?;
    let started = server
        .client
        .start(identifier, &server.default_pds, signup)
        .await
        .map_err(|_| Error::Invalid)?;
    let binding = generate_state();
    let name = login_cookie_name(&started.state)?;
    server
        .store
        .store_pending(
            &started.state,
            &binding,
            &PendingLogin {
                pending: started.pending,
                return_to,
            },
            Utc::now() + Duration::seconds(LOGIN_SECONDS),
        )
        .await
        .map_err(|_| Error::Internal)?;
    let mut response = Redirect::temporary(&started.location).into_response();
    set_cookie(&mut response, &name, &binding, LOGIN_SECONDS)?;
    Ok(no_store(response))
}

#[derive(Deserialize)]
struct CallbackQuery {
    state: String,
    code: String,
    iss: String,
}

async fn callback(
    State(server): State<Arc<Server>>,
    headers: HeaderMap,
    Query(query): Query<CallbackQuery>,
) -> Result<Response, Error> {
    let name = login_cookie_name(&query.state)?;
    let binding = unique_cookie(&headers, &name)
        .map_err(|()| Error::Invalid)?
        .ok_or(Error::Invalid)?;
    let pending: PendingLogin = server
        .store
        .consume_pending(&query.state, binding)
        .await
        .map_err(|_| Error::Internal)?
        .ok_or(Error::Invalid)?;
    let grant = server
        .client
        .finish(pending.pending, &query.code, &query.state, &query.iss)
        .await
        .map_err(|_| Error::Invalid)?;
    let cookie = server
        .store
        .create_session(&grant, Utc::now() + Duration::days(SESSION_DAYS))
        .await
        .map_err(|_| Error::Internal)?;
    let mut response = Redirect::see_other(&pending.return_to).into_response();
    set_cookie(&mut response, SESSION_COOKIE, &cookie, SESSION_DAYS * 86_400)?;
    set_cookie(&mut response, &name, "", 0)?;
    Ok(no_store(response))
}

async fn sign_out(State(server): State<Arc<Server>>, headers: HeaderMap) -> Result<Response, Error> {
    if !trusted_origin(&headers, &server.origin) {
        return Err(Error::Forbidden);
    }
    let cookie = unique_cookie(&headers, SESSION_COOKIE)
        .map_err(|()| Error::Invalid)?
        .ok_or(Error::Invalid)?;
    // Local revocation commits before the optional remote RFC 7009 request.
    // Clear the browser cookie even if the remote server cannot be reached.
    let status = if server.pds.revoke(cookie).await.is_ok() {
        StatusCode::NO_CONTENT
    } else {
        StatusCode::BAD_GATEWAY
    };
    let mut response = status.into_response();
    set_cookie(&mut response, SESSION_COOKIE, "", 0)?;
    Ok(no_store(response))
}

fn trusted_origin(headers: &HeaderMap, origin: &str) -> bool {
    let mut origins = headers.get_all(header::ORIGIN).iter();
    origins.next().and_then(|value| value.to_str().ok()) == Some(origin) && origins.next().is_none()
}

fn login_cookie_name(state: &str) -> Result<String, Error> {
    if !(16..=128).contains(&state.len())
        || !state
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"-_".contains(&byte))
    {
        return Err(Error::Invalid);
    }
    Ok(format!("{LOGIN_COOKIE_PREFIX}{state}"))
}

fn set_cookie(response: &mut Response, name: &str, value: &str, max_age: i64) -> Result<(), Error> {
    let value = HeaderValue::from_str(&format!(
        "{name}={value}; Path=/; HttpOnly; Secure; SameSite=Lax; Max-Age={max_age}"
    ))
    .map_err(|_| Error::Internal)?;
    response.headers_mut().append(header::SET_COOKIE, value);
    Ok(())
}

/// Extracts the opaque session credential from an XRPC handler's headers.
///
/// Only call this after the registry authorizes [`motet_xrpc::Access::AppviewSession`].
/// Extraction alone does not authenticate the request or check its origin. Pass
/// the result to [`PdsClient::service_token`] or [`PdsClient::create_record`].
pub fn session_cookie(headers: &HeaderMap) -> Result<&str, XrpcError> {
    unique_cookie(headers, SESSION_COOKIE)
        .map_err(|()| XrpcError::AuthRequired)?
        .ok_or(XrpcError::AuthRequired)
}

fn unique_cookie<'a>(headers: &'a HeaderMap, name: &str) -> Result<Option<&'a str>, ()> {
    let mut found = None;
    for header in headers.get_all(header::COOKIE) {
        let value = header.to_str().map_err(|_| ())?;
        for item in value.split(';') {
            let Some((key, value)) = item.trim().split_once('=') else {
                continue;
            };
            if key == name && (found.replace(value).is_some() || value.is_empty() || value.len() > 256) {
                return Err(());
            }
        }
    }
    Ok(found)
}

fn safe_return_path(origin: &str, path: &str) -> Result<String, Error> {
    if !path.starts_with('/')
        || path.starts_with("//")
        || path.bytes().any(|byte| byte.is_ascii_control() || byte == b'\\')
    {
        return Err(Error::Invalid);
    }
    // Encoded separators and controls can be reinterpreted by intermediaries.
    for pair in path.as_bytes().windows(3) {
        if let [b'%', first, second] = pair {
            let hex = |byte: u8| char::from(byte).to_digit(16);
            if let (Some(high), Some(low)) = (hex(*first), hex(*second)) {
                let decoded = high * 16 + low;
                if decoded < 32 || decoded == 127 || decoded == u32::from(b'\\') || decoded == u32::from(b'/') {
                    return Err(Error::Invalid);
                }
            }
        }
    }
    let base = Url::parse(origin).map_err(|_| Error::Internal)?;
    let target = base.join(path).map_err(|_| Error::Invalid)?;
    if target.origin() != base.origin() {
        return Err(Error::Invalid);
    }
    Ok(path.to_owned())
}
