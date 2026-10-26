//! Service auth: the short-lived JWT an account's PDS mints when it proxies a
//! call to another service (`atproto-proxy`), or that one service mints to
//! call another.
//!
//! A token is accepted only if all of these hold:
//!
//! - it is signed by the `#atproto` key in the issuer's current DID document;
//! - its audience is one this service answers for (the AppView's DID, or for
//!   a tenor, a World it hosts with the tenor's service fragment);
//! - it names exactly one method (`lxm`), which the registry then matches
//!   against the method being called;
//! - it has an expiry, is not expired, and doesn't live longer than
//!   [`MAX_LIFETIME_SECS`].
//!
//! The issuer is the caller. A DID inside a request body never is.
//!
//! The released `atproto-xrpcs` extractor verifies signatures but leaves
//! audience and method to each handler, which is exactly the per-handler
//! check this crate exists to avoid, so verification is done here instead.

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use atproto_identity::key::identify_key;
use atproto_identity::model::{Document, VerificationMethod};
use atproto_identity::traits::IdentityResolver;
use atproto_oauth::errors::JWTError;
use atproto_oauth::jwt::{Claims, JwtValidationConfig};
use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use thiserror::Error;

use crate::server::Authenticate;
use crate::{Caller, XrpcError};

/// Longest lifetime accepted for a token. PDSes mint them for about a minute;
/// anything much longer is a credential someone is keeping around.
pub const MAX_LIFETIME_SECS: u64 = 300;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ServiceAuthError {
    #[error("not a JWT")]
    Malformed,
    #[error("the token names no issuer")]
    NoIssuer,
    #[error("the issuer's DID document couldn't be resolved")]
    Unresolvable,
    #[error("the issuer has no #atproto signing key")]
    NoSigningKey,
    #[error("the signature doesn't verify against the issuer's signing key")]
    BadSignature,
    #[error("the token is for another service")]
    WrongAudience,
    #[error("the token names no method")]
    NoMethod,
    #[error("the token has no expiry, has expired, or lives too long")]
    BadLifetime,
}

/// Checks a token's claims, after its signature has been verified. Pure, so
/// the rules are testable without keys or clocks.
pub fn check_claims(
    claims: &Claims,
    accepts_audience: impl Fn(&str) -> bool,
    now: u64,
) -> Result<Caller, ServiceAuthError> {
    let jose = &claims.jose;
    let iss = jose.issuer.as_deref().ok_or(ServiceAuthError::NoIssuer)?;
    let audience = jose
        .audience
        .as_deref()
        .filter(|aud| accepts_audience(aud))
        .ok_or(ServiceAuthError::WrongAudience)?;
    let lxm = claims
        .private
        .get("lxm")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .ok_or(ServiceAuthError::NoMethod)?;
    let exp = jose.expiration.ok_or(ServiceAuthError::BadLifetime)?;
    let starts = jose.issued_at.or(jose.not_before).unwrap_or(now);
    if exp <= now || exp.saturating_sub(starts.min(now)) > MAX_LIFETIME_SECS {
        return Err(ServiceAuthError::BadLifetime);
    }
    if jose.not_before.is_some_and(|nbf| now < nbf) {
        return Err(ServiceAuthError::BadLifetime);
    }
    Ok(Caller::Service {
        did: did_of(iss).to_owned(),
        lxm: lxm.to_owned(),
        audience: audience.to_owned(),
    })
}

/// Verifies `token` end to end: resolves the issuer, checks the signature
/// against its `#atproto` key, then [`check_claims`].
pub async fn verify(
    token: &str,
    resolver: &dyn IdentityResolver,
    accepts_audience: impl Fn(&str) -> bool,
    now: u64,
) -> Result<Caller, ServiceAuthError> {
    let unverified = unverified_claims(token)?;
    let iss = unverified.jose.issuer.as_deref().ok_or(ServiceAuthError::NoIssuer)?;
    let document = resolver
        .resolve(did_of(iss))
        .await
        .map_err(|_| ServiceAuthError::Unresolvable)?;
    let key = signing_key(&document).ok_or(ServiceAuthError::NoSigningKey)?;
    let key = identify_key(key).map_err(|_| ServiceAuthError::NoSigningKey)?;
    let config = JwtValidationConfig {
        now: Some(now),
        ..JwtValidationConfig::default()
    };
    let claims = atproto_oauth::jwt::verify_with_config(token, &key, &config).map_err(|error| {
        match error.downcast_ref::<JWTError>() {
            Some(JWTError::TokenExpired | JWTError::TokenNotValidYet | JWTError::InvalidTimestamp { .. }) => {
                ServiceAuthError::BadLifetime
            }
            Some(JWTError::MissingClaim { claim }) if claim == "exp" || claim == "iat" => ServiceAuthError::BadLifetime,
            _ => ServiceAuthError::BadSignature,
        }
    })?;
    check_claims(&claims, accepts_audience, now)
}

/// An [`Authenticate`] for services that accept service auth only: a bearer
/// token is verified with [`verify`], no credentials at all is anonymous.
pub fn authenticate(
    resolver: Arc<dyn IdentityResolver>,
    accepts_audience: Arc<dyn Fn(&str) -> bool + Send + Sync>,
) -> Authenticate {
    Arc::new(move |headers| {
        let token = headers.get(axum::http::header::AUTHORIZATION).map(|v| {
            v.to_str()
                .ok()
                .and_then(|v| v.strip_prefix("Bearer "))
                .map(str::to_owned)
        });
        let resolver = Arc::<dyn IdentityResolver>::clone(&resolver);
        let accepts_audience = Arc::<dyn Fn(&str) -> bool + Send + Sync>::clone(&accepts_audience);
        Box::pin(async move {
            match token {
                None => Ok(Caller::Anonymous),
                Some(None) => Err(XrpcError::AuthRequired),
                Some(Some(token)) => {
                    let now = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
                    verify(&token, resolver.as_ref(), |aud| accepts_audience(aud), now)
                        .await
                        .map_err(|_| XrpcError::AuthRequired)
                }
            }
        })
    })
}

/// The DID part of an issuer, which may carry a service fragment
/// (`did:plc:…#atproto_labeler`).
fn did_of(iss: &str) -> &str {
    iss.split_once('#').map_or(iss, |(did, _)| did)
}

fn unverified_claims(token: &str) -> Result<Claims, ServiceAuthError> {
    let mut parts = token.split('.');
    let (Some(_), Some(claims), Some(_), None) = (parts.next(), parts.next(), parts.next(), parts.next()) else {
        return Err(ServiceAuthError::Malformed);
    };
    let bytes = URL_SAFE_NO_PAD
        .decode(claims)
        .map_err(|_| ServiceAuthError::Malformed)?;
    serde_json::from_slice(&bytes).map_err(|_| ServiceAuthError::Malformed)
}

/// The key atproto signs with: the verification method whose id ends in
/// `#atproto`. Other keys in the document (labeler keys, say) never sign
/// service auth.
fn signing_key(document: &Document) -> Option<&str> {
    document.verification_method.iter().find_map(|method| match method {
        VerificationMethod::Multikey {
            id,
            public_key_multibase,
            ..
        } if id.ends_with("#atproto") => Some(public_key_multibase.as_str()),
        _ => None,
    })
}
