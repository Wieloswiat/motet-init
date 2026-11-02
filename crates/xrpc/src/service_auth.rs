//! Service auth: the short-lived JWT an account's PDS mints when it proxies a
//! call to another service (`atproto-proxy`), or that one service mints to
//! call another.
//!
//! A token is accepted only if all of these hold:
//!
//! - it is signed by the `#atproto` key in the resolved issuer DID document;
//! - its audience is one this service answers for (the AppView's DID, or for
//!   a tenor, a World it hosts with the tenor's service fragment);
//! - it names exactly one method (`lxm`), which the registry then matches
//!   against the method being called;
//! - it has an issuance time and expiry, is not expired, and doesn't live
//!   longer than [`MAX_LIFETIME_SECS`].
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
pub const MAX_LIFETIME_SECS: u64 = 60;

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
    #[error("the token names another method")]
    WrongMethod,
    #[error("the token's issuance or expiry time is invalid, or its lifetime is too long")]
    BadLifetime,
}

/// Checks claims against the audience and clock. This does not verify a
/// signature; only callers of [`verify`] may treat the returned issuer and
/// token identifier as authenticated.
pub fn check_claims(
    claims: &Claims,
    accepts_audience: impl Fn(&str) -> bool,
    now: u64,
) -> Result<Caller, ServiceAuthError> {
    Ok(checked_claims(claims, &accepts_audience, now)?.into_caller())
}

struct CheckedClaims<'a> {
    did: &'a str,
    lxm: &'a str,
    audience: &'a str,
    jti: Option<&'a str>,
    expires_at: u64,
}

impl CheckedClaims<'_> {
    fn into_caller(self) -> Caller {
        Caller::Service {
            did: self.did.to_owned(),
            lxm: self.lxm.to_owned(),
            audience: self.audience.to_owned(),
            jti: self.jti.map(str::to_owned),
            expires_at: self.expires_at,
        }
    }
}

fn checked_claims<'a>(
    claims: &'a Claims,
    accepts_audience: &impl Fn(&str) -> bool,
    now: u64,
) -> Result<CheckedClaims<'a>, ServiceAuthError> {
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
    let issued_at = jose.issued_at.ok_or(ServiceAuthError::BadLifetime)?;
    if exp <= now
        || exp <= issued_at
        || exp.saturating_sub(issued_at.min(now)) > MAX_LIFETIME_SECS
        || issued_at > now.saturating_add(60)
        || jose.not_before.is_some_and(|nbf| now < nbf)
    {
        return Err(ServiceAuthError::BadLifetime);
    }
    Ok(CheckedClaims {
        did: iss,
        lxm,
        audience,
        jti: jose.json_web_token_id.as_deref(),
        expires_at: exp,
    })
}

/// Verifies `token` end to end using `now` for every time check.
///
/// Rejects invalid claims and methods before resolving the issuer, then
/// verifies its `#atproto` signature and checks the signed claims again.
/// Signed tokens with invalid time claims return [`ServiceAuthError::BadLifetime`].
pub async fn verify(
    token: &str,
    resolver: &dyn IdentityResolver,
    accepts_audience: impl Fn(&str) -> bool,
    expected_method: &str,
    now: u64,
) -> Result<Caller, ServiceAuthError> {
    let unverified = unverified_claims(token)?;
    let unverified = checked_claims(&unverified, &accepts_audience, now)?;
    if unverified.lxm != expected_method {
        return Err(ServiceAuthError::WrongMethod);
    }
    let issuer = did_of(unverified.did);
    let document = resolver
        .resolve(issuer)
        .await
        .map_err(|_| ServiceAuthError::Unresolvable)?;
    if document.id != issuer {
        return Err(ServiceAuthError::Unresolvable);
    }
    let key = signing_key(&document).ok_or(ServiceAuthError::NoSigningKey)?;
    let key = identify_key(key).map_err(|_| ServiceAuthError::NoSigningKey)?;
    let config = JwtValidationConfig {
        require_issued_at: true,
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
    let checked = checked_claims(&claims, &accepts_audience, now)?;
    if checked.lxm != expected_method {
        return Err(ServiceAuthError::WrongMethod);
    }
    Ok(checked.into_caller())
}

/// An [`Authenticate`] for services that accept service auth only: a bearer
/// token is verified with [`verify`], no credentials at all is anonymous.
pub fn authenticate(
    resolver: Arc<dyn IdentityResolver>,
    accepts_audience: Arc<dyn Fn(&str) -> bool + Send + Sync>,
) -> Authenticate {
    Arc::new(move |request| {
        let token = request
            .headers
            .get(axum::http::header::AUTHORIZATION)
            .map(|v| v.to_str().ok().and_then(|v| v.strip_prefix("Bearer ")));
        let resolver = Arc::<dyn IdentityResolver>::clone(&resolver);
        let accepts_audience = Arc::<dyn Fn(&str) -> bool + Send + Sync>::clone(&accepts_audience);
        Box::pin(async move {
            match token {
                None => Ok(Caller::Anonymous),
                Some(None) => Err(XrpcError::AuthRequired),
                Some(Some(token)) => {
                    let now = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
                    verify(token, resolver.as_ref(), |aud| accepts_audience(aud), request.nsid, now)
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

/// Atproto requires both the method ID and its controller to belong to the
/// issuer. The identity library's fragment lookup does not check controller.
fn signing_key(document: &Document) -> Option<&str> {
    document.verification_method.iter().find_map(|method| match method {
        VerificationMethod::Multikey {
            id,
            controller,
            public_key_multibase,
            ..
        } if controller == &document.id
            && (id == "#atproto" || id.strip_prefix(document.id.as_str()) == Some("#atproto")) =>
        {
            Some(public_key_multibase.as_str())
        }
        _ => None,
    })
}
