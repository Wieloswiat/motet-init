use std::sync::Arc;
use std::time::{Duration, SystemTime};

use anyhow::Context as _;
use clap::{Parser, Subcommand};
use motet_net::identity::DidResolver;
use motet_net::{Fetcher, Policy};
use motet_tenor::config::Config;
use motet_tenor::keys::MasterKey;
use motet_tenor::{api, identity, pds, shard};
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
        // Fencing needs create-only PUTs (see motet-log), and SlateDB needs
        // them for its manifest; one client serves both.
        .with_conditional_put(S3ConditionalPut::ETagMatch)
        .build()
        .context("bucket configuration")?;
    let master = MasterKey::from_file(&config.master_key_file).context("master key")?;
    let shard = shard::Shard::open(Arc::new(bucket), &config.shard, Arc::new(master))
        .await
        .context("replaying the shard")?;
    let handle = shard::spawn(
        shard,
        shard::Options {
            max_batch: config.max_batch,
            checkpoint_every: config.checkpoint_every,
        },
    );

    let mut policy = Policy::default();
    if config.allow_private_network {
        policy = policy.allow_private();
    }
    let fetcher = Fetcher::new(policy).context("HTTP client")?;
    let resolver = Arc::new(DidResolver::new(fetcher.clone(), config.plc_directory.clone()));
    let hosting = Arc::new(
        identity::Hosting::new(
            &config.public_url,
            config.handle_domains,
            config.plc_directory,
            fetcher.clone(),
            config.relay_urls,
        )
        .context("identity configuration")?,
    );
    let audience = hosting.audience().to_owned();
    let state = Arc::new(api::State {
        shard: handle.clone(),
        records: pds::Pds::new(fetcher.clone(), Arc::<DidResolver>::clone(&resolver)),
        hosting: Arc::clone(&hosting),
    });
    let authenticate = motet_xrpc::service_auth::authenticate(
        Arc::<DidResolver>::clone(&resolver),
        Arc::new(move |aud| api::accepts_audience(aud, &audience)),
    );
    let app = api::served_router(
        state,
        authenticate,
        motet_xrpc::limits::Limits::new(
            config.max_in_flight,
            config.max_firehose_connections,
            Duration::from_secs(config.request_timeout_secs),
            Some("/xrpc/com.atproto.sync.subscribeRepos"),
        ),
    )?;

    let listener = tokio::net::TcpListener::bind(config.listen).await?;
    tracing::info!(listen = %config.listen, shard = %config.shard, "tenor serving");
    let mut notifications = motet_tenor::spaces::notify::spawn(handle.clone(), fetcher, resolver);
    let mut publications = motet_tenor::publication::spawn(handle.clone());
    let maintenance = tokio::spawn(maintain(hosting, handle));
    let served = async {
        axum::serve(listener, app)
            .with_graceful_shutdown(async {
                let _ = tokio::signal::ctrl_c().await;
            })
            .await
    };
    let outcome = tokio::select! {
        result = served => result.context("serving HTTP"),
        result = &mut notifications => match result {
            Ok(Ok(())) => Err(anyhow::anyhow!("Space notification delivery stopped")),
            Ok(Err(error)) => Err(error).context("Space notification delivery failed"),
            Err(error) => Err(error).context("Space notification delivery task failed"),
        },
        result = &mut publications => match result {
            Ok(Ok(())) => Err(anyhow::anyhow!("admission publication stopped")),
            Ok(Err(error)) => Err(error).context("admission publication failed"),
            Err(error) => Err(error).context("admission publication task failed"),
        },
    };
    maintenance.abort();
    notifications.abort();
    publications.abort();
    outcome
}

async fn maintain(hosting: Arc<identity::Hosting>, handle: shard::Handle) {
    hosting.announce().await;
    let mut interval = tokio::time::interval(Duration::from_secs(60));
    loop {
        interval.tick().await;
        loop {
            match handle
                .run(identity::SweepReservations {
                    now: SystemTime::now(),
                    limit: 256,
                })
                .await
            {
                Ok(Ok(256)) => {}
                Ok(Ok(_)) => break,
                Ok(Err(error)) => {
                    tracing::error!(%error, "reservation sweep failed");
                    break;
                }
                Err(error) => {
                    tracing::error!(%error, "reservation writer stopped");
                    return;
                }
            }
        }
        loop {
            match handle
                .run(motet_tenor::spaces::credentials::SweepDelegationOrigins {
                    now: SystemTime::now(),
                    limit: 256,
                })
                .await
            {
                Ok(Ok(256)) => {}
                Ok(Ok(_)) => break,
                Ok(Err(error)) => {
                    tracing::error!(%error, "delegation-origin sweep failed");
                    break;
                }
                Err(error) => {
                    tracing::error!(%error, "delegation-origin writer stopped");
                    return;
                }
            }
        }
        loop {
            match handle
                .run(shard::SweepServiceTokens {
                    now: SystemTime::now(),
                    limit: 256,
                })
                .await
            {
                Ok(Ok(256)) => {}
                Ok(Ok(_)) => break,
                Ok(Err(error)) => {
                    tracing::error!(%error, "service-token sweep failed");
                    break;
                }
                Err(error) => {
                    tracing::error!(%error, "service-token writer stopped");
                    return;
                }
            }
        }
    }
}
