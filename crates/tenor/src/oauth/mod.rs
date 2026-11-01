//! World-DID OAuth sessions, attributed to the GM who authorized them.
//!
//! PAR, code exchange, refresh rotation and delegated sign-in follow
//! atproto-pds `oauth/{par,authorize,token}.rs` and `oauth/delegation/flow.rs`
//! at 09a868cd (MIT). Motet replaces SQL/`HttpState` with shard operations,
//! role-derived grants, sealed login state and hashed token indexes.

use std::sync::Arc;

use atproto_identity::key::KeyData;
use axum::Json;
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use motet_xrpc::XrpcError;
use thiserror::Error;

pub mod grants;
pub mod http;
mod login;
mod protocol;
mod state;
pub mod writes;

use crate::shard::Handle;

/// OAuth refusals remain separate from failures to make state durable.
#[derive(Debug, Error)]
pub enum Error {
    #[error("{description}")]
    Refused { code: &'static str, description: String },
    #[error("a fresh DPoP nonce is required")]
    Nonce { nonce: String },
    #[error("OAuth infrastructure failed")]
    Internal,
    #[error(transparent)]
    Txn(#[from] crate::shard::TxnError),
    #[error(transparent)]
    Read(#[from] crate::store::ReadError),
    #[error(transparent)]
    Submit(#[from] crate::shard::SubmitError),
    #[error(transparent)]
    Key(#[from] crate::keys::KeyError),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Space(#[from] crate::spaces::SpaceError),
    #[error(transparent)]
    Identity(#[from] crate::identity::IdentityError),
}

impl Error {
    pub(crate) fn invalid(code: &'static str, description: impl Into<String>) -> Self {
        Self::Refused {
            code,
            description: description.into(),
        }
    }

    fn xrpc(self) -> XrpcError {
        match self {
            Self::Nonce { nonce } => XrpcError::DpopNonceRequired { nonce },
            Self::Refused { code, description } => XrpcError::Protocol {
                status: StatusCode::UNAUTHORIZED,
                name: code,
                message: description,
            },
            _ => XrpcError::Internal,
        }
    }
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        let (status, code, description, nonce) = match self {
            Self::Refused { code, description } => {
                let status = match code {
                    "invalid_client" => StatusCode::UNAUTHORIZED,
                    "access_denied" => StatusCode::FORBIDDEN,
                    _ => StatusCode::BAD_REQUEST,
                };
                (status, code, description, None)
            }
            Self::Nonce { nonce } => (
                StatusCode::BAD_REQUEST,
                "use_dpop_nonce",
                "a fresh DPoP nonce is required".into(),
                Some(nonce),
            ),
            Self::Space(crate::spaces::SpaceError::NotAuthorized) => (
                StatusCode::FORBIDDEN,
                "access_denied",
                "your current roles do not allow a World session".into(),
                None,
            ),
            _ => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "server_error",
                "OAuth infrastructure failed".into(),
                None,
            ),
        };
        let mut response = (
            status,
            Json(serde_json::json!({"error":code,"error_description":description})),
        )
            .into_response();
        response
            .headers_mut()
            .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
        if let Some(nonce) = nonce.and_then(|nonce| HeaderValue::from_str(&nonce).ok()) {
            response.headers_mut().insert("dpop-nonce", nonce);
        }
        response
    }
}

/// One authorization server and confidential authentication client per tenor.
/// Keys are opened only in memory; neither private keys nor login secrets are logged.
pub struct Server {
    pub(crate) shard: Handle,
    pub(crate) issuer: String,
    pub(crate) fetcher: motet_net::Fetcher,
    signer: Arc<KeyData>,
    client_key: Arc<KeyData>,
    client: login::Client,
}

impl std::fmt::Debug for Server {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OAuthServer")
            .field("issuer", &self.issuer)
            .finish_non_exhaustive()
    }
}

impl Server {
    /// Initializes durable sealed keys before any OAuth endpoints are served.
    pub async fn open(
        shard: Handle,
        issuer: String,
        fetcher: motet_net::Fetcher,
        resolver: Arc<dyn atproto_identity::traits::IdentityResolver>,
    ) -> Result<Arc<Self>, Error> {
        let url = protocol::endpoint(&fetcher, &issuer)?;
        if url.path() != "/" || url.query().is_some() || issuer.ends_with('/') {
            return Err(Error::invalid(
                "invalid_request",
                "OAuth issuer must be a canonical origin",
            ));
        }
        let keys = shard.run(state::Initialize).await??;
        let signer = Arc::new(keys.signer);
        let client_key = Arc::new(keys.client);
        let client = login::Client::new(
            fetcher.clone(),
            resolver,
            format!("{issuer}/oauth/client-metadata.json"),
            format!("{issuer}/oauth/callback"),
            Arc::clone(&client_key),
        );
        Ok(Arc::new(Self {
            shard,
            issuer,
            fetcher,
            signer,
            client_key,
            client,
        }))
    }
}

/// Removes expired protocol state in bounded shard operations.
pub use state::Sweep;
