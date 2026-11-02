//! OAuth configuration for tests that exercise the AppView router.

use std::sync::Arc;

use motet_appview::auth;
use motet_net::identity::DidResolver;
use motet_net::{Fetcher, Policy};
use sqlx::PgPool;

pub(crate) const ORIGIN: &str = "https://motet.example";

pub(crate) fn oauth(pool: PgPool) -> Arc<auth::Server> {
    let files = tempfile::tempdir().expect("temporary key directory");
    let signing = files.path().join("signing");
    let sealing = files.path().join("sealing");
    std::fs::write(&signing, [1; 32]).expect("signing key");
    std::fs::write(&sealing, [2; 32]).expect("sealing key");
    let fetcher = Fetcher::new(Policy::default()).expect("fetcher");
    let resolver = Arc::new(DidResolver::new(fetcher.clone(), "https://plc.directory"));
    auth::Server::new(
        pool,
        ORIGIN.into(),
        "https://pds.example".into(),
        &signing,
        &sealing,
        fetcher,
        resolver,
    )
    .expect("OAuth server")
}
