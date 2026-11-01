//! Confidential OAuth sign-in authenticates a GM, never grants access to their repo.
//!
//! Adapted from `atproto-pds/src/oauth/delegation/{flow,identity}.rs` (MIT),
//! atproto-crates revision `09a868cdde938c789326f1db16ffc30bbf38c15e`.
//! Motet seals the pending state in the shard and rechecks the GM's current
//! identity authority. The parent consumes that state once and checks its
//! browser binding and lifetime before calling `finish`.

use std::sync::Arc;

use atproto_identity::key::{KeyData, KeyType, generate_key};
use atproto_identity::model::Document;
use atproto_identity::resolve::{HickoryDnsResolver, InputType, parse_input, resolve_handle};
use atproto_identity::traits::IdentityResolver;
use atproto_identity::validation::validate_service_endpoint;
use atproto_oauth::errors::OAuthClientError;
use atproto_oauth::resources::{
    AuthorizationServer, oauth_authorization_server, oauth_protected_resource, pds_resources,
};
use atproto_oauth::workflow::{
    OAuthClient, OAuthRequest, OAuthRequestState, generate_state, oauth_complete, oauth_init,
};
use chrono::{DateTime, Utc};
use motet_net::Fetcher;
use motet_rules::Did;
use serde::{Deserialize, Serialize};
use url::Url;

use super::Error;

pub(crate) struct Client {
    fetcher: Fetcher,
    resolver: Arc<dyn IdentityResolver>,
    dns: HickoryDnsResolver,
    oauth: OAuthClient,
}

pub(crate) struct Started {
    pub location: String,
    pub state: String,
    pub pending: Pending,
}

/// This entire value must be sealed; neither it nor its private key may be logged.
#[derive(Serialize, Deserialize)]
pub(crate) struct Pending {
    expected_did: String,
    handle: Option<String>,
    pds: String,
    issuer: String,
    authorization_endpoint: String,
    token_endpoint: String,
    pushed_authorization_request_endpoint: String,
    /// The OAuth state nonce. `DPoP` challenge nonces belong to the SDK middleware.
    nonce: String,
    pkce_verifier: String,
    dpop_private_key: Vec<u8>,
    created_at: i64,
}

impl Client {
    pub(crate) fn new(
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
            // The SDK owns its key. Keep one client instead of copying it per flow.
            oauth: OAuthClient {
                redirect_uri,
                client_id,
                private_signing_key_data: Arc::unwrap_or_clone(signing_key),
            },
        }
    }

    pub(crate) async fn start(&self, identifier: &str) -> Result<Started, Error> {
        let (expected_did, handle) =
            match parse_input(identifier).map_err(|_| Error::invalid("invalid_request", "enter a GM handle or DID"))? {
                InputType::Handle(handle) => {
                    let did = resolve_handle(self.fetcher.client(), &self.dns, &handle)
                        .await
                        .map_err(|_| Error::invalid("access_denied", "the GM handle could not be resolved"))?;
                    (did, Some(handle))
                }
                InputType::Plc(did) | InputType::Web(did) => (did, None),
            };
        let document = self.document(&expected_did).await?;
        confirm_handle(&document, handle.as_deref())?;
        let pds = self.pds_endpoint(&document)?.to_owned();
        let server = self.discover(&pds).await?;

        let (pkce_verifier, code_challenge) = atproto_oauth::pkce::generate();
        let request_state = OAuthRequestState {
            state: generate_state(),
            code_challenge,
            scope: "atproto".to_owned(),
        };
        let private_dpop_key = generate_key(KeyType::P256Private).map_err(|_| Error::Internal)?;
        let created_at = Utc::now().timestamp();
        let par = oauth_init(
            self.fetcher.client(),
            &self.oauth,
            &private_dpop_key,
            Some(&expected_did),
            &server,
            &request_state,
        )
        .await
        .map_err(oauth_error)?;

        let mut location = Url::parse(&server.authorization_endpoint)
            .map_err(|_| Error::invalid("access_denied", "the GM authorization endpoint is invalid"))?;
        location
            .query_pairs_mut()
            .append_pair("client_id", &self.oauth.client_id)
            .append_pair("request_uri", &par.request_uri);
        let KeyData(_, dpop_private_key) = private_dpop_key;
        Ok(Started {
            location: location.into(),
            state: request_state.state.clone(),
            pending: Pending {
                expected_did,
                handle,
                pds,
                issuer: server.issuer,
                authorization_endpoint: server.authorization_endpoint,
                token_endpoint: server.token_endpoint,
                pushed_authorization_request_endpoint: server.pushed_authorization_request_endpoint,
                nonce: request_state.state,
                pkce_verifier,
                dpop_private_key,
                created_at,
            },
        })
    }

    pub(crate) async fn finish(&self, pending: Pending, code: &str, state: &str, issuer: &str) -> Result<Did, Error> {
        if state != pending.nonce || issuer != pending.issuer || code.is_empty() {
            return Err(Error::invalid(
                "access_denied",
                "the GM callback does not match this login",
            ));
        }

        // Re-fetch the pinned issuer, not whichever issuer the PDS names now.
        // Otherwise the code and PKCE verifier could be sent to a substituted server.
        let server = self.authorization_server(&pending.issuer).await?;
        if server.token_endpoint != pending.token_endpoint
            || server.authorization_endpoint != pending.authorization_endpoint
            || server.pushed_authorization_request_endpoint != pending.pushed_authorization_request_endpoint
        {
            return Err(Error::invalid(
                "access_denied",
                "the GM authorization endpoints changed during login",
            ));
        }
        let private_dpop_key = KeyData::new(KeyType::P256Private, pending.dpop_private_key);
        let created_at = DateTime::from_timestamp(pending.created_at, 0).ok_or(Error::Internal)?;
        let lifetime = i64::try_from(super::state::LOGIN_TTL).map_err(|_| Error::Internal)?;
        let expires_at = created_at
            .checked_add_signed(chrono::Duration::seconds(lifetime))
            .ok_or(Error::Internal)?;
        let request = OAuthRequest {
            oauth_state: pending.nonce,
            subject: pending.expected_did,
            issuer: pending.issuer,
            pkce_verifier: pending.pkce_verifier,
            signing_public_key: String::new(),
            // Exchange receives the key separately; do not serialize private key
            // material into the SDK's unused storage field.
            dpop_private_key: String::new(),
            created_at,
            expires_at,
        };
        let token = oauth_complete(
            self.fetcher.client(),
            &self.oauth,
            &private_dpop_key,
            code,
            &request.subject,
            Some(issuer),
            &request,
            &server,
        )
        .await
        .map_err(oauth_error)?;
        let authenticated = token.token_type.eq_ignore_ascii_case("DPoP")
            && token.sub.as_deref() == Some(request.subject.as_str())
            && token.scope.split_ascii_whitespace().any(|scope| scope == "atproto");
        // Authentication is the only use of this grant. Never keep or refresh it.
        drop(token);
        drop(private_dpop_key);
        if !authenticated {
            return Err(Error::invalid(
                "access_denied",
                "the GM token did not authenticate the expected identity",
            ));
        }

        // Recheck authority without changing the exchange destination. The SDK's
        // verify_subject_authority uses strict discovery even for an explicit local PDS.
        let document = self.document(&request.subject).await?;
        confirm_handle(&document, pending.handle.as_deref())?;
        let pds = self.pds_endpoint(&document)?;
        if pds != pending.pds || self.discover(pds).await?.issuer != request.issuer {
            return Err(Error::invalid(
                "access_denied",
                "the GM identity authority changed during login",
            ));
        }
        Ok(Did::new(request.subject))
    }

    async fn document(&self, did: &str) -> Result<Document, Error> {
        // A token subject is a DID exactly as written, never a handle or a decorated DID.
        match parse_input(did) {
            Ok(InputType::Plc(parsed) | InputType::Web(parsed)) if parsed == did => {}
            _ => {
                return Err(Error::invalid(
                    "access_denied",
                    "the GM identity is not a supported DID",
                ));
            }
        }
        let document = self
            .resolver
            .resolve(did)
            .await
            .map_err(|_| Error::invalid("access_denied", "the GM DID could not be resolved"))?;
        if document.id != did {
            return Err(Error::invalid(
                "access_denied",
                "the resolved document belongs to another identity",
            ));
        }
        Ok(document)
    }

    fn pds_endpoint<'a>(&self, document: &'a Document) -> Result<&'a str, Error> {
        // The raw accessor is needed only so the same explicit development policy
        // can admit a loopback PDS. Every endpoint still passes our policy below.
        document
            .pds_endpoints()
            .into_iter()
            .find(|endpoint| self.permits_endpoint(endpoint))
            .ok_or_else(|| Error::invalid("access_denied", "the GM DID names no permitted PDS"))
    }

    async fn discover(&self, pds: &str) -> Result<AuthorizationServer, Error> {
        if !self.fetcher.allows_private_network() {
            let (_, server) = pds_resources(self.fetcher.client(), pds).await.map_err(oauth_error)?;
            return Ok(server);
        }

        // pds_resources always refuses loopback. These supported lower-level calls
        // preserve metadata checks without a process-wide unsafe URL-policy bypass.
        let resource = oauth_protected_resource(self.fetcher.client(), pds)
            .await
            .map_err(oauth_error)?;
        let issuer = resource
            .authorization_servers
            .first()
            .ok_or_else(|| Error::invalid("access_denied", "the GM PDS names no authorization server"))?;
        self.authorization_server(issuer).await
    }

    async fn authorization_server(&self, issuer: &str) -> Result<AuthorizationServer, Error> {
        if !self.permits_endpoint(issuer) {
            return Err(Error::invalid(
                "access_denied",
                "the GM authorization server URL is not permitted",
            ));
        }
        let server = oauth_authorization_server(self.fetcher.client(), issuer)
            .await
            .map_err(oauth_error)?;
        // Upstream skips endpoint policy when a caller explicitly names a local
        // issuer. Retain the URL shape checks in that supported development path.
        if [
            server.authorization_endpoint.as_str(),
            server.token_endpoint.as_str(),
            server.pushed_authorization_request_endpoint.as_str(),
        ]
        .into_iter()
        .any(|endpoint| !self.permits_endpoint(endpoint))
        {
            return Err(Error::invalid(
                "access_denied",
                "a GM authorization endpoint URL is not permitted",
            ));
        }
        Ok(server)
    }

    fn permits_endpoint(&self, endpoint: &str) -> bool {
        if !self.fetcher.allows_private_network() {
            return validate_service_endpoint(endpoint).is_ok();
        }
        Url::parse(endpoint).is_ok_and(|url| {
            matches!(url.scheme(), "http" | "https")
                && url.has_host()
                && url.username().is_empty()
                && url.password().is_none()
                && url.query().is_none()
                && url.fragment().is_none()
        })
    }
}

fn confirm_handle(document: &Document, handle: Option<&str>) -> Result<(), Error> {
    if handle.is_some_and(|handle| !document.confirms_handle(handle)) {
        return Err(Error::invalid(
            "access_denied",
            "the GM DID does not claim the resolved handle",
        ));
    }
    Ok(())
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
        // Peer error bodies can contain secrets. Do not log or return them.
        _ => Error::invalid("access_denied", "the GM authorization server could not complete login"),
    }
}
