//! Reading posts back from their authors' PDSes.
//!
//! Every request goes through `motet-net`'s hardened fetcher: the PDS
//! endpoint comes from a DID document someone else controls, so it is
//! exactly the SSRF surface that fetcher exists for.

use std::sync::Arc;

use atproto_identity::traits::IdentityResolver;
use motet_lexicons::at_motet_pilot_post::Post;
use motet_net::Fetcher;
use motet_rules::{Cid, Did, SceneId, StrongRef};
use serde::Deserialize;

use crate::admission::{FetchError, Fetched, Records};

/// Public posts, read with `com.atproto.repo.getRecord` from the PDS the
/// author's DID document names.
#[derive(Clone)]
pub struct Pds {
    fetcher: Fetcher,
    resolver: Arc<dyn IdentityResolver>,
}

impl std::fmt::Debug for Pds {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Pds")
            .field("fetcher", &self.fetcher)
            .finish_non_exhaustive()
    }
}

impl Pds {
    pub fn new(fetcher: Fetcher, resolver: Arc<dyn IdentityResolver>) -> Self {
        Self { fetcher, resolver }
    }
}

#[derive(Deserialize)]
struct GetRecord {
    cid: String,
    value: Post,
}

#[derive(Deserialize)]
struct XrpcError {
    error: String,
}

impl Records for Pds {
    async fn fetch(&self, post: &StrongRef) -> Result<Option<Fetched>, FetchError> {
        let Some((did, collection, rkey)) = split_at_uri(post.uri.as_str()) else {
            return Ok(None);
        };
        if collection != "at.motet.pilot.post" {
            return Ok(None);
        }
        let unavailable = |e: &dyn std::fmt::Display| FetchError::Unavailable(e.to_string());
        let document = self.resolver.resolve(did).await.map_err(|e| unavailable(&e))?;
        let pds = document
            .pds_endpoints()
            .first()
            .map(|s| s.trim_end_matches('/').to_owned())
            .ok_or_else(|| unavailable(&"the DID document names no PDS"))?;
        let mut url =
            url::Url::parse(&format!("{pds}/xrpc/com.atproto.repo.getRecord")).map_err(|e| unavailable(&e))?;
        url.query_pairs_mut()
            .append_pair("repo", did)
            .append_pair("collection", collection)
            .append_pair("rkey", rkey);
        let response = self.fetcher.get(url.as_str()).await.map_err(|e| unavailable(&e))?;
        if !response.status.is_success() {
            // atproto answers a missing record with 400 RecordNotFound; any
            // other failure says nothing about whether the record exists.
            let not_found =
                serde_json::from_slice::<XrpcError>(&response.body).is_ok_and(|e| e.error == "RecordNotFound");
            return if not_found {
                Ok(None)
            } else {
                Err(unavailable(&response.status))
            };
        }
        let record: GetRecord = serde_json::from_slice(&response.body).map_err(|e| unavailable(&e))?;
        Ok(Some(Fetched {
            cid: Cid::new(record.cid),
            author: Did::new(did),
            world: Did::new(record.value.world),
            scene: SceneId::new(record.value.scene),
        }))
    }
}

/// `at://did/collection/rkey` into its three parts. Validation proper (DID
/// and NSID syntax) is `atproto-record`'s job at ingest; this only refuses
/// anything that isn't the three-part shape.
fn split_at_uri(uri: &str) -> Option<(&str, &str, &str)> {
    let mut parts = uri.strip_prefix("at://")?.split('/');
    let did = parts.next().filter(|d| d.starts_with("did:"))?;
    let collection = parts.next().filter(|c| !c.is_empty())?;
    let rkey = parts.next().filter(|r| !r.is_empty())?;
    parts.next().is_none().then_some((did, collection, rkey))
}
