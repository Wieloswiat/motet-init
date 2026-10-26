//! Atomic OAuth transitions over the shard; no network runs on its writer.

use std::sync::Arc;

use atproto_identity::key::{KeyData, KeyType, generate_key};
use atproto_oauth::jwt::{Claims, Header, JoseClaims};
use bytes::Bytes;
use motet_rules::Did;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use super::protocol::{ClientProof, Proof};
use super::{Error, grants, login};
use crate::shard::{Op, Txn};
use crate::store::{Reader, keys};

pub(crate) const LOGIN_TTL: u64 = 600;
pub(crate) const PAR_TTL: u64 = 90;
const CODE_TTL: u64 = 60;
const ACCESS_TTL: u64 = 300;
const SESSION_TTL: u64 = 14 * 24 * 60 * 60;

pub(crate) fn now() -> u64 {
    chrono::Utc::now().timestamp().unsigned_abs()
}

pub(crate) fn random() -> Result<String, Error> {
    use base64::Engine as _;
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|_| Error::Internal)?;
    Ok(base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes))
}

pub(crate) fn digest(value: &str) -> String {
    atproto_oauth::pkce::challenge(value)
}

#[derive(Serialize, Deserialize)]
pub(crate) struct Entry<T> {
    pub expires: u64,
    pub value: T,
}

pub(crate) async fn read<T: DeserializeOwned>(reader: &Reader, key: &[u8], sealed: bool) -> Result<Entry<T>, Error> {
    let raw = reader.get(key).await?.ok_or_else(expired)?;
    let opened;
    let bytes = if sealed {
        opened = reader.unseal_secret(key, &raw)?;
        opened.as_slice()
    } else {
        raw.as_ref()
    };
    decode(bytes)
}

pub(crate) async fn load<T: DeserializeOwned>(txn: &Txn<'_>, key: &[u8], sealed: bool) -> Result<Entry<T>, Error> {
    let raw = txn.get(key).await?.ok_or_else(expired)?;
    let opened;
    let bytes = if sealed {
        opened = txn.unseal_secret(key, &raw)?;
        opened.as_slice()
    } else {
        raw.as_ref()
    };
    decode(bytes)
}

fn decode<T: DeserializeOwned>(bytes: &[u8]) -> Result<Entry<T>, Error> {
    let entry: Entry<T> = serde_json::from_slice(bytes)?;
    if entry.expires <= now() {
        return Err(expired());
    }
    Ok(entry)
}

fn expired() -> Error {
    Error::invalid("invalid_grant", "authorization is unknown, used or expired")
}

pub(crate) fn put<T: Serialize>(txn: &mut Txn<'_>, key: Vec<u8>, entry: &Entry<T>, sealed: bool) -> Result<(), Error> {
    let bytes = serde_json::to_vec(entry)?;
    let bytes = if sealed {
        txn.seal_secret(&key, &bytes)?
    } else {
        bytes.into()
    };
    txn.put(keys::oauth_expiry(entry.expires, &key), Bytes::new());
    txn.put(key, bytes);
    Ok(())
}

fn remove(txn: &mut Txn<'_>, key: Vec<u8>, expires: u64) {
    txn.delete(keys::oauth_expiry(expires, &key));
    txn.delete(key);
}

pub(crate) struct Keys {
    pub signer: KeyData,
    pub client: KeyData,
}

pub(crate) struct Initialize;

impl Op for Initialize {
    type Output = Keys;
    type Error = Error;

    async fn run(self, txn: &mut Txn<'_>) -> Result<Keys, Error> {
        Ok(Keys {
            signer: signing_key(txn, keys::OAUTH_SIGNING_KEY).await?,
            client: signing_key(txn, keys::OAUTH_CLIENT_KEY).await?,
        })
    }
}

async fn signing_key(txn: &mut Txn<'_>, slot: &[u8]) -> Result<KeyData, Error> {
    if let Some(sealed) = txn.get(slot).await? {
        return Ok(KeyData::new(KeyType::P256Private, txn.unseal_secret(slot, &sealed)?));
    }
    let key = generate_key(KeyType::P256Private).map_err(|_| Error::Internal)?;
    let sealed = txn.seal_secret(slot, key.bytes())?;
    txn.put(slot.to_vec(), sealed);
    Ok(key)
}

#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct Request {
    pub world: Did,
    pub client_id: String,
    pub client_name: String,
    pub client_key: Option<String>,
    pub redirect_uri: String,
    pub scope: Vec<String>,
    pub state: Option<String>,
    pub challenge: String,
    pub dpop_jkt: Option<String>,
}

pub(crate) struct Push {
    pub request: Request,
    pub proof: Option<Proof>,
    pub client: ClientProof,
}

impl Op for Push {
    type Output = String;
    type Error = Error;

    async fn run(self, txn: &mut Txn<'_>) -> Result<String, Error> {
        if let Some(proof) = self.proof {
            proof.consume(txn).await?;
        }
        self.client.consume(txn).await?;
        let request_uri = format!("urn:ietf:params:oauth:request_uri:{}", random()?);
        put(
            txn,
            keys::oauth("par", &digest(&request_uri)),
            &Entry {
                expires: now() + PAR_TTL,
                value: self.request,
            },
            false,
        )?;
        Ok(request_uri)
    }
}

#[derive(Serialize, Deserialize)]
pub(crate) struct Authorization {
    pub request: Request,
    pub binding: String,
    pub csrf: String,
}

pub(crate) struct Begin {
    pub request_uri: String,
    pub client_id: String,
    pub binding: String,
    pub csrf: String,
    pub flow: String,
}

impl Op for Begin {
    type Output = Request;
    type Error = Error;

    async fn run(self, txn: &mut Txn<'_>) -> Result<Request, Error> {
        let key = keys::oauth("par", &digest(&self.request_uri));
        let entry: Entry<Request> = load(txn, &key, false).await?;
        if entry.value.client_id != self.client_id {
            return Err(Error::invalid("invalid_request", "client_id does not match PAR"));
        }
        remove(txn, key, entry.expires);
        let request = entry.value.clone();
        put(
            txn,
            keys::oauth("authorization", &digest(&self.flow)),
            &Entry {
                expires: now() + LOGIN_TTL,
                value: Authorization {
                    request: entry.value,
                    binding: digest(&self.binding),
                    csrf: digest(&self.csrf),
                },
            },
            false,
        )?;
        Ok(request)
    }
}

pub(crate) fn browser(auth: &Authorization, binding: &str, csrf: &str) -> Result<(), Error> {
    if auth.binding != digest(binding) || auth.csrf != digest(csrf) {
        return Err(Error::invalid(
            "access_denied",
            "browser binding or CSRF token does not match",
        ));
    }
    Ok(())
}

#[derive(Serialize, Deserialize)]
pub(crate) struct Parked {
    pub authorization: Authorization,
    pub pending: login::Pending,
}

pub(crate) struct Park {
    pub flow: String,
    pub binding: String,
    pub csrf: String,
    pub state: String,
    pub pending: login::Pending,
}

impl Op for Park {
    type Output = ();
    type Error = Error;

    async fn run(self, txn: &mut Txn<'_>) -> Result<(), Error> {
        let key = keys::oauth("authorization", &digest(&self.flow));
        let entry: Entry<Authorization> = load(txn, &key, false).await?;
        browser(&entry.value, &self.binding, &self.csrf)?;
        remove(txn, key, entry.expires);
        put(
            txn,
            keys::oauth("login", &digest(&self.state)),
            &Entry {
                expires: now() + LOGIN_TTL,
                value: Parked {
                    authorization: entry.value,
                    pending: self.pending,
                },
            },
            true,
        )
    }
}

pub(crate) struct CompleteLogin {
    pub state: String,
    pub binding: String,
}

impl Op for CompleteLogin {
    type Output = Parked;
    type Error = Error;

    async fn run(self, txn: &mut Txn<'_>) -> Result<Parked, Error> {
        let key = keys::oauth("login", &digest(&self.state));
        let entry: Entry<Parked> = load(txn, &key, true).await?;
        if entry.value.authorization.binding != digest(&self.binding) {
            return Err(Error::invalid("access_denied", "callback belongs to another browser"));
        }
        remove(txn, key, entry.expires);
        Ok(entry.value)
    }
}

#[derive(Serialize, Deserialize)]
pub(crate) struct Consent {
    pub authorization: Authorization,
    pub actor: Did,
    pub scopes: Vec<String>,
}

pub(crate) struct OfferConsent {
    pub flow: String,
    pub consent: Consent,
}

impl Op for OfferConsent {
    type Output = ();
    type Error = Error;

    fn run(self, txn: &mut Txn<'_>) -> impl Future<Output = Result<(), Error>> + Send {
        std::future::ready(put(
            txn,
            keys::oauth("consent", &digest(&self.flow)),
            &Entry {
                expires: now() + LOGIN_TTL,
                value: self.consent,
            },
            false,
        ))
    }
}

pub(crate) enum Decision {
    Approve,
    Deny,
}

pub(crate) struct Approve {
    pub flow: String,
    pub binding: String,
    pub csrf: String,
    pub decision: Decision,
}

#[derive(Serialize, Deserialize)]
pub(crate) struct Code {
    pub request: Request,
    pub actor: Did,
    pub scopes: Vec<String>,
}

impl Op for Approve {
    type Output = (Request, Option<String>);
    type Error = Error;

    async fn run(self, txn: &mut Txn<'_>) -> Result<Self::Output, Error> {
        let key = keys::oauth("consent", &digest(&self.flow));
        let entry: Entry<Consent> = load(txn, &key, false).await?;
        let consent = entry.value;
        browser(&consent.authorization, &self.binding, &self.csrf)?;
        remove(txn, key, entry.expires);
        let request = consent.authorization.request;
        if matches!(self.decision, Decision::Deny) {
            return Ok((request, None));
        }
        let scopes = grants::granted_txn(txn, &request.world, &consent.actor, &consent.scopes).await?;
        let code = random()?;
        put(
            txn,
            keys::oauth("code", &digest(&code)),
            &Entry {
                expires: now() + CODE_TTL,
                value: Code {
                    request: request.clone(),
                    actor: consent.actor,
                    scopes,
                },
            },
            false,
        )?;
        Ok((request, Some(code)))
    }
}

#[derive(Serialize, Deserialize)]
pub(crate) struct Session {
    pub world: Did,
    pub actor: Did,
    pub client_id: String,
    pub scopes: Vec<String>,
    pub dpop_jkt: String,
    pub client_key: Option<String>,
}

impl Session {
    fn bind(&self, client_id: &str, client: &ClientProof, proof: &Proof) -> Result<(), Error> {
        if self.client_id != client_id || self.dpop_jkt != proof.jkt || self.client_key != client.key {
            return Err(expired());
        }
        Ok(())
    }
}

#[derive(Deserialize)]
pub(crate) struct TokenInput {
    pub grant_type: String,
    pub client_id: String,
    pub code: Option<String>,
    pub redirect_uri: Option<String>,
    pub code_verifier: Option<String>,
    pub refresh_token: Option<String>,
    pub client_assertion_type: Option<String>,
    pub client_assertion: Option<String>,
}

#[derive(Serialize)]
pub(crate) struct TokenResponse {
    pub access_token: String,
    pub token_type: &'static str,
    pub expires_in: u64,
    pub refresh_token: String,
    pub scope: String,
    pub sub: String,
}

pub(crate) struct Exchange {
    pub input: TokenInput,
    pub client: ClientProof,
    pub proof: Proof,
    pub issuer: String,
    pub signer: Arc<KeyData>,
}

pub(crate) enum ExchangeOutput {
    Issued(TokenResponse),
    Reused(Revoke),
}

enum Refresh {
    Active {
        session_id: String,
        session: Entry<Session>,
    },
    Reused {
        session_id: String,
    },
}

// This operation succeeds after revocation. Returning invalid_grant from
// inside its transaction would roll back the security decision with the error.
pub(crate) struct Revoke {
    session_id: String,
    client_id: String,
    client: ClientProof,
    proof: Proof,
}

impl Op for Revoke {
    type Output = ();
    type Error = Error;

    async fn run(self, txn: &mut Txn<'_>) -> Result<(), Error> {
        let key = keys::oauth("session", &self.session_id);
        let session: Entry<Session> = load(txn, &key, false).await?;
        session.value.bind(&self.client_id, &self.client, &self.proof)?;
        self.proof.consume(txn).await?;
        self.client.consume(txn).await?;
        // Every access and refresh token resolves this session at use.
        remove(txn, key, session.expires);
        Ok(())
    }
}

impl Op for Exchange {
    type Output = ExchangeOutput;
    type Error = Error;

    async fn run(self, txn: &mut Txn<'_>) -> Result<ExchangeOutput, Error> {
        let (session_id, session) = match self.input.grant_type.as_str() {
            "authorization_code" => self.authorize(txn).await?,
            "refresh_token" => match self.refresh(txn).await? {
                Refresh::Active { session_id, session } => (session_id, session),
                Refresh::Reused { session_id } => {
                    return Ok(ExchangeOutput::Reused(Revoke {
                        session_id,
                        client_id: self.input.client_id,
                        client: self.client,
                        proof: self.proof,
                    }));
                }
            },
            _ => return Err(Error::invalid("unsupported_grant_type", "grant_type is not supported")),
        };
        self.proof.consume(txn).await?;
        self.client.consume(txn).await?;
        issue(txn, &self.issuer, &self.signer, &session_id, &session).map(ExchangeOutput::Issued)
    }
}

impl Exchange {
    async fn authorize(&self, txn: &mut Txn<'_>) -> Result<(String, Entry<Session>), Error> {
        let code = self.input.code.as_deref().ok_or_else(expired)?;
        let key = keys::oauth("code", &digest(code));
        let entry: Entry<Code> = load(txn, &key, false).await?;
        let auth = entry.value;
        let verifier = self.input.code_verifier.as_deref().ok_or_else(expired)?;
        if auth.request.client_id != self.input.client_id
            || self.input.redirect_uri.as_deref() != Some(auth.request.redirect_uri.as_str())
            || !(43..=128).contains(&verifier.len())
            || !verifier
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-._~".contains(&b))
            || atproto_oauth::pkce::challenge(verifier) != auth.request.challenge
            || auth.request.dpop_jkt.as_ref().is_some_and(|jkt| jkt != &self.proof.jkt)
            || auth.request.client_key != self.client.key
        {
            return Err(expired());
        }
        let session_id = digest(&random()?);
        let session = Entry {
            expires: now() + SESSION_TTL,
            value: Session {
                world: auth.request.world,
                actor: auth.actor,
                client_id: auth.request.client_id,
                scopes: auth.scopes,
                dpop_jkt: self.proof.jkt.clone(),
                client_key: self.client.key.clone(),
            },
        };
        put(txn, keys::oauth("session", &session_id), &session, false)?;
        remove(txn, key, entry.expires);
        Ok((session_id, session))
    }

    async fn refresh(&self, txn: &mut Txn<'_>) -> Result<Refresh, Error> {
        let refresh = self.input.refresh_token.as_deref().ok_or_else(expired)?;
        let hash = digest(refresh);
        let key = keys::oauth("refresh", &hash);
        let (entry, reused): (Entry<String>, _) = match txn.get(&key).await? {
            Some(raw) => (decode(&raw)?, false),
            None => (load(txn, &keys::oauth("used-refresh", &hash), false).await?, true),
        };
        let session: Entry<Session> = load(txn, &keys::oauth("session", &entry.value), false).await?;
        session.value.bind(&self.input.client_id, &self.client, &self.proof)?;
        if reused {
            return Ok(Refresh::Reused {
                session_id: entry.value,
            });
        }
        put(
            txn,
            keys::oauth("used-refresh", &hash),
            &Entry {
                expires: session.expires,
                value: &entry.value,
            },
            false,
        )?;
        remove(txn, key, entry.expires);
        Ok(Refresh::Active {
            session_id: entry.value,
            session,
        })
    }
}

fn issue(
    txn: &mut Txn<'_>,
    issuer: &str,
    signer: &KeyData,
    session_id: &str,
    entry: &Entry<Session>,
) -> Result<TokenResponse, Error> {
    let session = &entry.value;
    let now = now();
    let expires = (now + ACCESS_TTL).min(entry.expires);
    let scope = session.scopes.join(" ");
    let mut claims = Claims::new(JoseClaims {
        issuer: Some(issuer.into()),
        subject: Some(session.world.as_str().into()),
        audience: Some(issuer.into()),
        issued_at: Some(now),
        expiration: Some(expires),
        json_web_token_id: Some(random()?),
        ..JoseClaims::default()
    });
    claims.private.insert("act".into(), serde_json::json!(session.actor));
    claims
        .private
        .insert("client_id".into(), serde_json::json!(session.client_id));
    claims.private.insert("scope".into(), serde_json::json!(scope));
    claims
        .private
        .insert("cnf".into(), serde_json::json!({"jkt":session.dpop_jkt}));
    let header = Header {
        algorithm: Some("ES256".into()),
        type_: Some("at+jwt".into()),
        key_id: Some("as".into()),
        ..Header::default()
    };
    let access_token = atproto_oauth::jwt::mint(signer, &header, &claims).map_err(|_| Error::Internal)?;
    let refresh_token = random()?;
    put(
        txn,
        keys::oauth("access", &digest(&access_token)),
        &Entry {
            expires,
            value: session_id,
        },
        false,
    )?;
    put(
        txn,
        keys::oauth("refresh", &digest(&refresh_token)),
        &Entry {
            expires: entry.expires,
            value: session_id,
        },
        false,
    )?;
    Ok(TokenResponse {
        access_token,
        token_type: "DPoP",
        expires_in: expires - now,
        refresh_token,
        scope,
        sub: session.world.as_str().into(),
    })
}

pub(crate) struct Authenticate {
    pub access_hash: String,
    pub proof: Proof,
}

impl Op for Authenticate {
    type Output = motet_xrpc::Caller;
    type Error = Error;

    async fn run(self, txn: &mut Txn<'_>) -> Result<Self::Output, Error> {
        let access: Entry<String> = load(txn, &keys::oauth("access", &self.access_hash), false).await?;
        let entry: Entry<Session> = load(txn, &keys::oauth("session", &access.value), false).await?;
        let session = entry.value;
        if session.dpop_jkt != self.proof.jkt {
            return Err(Error::invalid("invalid_dpop_proof", "DPoP key does not match token"));
        }
        self.proof.consume(txn).await?;
        Ok(motet_xrpc::Caller::Session {
            world: session.world.as_str().into(),
            actor: session.actor.as_str().into(),
            client_id: session.client_id,
            scopes: session.scopes,
        })
    }
}

/// OAuth expiration is an ordinary durable operation, not an in-memory timer.
#[derive(Debug)]
pub struct Sweep {
    pub limit: usize,
}

impl Op for Sweep {
    type Output = usize;
    type Error = Error;

    async fn run(self, txn: &mut Txn<'_>) -> Result<usize, Error> {
        let rows = txn.scan(keys::OAUTH_EXPIRIES, self.limit).await?;
        let mut count = 0;
        for (key, _) in rows {
            let suffix = key.strip_prefix(keys::OAUTH_EXPIRIES).ok_or(Error::Internal)?;
            let (time, target) = suffix.split_at_checked(8).ok_or(Error::Internal)?;
            let expires = u64::from_be_bytes(time.try_into().map_err(|_| Error::Internal)?);
            if expires > now() {
                break;
            }
            txn.delete(target.to_vec());
            txn.delete(key.to_vec());
            count += 1;
        }
        Ok(count)
    }
}
