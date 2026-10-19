//! Configuration, from flags or the environment (`MOTET_TENOR_*`).

use std::net::SocketAddr;

use clap::Args;

#[derive(Debug, Clone, Args)]
pub struct Config {
    /// Where to serve XRPC.
    #[arg(long, env = "MOTET_TENOR_LISTEN", default_value = "127.0.0.1:2584")]
    pub listen: SocketAddr,

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

    /// Entries per group commit.
    #[arg(long, env = "MOTET_TENOR_MAX_BATCH", default_value_t = 256)]
    pub max_batch: usize,

    #[arg(long, env = "MOTET_PLC_DIRECTORY", default_value = "https://plc.directory")]
    pub plc_directory: String,

    /// Allow requests to private addresses (local PDS, local PLC). Development only.
    #[arg(long, env = "MOTET_ALLOW_PRIVATE_NETWORK")]
    pub allow_private_network: bool,
}
