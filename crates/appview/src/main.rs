use std::sync::Arc;

use anyhow::Context as _;
use clap::{Parser, Subcommand};
use motet_appview::archive::Archive;
use motet_appview::config::{Config, Serve};
use motet_appview::{MIGRATOR, api, projection};
use motet_net::identity::DidResolver;
use motet_net::{Fetcher, Policy};
use object_store::aws::{AmazonS3Builder, S3ConditionalPut};
use sqlx::PgPool;

#[derive(Parser)]
#[command(version, about = "Motet's AppView")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Migrate, bring projections up to date and serve the API.
    Serve(Serve),
    /// Rebuild every projection from the archive, with no network access
    /// beyond the database and the bucket, and swap it in.
    Rebuild(Config),
    /// Apply service-state migrations and exit.
    Migrate(Config),
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .json()
        .init();
    match Cli::parse().command {
        Command::Serve(serve_args) => serve(serve_args).await,
        Command::Rebuild(config) => {
            let (pool, archive) = connect(&config).await?;
            projection::rebuild(&pool, &archive).await?;
            Ok(())
        }
        Command::Migrate(config) => {
            let pool = PgPool::connect(&config.database_url).await?;
            MIGRATOR.run(&pool).await?;
            Ok(())
        }
    }
}

async fn connect(config: &Config) -> anyhow::Result<(PgPool, Archive)> {
    let pool = PgPool::connect(&config.database_url).await.context("database")?;
    MIGRATOR.run(&pool).await.context("migrations")?;
    let bucket = AmazonS3Builder::new()
        .with_endpoint(&config.s3_endpoint)
        .with_bucket_name(&config.s3_bucket)
        .with_access_key_id(&config.s3_access_key)
        .with_secret_access_key(&config.s3_secret_key)
        .with_region(&config.s3_region)
        .with_allow_http(config.allow_private_network)
        .with_conditional_put(S3ConditionalPut::ETagMatch)
        .build()
        .context("bucket configuration")?;
    Ok((pool, Archive::new(Arc::new(bucket), &config.archive_prefix)))
}

async fn serve(args: Serve) -> anyhow::Result<()> {
    let (pool, archive) = connect(&args.config).await?;
    projection::ensure(&pool, &archive).await.context("projections")?;

    let mut policy = Policy::default();
    if args.config.allow_private_network {
        policy = policy.allow_private();
    }
    let resolver = Arc::new(DidResolver::new(
        Fetcher::new(policy)?,
        args.config.plc_directory.clone(),
    ));
    let service_did = args.service_did.clone();
    let authenticate = motet_xrpc::service_auth::authenticate(resolver, Arc::new(move |aud: &str| aud == service_did));
    let app = api::router(Arc::new(api::State { pool }), authenticate)?
        .route("/_health", axum::routing::get(|| async { "ok" }));

    let listener = tokio::net::TcpListener::bind(args.listen).await?;
    tracing::info!(listen = %args.listen, "appview serving");
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}
