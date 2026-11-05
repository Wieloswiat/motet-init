//! Public World metadata, read from the World's own meta Space.
//!
//! A public declaration only discovers a candidate. A fresh anonymous Space
//! credential and each signed read must still pass the authority's access
//! checks. We read only the current profile, not the Space's operation history:
//! an earlier profile may have been private even when the Space is public now.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use atproto_client::client::{Auth, CallLimits, DpopBody, xrpc_call_with_limits};
use atproto_client::errors::XrpcError;
use atproto_identity::key::{KeyType, generate_key, identify_key, to_public};
use atproto_identity::model::Document;
use atproto_identity::traits::IdentityResolver;
use atproto_identity::validation::validate_service_endpoint;
use atproto_record::tid::Tid;
use atproto_space::SpaceUri;
use atproto_space::credential::verify_space_credential;
use atproto_space_client::{ResolvedKemKeys, SpaceAccess, SpaceClientError, SpaceCredentialGrant};
use motet_net::Fetcher;
use reqwest::{Method, header::HeaderMap};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::PgPool;
use tokio::sync::Mutex;
use url::Url;

use crate::event::{Action, Event, SpaceRecordEvent};
use crate::indexer::{IndexError, Indexer};

const PROFILE: &str = "group.opensocial.profile";
const META: &str = "group.opensocial.meta";
const CREDENTIAL: &str = "com.atproto.space.getSpaceCredential";

// The SDK's resolver adapter is generic, while the application shares a
// trait object with its other outbound clients.
struct Resolver(Arc<dyn IdentityResolver>);

#[async_trait]
impl IdentityResolver for Resolver {
    async fn resolve(&self, subject: &str) -> anyhow::Result<Document> {
        self.0.resolve(subject).await
    }
}

/// A public-only reader. It never carries a member delegation or OAuth grant.
pub struct Client {
    fetcher: Fetcher,
    resolver: Arc<dyn IdentityResolver>,
    kem_keys: ResolvedKemKeys<Resolver>,
}

impl std::fmt::Debug for Client {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PublicSpaceClient").finish_non_exhaustive()
    }
}

impl Client {
    #[must_use]
    pub fn new(fetcher: Fetcher, resolver: Arc<dyn IdentityResolver>) -> Self {
        Self {
            fetcher,
            kem_keys: ResolvedKemKeys::new(Resolver(Arc::clone(&resolver))),
            resolver,
        }
    }

    async fn snapshot(&self, did: &str, space_text: &str) -> anyhow::Result<Snapshot> {
        let space: SpaceUri = space_text.parse()?;
        anyhow::ensure!(
            space.to_string() == space_text
                && space.space_did == did
                && space.space_type.as_str() == META
                && space.space_key.as_str() == "self",
            "declaration does not name the World's canonical meta Space"
        );
        let document = self.resolver.resolve(did).await?;
        anyhow::ensure!(document.id == did, "DID document names another account");
        let endpoint = pds_endpoint(&self.fetcher, &document)?;
        let signing_key = document
            .verification_method_multibase("atproto")
            .ok_or_else(|| anyhow::anyhow!("World has no atproto signing key"))?;
        let signing_key = identify_key(signing_key)?;
        let grant = self.credential(&endpoint, &space, &signing_key).await?;

        let mut access =
            SpaceAccess::credential(self.fetcher.client(), &endpoint, &grant).with_kem_keys(&self.kem_keys);
        access.limits.timeout = Duration::from_secs(10);
        access.limits.max_response_bytes = self.fetcher.max_response_bytes();
        let before = atproto_space_client::get_latest_commit(access, &space, did).await?;
        // A malformed revision must never win the projection's lexical ordering.
        Tid::decode(&before.rev)?;
        let record = match atproto_space_client::get_record(access, &space, did, PROFILE, "self").await {
            Ok(record) => Some(record),
            Err(SpaceClientError::Refused {
                error:
                    XrpcError::Lexicon {
                        status: 400 | 404,
                        code,
                        ..
                    },
                ..
            }) if code == "RecordNotFound" => None,
            Err(error) => return Err(error.into()),
        };
        let after = atproto_space_client::get_latest_commit(access, &space, did).await?;
        anyhow::ensure!(before == after, "Space changed during the profile read");
        Ok(Snapshot {
            rev: after.rev,
            record: record.map(|record| (record.cid, record.value)),
        })
    }

    async fn credential(
        &self,
        endpoint: &str,
        space: &SpaceUri,
        authority_key: &atproto_identity::key::KeyData,
    ) -> anyhow::Result<SpaceCredentialGrant> {
        #[derive(Deserialize)]
        struct Minted {
            credential: String,
        }

        let key = generate_key(KeyType::P256Private)?;
        let credential_kid = to_public(&key)?.to_string();
        let url = Url::parse(&format!("{endpoint}/xrpc/{CREDENTIAL}"))?;
        self.fetcher.check_destination(&url)?;
        let request = json!({"space": space.to_string(), "credentialKid": credential_kid});
        let response = xrpc_call_with_limits(
            self.fetcher.client(),
            &Auth::None,
            Method::POST,
            url.as_str(),
            Some(DpopBody::Json(&request)),
            &HeaderMap::new(),
            CallLimits::new(Duration::from_secs(10), self.fetcher.max_response_bytes()),
        )
        .await?;
        anyhow::ensure!(response.status.is_success(), "public Space credential was refused");
        let minted: Minted =
            serde_json::from_value(response.body.ok_or_else(|| anyhow::anyhow!("missing credential"))?)?;
        verify_space_credential(&minted.credential, &space.space_did, space, authority_key)?;
        Ok(SpaceCredentialGrant::from_parts(minted.credential, key)?)
    }
}

// Check the DID's actual PDS service, not another service of the same type.
// The SDK uses Fetcher's underlying client, so the literal-address check is
// needed here as well as its DNS filtering, timeout and no-redirect policy.
fn pds_endpoint(fetcher: &Fetcher, document: &Document) -> anyhow::Result<String> {
    let endpoint = document
        .service
        .iter()
        .find(|service| {
            service.id == "#atproto_pds" || service.id.strip_prefix(document.id.as_str()) == Some("#atproto_pds")
        })
        .filter(|service| service.r#type.as_str() == Some("AtprotoPersonalDataServer"))
        .and_then(|service| service.service_endpoint.as_uri())
        .ok_or_else(|| anyhow::anyhow!("World has no PDS service"))?;
    let url = Url::parse(endpoint)?;
    anyhow::ensure!(
        url.host().is_some()
            && url.username().is_empty()
            && url.password().is_none()
            && url.query().is_none()
            && url.fragment().is_none()
            && url.path() == "/"
            && matches!(url.scheme(), "http" | "https"),
        "invalid PDS endpoint"
    );
    if !fetcher.allows_private_network() {
        validate_service_endpoint(endpoint)?;
    }
    fetcher.check_destination(&url)?;
    Ok(endpoint.trim_end_matches('/').to_owned())
}

struct Snapshot {
    rev: String,
    record: Option<(String, Value)>,
}

/// A local persistence failure. Remote World failures are isolated and logged.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Database(#[from] sqlx::Error),
    #[error(transparent)]
    Index(#[from] IndexError),
}

/// Poll public World declarations and archive changed current profile snapshots.
///
/// All inputs use the same indexer as Tap. It is locked only for the final
/// declaration/projection check and archive-first ingest, not during network IO.
/// A failed host cannot stop other Worlds from being read. Database and indexer
/// failures are fatal: the caller must stop all ingestion.
///
/// # Errors
///
/// Returns a database or archive/projection failure. No remote failure deletes
/// a projected profile; only an explicit `RecordNotFound` does so.
pub async fn sync(indexer: &Arc<Mutex<Indexer>>, pool: &PgPool, client: &Client) -> Result<(), Error> {
    let worlds: Vec<(String, String)> =
        sqlx::query_as("SELECT did, space FROM projection.public_world_meta ORDER BY did")
            .fetch_all(pool)
            .await?;
    for (did, space) in worlds {
        let snapshot = match client.snapshot(&did, &space).await {
            Ok(snapshot) => snapshot,
            Err(error) => {
                tracing::warn!(world = %did, %error, "public Space profile could not be read");
                continue;
            }
        };
        let mut indexer = indexer.lock().await;
        let still_public: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM projection.public_world_meta WHERE did = $1 AND space = $2)",
        )
        .bind(&did)
        .bind(&space)
        .fetch_one(pool)
        .await?;
        if !still_public {
            continue;
        }
        let previous: Option<(String, Option<String>, Option<Value>)> = sqlx::query_as(
            "SELECT rev, cid, record FROM projection.space_records \
             WHERE space = $1 AND did = $2 AND collection = $3 AND rkey = 'self'",
        )
        .bind(&space)
        .bind(&did)
        .bind(PROFILE)
        .fetch_optional(pool)
        .await?;
        if previous.as_ref().is_some_and(|(rev, _, _)| rev >= &snapshot.rev) {
            continue;
        }
        let unchanged = match (&snapshot.record, &previous) {
            (Some((cid, record)), Some((_, Some(old_cid), Some(old_record)))) => cid == old_cid && record == old_record,
            (None, None | Some((_, None, _))) => true,
            _ => false,
        };
        if unchanged {
            continue;
        }
        let (action, cid, record) = match snapshot.record {
            Some((cid, record)) => (
                if previous.is_none() {
                    Action::Create
                } else {
                    Action::Update
                },
                Some(cid),
                Some(record),
            ),
            None => (Action::Delete, None, None),
        };
        indexer
            .ingest(&[Event::SpaceRecord(SpaceRecordEvent {
                space,
                did,
                rev: snapshot.rev,
                collection: PROFILE.into(),
                rkey: "self".into(),
                action,
                cid,
                record,
            })])
            .await?;
        drop(indexer);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use motet_net::Policy;

    fn document(service_id: &str, endpoint: &str) -> Document {
        serde_json::from_value(json!({
            "id": "did:plc:aaaaaaaaaaaaaaaaaaaaaaaa",
            "service": [{
                "id": service_id,
                "type": "AtprotoPersonalDataServer",
                "serviceEndpoint": endpoint,
            }],
        }))
        .expect("fixture DID document is valid")
    }

    /// A DID document cannot steer credential requests to an internal host,
    /// smuggle a URL authority, or substitute an unrelated PDS service.
    #[test]
    fn public_space_credentials_cannot_target_untrusted_url_shapes_or_private_addresses() {
        let fetcher = Fetcher::new(Policy::default()).unwrap();
        assert_eq!(
            pds_endpoint(&fetcher, &document("#atproto_pds", "https://pds.bsky.social/")).unwrap(),
            "https://pds.bsky.social"
        );
        for endpoint in [
            "https://127.0.0.1",
            "https://169.254.169.254",
            "https://2852039166",
            "https://[::1]",
            "http://pds.bsky.social",
            "https://pds.bsky.social@127.0.0.1",
            "https://pds.bsky.social/path",
            "https://pds.bsky.social/?next=127.0.0.1",
            "https://pds.bsky.social/#fragment",
        ] {
            assert!(
                pds_endpoint(&fetcher, &document("#atproto_pds", endpoint)).is_err(),
                "{endpoint}"
            );
        }
        assert!(pds_endpoint(&fetcher, &document("#other_pds", "https://pds.bsky.social")).is_err());
    }
}
