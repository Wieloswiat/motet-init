//! DID resolution through the hardened fetcher.
//!
//! A `did:web` names a host its controller chose, and a PLC directory is a
//! host Motet configures; either way the request goes through [`Fetcher`],
//! so identity resolution can't be pointed at an internal address. This
//! implements atproto-crates' `IdentityResolver`, so their code (service
//! auth, OAuth) resolves through it too.
//!
//! Handles are resolved elsewhere, and always confirmed against the DID
//! document's `alsoKnownAs` before being shown.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use atproto_identity::model::Document;
use atproto_identity::traits::IdentityResolver;

use crate::Fetcher;

// A cached signing key may be reused for at most this interval.
const DOCUMENT_CACHE_TTL: Duration = Duration::from_secs(5);
const DOCUMENT_CACHE_ENTRIES: usize = 128;
const DOCUMENT_CACHE_BYTES: usize = 1024 * 1024;

#[derive(Debug)]
struct CachedDocument {
    document: Arc<Document>,
    started: Instant,
    bytes: usize,
}

#[derive(Debug, Default)]
struct DocumentCache {
    entries: BTreeMap<String, CachedDocument>,
    bytes: usize,
}

impl DocumentCache {
    fn remove(&mut self, subject: &str) {
        if let Some(entry) = self.entries.remove(subject) {
            self.bytes -= entry.bytes;
        }
    }

    fn get(&mut self, subject: &str, now: Instant) -> Option<(Instant, Arc<Document>)> {
        let entry = self.entries.get(subject)?;
        if now.saturating_duration_since(entry.started) >= DOCUMENT_CACHE_TTL {
            self.remove(subject);
            return None;
        }
        Some((entry.started, Arc::clone(&entry.document)))
    }

    fn insert(&mut self, subject: &str, document: Arc<Document>, bytes: usize, started: Instant, now: Instant) {
        if bytes > DOCUMENT_CACHE_BYTES || now.saturating_duration_since(started) >= DOCUMENT_CACHE_TTL {
            return;
        }
        if self.entries.get(subject).is_some_and(|entry| entry.started >= started) {
            // An older in-flight response must not replace a newer document.
            return;
        }
        self.remove(subject);
        let total_bytes = &mut self.bytes;
        self.entries.retain(|_, entry| {
            if now.saturating_duration_since(entry.started) >= DOCUMENT_CACHE_TTL {
                *total_bytes -= entry.bytes;
                false
            } else {
                true
            }
        });
        while self.entries.len() >= DOCUMENT_CACHE_ENTRIES || self.bytes > DOCUMENT_CACHE_BYTES - bytes {
            let Some(oldest) = self
                .entries
                .iter()
                .min_by_key(|(_, entry)| entry.started)
                .map(|(subject, _)| subject.clone())
            else {
                break;
            };
            self.remove(&oldest);
        }
        self.bytes += bytes;
        self.entries.insert(
            subject.to_owned(),
            CachedDocument {
                document,
                started,
                bytes,
            },
        );
    }
}

/// Resolves `did:plc` through a PLC directory and `did:web` from the host it
/// names. Nothing else.
///
/// Successful documents are cached for at most five seconds from the start
/// of resolution, so key rotations can take that long to take effect. The
/// cache holds at most 128 documents and 1 MiB of document wire bytes plus
/// DID keys. Failures and oversized documents are not cached. Clones share a
/// cache; separately configured resolvers do not.
/// A slow response is refused once its freshness window has elapsed unless a
/// newer, still-fresh response has already been cached.
#[derive(Clone, Debug)]
pub struct DidResolver {
    fetcher: Fetcher,
    plc_directory: String,
    cache: Arc<Mutex<DocumentCache>>,
}

impl DidResolver {
    /// `plc_directory` is a base URL such as `https://plc.directory`.
    pub fn new(fetcher: Fetcher, plc_directory: impl Into<String>) -> Self {
        Self {
            fetcher,
            plc_directory: plc_directory.into().trim_end_matches('/').to_owned(),
            cache: Arc::new(Mutex::new(DocumentCache::default())),
        }
    }

    fn finish_resolution(
        &self,
        subject: &str,
        document: Document,
        body_bytes: usize,
        started: Instant,
    ) -> anyhow::Result<Document> {
        let bytes = body_bytes.saturating_add(subject.len());
        let newer = {
            let mut cache = self.cache.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            let now = Instant::now();
            if let Some((cached_at, cached)) = cache.get(subject, now)
                && cached_at >= started
            {
                Some((cached_at, cached))
            } else {
                if now.saturating_duration_since(started) >= DOCUMENT_CACHE_TTL {
                    anyhow::bail!("DID resolution exceeded its freshness limit");
                }
                if bytes <= DOCUMENT_CACHE_BYTES {
                    cache.insert(subject, Arc::new(document.clone()), bytes, started, now);
                }
                None
            }
        };
        if let Some((cached_at, cached)) = newer {
            let document = (*cached).clone();
            require_fresh(cached_at)?;
            return Ok(document);
        }
        require_fresh(started)?;
        Ok(document)
    }

    /// Where a DID's document is published, or `None` for a DID method Motet
    /// doesn't resolve.
    #[must_use]
    pub fn document_url(&self, did: &str) -> Option<String> {
        if let Some(id) = did.strip_prefix("did:plc:") {
            let valid = !id.is_empty() && id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit());
            return valid.then(|| format!("{}/{did}", self.plc_directory));
        }
        let authority = did.strip_prefix("did:web:")?;
        // A did:web port uses an encoded colon; no other URL syntax is allowed.
        let (host, port) = match authority.find('%') {
            Some(index) => {
                let (host, encoded_port) = authority.split_at(index);
                let port = encoded_port
                    .strip_prefix("%3A")
                    .or_else(|| encoded_port.strip_prefix("%3a"))?;
                if port.is_empty() || !port.bytes().all(|b| b.is_ascii_digit()) || port.parse::<u16>().is_err() {
                    return None;
                }
                (host, Some(port))
            }
            None => (authority, None),
        };
        let valid = !host.is_empty()
            && host
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-')
            && !host.starts_with('.')
            && !host.ends_with('.');
        valid.then(|| {
            port.map_or_else(
                || format!("https://{host}/.well-known/did.json"),
                |port| format!("https://{host}:{port}/.well-known/did.json"),
            )
        })
    }
}

fn require_fresh(started: Instant) -> anyhow::Result<()> {
    if started.elapsed() >= DOCUMENT_CACHE_TTL {
        anyhow::bail!("DID resolution exceeded its freshness limit");
    }
    Ok(())
}

#[async_trait]
impl IdentityResolver for DidResolver {
    async fn resolve(&self, subject: &str) -> anyhow::Result<Document> {
        let url = self
            .document_url(subject)
            .ok_or_else(|| anyhow::anyhow!("{subject} is not a DID Motet resolves"))?;
        let started = Instant::now();
        let cached = self
            .cache
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(subject, Instant::now());
        if let Some((cached_at, document)) = cached {
            let document = (*document).clone();
            require_fresh(cached_at)?;
            return Ok(document);
        }
        let body = self.fetcher.get_bytes(&url).await?;
        let document: Document = serde_json::from_slice(&body)?;
        if document.id != subject {
            anyhow::bail!("the document at {url} is for {}, not {subject}", document.id);
        }
        self.finish_resolution(subject, document, body.len(), started)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::{SystemTime, UNIX_EPOCH};

    use atproto_identity::key::{KeyData, KeyType, generate_key, to_public};
    use atproto_oauth::jwt::{Claims, Header, JoseClaims, mint};
    use axum::extract::{Path, State};
    use axum::http::StatusCode;
    use axum::routing::get;
    use proptest::prelude::*;
    use serde_json::json;
    use tokio::sync::oneshot;

    use super::*;
    use crate::Policy;

    fn resolver() -> DidResolver {
        DidResolver::new(
            Fetcher::new(Policy::default()).expect("client"),
            "https://plc.directory/",
        )
    }

    const ALICE: &str = "did:plc:alice";
    const AUDIENCE: &str = "did:web:tenor.example";
    const METHOD: &str = "at.motet.pilot.play.openScene";

    #[derive(Debug)]
    struct DirectoryState {
        public_key: String,
        calls: usize,
        failure: bool,
        malformed: bool,
        wrong_id: bool,
        first_started: Option<oneshot::Sender<()>>,
        release_first: Option<oneshot::Receiver<()>>,
    }

    fn public_key(key: &KeyData) -> String {
        let public = to_public(key).expect("generated key has a public half").to_string();
        public.strip_prefix("did:key:").unwrap_or(&public).to_owned()
    }

    fn token(key: &KeyData, now: u64) -> String {
        let mut claims = Claims::new(JoseClaims {
            issuer: Some(ALICE.into()),
            audience: Some(AUDIENCE.into()),
            issued_at: Some(now),
            expiration: Some(now + 60),
            ..JoseClaims::default()
        });
        claims.private.insert("lxm".into(), METHOD.into());
        let header = Header::try_from(key.clone()).expect("header for generated key");
        mint(key, &header, &claims).expect("service token")
    }

    async fn directory(key: &KeyData) -> (String, Arc<Mutex<DirectoryState>>, tokio::task::JoinHandle<()>) {
        let state = Arc::new(Mutex::new(DirectoryState {
            public_key: public_key(key),
            calls: 0,
            failure: false,
            malformed: false,
            wrong_id: false,
            first_started: None,
            release_first: None,
        }));
        let router = axum::Router::new()
            .route(
                "/{did}",
                get(
                    |State(state): State<Arc<Mutex<DirectoryState>>>, Path(did): Path<String>| async move {
                        let (response, release) = {
                            let mut state = state.lock().expect("fixture state");
                            state.calls += 1;
                            let release = state.release_first.take();
                            if let Some(first_started) = state.first_started.take() {
                                first_started.send(()).expect("waiting for first request");
                            }
                            let response = if state.failure {
                                (StatusCode::BAD_GATEWAY, "directory unavailable".to_owned())
                            } else if state.malformed {
                                (StatusCode::OK, "not json".to_owned())
                            } else {
                                let id = if state.wrong_id { "did:plc:mallory" } else { &did };
                                let body = json!({
                                    "id": id,
                                    "verificationMethod": [{
                                        "id": format!("{did}#atproto"),
                                        "type": "Multikey",
                                        "controller": did,
                                        "publicKeyMultibase": state.public_key,
                                    }],
                                })
                                .to_string();
                                (StatusCode::OK, body)
                            };
                            drop(state);
                            (response, release)
                        };
                        if let Some(release) = release {
                            release.await.expect("release first response");
                        }
                        response
                    },
                ),
            )
            .with_state(Arc::clone(&state));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind fixture directory");
        let address = listener.local_addr().expect("fixture address");
        let task = tokio::spawn(async move { axum::serve(listener, router).await.expect("fixture directory") });
        (format!("http://{address}"), state, task)
    }

    fn local_resolver(directory: &str) -> DidResolver {
        DidResolver::new(
            Fetcher::new(Policy::default().allow_private()).expect("local test client"),
            directory,
        )
    }

    fn cache_document(subject: &str, version: &str) -> Document {
        serde_json::from_value(json!({ "id": subject, "version": version })).expect("DID document")
    }

    #[test]
    fn documents_are_fetched_from_where_the_method_says() {
        let r = resolver();
        assert_eq!(
            r.document_url("did:plc:ewvi7nxzyoun6zhxrhs64oiz").as_deref(),
            Some("https://plc.directory/did:plc:ewvi7nxzyoun6zhxrhs64oiz")
        );
        assert_eq!(
            r.document_url("did:web:gilded-reach.motet.at").as_deref(),
            Some("https://gilded-reach.motet.at/.well-known/did.json")
        );
        assert_eq!(
            r.document_url("did:web:tenor.example%3A8443").as_deref(),
            Some("https://tenor.example:8443/.well-known/did.json")
        );
        assert_eq!(
            r.document_url("did:web:tenor.example%3a8443").as_deref(),
            Some("https://tenor.example:8443/.well-known/did.json")
        );
        for did in [
            "did:web:tenor.example%3A",
            "did:web:tenor.example%3A65536",
            "did:web:tenor.example%3A443%40attacker.example",
            "did:web:tenor.example%2Fattacker",
            "did:web:tenor.example:8443",
        ] {
            assert!(r.document_url(did).is_none(), "{did}");
        }
    }

    /// A removed key stops authorizing service tokens when the short cache
    /// window expires; otherwise key rotation could leave a stolen key valid.
    #[tokio::test]
    async fn rotated_signing_keys_replace_cached_authority_after_expiry() {
        let old_key = generate_key(KeyType::K256Private).expect("old key");
        let new_key = generate_key(KeyType::K256Private).expect("new key");
        let (url, directory, server) = directory(&old_key).await;
        let resolver = local_resolver(&url);
        let clone = resolver.clone();
        let now = SystemTime::now().duration_since(UNIX_EPOCH).expect("clock").as_secs();
        let old_token = token(&old_key, now);
        let new_token = token(&new_key, now);

        assert!(
            motet_xrpc::service_auth::verify(&old_token, &resolver, |aud| aud == AUDIENCE, METHOD, now)
                .await
                .is_ok()
        );
        directory.lock().expect("fixture state").public_key = public_key(&new_key);
        assert!(
            motet_xrpc::service_auth::verify(&old_token, &clone, |aud| aud == AUDIENCE, METHOD, now)
                .await
                .is_ok()
        );
        assert!(
            motet_xrpc::service_auth::verify(&new_token, &resolver, |aud| aud == AUDIENCE, METHOD, now)
                .await
                .is_err()
        );
        assert_eq!(directory.lock().expect("fixture state").calls, 1);

        // Advance the cache's monotonic entry age without sleeping or changing
        // the JWT clock. The next verification must fetch the new document.
        resolver
            .cache
            .lock()
            .expect("cache")
            .entries
            .get_mut(ALICE)
            .expect("cached issuer")
            .started = Instant::now()
            .checked_sub(DOCUMENT_CACHE_TTL)
            .expect("clock covers cache age");
        assert!(
            motet_xrpc::service_auth::verify(&old_token, &resolver, |aud| aud == AUDIENCE, METHOD, now)
                .await
                .is_err()
        );
        assert!(
            motet_xrpc::service_auth::verify(&new_token, &resolver, |aud| aud == AUDIENCE, METHOD, now)
                .await
                .is_ok()
        );
        assert_eq!(directory.lock().expect("fixture state").calls, 2);
        server.abort();
    }

    /// An older in-flight response cannot reauthorize a removed key after
    /// another request has already resolved the rotated signing key.
    #[tokio::test]
    async fn in_flight_old_response_uses_the_newer_verified_authority() {
        let old_key = generate_key(KeyType::K256Private).expect("old key");
        let new_key = generate_key(KeyType::K256Private).expect("new key");
        let (url, directory, server) = directory(&old_key).await;
        let resolver = local_resolver(&url);
        let (started_tx, started_rx) = oneshot::channel();
        let (release_tx, release_rx) = oneshot::channel();
        {
            let mut state = directory.lock().expect("fixture state");
            state.first_started = Some(started_tx);
            state.release_first = Some(release_rx);
        }
        let now = SystemTime::now().duration_since(UNIX_EPOCH).expect("clock").as_secs();
        let old_token = token(&old_key, now);
        let new_token = token(&new_key, now);
        let old_resolver = resolver.clone();
        let old_request = tokio::spawn(async move {
            motet_xrpc::service_auth::verify(&old_token, &old_resolver, |aud| aud == AUDIENCE, METHOD, now).await
        });
        started_rx.await.expect("first request reached the directory");
        directory.lock().expect("fixture state").public_key = public_key(&new_key);
        assert!(
            motet_xrpc::service_auth::verify(&new_token, &resolver, |aud| aud == AUDIENCE, METHOD, now)
                .await
                .is_ok()
        );
        release_tx.send(()).expect("release old response");
        assert_eq!(
            old_request.await.expect("old verification finished"),
            Err(motet_xrpc::service_auth::ServiceAuthError::BadSignature)
        );
        assert_eq!(directory.lock().expect("fixture state").calls, 2);
        server.abort();
    }

    /// A response older than the freshness window cannot authenticate even
    /// when it was never cached. A newer cached response can replace it.
    #[test]
    fn slow_responses_fail_closed_or_use_newer_fresh_authority() {
        let resolver = resolver();
        let start = Instant::now();
        let old_start = start.checked_sub(DOCUMENT_CACHE_TTL).expect("clock covers cache age");
        let old = cache_document(ALICE, "old");
        let bytes = serde_json::to_vec(&old).expect("document JSON").len();
        assert!(
            resolver
                .finish_resolution(ALICE, old.clone(), bytes, old_start)
                .is_err()
        );

        let new = cache_document(ALICE, "new");
        let new_bytes = serde_json::to_vec(&new).expect("document JSON").len();
        assert!(resolver.finish_resolution(ALICE, new, new_bytes, start).is_ok());
        let result = resolver
            .finish_resolution(ALICE, old, bytes, old_start)
            .expect("newer document replaces the slow response");
        assert_eq!(result.extra.get("version"), Some(&json!("new")));
    }

    /// A transient HTTP error, malformed document or mismatched DID must not
    /// poison later resolutions after the directory recovers.
    #[tokio::test]
    async fn failed_resolutions_are_never_cached() {
        let key = generate_key(KeyType::K256Private).expect("key");
        let (url, directory, server) = directory(&key).await;
        let resolver = local_resolver(&url);
        directory.lock().expect("fixture state").failure = true;
        assert!(resolver.resolve(ALICE).await.is_err());
        {
            let mut state = directory.lock().expect("fixture state");
            state.failure = false;
            state.malformed = true;
        }
        assert!(resolver.resolve(ALICE).await.is_err());
        {
            let mut state = directory.lock().expect("fixture state");
            state.malformed = false;
            state.wrong_id = true;
        }
        assert!(resolver.resolve(ALICE).await.is_err());
        directory.lock().expect("fixture state").wrong_id = false;
        assert_eq!(resolver.resolve(ALICE).await.expect("recovered document").id, ALICE);
        assert_eq!(resolver.resolve(ALICE).await.expect("cached document").id, ALICE);
        assert_eq!(directory.lock().expect("fixture state").calls, 4);
        server.abort();
    }

    /// An identity cached under a permissive local policy cannot bypass the
    /// HTTPS and public-address restrictions of a separately configured client.
    #[tokio::test]
    async fn cached_documents_cannot_cross_fetcher_security_policies() {
        let key = generate_key(KeyType::K256Private).expect("key");
        let (url, directory, server) = directory(&key).await;
        let local = local_resolver(&url);
        assert_eq!(local.resolve(ALICE).await.expect("local document").id, ALICE);
        let public = DidResolver::new(Fetcher::new(Policy::default()).expect("public-only client"), &url);
        assert!(public.resolve(ALICE).await.is_err());
        assert_eq!(directory.lock().expect("fixture state").calls, 1);
        assert_eq!(local.resolve(ALICE).await.expect("local cache").id, ALICE);
        server.abort();
    }

    /// Many valid issuers or large documents cannot grow the shared cache
    /// without bound. An evicted issuer must fetch rather than reuse its entry.
    #[test]
    fn document_cache_bounds_both_issuers_and_wire_bytes() {
        let mut cache = DocumentCache::default();
        let start = Instant::now();
        for index in 0..=DOCUMENT_CACHE_ENTRIES {
            let subject = format!("did:plc:account{index}");
            let document = cache_document(&subject, "small");
            let bytes = serde_json::to_vec(&document).expect("document JSON").len() + subject.len();
            let instant = start + Duration::from_micros(u64::try_from(index).expect("cache index"));
            cache.insert(&subject, Arc::new(document), bytes, instant, instant);
            assert!(cache.entries.len() <= DOCUMENT_CACHE_ENTRIES);
            assert!(cache.bytes <= DOCUMENT_CACHE_BYTES);
        }
        let now = start + Duration::from_millis(1);
        assert!(cache.get("did:plc:account0", now).is_none());
        assert!(
            cache
                .get(&format!("did:plc:account{DOCUMENT_CACHE_ENTRIES}"), now)
                .is_some()
        );

        let mut large = DocumentCache::default();
        let padding = "x".repeat(DOCUMENT_CACHE_BYTES / 2);
        for index in 0..3 {
            let subject = format!("did:plc:large{index}");
            let document = cache_document(&subject, &padding);
            let bytes = serde_json::to_vec(&document).expect("document JSON").len() + subject.len();
            let instant = start + Duration::from_micros(index);
            large.insert(&subject, Arc::new(document), bytes, instant, instant);
            assert!(large.bytes <= DOCUMENT_CACHE_BYTES);
        }
        assert!(large.get("did:plc:large0", now).is_none());
        assert!(large.get("did:plc:large2", now).is_some());
        let oversized = cache_document("did:plc:huge", &"x".repeat(DOCUMENT_CACHE_BYTES));
        let bytes = serde_json::to_vec(&oversized).expect("document JSON").len() + "did:plc:huge".len();
        large.insert("did:plc:huge", Arc::new(oversized), bytes, now, now);
        assert!(large.get("did:plc:huge", now).is_none());
        assert!(large.bytes <= DOCUMENT_CACHE_BYTES);
    }

    /// A slow response started before a key rotation must not overwrite a
    /// newer document fetched while it was in flight.
    #[test]
    fn older_in_flight_documents_cannot_replace_newer_cached_authority() {
        let mut cache = DocumentCache::default();
        let start = Instant::now();
        let newer = start + Duration::from_secs(1);
        let finished = start + Duration::from_secs(2);
        let old = cache_document(ALICE, "old");
        let new = cache_document(ALICE, "new");
        let old_bytes = serde_json::to_vec(&old).expect("document JSON").len() + ALICE.len();
        let new_bytes = serde_json::to_vec(&new).expect("document JSON").len() + ALICE.len();
        cache.insert(ALICE, Arc::new(new), new_bytes, newer, finished);
        cache.insert(ALICE, Arc::new(old), old_bytes, start, finished);
        assert_eq!(
            cache
                .get(ALICE, finished)
                .expect("newer document")
                .1
                .extra
                .get("version"),
            Some(&json!("new"))
        );
        assert!(cache.get(ALICE, newer + DOCUMENT_CACHE_TTL).is_none());
    }

    proptest! {
        /// A DID cannot smuggle a path, credentials or fragment into its URL;
        /// an encoded port must not redirect resolution to a different host.
        #[test]
        fn subjects_cannot_redirect_resolution(subject in "did:(plc|web|key):[ -~]{0,40}") {
            if let Some(url) = resolver().document_url(&subject) {
                let parsed = url::Url::parse(&url).expect("valid URL");
                prop_assert_eq!(parsed.scheme(), "https");
                prop_assert!(parsed.username().is_empty() && parsed.password().is_none());
                prop_assert!(parsed.query().is_none() && parsed.fragment().is_none());
                let plc = parsed.host_str() == Some("plc.directory") && parsed.path().matches('/').count() == 1;
                let web = parsed.path() == "/.well-known/did.json";
                prop_assert!(plc || web, "{}", url);
            }
        }
    }
}
