//! Configuration, from flags or the environment (`MOTET_TENOR_*`).

use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

use clap::Args;

#[derive(Debug, Clone, Args)]
pub struct Config {
    /// Where to serve XRPC.
    #[arg(long, env = "MOTET_TENOR_LISTEN", default_value = "127.0.0.1:2584")]
    pub listen: SocketAddr,

    /// Public origin advertised in each World's DID document.
    #[arg(long, env = "MOTET_TENOR_PUBLIC_URL")]
    pub public_url: url::Url,

    /// Handle suffixes, including the leading dot (for example `.motet.test`).
    #[arg(long, env = "MOTET_TENOR_HANDLE_DOMAINS", value_delimiter = ',')]
    pub handle_domains: Vec<String>,

    /// Relay origins to announce this tenor to on startup and World creation.
    #[arg(long, env = "MOTET_TENOR_CRAWLERS", value_delimiter = ',')]
    pub relay_urls: Vec<url::Url>,

    /// S3-compatible endpoint for the tenor's bucket.
    #[arg(long, env = "MOTET_TENOR_S3_ENDPOINT")]
    pub s3_endpoint: String,
    #[arg(long, env = "MOTET_TENOR_S3_BUCKET")]
    pub s3_bucket: String,
    #[arg(long, env = "MOTET_TENOR_S3_ACCESS_KEY")]
    pub s3_access_key: String,
    #[arg(long, env = "MOTET_TENOR_S3_SECRET_KEY", hide_env_values = true)]
    pub s3_secret_key: String,
    #[arg(long, env = "MOTET_TENOR_S3_REGION", default_value = "us-east-1")]
    pub s3_region: String,

    /// The shard this process writes. One shard to start; more come with
    /// multi-node takeover (see docs/adr/0006-storage.md).
    #[arg(long, env = "MOTET_TENOR_SHARD", default_value = "shard-0")]
    pub shard: String,

    /// Operations per group commit.
    #[arg(long, env = "MOTET_TENOR_MAX_BATCH", default_value_t = 256)]
    pub max_batch: usize,

    /// Seconds between checkpoints of the shard's state, when it changed.
    /// A takeover replays at most about this much of the log.
    #[arg(long, env = "MOTET_TENOR_CHECKPOINT_SECS", default_value = "10", value_parser = parse_secs)]
    pub checkpoint_every: Duration,

    /// The file holding the master key that seals every World's keys: 32
    /// bytes, raw or base64. Keep it outside the bucket, and back it up:
    /// without it no World on this tenor can sign again.
    #[arg(long, env = "MOTET_TENOR_MASTER_KEY_FILE")]
    pub master_key_file: PathBuf,

    #[arg(long, env = "MOTET_PLC_DIRECTORY", default_value = "https://plc.directory")]
    pub plc_directory: String,

    /// Allow requests to private addresses (local PDS, local PLC). Development only.
    #[arg(long, env = "MOTET_ALLOW_PRIVATE_NETWORK")]
    pub allow_private_network: bool,
}

fn parse_secs(s: &str) -> Result<Duration, std::num::ParseIntError> {
    s.parse().map(Duration::from_secs)
}
