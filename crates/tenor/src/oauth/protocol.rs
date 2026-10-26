//! atproto OAuth proof checks, following atproto-pds `oauth/{par,token,client_auth,dpop}.rs`
//! at 09a868cd (MIT). Network keys are fetched through motet-net; replay and nonce
//! decisions commit with the operation they authorize, under the shard's o/ prefix.

use atproto_oauth::dpop::{DpopValidationConfig, validate_dpop_jwt};
use atproto_oauth::jwk::{WrappedJsonWebKeySet, thumbprint, to_key_data};
use atproto_oauth::jwt::Header;
use axum::http::{HeaderMap, header};
use base64::Engine as _;
use motet_net::Fetcher;
use serde::Deserialize;
use serde::de::DeserializeOwned;
use url::Url;

use super::state::{self, Entry, digest};
use super::{Error, Server};
use crate::shard::{Op, Txn};
use crate::store::keys;

const ASSERTION_TYPE: &str = "urn:ietf:params:oauth:client-assertion-type:jwt-bearer";

#[derive(Deserialize)]
pub(crate) struct Metadata {
    pub client_id: String,
    pub client_name: Option<String>,
    pub redirect_uris: Vec<String>,
    pub grant_types: Vec<String>,
    pub response_types: Vec<String>,
    pub scope: String,
    pub token_endpoint_auth_method: String,
    pub token_endpoint_auth_signing_alg: Option<String>,
    pub dpop_bound_access_tokens: bool,
    pub jwks_uri: Option<String>,
    pub jwks: Option<serde_json::Value>,
}

pub(crate) async fn metadata(fetcher: &Fetcher, client_id: &str) -> Result<Metadata, Error> {
    endpoint(fetcher, client_id)?;
    let bytes = fetcher
        .get_bytes(client_id)
        .await
        .map_err(|_| Error::invalid("invalid_client", "client metadata is unavailable"))?;
    let metadata: Metadata = serde_json::from_slice(&bytes)
        .map_err(|_| Error::invalid("invalid_client_metadata", "invalid client metadata document"))?;
    if metadata.client_id != client_id
        || !metadata.dpop_bound_access_tokens
        || !metadata.grant_types.iter().any(|grant| grant == "authorization_code")
        || !metadata.response_types.iter().any(|response| response == "code")
        || !matches!(metadata.token_endpoint_auth_method.as_str(), "none" | "private_key_jwt")
        || !metadata.scope.split_whitespace().any(|scope| scope == "atproto")
        || metadata.redirect_uris.is_empty()
    {
        return Err(Error::invalid(
            "invalid_client_metadata",
            "client metadata contradicts the atproto OAuth profile",
        ));
    }
    if metadata.token_endpoint_auth_method == "private_key_jwt"
        && (!matches!(
            metadata.token_endpoint_auth_signing_alg.as_deref(),
            Some("ES256" | "ES256K")
        ) || (metadata.jwks.is_none() && metadata.jwks_uri.is_none()))
    {
        return Err(Error::invalid(
            "invalid_client_metadata",
            "confidential client must publish a signing key and supported algorithm",
        ));
    }
    Ok(metadata)
}

pub(crate) fn endpoint(fetcher: &Fetcher, value: &str) -> Result<Url, Error> {
    let url = Url::parse(value).map_err(|_| Error::invalid("invalid_client_metadata", "invalid metadata URL"))?;
    if (!fetcher.allows_private_network() && url.scheme() != "https")
        || !matches!(url.scheme(), "https" | "http")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
    {
        return Err(Error::invalid(
            "invalid_client_metadata",
            "metadata URL must be an absolute HTTPS URL without credentials or fragment",
        ));
    }
    // Hyper connects to literals directly, without consulting motet-net's DNS
    // resolver. Apply the same address policy before either metadata fetch.
    if !fetcher.allows_private_network() {
        let permitted = match url.host() {
            Some(url::Host::Ipv4(addr)) => motet_net::is_public(std::net::IpAddr::V4(addr)),
            Some(url::Host::Ipv6(addr)) => motet_net::is_public(std::net::IpAddr::V6(addr)),
            _ => true,
        };
        if !permitted {
            return Err(Error::invalid(
                "invalid_client_metadata",
                "private metadata addresses are not permitted",
            ));
        }
    }
    Ok(url)
}

pub(crate) struct ClientProof {
    pub key: Option<String>,
    replay: Option<(String, u64)>,
}

impl ClientProof {
    pub(super) async fn consume(self, txn: &mut Txn<'_>) -> Result<(), Error> {
        if let Some((replay, expires)) = self.replay {
            if expires <= state::now() {
                return Err(Error::invalid("invalid_client", "client assertion has expired"));
            }
            let key = keys::oauth("assertion", &replay);
            if txn.get(&key).await?.is_some() {
                return Err(Error::invalid("invalid_client", "client assertion was already used"));
            }
            state::put(txn, key, &Entry { expires, value: () }, false)?;
        }
        Ok(())
    }
}

pub(crate) async fn client_auth(
    server: &Server,
    metadata: &Metadata,
    assertion_type: Option<&str>,
    assertion: Option<&str>,
    endpoint: &str,
) -> Result<ClientProof, Error> {
    let refused = || Error::invalid("invalid_client", "client authentication failed");
    if metadata.token_endpoint_auth_method == "none" {
        if assertion.is_some() || assertion_type.is_some() {
            return Err(refused());
        }
        return Ok(ClientProof {
            key: None,
            replay: None,
        });
    }
    if assertion_type != Some(ASSERTION_TYPE) {
        return Err(refused());
    }
    let assertion = assertion.ok_or_else(refused)?;
    let header: Header = jwt_part(assertion, 0).map_err(|_| refused())?;
    if header.algorithm != metadata.token_endpoint_auth_signing_alg {
        return Err(refused());
    }
    let mut keyset = match (&metadata.jwks, &metadata.jwks_uri) {
        (Some(keys), None) => keys.clone(),
        (None, Some(uri)) => {
            self::endpoint(&server.fetcher, uri)?;
            let bytes = server.fetcher.get_bytes(uri).await.map_err(|_| refused())?;
            serde_json::from_slice(&bytes).map_err(|_| refused())?
        }
        _ => return Err(refused()),
    };
    if keyset
        .get("keys")
        .and_then(serde_json::Value::as_array)
        .is_none_or(|keys| keys.iter().any(|key| key.get("d").is_some()))
    {
        return Err(refused());
    }
    let keyset: WrappedJsonWebKeySet = serde_json::from_value(keyset.take()).map_err(|_| refused())?;
    let mut candidates = keyset.keys.iter().filter(|key| {
        key.kid == header.key_id
            && key
                .alg
                .as_ref()
                .is_none_or(|alg| Some(alg) == header.algorithm.as_ref())
    });
    let key = candidates.next().ok_or_else(refused)?;
    if candidates.next().is_some() {
        return Err(refused());
    }
    let public = to_key_data(key).map_err(|_| refused())?;
    if header.algorithm.as_deref() != Some(atproto_identity::key::jws_alg(&public)) {
        return Err(refused());
    }
    let claims = verified_assertion(assertion, &public)?;
    let audience_matches = |value: &serde_json::Value| {
        value
            .as_str()
            .is_some_and(|aud| aud == server.issuer || aud == endpoint)
    };
    if claims.get("iss").and_then(serde_json::Value::as_str) != Some(metadata.client_id.as_str())
        || claims.get("sub").and_then(serde_json::Value::as_str) != Some(metadata.client_id.as_str())
        || !claims.get("aud").is_some_and(|aud| {
            audience_matches(aud) || aud.as_array().is_some_and(|items| items.iter().any(audience_matches))
        })
    {
        return Err(refused());
    }
    let jti = claims
        .get("jti")
        .and_then(serde_json::Value::as_str)
        .filter(|jti| !jti.is_empty())
        .ok_or_else(refused)?;
    let expires = assertion_expiry(&claims)?;
    let replay = digest(&format!("{}\0{jti}", metadata.client_id));
    Ok(ClientProof {
        key: Some(thumbprint(key).map_err(|_| refused())?),
        replay: Some((replay, expires)),
    })
}

fn verified_assertion(assertion: &str, key: &atproto_identity::key::KeyData) -> Result<serde_json::Value, Error> {
    let refused = || Error::invalid("invalid_client", "client assertion did not verify");
    let (input, signature) = assertion.rsplit_once('.').ok_or_else(refused)?;
    if input.bytes().filter(|byte| *byte == b'.').count() != 1 {
        return Err(refused());
    }
    let signature = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(signature)
        .map_err(|_| refused())?;
    // OAuth uses ordinary JWS, not AT Protocol's low-S signature restriction.
    atproto_identity::key::validate_with_policy(
        key,
        &signature,
        input.as_bytes(),
        atproto_identity::key::SignaturePolicy::AnyS,
    )
    .map_err(|_| refused())?;
    jwt_part(assertion, 1).map_err(|_| refused())
}

fn assertion_expiry(claims: &serde_json::Value) -> Result<u64, Error> {
    let refused = || {
        Error::invalid(
            "invalid_client",
            "client assertion is expired or has invalid time claims",
        )
    };
    let issued = claims.get("iat").and_then(numeric_date).ok_or_else(refused)?;
    let now = state::now();
    if issued > now.saturating_add(60) {
        return Err(refused());
    }
    let latest = issued.saturating_add(360);
    // atproto-oauth clients omit exp. Bound that signed assertion to five
    // minutes rather than treating a missing expiry as an immortal credential.
    let expires = match claims.get("exp") {
        None | Some(serde_json::Value::Null) => issued.saturating_add(300),
        Some(value) => numeric_date(value).ok_or_else(refused)?,
    }
    .min(latest);
    if expires <= now
        || claims
            .get("nbf")
            .filter(|value| !value.is_null())
            .is_some_and(|value| numeric_date(value).is_none_or(|not_before| not_before > now.saturating_add(60)))
    {
        return Err(refused());
    }
    Ok(expires)
}

pub(crate) struct Proof {
    pub jkt: String,
    replay: String,
    nonce: String,
    expires: u64,
}

impl Proof {
    pub(super) async fn consume(self, txn: &mut Txn<'_>) -> Result<(), Error> {
        if self.expires <= state::now() {
            return Err(Error::invalid("invalid_dpop_proof", "DPoP proof has expired"));
        }
        state::load::<()>(txn, &keys::oauth("nonce", &self.nonce), false)
            .await
            .map_err(|error| match error {
                Error::Refused { .. } => Error::invalid("invalid_dpop_proof", "DPoP nonce has expired"),
                other => other,
            })?;
        let key = keys::oauth("proof", &self.replay);
        if txn.get(&key).await?.is_some() {
            return Err(Error::invalid("invalid_dpop_proof", "DPoP proof was already used"));
        }
        state::put(
            txn,
            key,
            &Entry {
                expires: self.expires,
                value: (),
            },
            false,
        )
    }
}

struct Nonce;

impl Op for Nonce {
    type Output = String;
    type Error = Error;

    fn run(self, txn: &mut Txn<'_>) -> impl Future<Output = Result<String, Error>> + Send {
        std::future::ready(state::random().and_then(|nonce| {
            state::put(
                txn,
                keys::oauth("nonce", &digest(&nonce)),
                &Entry {
                    expires: state::now() + 60,
                    value: (),
                },
                false,
            )?;
            Ok(nonce)
        }))
    }
}

impl Server {
    pub(crate) async fn proof(
        &self,
        headers: &HeaderMap,
        method: &str,
        path: &str,
        access: Option<&str>,
    ) -> Result<Proof, Error> {
        let token = unique_header(headers, "dpop")?
            .ok_or_else(|| Error::invalid("invalid_dpop_proof", "DPoP proof is required"))?;
        let uri = format!("{}{path}", self.issuer);
        let config = access.map_or_else(
            || DpopValidationConfig::for_authorization(method, &uri),
            |token| DpopValidationConfig::for_resource_request(method, &uri, token),
        );
        let jkt = validate_dpop_jwt(token, &config)
            .map_err(|_| Error::invalid("invalid_dpop_proof", "DPoP proof did not verify"))?;
        let claims: serde_json::Value = jwt_part(token, 1)?;
        let nonce = match claims.get("nonce").and_then(serde_json::Value::as_str) {
            Some(nonce) => {
                let digest = digest(nonce);
                match state::read::<()>(self.shard.reader(), &keys::oauth("nonce", &digest), false).await {
                    Ok(_) => Some(digest),
                    Err(Error::Refused { .. }) => None,
                    Err(error) => return Err(error),
                }
            }
            None => None,
        };
        let Some(nonce) = nonce else {
            return Err(Error::Nonce {
                nonce: self.shard.run(Nonce).await??,
            });
        };
        let jti = claims
            .get("jti")
            .and_then(serde_json::Value::as_str)
            .filter(|jti| !jti.is_empty())
            .ok_or_else(|| Error::invalid("invalid_dpop_proof", "DPoP jti is empty"))?;
        let expires = claims
            .get("iat")
            .and_then(numeric_date)
            .ok_or(Error::Internal)?
            .saturating_add(90)
            .min(claims.get("exp").and_then(numeric_date).unwrap_or(u64::MAX));
        Ok(Proof {
            replay: digest(&format!("{jkt}\0{jti}")),
            jkt,
            nonce,
            expires,
        })
    }
}

// Match the validated proof's NumericDate interpretation, including fractions.
// Duration performs a checked float-to-integer conversion after truncation.
fn numeric_date(value: &serde_json::Value) -> Option<u64> {
    value.as_u64().or_else(|| {
        std::time::Duration::try_from_secs_f64(value.as_f64()?.trunc())
            .ok()
            .map(|value| value.as_secs())
    })
}

pub(crate) fn unique_header<'a>(headers: &'a HeaderMap, name: &str) -> Result<Option<&'a str>, Error> {
    let mut values = headers.get_all(name).iter();
    let value = values
        .next()
        .map(|value| value.to_str())
        .transpose()
        .map_err(|_| Error::invalid("invalid_request", "invalid HTTP header"))?;
    if values.next().is_some() {
        return Err(Error::invalid("invalid_request", "duplicate credential header"));
    }
    Ok(value)
}

fn jwt_part<T: DeserializeOwned>(token: &str, part: usize) -> Result<T, Error> {
    let encoded = token
        .split('.')
        .nth(part)
        .ok_or_else(|| Error::invalid("invalid_request", "malformed JWT"))?;
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(encoded)
        .map_err(|_| Error::invalid("invalid_request", "malformed JWT"))?;
    serde_json::from_slice(&bytes).map_err(|_| Error::invalid("invalid_request", "malformed JWT"))
}

/// Only `DPoP`-bound tokens reach a verified World session; Bearer is never an alias.
#[must_use]
pub fn authenticate(
    next: motet_xrpc::server::Authenticate,
    server: std::sync::Arc<Server>,
) -> motet_xrpc::server::Authenticate {
    std::sync::Arc::new(move |context| {
        let server = std::sync::Arc::clone(&server);
        let next = std::sync::Arc::clone(&next);
        Box::pin(async move {
            let authorization = unique_header(context.headers, header::AUTHORIZATION.as_str()).map_err(Error::xrpc)?;
            let Some(value) = authorization else {
                return next(context).await;
            };
            let Some((scheme, token)) = value.split_once(' ') else {
                return Err(motet_xrpc::XrpcError::AuthRequired);
            };
            if !scheme.eq_ignore_ascii_case("DPoP") {
                return next(context).await;
            }
            if token.is_empty() || token.contains(char::is_whitespace) {
                return Err(motet_xrpc::XrpcError::AuthRequired);
            }
            let proof = server
                .proof(
                    context.headers,
                    context.method.as_str(),
                    context.uri.path(),
                    Some(token),
                )
                .await
                .map_err(Error::xrpc)?;
            server
                .shard
                .run(state::Authenticate {
                    access_hash: digest(token),
                    proof,
                })
                .await
                .map_err(Error::from)
                .map_err(Error::xrpc)?
                .map_err(Error::xrpc)
        })
    })
}

/// Registration cannot authorize executable URLs or another native app's scheme.
/// Follows atproto-pds `oauth/client_metadata.rs` at the same pinned revision.
pub(crate) fn redirect(client_id: &str, redirect_uri: &str) -> Result<(), Error> {
    let refused = || {
        Error::invalid(
            "invalid_request",
            "redirect_uri is not permitted by the atproto OAuth profile",
        )
    };
    let url = Url::parse(redirect_uri).map_err(|_| refused())?;
    if url.fragment().is_some() || !url.username().is_empty() || url.password().is_some() {
        return Err(refused());
    }
    let allowed = match url.scheme() {
        "https" => url.has_host(),
        "http" => match url.host() {
            Some(url::Host::Ipv4(addr)) => addr.is_loopback(),
            Some(url::Host::Ipv6(addr)) => addr.is_loopback(),
            _ => false,
        },
        scheme => {
            let client = Url::parse(client_id).map_err(|_| refused())?;
            !url.has_host()
                && url.path().starts_with('/')
                && !url.path().starts_with("//")
                && client
                    .host_str()
                    .is_some_and(|host| host.rsplit('.').eq(scheme.split('.')))
        }
    };
    if allowed { Ok(()) } else { Err(refused()) }
}

#[cfg(test)]
mod tests {
    /// A signed assertion never becomes a long-lived client password.
    /// Catches trusting a generous expiry or the SDK's omitted expiry.
    #[test]
    fn client_assertion_age_remains_bounded_without_a_client_chosen_expiry() {
        let now = super::state::now();
        let absent = serde_json::json!({"iat": now});
        assert!(super::assertion_expiry(&absent).unwrap() <= now + 360);
        let generous = serde_json::json!({"iat": now, "exp": now + 86_400});
        assert!(super::assertion_expiry(&generous).unwrap() <= now + 360);
        for invalid in [
            serde_json::json!({"iat": now - 400, "exp": now + 86_400}),
            serde_json::json!({"iat": now + 1000, "exp": now + 86_400}),
            serde_json::json!({"iat": now, "exp": now - 1}),
            serde_json::json!({"iat": now, "exp": "tomorrow"}),
            serde_json::json!({"exp": now + 30}),
        ] {
            assert!(super::assertion_expiry(&invalid).is_err(), "{invalid}");
        }
    }

    /// Literal addresses bypass DNS filtering; metadata must not reach an
    /// internal HTTPS service merely because its `client_id` names an IP.
    #[test]
    fn metadata_literals_obey_the_same_private_address_policy_as_dns() {
        let public = motet_net::Fetcher::new(motet_net::Policy::default()).unwrap();
        let local = motet_net::Fetcher::new(motet_net::Policy::default().allow_private()).unwrap();
        for url in [
            "https://127.0.0.1/client",
            "https://10.0.0.1/client",
            "https://[::1]/jwks",
            "https://[::ffff:127.0.0.1]/jwks",
            "https://169.254.169.254/client",
        ] {
            assert!(super::endpoint(&public, url).is_err(), "{url}");
            assert!(super::endpoint(&local, url).is_ok(), "{url}");
        }
        assert!(super::endpoint(&public, "https://app.example/client.json").is_ok());
    }

    /// A client cannot register a script or another application's private scheme.
    /// Catches treating the untrusted redirect list as sufficient validation.
    #[test]
    fn redirect_registration_does_not_authorize_executable_or_foreign_schemes() {
        let client = "https://app.example.com/client.json";
        for uri in [
            "javascript:fetch('/oauth/consent')",
            "data:text/html,script",
            "file:///tmp/code",
            "com.attacker:/callback",
            "com.example.app://attacker/callback",
            "http://example.com/callback",
            "https://app.example.com/cb#fragment",
        ] {
            assert!(super::redirect(client, uri).is_err(), "{uri}");
        }
        for uri in [
            "https://app.example.com/callback",
            "http://127.0.0.1:8080/cb",
            "http://[::1]:8080/cb",
            "com.example.app:/callback",
        ] {
            assert!(super::redirect(client, uri).is_ok(), "{uri}");
        }
    }
}
