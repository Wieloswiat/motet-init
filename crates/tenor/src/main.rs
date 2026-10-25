use std::sync::Arc;

use anyhow::Context as _;
use clap::{Parser, Subcommand};
use motet_log::Log;
use motet_net::identity::DidResolver;
use motet_net::{Fetcher, Policy};
use motet_tenor::config::Config;
use motet_tenor::{api, pds, shard};
use object_store::aws::{AmazonS3Builder, S3ConditionalPut};

#[derive(Parser)]
#[command(version, about = "Motet's group host")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Replay the shard, take over as its writer and serve.
    Serve(Config),
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .json()
        .init();
    match Cli::parse().command {
        Command::Serve(config) => serve(config).await,
    }
}

async fn serve(config: Config) -> anyhow::Result<()> {
    let bucket = AmazonS3Builder::new()
        .with_endpoint(&config.s3_endpoint)
        .with_bucket_name(&config.s3_bucket)
        .with_access_key_id(&config.s3_access_key)
        .with_secret_access_key(&config.s3_secret_key)
        .with_region(&config.s3_region)
        .with_allow_http(config.allow_private_network)
        // Fencing needs create-only PUTs (see motet-log).
        .with_conditional_put(S3ConditionalPut::ETagMatch)
        .build()
        .context("bucket configuration")?;
    let log = Log::new(Arc::new(bucket), config.shard.as_str());
    let shard = shard::Shard::open(log).await.context("replaying the shard")?;
    let handle = shard::spawn(shard, config.max_batch);

    let mut policy = Policy::default();
    if config.allow_private_network {
        policy = policy.allow_private();
    }
    let fetcher = Fetcher::new(policy).context("HTTP client")?;
    let resolver = Arc::new(DidResolver::new(fetcher.clone(), config.plc_directory.clone()));
    let state = Arc::new(api::State {
        shard: handle,
        records: pds::Pds::new(fetcher, Arc::<DidResolver>::clone(&resolver)),
    });
    let authenticate = motet_xrpc::service_auth::authenticate(resolver, Arc::new(api::accepts_audience));
    let app = api::router(state, authenticate)?;

    let listener = tokio::net::TcpListener::bind(config.listen).await?;
    tracing::info!(listen = %config.listen, shard = %config.shard, "tenor serving");
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}
