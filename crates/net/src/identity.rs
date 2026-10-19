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

use async_trait::async_trait;
use atproto_identity::model::Document;
use atproto_identity::traits::IdentityResolver;

use crate::Fetcher;

/// Resolves `did:plc` through a PLC directory and `did:web` from the host it
/// names. Nothing else.
#[derive(Clone, Debug)]
pub struct DidResolver {
    fetcher: Fetcher,
    plc_directory: String,
}

impl DidResolver {
    /// `plc_directory` is a base URL such as `https://plc.directory`.
    pub fn new(fetcher: Fetcher, plc_directory: impl Into<String>) -> Self {
        Self {
            fetcher,
            plc_directory: plc_directory.into().trim_end_matches('/').to_owned(),
        }
    }

    /// Where a DID's document is published, or `None` for a DID method Motet
    /// doesn't resolve.
    pub fn document_url(&self, did: &str) -> Option<String> {
        if let Some(id) = did.strip_prefix("did:plc:") {
            let valid = !id.is_empty() && id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit());
            return valid.then(|| format!("{}/{did}", self.plc_directory));
        }
        let host = did.strip_prefix("did:web:")?;
        // atproto allows only a hostname (no path, no port) in did:web.
        let valid = !host.is_empty()
            && host
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-')
            && !host.starts_with('.')
            && !host.ends_with('.');
        valid.then(|| format!("https://{host}/.well-known/did.json"))
    }
}

#[async_trait]
impl IdentityResolver for DidResolver {
    async fn resolve(&self, subject: &str) -> anyhow::Result<Document> {
        let url = self
            .document_url(subject)
            .ok_or_else(|| anyhow::anyhow!("{subject} is not a DID Motet resolves"))?;
        let body = self.fetcher.get_bytes(&url).await?;
        let document: Document = serde_json::from_slice(&body)?;
        if document.id != subject {
            anyhow::bail!("the document at {url} is for {}, not {subject}", document.id);
        }
        Ok(document)
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;
    use crate::Policy;

    fn resolver() -> DidResolver {
        DidResolver::new(
            Fetcher::new(Policy::default()).expect("client"),
            "https://plc.directory/",
        )
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
    }

    proptest! {
        /// Whatever a DID says, its document URL stays on the PLC directory or
        /// on the bare host a did:web names: no paths, ports, credentials or
        /// fragments can be smuggled into it.
        #[test]
        fn subjects_cannot_redirect_resolution(subject in "did:(plc|web|key):[ -~]{0,40}") {
            if let Some(url) = resolver().document_url(&subject) {
                let parsed = url::Url::parse(&url).expect("valid URL");
                prop_assert_eq!(parsed.scheme(), "https");
                prop_assert!(parsed.username().is_empty() && parsed.password().is_none());
                prop_assert!(parsed.port().is_none() && parsed.query().is_none() && parsed.fragment().is_none());
                let plc = parsed.host_str() == Some("plc.directory") && parsed.path().matches('/').count() == 1;
                let web = parsed.path() == "/.well-known/did.json";
                prop_assert!(plc || web, "{}", url);
            }
        }
    }
}
