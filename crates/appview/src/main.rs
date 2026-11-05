use std::future::IntoFuture as _;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context as _;
use atproto_tap::TapConfig;
use clap::{Parser, Subcommand};
use motet_appview::archive::Archive;
use motet_appview::config::{Config, Serve};
use motet_appview::indexer::Indexer;
use motet_appview::{MIGRATOR, api, auth, outbox, projection, public_space, tap, writes};
use motet_net::identity::DidResolver;
use motet_net::{Fetcher, Policy};
use object_store::aws::{AmazonS3Builder, S3ConditionalPut};
use sqlx::PgPool;
use tokio::sync::Mutex;

#[derive(Parser)]
#[command(version, about = "Motet's AppView")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
#[expect(
    clippy::large_enum_variant,
    reason = "The CLI is parsed once; boxing adds an unnecessary allocation."
)]
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
    // Tap and the database enable different rustls providers in one process.
    rustls::crypto::ring::default_provider()
        .install_default()
        .map_err(|_| anyhow::anyhow!("cannot install the AppView TLS crypto provider"))?;
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

#[derive(PartialEq, Eq)]
enum Stopped {
    Input,
    Api,
    Space,
    Outbox,
    Signal,
}

async fn ingest_public_spaces(
    indexer: Arc<Mutex<Indexer>>,
    pool: PgPool,
    client: public_space::Client,
    mut shutdown: tokio::sync::watch::Receiver<bool>,
) -> Result<(), public_space::Error> {
    let mut interval = tokio::time::interval(Duration::from_secs(5));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        tokio::select! {
            _ = shutdown.wait_for(|stopping| *stopping) => return Ok(()),
            _ = interval.tick() => {}
        }
        // Do not cancel an append after it may have reached the archive.
        public_space::sync(&indexer, &pool, &client).await?;
    }
}

async fn ingest_tap(
    indexer: Arc<Mutex<Indexer>>,
    url: Option<String>,
    password: Option<String>,
    mut shutdown: tokio::sync::watch::Receiver<bool>,
) -> Result<(), tap::Error> {
    let stop = async {
        let _ = shutdown.wait_for(|stopping| *stopping).await;
    };
    if let Some(url) = url {
        let mut config = TapConfig::builder().hostname(url);
        if let Some(password) = password {
            config = config.admin_password(password);
        }
        tap::run(indexer, config.build(), stop).await
    } else {
        tracing::info!("Tap URL unset; serving without ingestion");
        stop.await;
        Ok(())
    }
}

#[expect(
    clippy::too_many_lines,
    reason = "Service startup and graceful task joins share a lifecycle and must not abandon in-flight batches."
)]
async fn serve(args: Serve) -> anyhow::Result<()> {
    let (pool, archive) = connect(&args.config).await?;
    let indexer = Arc::new(Mutex::new(
        Indexer::start(pool.clone(), &archive).await.context("indexer")?,
    ));

    let mut policy = Policy::default();
    if args.config.allow_private_network {
        policy = policy.allow_private();
    }
    let fetcher = Fetcher::new(policy)?;
    let resolver: Arc<dyn motet_xrpc::IdentityResolver> =
        Arc::new(DidResolver::new(fetcher.clone(), args.config.plc_directory.clone()));
    let client = Arc::new(
        writes::client::Client::new(fetcher.clone(), Arc::clone(&resolver), &args.tenor_url, &args.tenor_did)
            .context("tenor client")?,
    );
    let space_client = public_space::Client::new(fetcher.clone(), Arc::clone(&resolver));
    let oauth = auth::Server::new(
        pool.clone(),
        args.public_origin.clone(),
        args.default_pds.clone(),
        &args.oauth_signing_key_file,
        &args.session_key_file,
        fetcher,
        Arc::clone(&resolver),
    )
    .context("AppView OAuth")?;
    let service_did = args.service_did.clone();
    let service_auth =
        motet_xrpc::service_auth::authenticate(Arc::clone(&resolver), Arc::new(move |aud: &str| aud == service_did));
    let authenticate = oauth.authenticate(service_auth);
    let app = api::served_router(
        Arc::new(api::State {
            pool: pool.clone(),
            auth: Arc::clone(&oauth),
            client: Arc::clone(&client),
        }),
        authenticate,
        motet_xrpc::limits::Limits::new(
            args.max_in_flight,
            1,
            Duration::from_secs(args.request_timeout_secs),
            None,
        ),
    )?;

    let listener = tokio::net::TcpListener::bind(args.listen).await?;
    tracing::info!(listen = %args.listen, "appview serving");
    let (shutdown, input_shutdown) = tokio::sync::watch::channel(false);
    let mut api_shutdown = input_shutdown.clone();
    let mut space = tokio::spawn(ingest_public_spaces(
        Arc::clone(&indexer),
        pool.clone(),
        space_client,
        input_shutdown.clone(),
    ));
    let mut outbox = tokio::spawn(outbox::run(pool, Arc::clone(&oauth), client, input_shutdown.clone()));
    let mut input = tokio::spawn(ingest_tap(
        indexer,
        args.tap_url,
        args.tap_admin_password,
        input_shutdown,
    ));
    let server = axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            let _ = api_shutdown.wait_for(|stopping| *stopping).await;
        })
        .into_future();
    tokio::pin!(server);
    let (result, stopped) = tokio::select! {
        result = &mut input => (
            result.context("Tap task").and_then(|result| result.context("Tap ingestion")),
            Stopped::Input,
        ),
        result = &mut server => (result.context("API server"), Stopped::Api),
        result = &mut space => (
            result.context("public Space task").and_then(|result| result.context("public Space ingestion")),
            Stopped::Space,
        ),
        result = &mut outbox => (
            result.context("outbox task").and_then(|()| Err(anyhow::anyhow!("outbox stopped unexpectedly"))),
            Stopped::Outbox,
        ),
        result = shutdown_signal() => (result, Stopped::Signal),
    };
    shutdown.send_replace(true);
    // Let an in-flight batch commit and flush its acks before dropping the input.
    let input_result = if stopped == Stopped::Input {
        Ok(())
    } else {
        input
            .await
            .context("Tap task")
            .and_then(|result| result.context("Tap ingestion"))
    };
    let api_result = if stopped == Stopped::Api {
        Ok(())
    } else {
        server.await.context("API server")
    };
    let space_result = if stopped == Stopped::Space {
        Ok(())
    } else {
        space
            .await
            .context("public Space task")
            .and_then(|result| result.context("public Space ingestion"))
    };
    let outbox_result = if stopped == Stopped::Outbox {
        Ok(())
    } else {
        outbox.await.context("outbox task")
    };
    result
        .and(input_result)
        .and(api_result)
        .and(space_result)
        .and(outbox_result)
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
