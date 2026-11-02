//! The AppView's authenticated calls to a member's own PDS.
//!
//! OAuth credentials stay server-side in the sealed session store. A World
//! write obtains a short-lived service token for one audience and method; it
//! never hands the member's OAuth token to the tenor.

use std::fmt;

use atproto_client::client::{
    Auth, CallLimits, DpopBody, DpopPresentation, XrpcResponse, dpop_call_as, is_nonce_challenge, xrpc_call_with_limits,
};
use atproto_identity::key::{KeyData, KeyType};
use atproto_identity::validation::validate_service_endpoint;
use atproto_oauth::dpop::{auth_dpop, dpop_with_nonce};
use atproto_oauth::encoding::FromBase64;
use atproto_oauth::jwt::{Claims, Header, JoseClaims, mint};
use atproto_oauth::workflow::{OAuthClient, TokenResponse};
use chrono::Utc;
use motet_net::Fetcher;
use reqwest::header::{HeaderMap, HeaderValue, WWW_AUTHENTICATE};
use reqwest::{Method, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use thiserror::Error;
use url::Url;
use uuid::Uuid;

use super::store::{SessionRef, SessionStore, StoreError};

const GET_SERVICE_AUTH: &str = "com.atproto.server.getServiceAuth";
const CREATE_RECORD: &str = "com.atproto.repo.createRecord";
// Leave room for modest clock skew within the service's one-minute cap.
const SERVICE_TOKEN_LIFETIME_SECS: i64 = 55;
// Refresh before a token expires while a request is in flight.
const REFRESH_MARGIN_SECS: i64 = 30;

/// One OAuth grant. The entire value, including the `DPoP` key, must be sealed.
#[derive(Serialize, Deserialize)]
pub struct Grant {
    pub did: String,
    pub handle: String,
    pub pds: String,
    pub issuer: String,
    pub token_endpoint: String,
    /// Optional revocation endpoint pinned during OAuth discovery.
    pub revocation_endpoint: Option<String>,
    pub access_token: String,
    pub refresh_token: String,
    /// Access-token expiry in Unix seconds, not the browser-session expiry.
    pub expires_at: i64,
    pub scope: String,
    /// The raw P-256 private key that binds this grant's tokens.
    pub dpop_private_key: Vec<u8>,
}

impl fmt::Debug for Grant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Grant")
            .field("did", &self.did)
            .field("handle", &self.handle)
            .field("pds", &self.pds)
            .field("issuer", &self.issuer)
            .field("token_endpoint", &self.token_endpoint)
            .field("revocation_endpoint", &self.revocation_endpoint)
            .field("expires_at", &self.expires_at)
            .field("scope", &self.scope)
            .field("credentials", &"[REDACTED]")
            .finish_non_exhaustive()
    }
}

/// A failure to use a browser session or its PDS grant.
#[derive(Debug, Error)]
pub enum PdsError {
    #[error("the session is missing, expired, or revoked")]
    Session,
    #[error("the PDS or token endpoint is not permitted")]
    Endpoint,
    #[error("the requested audience or method is invalid")]
    InvalidRequest,
    #[error("the PDS request failed")]
    Transport,
    #[error("the PDS refused the request with HTTP {0}")]
    Status(StatusCode),
    #[error("the PDS returned an invalid response")]
    Response,
    #[error("the OAuth grant could not be refreshed")]
    Refresh,
    #[error("an OAuth client assertion or DPoP proof could not be created")]
    Proof,
    #[error(transparent)]
    Store(#[from] StoreError),
}

/// What sign-out could revoke after removing the local session.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Revocation {
    /// The session was already absent or locally revoked.
    NoSession,
    /// The issuer advertised no revocation endpoint.
    LocalOnly,
    /// The issuer accepted the revocation request.
    Remote,
}

/// Uses the member's OAuth grant, never a password or a tenor-issued session.
pub struct PdsClient {
    fetcher: Fetcher,
    store: SessionStore,
    oauth: OAuthClient,
}

impl fmt::Debug for PdsClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PdsClient").finish_non_exhaustive()
    }
}

impl PdsClient {
    #[must_use]
    pub fn new(fetcher: Fetcher, store: SessionStore, oauth: OAuthClient) -> Self {
        Self { fetcher, store, oauth }
    }

    /// Ask the member's PDS for a token for exactly `audience` and `method`.
    ///
    /// The caller must send it only to the service named by `audience`. The
    /// receiving service verifies the signature and authorizes the member.
    /// Obtain `session` with [`super::session_cookie`] after the registry
    /// authorizes the request. Expired access tokens refresh under a
    /// cross-instance lock. Missing, expired or revoked sessions are refused.
    pub async fn service_token(&self, session: &str, audience: &str, method: &str) -> Result<String, PdsError> {
        let session = SessionRef::from_cookie(session).ok_or(PdsError::Session)?;
        self.service_token_by_ref(&session, audience, method).await
    }

    /// Obtain a service token for an already-authorized internal session.
    ///
    /// A session reference is a database identifier, not a browser credential.
    /// The session must still be live when a worker uses this reference.
    pub async fn service_token_by_ref(
        &self,
        session: &SessionRef,
        audience: &str,
        method: &str,
    ) -> Result<String, PdsError> {
        if !audience.starts_with("did:") || !atproto_record::nsid::is_valid_nsid(method) {
            return Err(PdsError::InvalidRequest);
        }
        let grant = self.fresh_grant(session).await?;
        let mut url = method_url(&self.fetcher, &grant.pds, GET_SERVICE_AUTH)?;
        let expiry = Utc::now()
            .timestamp()
            .saturating_add(SERVICE_TOKEN_LIFETIME_SECS)
            .to_string();
        url.query_pairs_mut()
            .append_pair("aud", audience)
            .append_pair("lxm", method)
            .append_pair("exp", &expiry);
        let response = resource_call(&self.fetcher, &grant, Method::GET, &url, None).await?;
        let body = successful_body(response)?;
        let answer: ServiceAuthResponse = serde_json::from_value(body).map_err(|_| PdsError::Response)?;
        check_service_token(&answer.token, &grant.did, audience, method)?;
        Ok(answer.token)
    }

    /// Create a record in the signed-in member's repo, not a caller-named repo.
    ///
    /// Obtain `session` with [`super::session_cookie`] after authorization.
    /// Expired access tokens refresh under a cross-instance lock. `rkey` is
    /// optional; when absent the PDS assigns a record key. The PDS validates
    /// the collection, record and supplied key. The returned JSON is the PDS
    /// `com.atproto.repo.createRecord` response, including its URI and CID.
    pub async fn create_record(
        &self,
        session: &str,
        collection: &str,
        rkey: Option<&str>,
        record: Value,
    ) -> Result<Value, PdsError> {
        let session = SessionRef::from_cookie(session).ok_or(PdsError::Session)?;
        self.create_record_by_ref(&session, collection, rkey, record).await
    }

    /// Create a record for an already-authorized internal session.
    ///
    /// The worker must retain only a session reference, never a browser
    /// cookie. Revoked and expired sessions cannot write through a reference.
    pub async fn create_record_by_ref(
        &self,
        session: &SessionRef,
        collection: &str,
        rkey: Option<&str>,
        record: Value,
    ) -> Result<Value, PdsError> {
        if !atproto_record::nsid::is_valid_nsid(collection) {
            return Err(PdsError::InvalidRequest);
        }
        let grant = self.fresh_grant(session).await?;
        let url = method_url(&self.fetcher, &grant.pds, CREATE_RECORD)?;
        let mut body = Map::new();
        body.insert("repo".to_owned(), Value::String(grant.did.clone()));
        body.insert("collection".to_owned(), Value::String(collection.to_owned()));
        body.insert("record".to_owned(), record);
        if let Some(rkey) = rkey {
            body.insert("rkey".to_owned(), Value::String(rkey.to_owned()));
        }
        let response = resource_call(&self.fetcher, &grant, Method::POST, &url, Some(&Value::Object(body))).await?;
        successful_body(response)
    }

    /// Delete the local session before attempting RFC 7009 grant revocation.
    ///
    /// A concurrent refresh is serialized with deletion. On remote failure
    /// the local session remains deleted and the error reaches the caller.
    pub async fn revoke(&self, session: &str) -> Result<Revocation, PdsError> {
        let Some(stored) = self.store.take_session::<Grant>(session).await? else {
            return Ok(Revocation::NoSession);
        };
        let grant = stored.grant;
        let Some(endpoint) = grant.revocation_endpoint.as_deref() else {
            return Ok(Revocation::LocalOnly);
        };
        validate_endpoint(&self.fetcher, &grant.issuer)?;
        let form = url::form_urlencoded::Serializer::new(String::new())
            .append_pair("client_id", &self.oauth.client_id)
            .append_pair("token", &grant.access_token)
            .append_pair("token_type_hint", "access_token")
            .finish();
        let response = authorization_call(
            &self.fetcher,
            &dpop_key(&grant),
            endpoint,
            &self.oauth,
            &grant.issuer,
            &form,
        )
        .await?;
        if !response.status.is_success() {
            return Err(PdsError::Status(response.status));
        }
        Ok(Revocation::Remote)
    }

    async fn fresh_grant(&self, session: &SessionRef) -> Result<Grant, PdsError> {
        let stored = self
            .store
            .get_session_by_ref::<Grant>(session)
            .await?
            .ok_or(PdsError::Session)?;
        if stored.grant.expires_at > Utc::now().timestamp().saturating_add(REFRESH_MARGIN_SECS) {
            return Ok(stored.grant);
        }

        // The store rereads the grant while holding its cross-instance lock. A
        // waiter uses a newer generation instead of spending its refresh token.
        let refreshed = self
            .store
            .refresh_by_ref(session, stored.generation, |grant: Grant| self.refresh_grant(grant))
            .await?
            .ok_or(PdsError::Session)?;
        if refreshed.grant.expires_at <= Utc::now().timestamp() {
            return Err(PdsError::Refresh);
        }
        Ok(refreshed.grant)
    }

    async fn refresh_grant(&self, mut grant: Grant) -> Result<Grant, PdsError> {
        // Both endpoints were established during login. Do not rediscover a
        // token endpoint from a PDS that may have changed since authorization.
        validate_endpoint(&self.fetcher, &grant.issuer)?;
        let key = dpop_key(&grant);
        let started_at = Utc::now().timestamp();
        let form = url::form_urlencoded::Serializer::new(String::new())
            .append_pair("client_id", &self.oauth.client_id)
            .append_pair("redirect_uri", &self.oauth.redirect_uri)
            .append_pair("grant_type", "refresh_token")
            .append_pair("refresh_token", &grant.refresh_token)
            .finish();
        let response = authorization_call(
            &self.fetcher,
            &key,
            &grant.token_endpoint,
            &self.oauth,
            &grant.issuer,
            &form,
        )
        .await?;
        let body = successful_body(response)?;
        let token: TokenResponse = serde_json::from_value(body).map_err(|_| PdsError::Response)?;
        if !token.token_type.eq_ignore_ascii_case("dpop")
            || token.sub.as_deref() != Some(grant.did.as_str())
            || token.access_token.is_empty()
            || token.expires_in == 0
            || !super::login::has_required_scope(&token.scope)
        {
            return Err(PdsError::Refresh);
        }
        let expires_at = started_at.saturating_add(i64::from(token.expires_in));
        if expires_at <= Utc::now().timestamp() {
            return Err(PdsError::Refresh);
        }
        grant.access_token = token.access_token;
        if let Some(refresh_token) = token.refresh_token {
            if refresh_token.is_empty() {
                return Err(PdsError::Refresh);
            }
            grant.refresh_token = refresh_token;
        }
        grant.scope = token.scope;
        grant.expires_at = expires_at;
        Ok(grant)
    }
}

/// Create the short-lived private-key JWT used for this authorization server.
fn client_assertion(oauth: &OAuthClient, issuer: &str) -> Result<String, PdsError> {
    let issued_at = u64::try_from(Utc::now().timestamp()).map_err(|_| PdsError::Proof)?;
    let header: Header = oauth
        .private_signing_key_data
        .clone()
        .try_into()
        .map_err(|_| PdsError::Proof)?;
    let claims = Claims::new(JoseClaims {
        issuer: Some(oauth.client_id.clone()),
        subject: Some(oauth.client_id.clone()),
        audience: Some(issuer.to_owned()),
        json_web_token_id: Some(Uuid::now_v7().to_string()),
        issued_at: Some(issued_at),
        expiration: Some(issued_at.saturating_add(60)),
        ..Default::default()
    });
    mint(&oauth.private_signing_key_data, &header, &claims).map_err(|_| PdsError::Proof)
}

#[derive(Deserialize)]
struct ServiceAuthResponse {
    token: String,
}

fn dpop_key(grant: &Grant) -> KeyData {
    KeyData::new(KeyType::P256Private, grant.dpop_private_key.clone())
}

fn validate_endpoint(fetcher: &Fetcher, endpoint: &str) -> Result<Url, PdsError> {
    let url = Url::parse(endpoint).map_err(|_| PdsError::Endpoint)?;
    if url.host().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || !matches!(url.scheme(), "http" | "https")
    {
        return Err(PdsError::Endpoint);
    }
    if !fetcher.allows_private_network() {
        validate_service_endpoint(endpoint).map_err(|_| PdsError::Endpoint)?;
    }
    fetcher.check_destination(&url).map_err(|_| PdsError::Endpoint)?;
    Ok(url)
}

fn method_url(fetcher: &Fetcher, pds: &str, method: &str) -> Result<Url, PdsError> {
    validate_endpoint(fetcher, pds)?;
    // `method` is one of the constants above, not an input to URL resolution.
    Url::parse(&format!("{}/xrpc/{method}", pds.trim_end_matches('/'))).map_err(|_| PdsError::Endpoint)
}

fn limits(fetcher: &Fetcher) -> CallLimits {
    CallLimits {
        // Fetcher's reqwest client already has its configured deadline.
        timeout: None,
        max_response_bytes: Some(fetcher.max_response_bytes()),
    }
}

async fn resource_call(
    fetcher: &Fetcher,
    grant: &Grant,
    method: Method,
    url: &Url,
    body: Option<&Value>,
) -> Result<XrpcResponse, PdsError> {
    fetcher.check_destination(url).map_err(|_| PdsError::Endpoint)?;
    let key = dpop_key(grant);
    let response = dpop_call_as(
        fetcher.client(),
        &key,
        DpopPresentation::Bound(&grant.access_token),
        method,
        url.as_str(),
        body.map(DpopBody::Json),
        &HeaderMap::new(),
        limits(fetcher),
    )
    .await
    .map_err(|_| PdsError::Transport)?;
    require_nonce(response)
}

/// Submit a confidential-client request with one explicit nonce retry.
///
/// Each attempt gets its own client assertion, because a server may consume
/// the first assertion's `jti` before it challenges the `DPoP` proof.
pub(super) async fn authorization_call(
    fetcher: &Fetcher,
    key: &KeyData,
    endpoint: &str,
    oauth: &OAuthClient,
    issuer: &str,
    form: &str,
) -> Result<XrpcResponse, PdsError> {
    validate_endpoint(fetcher, endpoint)?;
    let (proof, _, _) = auth_dpop(key, "POST", endpoint).map_err(|_| PdsError::Proof)?;
    let first = authorization_attempt(fetcher, endpoint, oauth, issuer, form, &proof).await?;
    if first.dpop_nonce().is_none_or(str::is_empty) {
        return Err(PdsError::Response);
    }
    let code = first.xrpc_error_fields().0;
    let explicit_proof_error = code.is_empty() || matches!(code.as_str(), "use_dpop_nonce" | "invalid_dpop_proof");
    // A bare 400/401 with a nonce does not establish that the server refused
    // the grant. Retrying an already-spent refresh token could revoke it.
    let challenged = matches!(first.status, StatusCode::BAD_REQUEST | StatusCode::UNAUTHORIZED)
        && explicit_proof_error
        && is_nonce_challenge(
            &code,
            first
                .headers
                .get(WWW_AUTHENTICATE)
                .and_then(|value| value.to_str().ok())
                .unwrap_or_default(),
            false,
        );
    if !challenged {
        return Ok(first);
    }
    let nonce = first.dpop_nonce().ok_or(PdsError::Response)?;
    let (proof, _, _) = dpop_with_nonce(key, "POST", endpoint, None, nonce).map_err(|_| PdsError::Proof)?;
    let response = authorization_attempt(fetcher, endpoint, oauth, issuer, form, &proof).await?;
    require_nonce(response)
}

// A DPoP response without a server nonce violates the atproto OAuth profile.
fn require_nonce(response: XrpcResponse) -> Result<XrpcResponse, PdsError> {
    if response.dpop_nonce().is_none_or(str::is_empty) {
        return Err(PdsError::Response);
    }
    Ok(response)
}

async fn authorization_attempt(
    fetcher: &Fetcher,
    endpoint: &str,
    oauth: &OAuthClient,
    issuer: &str,
    form: &str,
    proof: &str,
) -> Result<XrpcResponse, PdsError> {
    let assertion = client_assertion(oauth, issuer)?;
    let form = url::form_urlencoded::Serializer::new(form.to_owned())
        .append_pair(
            "client_assertion_type",
            "urn:ietf:params:oauth:client-assertion-type:jwt-bearer",
        )
        .append_pair("client_assertion", &assertion)
        .finish()
        .into_bytes();
    let mut headers = HeaderMap::new();
    let mut proof = HeaderValue::from_str(proof).map_err(|_| PdsError::Proof)?;
    proof.set_sensitive(true);
    headers.insert("dpop", proof);
    xrpc_call_with_limits(
        fetcher.client(),
        &Auth::None,
        Method::POST,
        endpoint,
        Some(DpopBody::Bytes {
            content_type: "application/x-www-form-urlencoded",
            data: form,
        }),
        &headers,
        limits(fetcher),
    )
    .await
    .map_err(|_| PdsError::Transport)
}

fn successful_body(response: XrpcResponse) -> Result<Value, PdsError> {
    if !response.status.is_success() {
        return Err(PdsError::Status(response.status));
    }
    response.body.ok_or(PdsError::Response)
}

fn check_service_token(token: &str, did: &str, audience: &str, method: &str) -> Result<(), PdsError> {
    let mut parts = token.split('.');
    let (Some(header), Some(encoded), Some(signature), None) = (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return Err(PdsError::Response);
    };
    if header.is_empty() || signature.is_empty() {
        return Err(PdsError::Response);
    }
    let claims = Claims::from_base64(encoded).map_err(|_| PdsError::Response)?;
    if claims.jose.issuer.as_deref() != Some(did) || claims.private.get("lxm").and_then(Value::as_str) != Some(method) {
        return Err(PdsError::Response);
    }
    let now = u64::try_from(Utc::now().timestamp()).map_err(|_| PdsError::Response)?;
    motet_xrpc::service_auth::check_claims(&claims, |candidate| candidate == audience, now)
        .map_err(|_| PdsError::Response)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    use atproto_identity::key::{generate_key, to_public};
    use atproto_oauth::jwt::verify;
    use axum::body::to_bytes;
    use axum::extract::{Request, State};
    use axum::response::{IntoResponse, Response};
    use axum::routing::{get, post};
    use axum::{Json, Router};
    use motet_net::Policy;
    use serde_json::json;
    use tokio::net::TcpListener;

    use super::*;

    const DID: &str = "did:plc:abcdefghijklmnopqrstuvwx";
    const AUDIENCE: &str = "did:plc:aaaaaaaaaaaaaaaaaaaaaaaa#motet_tenor";
    const METHOD: &str = "at.motet.pilot.play.openScene";
    const ACCESS: &str = "member-access-token";
    const NONCE: &str = "server-issued-nonce";
    const CLIENT_ID: &str = "https://app.example/oauth/client-metadata.json";

    #[derive(Clone)]
    struct Server {
        public_key: KeyData,
        client_public_key: KeyData,
        base: String,
        challenged: Arc<Mutex<HashMap<String, String>>>,
        assertions: Arc<Mutex<HashSet<String>>>,
        ambiguous_requests: Arc<AtomicUsize>,
    }

    async fn challenge(State(server): State<Server>, request: Request) -> Response {
        let uri = request.uri().clone();
        let method = request.method().clone();
        let proof = request
            .headers()
            .get("dpop")
            .expect("a DPoP proof is required")
            .to_str()
            .expect("proof header");
        let claims = verify(proof, &server.public_key).expect("proof has a valid signature");
        assert_eq!(claims.jose.http_method.as_deref(), Some(method.as_str()));
        assert_eq!(
            claims.jose.http_uri.as_deref(),
            Some(format!("{}{}", server.base, uri.path()).as_str())
        );

        if method == Method::GET {
            assert_eq!(
                request.headers().get("authorization").and_then(|v| v.to_str().ok()),
                Some("DPoP member-access-token")
            );
            assert_eq!(
                claims.jose.auth.as_deref(),
                Some(atproto_oauth::pkce::challenge(ACCESS).as_str())
            );
            let requested = Url::parse(&format!("{}{}", server.base, uri)).expect("request URL");
            let params: HashMap<_, _> = requested.query_pairs().into_owned().collect();
            assert_eq!(params.get("aud").map(String::as_str), Some(AUDIENCE));
            assert_eq!(params.get("lxm").map(String::as_str), Some(METHOD));
        } else {
            assert!(
                claims.jose.auth.is_none(),
                "authorization endpoint has no access-token binding"
            );
            assert!(request.headers().get("authorization").is_none());
            let body = to_bytes(request.into_body(), 4096).await.expect("form body");
            let form: HashMap<_, _> = url::form_urlencoded::parse(&body).into_owned().collect();
            assert_eq!(form.get("grant_type").map(String::as_str), Some("refresh_token"));
            assert_eq!(form.get("refresh_token").map(String::as_str), Some("secret"));
            assert_eq!(
                form.get("client_assertion_type").map(String::as_str),
                Some("urn:ietf:params:oauth:client-assertion-type:jwt-bearer")
            );
            let assertion = verify(
                form.get("client_assertion").expect("client assertion"),
                &server.client_public_key,
            )
            .expect("valid client assertion");
            assert_eq!(assertion.jose.issuer.as_deref(), Some(CLIENT_ID));
            assert_eq!(assertion.jose.audience.as_deref(), Some(server.base.as_str()));
            let assertion_id = assertion.jose.json_web_token_id.expect("assertion has a jti");
            // This server authenticates before asking for a DPoP nonce.
            assert!(
                server.assertions.lock().expect("test lock").insert(assertion_id),
                "assertion replay"
            );
        }

        let jti = claims.jose.json_web_token_id.expect("proof has a jti");
        let retry = if let Some(nonce) = claims.jose.nonce.as_deref() {
            let original = server
                .challenged
                .lock()
                .expect("test lock")
                .remove(uri.path())
                .expect("a challenge came first");
            assert_eq!(nonce, NONCE);
            assert_ne!(jti, original, "the retry must not replay the first proof");
            true
        } else {
            let previous = server
                .challenged
                .lock()
                .expect("test lock")
                .insert(uri.path().to_owned(), jti);
            assert!(previous.is_none());
            false
        };
        if retry {
            let mut response = Json(json!({"accepted": true})).into_response();
            response
                .headers_mut()
                .insert("dpop-nonce", HeaderValue::from_static(NONCE));
            return response;
        }
        let mut response = (StatusCode::UNAUTHORIZED, Json(json!({"error": "use_dpop_nonce"}))).into_response();
        response
            .headers_mut()
            .insert("dpop-nonce", HeaderValue::from_static(NONCE));
        if method == Method::GET {
            response.headers_mut().insert(
                WWW_AUTHENTICATE,
                HeaderValue::from_static("DPoP error=\"use_dpop_nonce\""),
            );
        }
        response
    }

    async fn ambiguous(State(server): State<Server>) -> Response {
        server.ambiguous_requests.fetch_add(1, Ordering::Relaxed);
        let mut response = (StatusCode::BAD_REQUEST, Json(json!({}))).into_response();
        response
            .headers_mut()
            .insert("dpop-nonce", HeaderValue::from_static(NONCE));
        response
    }

    async fn assert_response_boundaries(
        fetcher: &Fetcher,
        private_key: &KeyData,
        oauth: &OAuthClient,
        grant: &Grant,
        state: &Server,
    ) {
        // A 400 carrying a nonce but no DPoP error may follow a spent grant.
        // Do not send the same refresh token a second time in that case.
        let base = &state.base;
        let ambiguous = authorization_call(
            fetcher,
            private_key,
            &format!("{base}/oauth/ambiguous"),
            oauth,
            &grant.issuer,
            "grant_type=refresh_token&refresh_token=secret",
        )
        .await
        .expect("ambiguous response");
        assert_eq!(ambiguous.status, StatusCode::BAD_REQUEST);
        assert_eq!(state.ambiguous_requests.load(Ordering::Relaxed), 1);

        let no_nonce = Url::parse(&format!("{base}/xrpc/missing.example.nonce")).expect("test URL");
        assert!(matches!(
            resource_call(fetcher, grant, Method::GET, &no_nonce, None).await,
            Err(PdsError::Response)
        ));
        assert!(matches!(
            authorization_call(
                fetcher,
                private_key,
                &format!("{base}/oauth/missing-nonce"),
                oauth,
                &grant.issuer,
                ""
            )
            .await,
            Err(PdsError::Response)
        ));

        let oversized = Url::parse(&format!("{base}/xrpc/large.example.query")).expect("test URL");
        assert!(matches!(
            resource_call(fetcher, grant, Method::GET, &oversized, None).await,
            Err(PdsError::Transport)
        ));
    }

    /// A nonce retry must use fresh `DPoP` and client-assertion IDs. Otherwise an
    /// authorization server that consumes the assertion first refuses it.
    #[tokio::test]
    async fn pds_and_authorization_nonces_use_fresh_bound_proofs_and_bounded_responses() {
        let private_key = generate_key(KeyType::P256Private).expect("private key");
        let public_key = to_public(&private_key).expect("public key");
        let client_private_key = generate_key(KeyType::P256Private).expect("client key");
        let client_public_key = to_public(&client_private_key).expect("client public key");
        let oauth = OAuthClient {
            client_id: CLIENT_ID.to_owned(),
            redirect_uri: "https://app.example/oauth/callback".to_owned(),
            private_signing_key_data: client_private_key,
        };
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("test listener");
        let base = format!("http://{}", listener.local_addr().expect("listener address"));
        let state = Server {
            public_key,
            client_public_key,
            base: base.clone(),
            challenged: Arc::new(Mutex::new(HashMap::new())),
            assertions: Arc::new(Mutex::new(HashSet::new())),
            ambiguous_requests: Arc::new(AtomicUsize::new(0)),
        };
        let router = Router::new()
            .route("/xrpc/com.atproto.server.getServiceAuth", get(challenge))
            .route("/oauth/token", post(challenge))
            .route("/xrpc/large.example.query", get(|| async { "x".repeat(2048) }))
            .route(
                "/xrpc/missing.example.nonce",
                get(|| async { Json(json!({"ok": true})) }),
            )
            .route("/oauth/missing-nonce", post(|| async { Json(json!({"ok": true})) }))
            .route("/oauth/ambiguous", post(ambiguous))
            .with_state(state.clone());
        let task = tokio::spawn(async move { axum::serve(listener, router).await.expect("test server") });

        let mut policy = Policy::default().allow_private();
        policy.max_response_bytes = 256;
        let fetcher = Fetcher::new(policy).expect("fetcher");
        let grant = Grant {
            did: DID.to_owned(),
            handle: "member.example".to_owned(),
            pds: base.clone(),
            issuer: base.clone(),
            token_endpoint: format!("{base}/oauth/token"),
            revocation_endpoint: None,
            access_token: ACCESS.to_owned(),
            refresh_token: "secret".to_owned(),
            expires_at: Utc::now().timestamp() + 60,
            scope: "atproto".to_owned(),
            dpop_private_key: private_key.1.clone(),
        };
        let mut url = method_url(&fetcher, &grant.pds, GET_SERVICE_AUTH).expect("PDS method");
        url.query_pairs_mut()
            .append_pair("aud", AUDIENCE)
            .append_pair("lxm", METHOD);
        let resource = resource_call(&fetcher, &grant, Method::GET, &url, None)
            .await
            .expect("protected resource");
        assert!(resource.status.is_success());
        let auth = authorization_call(
            &fetcher,
            &private_key,
            &grant.token_endpoint,
            &oauth,
            &grant.issuer,
            "grant_type=refresh_token&refresh_token=secret",
        )
        .await
        .expect("authorization endpoint");
        assert!(auth.status.is_success());
        assert!(state.challenged.lock().expect("test lock").is_empty());
        assert_eq!(state.assertions.lock().expect("test lock").len(), 2);

        assert_response_boundaries(&fetcher, &private_key, &oauth, &grant, &state).await;
        task.abort();
    }

    /// A wrong audience or repeated assertion ID lets an authorization server
    /// refuse PAR and token requests, even with a valid confidential key.
    #[test]
    fn client_assertions_name_the_issuer_and_do_not_reuse_ids() {
        let key = generate_key(KeyType::P256Private).expect("private key");
        let public = to_public(&key).expect("public key");
        let client_id = "https://app.example/oauth/client-metadata.json";
        let issuer = "https://pds.example";
        let oauth = OAuthClient {
            client_id: client_id.to_owned(),
            redirect_uri: "https://app.example/oauth/callback".to_owned(),
            private_signing_key_data: key,
        };
        let first = client_assertion(&oauth, issuer).expect("first assertion");
        let second = client_assertion(&oauth, issuer).expect("second assertion");
        let first = verify(&first, &public).expect("first signature");
        let second = verify(&second, &public).expect("second signature");
        assert_eq!(first.jose.issuer.as_deref(), Some(client_id));
        assert_eq!(first.jose.subject.as_deref(), Some(client_id));
        assert_eq!(first.jose.audience.as_deref(), Some(issuer));
        assert!(first.jose.json_web_token_id.is_some());
        assert_ne!(first.jose.json_web_token_id, second.jose.json_web_token_id);
    }

    /// A PDS response for another issuer, service or method must not be
    /// passed on as if it were the one requested.
    #[test]
    fn service_tokens_remain_bound_to_the_member_audience_and_method() {
        let key = generate_key(KeyType::P256Private).expect("private key");
        let header: Header = key.clone().try_into().expect("JWT header");
        let now = u64::try_from(Utc::now().timestamp()).expect("time after epoch");
        let mut claims = Claims::new(JoseClaims {
            issuer: Some(DID.to_owned()),
            audience: Some(AUDIENCE.to_owned()),
            issued_at: Some(now),
            expiration: Some(now + 60),
            ..Default::default()
        });
        claims.private.insert("lxm".to_owned(), json!(METHOD));
        let token = mint(&key, &header, &claims).expect("service token");
        assert!(check_service_token(&token, DID, AUDIENCE, METHOD).is_ok());
        assert!(check_service_token(&token, "did:plc:someoneelse", AUDIENCE, METHOD).is_err());
        assert!(check_service_token(&token, DID, "did:plc:aaaaaaaaaaaaaaaaaaaaaaaa", METHOD).is_err());
        assert!(check_service_token(&token, DID, AUDIENCE, "at.motet.pilot.play.closeScene").is_err());

        claims.jose.audience = Some("did:web:tenor.example".to_owned());
        let bare_audience = mint(&key, &header, &claims).expect("service token for bare audience");
        assert!(check_service_token(&bare_audience, DID, "did:web:tenor.example", METHOD).is_ok());
        claims.jose.audience = Some(AUDIENCE.to_owned());

        claims.jose.expiration = Some(now + motet_xrpc::service_auth::MAX_LIFETIME_SECS + 1);
        let long_lived = mint(&key, &header, &claims).expect("long-lived token");
        assert!(check_service_token(&long_lived, DID, AUDIENCE, METHOD).is_err());
    }
}
