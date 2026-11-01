use std::future::IntoFuture as _;
use std::sync::Arc;

use anyhow::Context as _;
use atproto_tap::TapConfig;
use clap::{Parser, Subcommand};
use motet_appview::archive::Archive;
use motet_appview::config::{Config, Serve};
use motet_appview::indexer::Indexer;
use motet_appview::{MIGRATOR, api, projection, tap};
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
    /// Migrate, catch up projections, ingest from Tap and serve the API.
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
    let indexer = Indexer::start(pool.clone(), &archive).await.context("indexer")?;

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
    let (shutdown, mut input_shutdown) = tokio::sync::watch::channel(false);
    let mut api_shutdown = input_shutdown.clone();
    let mut input = tokio::spawn(async move {
        let stop = async {
            let _ = input_shutdown.wait_for(|stopping| *stopping).await;
        };
        if let Some(url) = args.tap_url {
            let mut config = TapConfig::builder().hostname(url);
            if let Some(password) = args.tap_admin_password {
                config = config.admin_password(password);
            }
            tap::run(indexer, config.build(), stop).await
        } else {
            tracing::info!("Tap URL unset; serving without ingestion");
            stop.await;
            Ok(())
        }
    });
    let server = axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            let _ = api_shutdown.wait_for(|stopping| *stopping).await;
        })
        .into_future();
    tokio::pin!(server);
    let (result, input_done, api_done) = tokio::select! {
        result = &mut input => (
            result.context("Tap task").and_then(|result| result.context("Tap ingestion")),
            true,
            false,
        ),
        result = &mut server => (result.context("API server"), false, true),
        result = shutdown_signal() => (result, false, false),
    };
    shutdown.send_replace(true);
    // Let an in-flight batch commit and flush its acks before dropping the input.
    let input_result = if input_done {
        Ok(())
    } else {
        input
            .await
            .context("Tap task")
            .and_then(|result| result.context("Tap ingestion"))
    };
    let api_result = if api_done {
        Ok(())
    } else {
        server.await.context("API server")
    };
    result.and(input_result).and(api_result)
}

async fn shutdown_signal() -> anyhow::Result<()> {
    #[cfg(unix)]
    {
        let mut terminate = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
        tokio::select! {
            result = tokio::signal::ctrl_c() => result?,
            _ = terminate.recv() => {}
        }
    }
    #[cfg(not(unix))]
    tokio::signal::ctrl_c().await?;
    Ok(())
}
