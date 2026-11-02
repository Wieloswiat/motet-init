//! Confidential OAuth sign-in against a person's PDS or entryway.
//!
//! Adapted from the tenor's former OAuth client, itself adapted from
//! `atproto-pds/src/oauth/delegation/{flow,identity}.rs` (MIT), atproto-crates
//! revision `09a868cdde938c789326f1db16ffc30bbf38c15e`.
//! The caller seals pending state, binds it to the browser, and consumes it once.

use std::fmt;
use std::sync::Arc;

use atproto_identity::key::{KeyData, KeyType, generate_key};
use atproto_identity::model::Document;
use atproto_identity::resolve::{HickoryDnsResolver, InputType, parse_input, resolve_handle};
use atproto_identity::traits::IdentityResolver;
use atproto_identity::validation::validate_service_endpoint;
use atproto_oauth::errors::OAuthClientError;
use atproto_oauth::resources::{AuthorizationServer, oauth_protected_resource, validate_authorization_server};
use atproto_oauth::scopes::{RepoAction, ScopesSet};
use atproto_oauth::workflow::{OAuthClient, ParResponse, TokenResponse, bind_token_subject, generate_state};
use chrono::Utc;
use motet_net::Fetcher;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use url::Url;

use super::pds::{Grant, authorization_call};

/// The client metadata and every PAR request must declare the same permissions.
pub const SCOPE: &str = concat!(
    "atproto ",
    "repo:at.motet.pilot.post ",
    "repo:at.motet.pilot.profile ",
    "repo:at.motet.pilot.character ",
    "rpc:at.motet.pilot.tenor.reserveWorld?aud=* ",
    "rpc:at.motet.pilot.tenor.createWorld?aud=* ",
    "rpc:at.motet.pilot.play.openScene?aud=* ",
    "rpc:at.motet.pilot.play.setSceneCast?aud=* ",
    "rpc:at.motet.pilot.play.closeScene?aud=* ",
    "rpc:at.motet.pilot.play.submitAdmission?aud=*",
);

const LOGIN_SECONDS: i64 = 600;
const MAX_METADATA_BYTES: usize = 256 * 1024;

/// A refusal or local failure without peer-supplied error bodies.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Invalid(&'static str),
    #[error("the OAuth client is unavailable")]
    Internal,
}

/// Configuration shared by sign-in requests. The signing key never leaves the server.
pub struct Client {
    fetcher: Fetcher,
    resolver: Arc<dyn IdentityResolver>,
    dns: HickoryDnsResolver,
    oauth: OAuthClient,
}

impl fmt::Debug for Client {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OAuthLoginClient").finish_non_exhaustive()
    }
}

/// The authorization redirect and the state the caller must seal before redirecting.
pub struct Started {
    pub location: String,
    pub state: String,
    pub pending: Pending,
}

impl fmt::Debug for Started {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Started").finish_non_exhaustive()
    }
}

/// Seal this whole value: the verifier and DPoP private key are credentials.
#[derive(Serialize, Deserialize)]
pub struct Pending {
    expected_did: Option<String>,
    handle: Option<String>,
    pds: String,
    issuer: String,
    authorization_endpoint: String,
    token_endpoint: String,
    pushed_authorization_request_endpoint: String,
    revocation_endpoint: Option<String>,
    state: String,
    pkce_verifier: String,
    dpop_private_key: Vec<u8>,
    created_at: i64,
}

impl fmt::Debug for Pending {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Pending").finish_non_exhaustive()
    }
}

// The SDK discards the optional revocation endpoint. Read it alongside the
// SDK's metadata so the same validated document pins all credential targets.
#[derive(Deserialize)]
struct Metadata {
    #[serde(flatten)]
    server: AuthorizationServer,
    revocation_endpoint: Option<String>,
}

impl Client {
    #[must_use]
    pub fn new(
        fetcher: Fetcher,
        resolver: Arc<dyn IdentityResolver>,
        client_id: String,
        redirect_uri: String,
        signing_key: Arc<KeyData>,
    ) -> Self {
        Self {
            fetcher,
            resolver,
            dns: HickoryDnsResolver::create_resolver(&[]),
            oauth: OAuthClient {
                redirect_uri,
                client_id,
                private_signing_key_data: Arc::unwrap_or_clone(signing_key),
            },
        }
    }

    /// Start with an account, a PDS URL, or the configured PDS for signup.
    /// Signup deliberately has no subject until the token response.
    pub async fn start(&self, identifier: Option<&str>, default_pds: &str, signup: bool) -> Result<Started, Error> {
        let (expected_did, handle, pds) = if signup {
            (None, None, pds_url(&self.fetcher, default_pds)?)
        } else if let Some(identifier) = identifier {
            let identifier = identifier.trim();
            if identifier
                .split_once("://")
                .is_some_and(|(scheme, _)| scheme.eq_ignore_ascii_case("https") || scheme.eq_ignore_ascii_case("http"))
            {
                (None, None, pds_url(&self.fetcher, identifier)?)
            } else {
                let (did, handle) = match parse_input(identifier)
                    .map_err(|_| Error::Invalid("enter a valid handle, DID or PDS URL"))?
                {
                    InputType::Handle(handle) => {
                        let did = self.resolve_account_handle(&handle).await?;
                        (did, Some(handle))
                    }
                    InputType::Plc(did) | InputType::Web(did) => (did, None),
                };
                let document = self.document(&did).await?;
                confirm_handle(&document, handle.as_deref())?;
                let pds = pds_endpoint(&self.fetcher, &document)?.to_owned();
                (Some(did), handle, pds)
            }
        } else {
            (None, None, pds_url(&self.fetcher, default_pds)?)
        };

        let metadata = self.discover(&pds).await?;
        let (pkce_verifier, code_challenge) = atproto_oauth::pkce::generate();
        let state = generate_state();
        let private_dpop_key = generate_key(KeyType::P256Private).map_err(|_| Error::Internal)?;
        let form = {
            let mut form = url::form_urlencoded::Serializer::new(String::new());
            form.append_pair("response_type", "code")
                .append_pair("code_challenge", &code_challenge)
                .append_pair("code_challenge_method", "S256")
                .append_pair("client_id", &self.oauth.client_id)
                .append_pair("state", &state)
                .append_pair("redirect_uri", &self.oauth.redirect_uri)
                .append_pair("scope", SCOPE);
            if let Some(hint) = handle.as_deref().or(expected_did.as_deref()) {
                form.append_pair("login_hint", hint);
            }
            if signup {
                form.append_pair("prompt", "create");
            }
            form.finish()
        };
        let par: ParResponse = oauth_response(
            authorization_call(
                &self.fetcher,
                &private_dpop_key,
                &metadata.server.pushed_authorization_request_endpoint,
                &self.oauth,
                &metadata.server.issuer,
                &form,
            )
            .await
            .map_err(|_| Error::Invalid("the authorization server could not complete login"))?,
        )?;
        if par.request_uri.is_empty() || par.expires_in == 0 {
            return Err(Error::Invalid("the authorization request was refused"));
        }

        let mut location = Url::parse(&metadata.server.authorization_endpoint)
            .map_err(|_| Error::Invalid("the authorization endpoint is invalid"))?;
        location
            .query_pairs_mut()
            .append_pair("client_id", &self.oauth.client_id)
            .append_pair("request_uri", &par.request_uri);
        let KeyData(_, dpop_private_key) = private_dpop_key;
        Ok(Started {
            location: location.into(),
            state: state.clone(),
            pending: Pending {
                expected_did,
                handle,
                pds,
                issuer: metadata.server.issuer,
                authorization_endpoint: metadata.server.authorization_endpoint,
                token_endpoint: metadata.server.token_endpoint,
                pushed_authorization_request_endpoint: metadata.server.pushed_authorization_request_endpoint,
                revocation_endpoint: metadata.revocation_endpoint,
                state,
                pkce_verifier,
                dpop_private_key,
                created_at: Utc::now().timestamp(),
            },
        })
    }

    /// Complete a browser-bound, already consumed pending request.
    pub async fn finish(&self, mut pending: Pending, code: &str, state: &str, issuer: &str) -> Result<Grant, Error> {
        let now = Utc::now().timestamp();
        if state != pending.state
            || issuer != pending.issuer
            || code.is_empty()
            || now < pending.created_at
            || now >= pending.created_at.saturating_add(LOGIN_SECONDS)
        {
            return Err(Error::Invalid("the callback does not match this login"));
        }

        // The code and verifier must go only to the server and endpoints that
        // received PAR, even if the PDS changes its metadata during login.
        let metadata = self.authorization_server(&pending.issuer).await?;
        if !matches_pending(&metadata, &pending) {
            return Err(Error::Invalid("the authorization endpoints changed during login"));
        }
        let private_dpop_key = KeyData::new(KeyType::P256Private, std::mem::take(&mut pending.dpop_private_key));
        let form = url::form_urlencoded::Serializer::new(String::new())
            .append_pair("client_id", &self.oauth.client_id)
            .append_pair("redirect_uri", &self.oauth.redirect_uri)
            .append_pair("grant_type", "authorization_code")
            .append_pair("code", code)
            .append_pair("code_verifier", &pending.pkce_verifier)
            .finish();
        let exchange_started_at = Utc::now().timestamp();
        let token: TokenResponse = oauth_response(
            authorization_call(
                &self.fetcher,
                &private_dpop_key,
                &pending.token_endpoint,
                &self.oauth,
                &pending.issuer,
                &form,
            )
            .await
            .map_err(|_| Error::Invalid("the authorization server could not complete login"))?,
        )?;
        if let Some(expected) = pending.expected_did.as_deref() {
            bind_token_subject(expected, &token).map_err(oauth_error)?;
        }
        let expires_at = exchange_started_at
            .checked_add(i64::from(token.expires_in))
            .ok_or(Error::Internal)?;
        if !token.token_type.eq_ignore_ascii_case("dpop")
            || token.access_token.is_empty()
            || token.expires_in == 0
            || !has_required_scope(&token.scope)
        {
            return Err(Error::Invalid("the authorization server returned an unusable grant"));
        }
        let refresh_token = token
            .refresh_token
            .filter(|token| !token.is_empty())
            .ok_or(Error::Invalid("the authorization server returned an unusable grant"))?;
        let did = token
            .sub
            .ok_or(Error::Invalid("the authorization server returned no subject"))?;
        let document = self.document(&did).await?;
        confirm_handle(&document, pending.handle.as_deref())?;
        let pds = pds_endpoint(&self.fetcher, &document)?;
        if pending.expected_did.is_some() && pds != pending.pds {
            return Err(Error::Invalid("the identity authority changed during login"));
        }

        // An entryway can authorize accounts on another PDS. Both the selected
        // resource and the subject's current PDS must still name this issuer.
        let selected = self.discover(&pending.pds).await?;
        if !matches_pending(&selected, &pending) {
            return Err(Error::Invalid("the identity authority changed during login"));
        }
        if pds != pending.pds && !matches_pending(&self.discover(pds).await?, &pending) {
            return Err(Error::Invalid("the identity authority changed during login"));
        }
        let handle = self.verified_handle(&document).await?;
        if let Some(expected) = pending.handle.as_deref()
            && !expected.eq_ignore_ascii_case(&handle)
        {
            let resolved = self.resolve_account_handle(expected).await?;
            if resolved != did {
                return Err(Error::Invalid("the handle no longer identifies this account"));
            }
        }
        if expires_at <= Utc::now().timestamp() {
            return Err(Error::Invalid("the authorization server returned an expired grant"));
        }
        let KeyData(_, dpop_private_key) = private_dpop_key;
        Ok(Grant {
            did,
            handle,
            pds: pds.to_owned(),
            issuer: pending.issuer,
            token_endpoint: pending.token_endpoint,
            revocation_endpoint: pending.revocation_endpoint,
            access_token: token.access_token,
            refresh_token,
            expires_at,
            scope: token.scope,
            dpop_private_key,
        })
    }

    async fn document(&self, did: &str) -> Result<Document, Error> {
        match parse_input(did) {
            Ok(InputType::Plc(parsed) | InputType::Web(parsed)) if parsed == did => {}
            _ => return Err(Error::Invalid("the authenticated subject is not a supported DID")),
        }
        let document = self
            .resolver
            .resolve(did)
            .await
            .map_err(|_| Error::Invalid("the identity could not be resolved"))?;
        if document.id != did {
            return Err(Error::Invalid("the resolved document belongs to another identity"));
        }
        Ok(document)
    }

    async fn verified_handle(&self, document: &Document) -> Result<String, Error> {
        let claimed = document
            .handles()
            .ok_or(Error::Invalid("the identity does not claim a handle"))?;
        let handle = match parse_input(claimed) {
            Ok(InputType::Handle(handle)) if handle.eq_ignore_ascii_case(claimed) => handle,
            _ => return Err(Error::Invalid("the identity claims an invalid handle")),
        };
        let resolved = self.resolve_account_handle(&handle).await?;
        if resolved != document.id {
            return Err(Error::Invalid("the handle does not identify this account"));
        }
        Ok(handle)
    }

    async fn resolve_account_handle(&self, handle: &str) -> Result<String, Error> {
        // The SDK also queries DNS, but its HTTP half always uses this host.
        permitted_endpoint(&self.fetcher, &format!("https://{handle}"))?;
        resolve_handle(self.fetcher.client(), &self.dns, handle)
            .await
            .map_err(|_| Error::Invalid("the handle could not be resolved"))
    }

    async fn discover(&self, pds: &str) -> Result<Metadata, Error> {
        permitted_endpoint(&self.fetcher, pds)?;
        let resource = oauth_protected_resource(self.fetcher.client(), pds)
            .await
            .map_err(oauth_error)?;
        let issuer = resource
            .authorization_servers
            .first()
            .ok_or(Error::Invalid("the PDS names no authorization server"))?;
        self.authorization_server(issuer).await
    }

    async fn authorization_server(&self, issuer: &str) -> Result<Metadata, Error> {
        permitted_endpoint(&self.fetcher, issuer)?;
        let url = format!("{issuer}/.well-known/oauth-authorization-server");
        let body = self
            .fetcher
            .get_bytes(&url)
            .await
            .map_err(|_| Error::Invalid("the authorization server could not be discovered"))?;
        if body.len() > MAX_METADATA_BYTES {
            return Err(Error::Invalid("the authorization server metadata is too large"));
        }
        let metadata: Metadata = serde_json::from_slice(&body)
            .map_err(|_| Error::Invalid("the authorization server metadata is invalid"))?;
        validate_authorization_server(&metadata.server, issuer).map_err(oauth_error)?;
        for endpoint in [
            metadata.server.authorization_endpoint.as_str(),
            metadata.server.token_endpoint.as_str(),
            metadata.server.pushed_authorization_request_endpoint.as_str(),
        ] {
            permitted_endpoint(&self.fetcher, endpoint)?;
        }
        if let Some(endpoint) = &metadata.revocation_endpoint {
            permitted_endpoint(&self.fetcher, endpoint)?;
        }
        Ok(metadata)
    }
}

fn pds_url(fetcher: &Fetcher, input: &str) -> Result<String, Error> {
    let url = permitted_endpoint(fetcher, input)?;
    Ok(url.as_str().trim_end_matches('/').to_owned())
}

fn permitted_endpoint(fetcher: &Fetcher, endpoint: &str) -> Result<Url, Error> {
    validate_service_endpoint(endpoint).map_err(|_| Error::Invalid("an OAuth endpoint URL is not permitted"))?;
    let url = Url::parse(endpoint).map_err(|_| Error::Invalid("an OAuth endpoint URL is not permitted"))?;
    fetcher
        .check_destination(&url)
        .map_err(|_| Error::Invalid("an OAuth endpoint URL is not permitted"))?;
    Ok(url)
}

fn pds_endpoint<'a>(fetcher: &Fetcher, document: &'a Document) -> Result<&'a str, Error> {
    // A different service of the same type is not the account's PDS.
    let pds = document
        .service
        .iter()
        .find(|service| {
            service.id == "#atproto_pds" || service.id.strip_prefix(document.id.as_str()) == Some("#atproto_pds")
        })
        .filter(|service| service.r#type.as_str() == Some("AtprotoPersonalDataServer"))
        .and_then(|service| service.service_endpoint.as_uri())
        .ok_or(Error::Invalid("the identity names no PDS"))?;
    permitted_endpoint(fetcher, pds)?;
    Ok(pds)
}

fn confirm_handle(document: &Document, handle: Option<&str>) -> Result<(), Error> {
    if handle.is_some_and(|handle| !document.confirms_handle(handle)) {
        return Err(Error::Invalid("the identity does not claim the resolved handle"));
    }
    Ok(())
}

fn matches_pending(metadata: &Metadata, pending: &Pending) -> bool {
    metadata.server.issuer == pending.issuer
        && metadata.server.authorization_endpoint == pending.authorization_endpoint
        && metadata.server.token_endpoint == pending.token_endpoint
        && metadata.server.pushed_authorization_request_endpoint == pending.pushed_authorization_request_endpoint
        && metadata.revocation_endpoint == pending.revocation_endpoint
}

pub(super) fn has_required_scope(scope: &str) -> bool {
    if !scope.split_ascii_whitespace().any(|item| item == "atproto") {
        return false;
    }
    let granted = ScopesSet::from_scope_string(scope);
    let repos = [
        "at.motet.pilot.post",
        "at.motet.pilot.profile",
        "at.motet.pilot.character",
    ];
    let methods = [
        "at.motet.pilot.tenor.reserveWorld",
        "at.motet.pilot.tenor.createWorld",
        "at.motet.pilot.play.openScene",
        "at.motet.pilot.play.setSceneCast",
        "at.motet.pilot.play.closeScene",
        "at.motet.pilot.play.submitAdmission",
    ];
    repos.into_iter().all(|collection| {
        [RepoAction::Create, RepoAction::Update, RepoAction::Delete]
            .into_iter()
            .all(|action| granted.allows_repo(collection, &action))
    }) && methods.into_iter().all(|method| granted.allows_rpc(method, "*"))
}

fn oauth_response<T: DeserializeOwned>(response: atproto_client::client::XrpcResponse) -> Result<T, Error> {
    if !response.status.is_success() {
        return Err(Error::Invalid("the authorization server could not complete login"));
    }
    let body = response
        .body
        .ok_or(Error::Invalid("the authorization server returned an invalid response"))?;
    serde_json::from_value(body).map_err(|_| Error::Invalid("the authorization server returned an invalid response"))
}

#[expect(
    clippy::needless_pass_by_value,
    reason = "map_err consumes the peer error; discard its secret-bearing details"
)]
fn oauth_error(error: OAuthClientError) -> Error {
    match error {
        OAuthClientError::JWTHeaderCreationFailed(_)
        | OAuthClientError::MintTokenFailed(_)
        | OAuthClientError::DpopTokenCreationFailed(_) => Error::Internal,
        _ => Error::Invalid("the authorization server could not complete login"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use motet_net::Policy;

    fn pending() -> Pending {
        Pending {
            expected_did: None,
            handle: None,
            pds: "https://pds.example.com".to_owned(),
            issuer: "https://issuer.example.com".to_owned(),
            authorization_endpoint: "https://issuer.example.com/authorize".to_owned(),
            token_endpoint: "https://issuer.example.com/token".to_owned(),
            pushed_authorization_request_endpoint: "https://issuer.example.com/par".to_owned(),
            revocation_endpoint: None,
            state: "expected-state".to_owned(),
            pkce_verifier: "verifier".to_owned(),
            dpop_private_key: Vec::new(),
            created_at: Utc::now().timestamp(),
        }
    }

    /// A substituted state or issuer must be rejected before any endpoint is contacted.
    #[tokio::test]
    async fn mismatched_callback_cannot_send_a_code_to_an_authorization_server() {
        let fetcher = Fetcher::new(Policy::default()).expect("the default network policy builds");
        let resolver = Arc::new(motet_net::identity::DidResolver::new(
            fetcher.clone(),
            "https://plc.example.com",
        ));
        let signing_key = Arc::new(generate_key(KeyType::P256Private).expect("the OS generates a signing key"));
        let client = Client::new(
            fetcher,
            resolver,
            "https://app.example.com/oauth/client-metadata.json".to_owned(),
            "https://app.example.com/oauth/callback".to_owned(),
            signing_key,
        );
        assert!(matches!(
            client
                .finish(pending(), "code", "other-state", "https://issuer.example.com")
                .await,
            Err(Error::Invalid("the callback does not match this login"))
        ));
        assert!(matches!(
            client
                .finish(pending(), "code", "expected-state", "https://other.example.com")
                .await,
            Err(Error::Invalid("the callback does not match this login"))
        ));
    }

    /// Repackaging equivalent grants must work, while a missing method must not
    /// leave a session that cannot perform the authorized World workflow.
    #[test]
    fn complete_permissions_accept_equivalent_scopes_but_refuse_a_missing_method() {
        let complete = concat!(
            "atproto ",
            "repo?collection=at.motet.pilot.post&collection=at.motet.pilot.profile",
            "&collection=at.motet.pilot.character&action=create&action=update&action=delete ",
            "rpc?lxm=at.motet.pilot.tenor.reserveWorld&lxm=at.motet.pilot.tenor.createWorld",
            "&lxm=at.motet.pilot.play.openScene&lxm=at.motet.pilot.play.setSceneCast",
            "&lxm=at.motet.pilot.play.closeScene&lxm=at.motet.pilot.play.submitAdmission&aud=*",
        );
        assert!(has_required_scope(complete));
        let incomplete = complete.replace("&lxm=at.motet.pilot.play.closeScene", "");
        assert!(!has_required_scope(&incomplete));
    }
}
