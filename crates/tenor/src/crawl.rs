//! Relay discovery, outside the shard writer: announcing never changes state.

use motet_net::{FetchError, Fetcher};
use thiserror::Error;
use url::Url;

#[derive(Debug, Error)]
pub enum CrawlError {
    #[error("invalid relay URL: {0}")]
    Url(#[from] url::ParseError),
    #[error("relay {relay} refused the crawl announcement: {source}")]
    Relay { relay: Url, source: FetchError },
}

/// Announces the public host to every configured relay, even if one fails.
///
/// `hostname` is the public URL's authority (host and optional port), not a DID
/// or a full URL. The hardened fetcher controls local-network opt-in and timeouts.
/// No response body is needed or downloaded for this notification.
pub async fn announce(fetcher: &Fetcher, relays: &[Url], hostname: &str) -> Result<(), CrawlError> {
    let body = serde_json::json!({ "hostname": hostname });
    let mut failed = None;
    for relay in relays {
        let endpoint = relay.join("/xrpc/com.atproto.sync.requestCrawl")?;
        let result = fetcher.client().post(endpoint).json(&body).send().await;
        let error = match result {
            Ok(response) if response.status().is_success() => None,
            Ok(response) => Some(FetchError::Status(response.status())),
            Err(error) => Some(FetchError::Http(error)),
        };
        if let Some(source) = error {
            failed.get_or_insert_with(|| CrawlError::Relay {
                relay: relay.clone(),
                source,
            });
        }
    }
    failed.map_or(Ok(()), Err)
}
