//! Outbound tenor calls and verified identity discovery for AppView writes.

use std::fmt;
use std::sync::Arc;

use atproto_identity::errors::ResolveError;
use atproto_identity::resolve::{HickoryDnsResolver, InputType, parse_input, resolve_handle};
use atproto_identity::traits::IdentityResolver;
use atproto_identity::validation::validate_service_endpoint;
use motet_net::Fetcher;
use reqwest::StatusCode;
use serde_json::Value;
use url::Url;

const TENOR_FRAGMENT: &str = "motet_tenor";

/// A resolved destination and the service-auth audience it represents.
/// Only this module can construct one, after checking its endpoint.
#[derive(Debug)]
pub struct Target {
    endpoint: Url,
    audience: String,
}

impl Target {
    /// The exact audience to request from the member's PDS.
    #[must_use]
    pub fn audience(&self) -> &str {
        &self.audience
    }

    /// The validated endpoint of this tenor.
    #[must_use]
    pub fn endpoint(&self) -> &str {
        self.endpoint.as_str()
    }
}

/// Failures that affect whether a write may be retried.
#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    #[error("the World or identity could not be resolved")]
    NotFound,
    #[error("the tenor forbids this operation")]
    Forbidden,
    #[error("{0}")]
    InvalidRequest(String),
    #[error("the scene changed since the supplied swapRecord")]
    InvalidSwap,
    #[error("the scene is already closed")]
    SceneClosed,
    #[error("the tenor rejected the service token")]
    Unauthorized,
    #[error("the identity or tenor service is unavailable")]
    Upstream,
}

/// Resolves World services and sends bounded XRPC requests without retaining tokens.
pub struct Client {
    fetcher: Fetcher,
    resolver: Arc<dyn IdentityResolver>,
    dns: HickoryDnsResolver,
    default: Target,
}

impl fmt::Debug for Client {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TenorClient")
            .field("default", &self.default)
            .finish_non_exhaustive()
    }
}

impl Client {
    /// Validate the configured creation service before accepting writes.
    pub fn new(
        fetcher: Fetcher,
        resolver: Arc<dyn IdentityResolver>,
        default_tenor_url: &str,
        default_tenor_did: &str,
    ) -> anyhow::Result<Self> {
        match parse_input(default_tenor_did) {
            Ok(InputType::Plc(did) | InputType::Web(did)) if did == default_tenor_did => {}
            _ => anyhow::bail!("the default tenor DID must be a supported bare DID"),
        }
        let endpoint = endpoint(&fetcher, default_tenor_url)
            .map_err(|()| anyhow::anyhow!("the default tenor URL is not permitted"))?;
        Ok(Self {
            fetcher,
            resolver,
            dns: HickoryDnsResolver::create_resolver(&[]),
            default: Target {
                endpoint,
                audience: default_tenor_did.to_owned(),
            },
        })
    }

    /// The configured creation service, whose audience is the bare service DID.
    #[must_use]
    pub fn default_target(&self) -> &Target {
        &self.default
    }

    /// Resolve a World's own tenor service, never a service named by a request.
    pub async fn world_target(&self, world: &str) -> Result<Target, ClientError> {
        match parse_input(world) {
            Ok(InputType::Plc(did) | InputType::Web(did)) if did == world => {}
            _ => return Err(ClientError::InvalidRequest("world must be a supported bare DID".into())),
        }
        let document = self
            .resolver
            .resolve(world)
            .await
            .map_err(|error| resolution_error(&error))?;
        if document.id != world {
            return Err(ClientError::NotFound);
        }
        let mut entries = document
            .service
            .iter()
            .filter(|service| service.id == "#motet_tenor" || service.id.strip_prefix(world) == Some("#motet_tenor"));
        let service = entries.next().ok_or(ClientError::NotFound)?;
        if entries.next().is_some() || service.r#type.as_str() != Some("MotetTenor") {
            return Err(ClientError::NotFound);
        }
        let address = service.service_endpoint.as_uri().ok_or(ClientError::NotFound)?;
        let endpoint = endpoint(&self.fetcher, address).map_err(|()| ClientError::Upstream)?;
        Ok(Target {
            endpoint,
            audience: format!("{world}#{TENOR_FRAGMENT}"),
        })
    }

    /// Resolve an account identifier and require both halves of a handle binding.
    pub async fn member_did(&self, identifier: &str) -> Result<String, ClientError> {
        let parsed = parse_input(identifier)
            .map_err(|_| ClientError::InvalidRequest("cast contains an invalid account identifier".into()))?;
        let (did, handle) = match parsed {
            InputType::Plc(did) | InputType::Web(did) if did == identifier => (did, None),
            InputType::Handle(handle) if handle.eq_ignore_ascii_case(identifier) => {
                // The SDK also makes an HTTPS request to this untrusted handle.
                let handle_url = Url::parse(&format!("https://{handle}"))
                    .map_err(|_| ClientError::InvalidRequest("cast contains an invalid handle".into()))?;
                self.fetcher
                    .check_destination(&handle_url)
                    .map_err(|_| ClientError::NotFound)?;
                let did = resolve_handle(self.fetcher.client(), &self.dns, &handle)
                    .await
                    .map_err(|error| match error {
                        ResolveError::DNSResolverUnavailable { .. } => ClientError::Upstream,
                        _ => ClientError::NotFound,
                    })?;
                (did, Some(handle))
            }
            _ => {
                return Err(ClientError::InvalidRequest(
                    "cast contains an invalid account identifier".into(),
                ));
            }
        };
        // The handle lookup can itself return an arbitrary DID.
        match parse_input(&did) {
            Ok(InputType::Plc(parsed) | InputType::Web(parsed)) if parsed == did => {}
            _ => return Err(ClientError::NotFound),
        }
        let document = self
            .resolver
            .resolve(&did)
            .await
            .map_err(|error| resolution_error(&error))?;
        if document.id != did
            || handle
                .as_deref()
                .is_some_and(|handle| !document.confirms_handle(handle))
        {
            return Err(ClientError::NotFound);
        }
        Ok(did)
    }

    /// Submit to a previously checked target. The token must name that target
    /// and the exact method; its issuer and permissions are checked by the tenor.
    pub async fn post(&self, target: &Target, bearer: &str, method: &str, body: &Value) -> Result<Value, ClientError> {
        if !atproto_record::nsid::is_valid_nsid(method) {
            return Err(ClientError::InvalidRequest("invalid tenor method".into()));
        }
        let url = Url::parse(&format!(
            "{}/xrpc/{method}",
            target.endpoint.as_str().trim_end_matches('/')
        ))
        .map_err(|_| ClientError::Upstream)?;
        self.fetcher
            .check_destination(&url)
            .map_err(|_| ClientError::Upstream)?;
        let response = self
            .fetcher
            .client()
            .post(url)
            .bearer_auth(bearer)
            .json(body)
            .send()
            .await
            .map_err(|_| ClientError::Upstream)?;
        self.response(response).await
    }

    /// Query a checked tenor target with method-bound member service auth.
    pub async fn query(
        &self,
        target: &Target,
        bearer: &str,
        method: &str,
        params: &[(&str, &str)],
    ) -> Result<Value, ClientError> {
        if !atproto_record::nsid::is_valid_nsid(method) {
            return Err(ClientError::InvalidRequest("invalid tenor method".into()));
        }
        let mut url = Url::parse(&format!(
            "{}/xrpc/{method}",
            target.endpoint.as_str().trim_end_matches('/')
        ))
        .map_err(|_| ClientError::Upstream)?;
        url.query_pairs_mut().extend_pairs(params.iter().copied());
        self.fetcher
            .check_destination(&url)
            .map_err(|_| ClientError::Upstream)?;
        let response = self
            .fetcher
            .client()
            .get(url)
            .bearer_auth(bearer)
            .send()
            .await
            .map_err(|_| ClientError::Upstream)?;
        self.response(response).await
    }

    async fn response(&self, mut response: reqwest::Response) -> Result<Value, ClientError> {
        let status = response.status();
        let limit = self.fetcher.max_response_bytes();
        if response.content_length().is_some_and(|length| length > limit) {
            return Err(ClientError::Upstream);
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|_| ClientError::Upstream)? {
            if u64::try_from(bytes.len())
                .map_err(|_| ClientError::Upstream)?
                .saturating_add(u64::try_from(chunk.len()).map_err(|_| ClientError::Upstream)?)
                > limit
            {
                return Err(ClientError::Upstream);
            }
            bytes.extend_from_slice(&chunk);
        }
        let value = serde_json::from_slice::<Value>(&bytes).ok();
        if !status.is_success() {
            return Err(response_error(status, value.as_ref()));
        }
        value.ok_or(ClientError::Upstream)
    }
}

// An unavailable directory is retryable; an absent DID is not.
fn resolution_error(error: &anyhow::Error) -> ClientError {
    match error.downcast_ref::<motet_net::FetchError>() {
        Some(motet_net::FetchError::Status(StatusCode::NOT_FOUND | StatusCode::GONE)) => ClientError::NotFound,
        _ => ClientError::Upstream,
    }
}

fn endpoint(fetcher: &Fetcher, address: &str) -> Result<Url, ()> {
    let url = Url::parse(address).map_err(|_| ())?;
    if url.host().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || !matches!(url.scheme(), "http" | "https")
    {
        return Err(());
    }
    if !fetcher.allows_private_network() {
        validate_service_endpoint(address).map_err(|_| ())?;
    }
    fetcher.check_destination(&url).map_err(|_| ())?;
    Ok(url)
}

fn response_error(status: StatusCode, body: Option<&Value>) -> ClientError {
    match status {
        StatusCode::UNAUTHORIZED => ClientError::Unauthorized,
        StatusCode::FORBIDDEN => ClientError::Forbidden,
        StatusCode::NOT_FOUND => ClientError::NotFound,
        StatusCode::BAD_REQUEST => {
            let code = body.and_then(|value| value.get("error")).and_then(Value::as_str);
            match code {
                Some("Forbidden") => ClientError::Forbidden,
                Some("NotFound") => ClientError::NotFound,
                Some("InvalidSwap") => ClientError::InvalidSwap,
                Some("SceneClosed") => ClientError::SceneClosed,
                _ => {
                    let message = body
                        .and_then(|value| value.get("message"))
                        .and_then(Value::as_str)
                        .map_or_else(
                            || "the tenor rejected the request".into(),
                            |message| message.chars().take(512).collect(),
                        );
                    ClientError::InvalidRequest(message)
                }
            }
        }
        _ => ClientError::Upstream,
    }
}
