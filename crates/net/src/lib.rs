//! Outbound HTTP, hardened once.
//!
//! Every request Motet makes to a host someone else chose (a PDS, an OAuth
//! server, a DID document's endpoint, an image) goes through [`Fetcher`]. The
//! OAuth spec names SSRF and malicious responses; NodeBB's history shows what
//! happens when each call site defends itself. Here the policy is in one place:
//!
//! - HTTPS only, no redirects followed, no ambient proxy.
//! - Names resolve through [`PublicOnly`], which refuses private, loopback,
//!   link-local and other non-public addresses *after* DNS, so rebinding a
//!   public name to an internal address doesn't get through.
//! - Bounded time and response size.
//!
//! Development against a local PDS needs the opposite policy; that is an
//! explicit [`Policy::allow_private`] in configuration, never a fallback.

pub mod identity;

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use reqwest::dns::{Addrs, Name, Resolve, Resolving};
use thiserror::Error;

/// What outbound requests may do.
#[derive(Clone, Debug)]
pub struct Policy {
    pub timeout: Duration,
    pub max_response_bytes: u64,
    allow_private: bool,
}

impl Default for Policy {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(10),
            max_response_bytes: 4 * 1024 * 1024,
            allow_private: false,
        }
    }
}

impl Policy {
    /// For local development and tests against services on this machine.
    #[must_use]
    pub fn allow_private(mut self) -> Self {
        self.allow_private = true;
        self
    }
}

#[derive(Debug, Error)]
pub enum FetchError {
    #[error(transparent)]
    Http(#[from] reqwest::Error),
    #[error("the response is larger than {0} bytes")]
    TooLarge(u64),
    #[error("the server answered {0}")]
    Status(reqwest::StatusCode),
}

/// The client every outbound request goes through.
#[derive(Clone, Debug)]
pub struct Fetcher {
    client: reqwest::Client,
    policy: Policy,
}

impl Fetcher {
    pub fn new(policy: Policy) -> Result<Self, FetchError> {
        let mut builder = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy()
            .timeout(policy.timeout)
            .user_agent(concat!("motet/", env!("CARGO_PKG_VERSION")));
        if !policy.allow_private {
            builder = builder.https_only(true).dns_resolver(Arc::new(PublicOnly));
        }
        Ok(Self {
            client: builder.build()?,
            policy,
        })
    }

    /// The underlying client, for SDKs that take one (atproto-crates' clients
    /// accept a `reqwest::Client`; the workspace pins the same reqwest), so
    /// they inherit this policy.
    pub fn client(&self) -> &reqwest::Client {
        &self.client
    }

    /// GETs `url` and returns the body of a successful response, refusing
    /// anything over the size limit.
    pub async fn get_bytes(&self, url: &str) -> Result<Vec<u8>, FetchError> {
        let response = self.get(url).await?;
        if !response.status.is_success() {
            return Err(FetchError::Status(response.status));
        }
        Ok(response.body)
    }

    /// GETs `url` and returns the status and body whatever the status, for
    /// protocols whose error bodies mean something (XRPC's `RecordNotFound`).
    pub async fn get(&self, url: &str) -> Result<Response, FetchError> {
        let mut response = self.client.get(url).send().await?;
        let status = response.status();
        let limit = self.policy.max_response_bytes;
        if response.content_length().is_some_and(|n| n > limit) {
            return Err(FetchError::TooLarge(limit));
        }
        let mut body = Vec::new();
        while let Some(chunk) = response.chunk().await? {
            body.extend_from_slice(&chunk);
            if body.len() as u64 > limit {
                return Err(FetchError::TooLarge(limit));
            }
        }
        Ok(Response { status, body })
    }
}

/// A bounded response.
#[derive(Clone, Debug)]
pub struct Response {
    pub status: reqwest::StatusCode,
    pub body: Vec<u8>,
}

/// A resolver that only ever returns publicly routable addresses.
#[derive(Debug)]
pub struct PublicOnly;

impl Resolve for PublicOnly {
    fn resolve(&self, name: Name) -> Resolving {
        Box::pin(async move {
            let addrs: Vec<SocketAddr> = tokio::net::lookup_host((name.as_str(), 0))
                .await?
                .filter(|a| is_public(a.ip()))
                .collect();
            if addrs.is_empty() {
                return Err(format!("{} has no public address", name.as_str()).into());
            }
            Ok(Box::new(addrs.into_iter()) as Addrs)
        })
    }
}

/// Whether an address is publicly routable: not loopback, private, link-local,
/// shared, documentation, multicast, unspecified or reserved, including IPv4
/// addresses smuggled inside IPv6.
pub fn is_public(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => is_public_v4(v4),
        IpAddr::V6(v6) => {
            if let Some(v4) = v6.to_ipv4_mapped() {
                return is_public_v4(v4);
            }
            let s = v6.segments();
            !(v6.is_loopback()
                || v6.is_unspecified()
                || v6.is_multicast()
                || (s[0] & 0xfe00) == 0xfc00 // unique local fc00::/7
                || (s[0] & 0xffc0) == 0xfe80 // link-local fe80::/10
                || (s[0] == 0x2001 && s[1] == 0x0db8) // documentation
                || (s[0] == 0x0064 && s[1] == 0xff9b) // NAT64 64:ff9b::/96 embeds IPv4
                || s[0] == 0x2002 // 6to4 embeds IPv4
                || v6 == Ipv6Addr::UNSPECIFIED)
        }
    }
}

fn is_public_v4(ip: Ipv4Addr) -> bool {
    let [a, b, ..] = ip.octets();
    !(ip.is_private()
        || ip.is_loopback()
        || ip.is_link_local()
        || ip.is_broadcast()
        || ip.is_documentation()
        || ip.is_unspecified()
        || ip.is_multicast()
        || a == 0
        || (a == 100 && (64..128).contains(&b)) // shared address space
        || (a == 192 && b == 0) // IETF protocol assignments
        || (a == 198 && (18..20).contains(&b)) // benchmarking
        || a >= 240) // reserved
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_internal_addresses_are_refused() {
        for addr in [
            "127.0.0.1",
            "10.1.2.3",
            "172.16.0.1",
            "192.168.1.1",
            "169.254.169.254",
            "100.64.0.1",
            "0.0.0.0",
            "255.255.255.255",
            "::1",
            "::",
            "fc00::1",
            "fe80::1",
            "::ffff:127.0.0.1",
            "::ffff:169.254.169.254",
            "64:ff9b::a9fe:a9fe",
            "2002:7f00:1::",
        ] {
            let ip: IpAddr = addr.parse().expect("test address parses");
            assert!(!is_public(ip), "{addr} must not be reachable");
        }
    }

    #[test]
    fn ordinary_public_addresses_are_allowed() {
        for addr in ["1.1.1.1", "8.8.8.8", "2606:4700:4700::1111"] {
            let ip: IpAddr = addr.parse().expect("test address parses");
            assert!(is_public(ip), "{addr} is public");
        }
    }

    proptest::proptest! {
        /// An IPv4 address wrapped as IPv4-mapped IPv6 gets the same answer.
        #[test]
        fn mapped_addresses_get_the_same_answer(raw: u32) {
            let v4 = Ipv4Addr::from(raw);
            proptest::prop_assert_eq!(is_public(IpAddr::V4(v4)), is_public(IpAddr::V6(v4.to_ipv6_mapped())));
        }
    }
}
