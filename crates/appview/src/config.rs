//! Configuration, from flags or the environment (`MOTET_APPVIEW_*`).

use std::net::SocketAddr;
use std::path::PathBuf;

use clap::Args;

#[derive(Debug, Clone, Args)]
pub struct Config {
    #[arg(long, env = "DATABASE_URL", hide_env_values = true)]
    pub database_url: String,

    /// S3-compatible storage for the archive.
    #[arg(long, env = "MOTET_APPVIEW_S3_ENDPOINT")]
    pub s3_endpoint: String,
    #[arg(long, env = "MOTET_APPVIEW_S3_BUCKET")]
    pub s3_bucket: String,
    #[arg(long, env = "MOTET_APPVIEW_S3_ACCESS_KEY")]
    pub s3_access_key: String,
    #[arg(long, env = "MOTET_APPVIEW_S3_SECRET_KEY", hide_env_values = true)]
    pub s3_secret_key: String,
    #[arg(long, env = "MOTET_APPVIEW_S3_REGION", default_value = "us-east-1")]
    pub s3_region: String,
    #[arg(long, env = "MOTET_APPVIEW_ARCHIVE_PREFIX", default_value = "archive")]
    pub archive_prefix: String,

    /// Allow plain HTTP and private addresses (local S3, PDS, PLC). Development only.
    #[arg(long, env = "MOTET_ALLOW_PRIVATE_NETWORK")]
    pub allow_private_network: bool,

    #[arg(long, env = "MOTET_PLC_DIRECTORY", default_value = "https://plc.directory")]
    pub plc_directory: String,
}

#[derive(Debug, Clone, Args)]
pub struct Serve {
    #[command(flatten)]
    pub config: Config,

    #[arg(long, env = "MOTET_APPVIEW_LISTEN", default_value = "127.0.0.1:2583")]
    pub listen: SocketAddr,

    /// The DID this AppView answers service auth for, with its fragment.
    #[arg(
        long,
        env = "MOTET_APPVIEW_SERVICE_DID",
        default_value = "did:web:localhost#motet_appview"
    )]
    pub service_did: String,
    /// Public HTTPS origin shared by the web app and AppView.
    #[arg(long, env = "MOTET_APPVIEW_PUBLIC_ORIGIN")]
    pub public_origin: String,

    /// PDS used for account creation.
    #[arg(long, env = "MOTET_APPVIEW_DEFAULT_PDS")]
    pub default_pds: String,

    /// P-256 OAuth client signing key, as 32 raw bytes or standard base64.
    #[arg(long, env = "MOTET_APPVIEW_OAUTH_SIGNING_KEY_FILE")]
    pub oauth_signing_key_file: PathBuf,

    /// Independent 32-byte key used to seal pending logins and sessions.
    #[arg(long, env = "MOTET_APPVIEW_SESSION_KEY_FILE")]
    pub session_key_file: PathBuf,

    /// Tap HTTP(S) URL. Without it, only the API runs.
    #[arg(long, env = "MOTET_APPVIEW_TAP_URL")]
    pub tap_url: Option<String>,

    /// Tap administrator password for HTTP Basic authentication.
    #[arg(long, env = "MOTET_APPVIEW_TAP_ADMIN_PASSWORD", hide_env_values = true)]
    pub tap_admin_password: Option<String>,
}
