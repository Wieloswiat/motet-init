//! Real services for integration tests, isolated per test.
//!
//! The services come from `devenv up` locally and the same devenv
//! configuration in CI; tests read where they are from the environment the
//! dev shell exports. Each test gets its own database and its own key prefix,
//! so tests can run in parallel against one PostgreSQL and one bucket.
//!
//! Integration tests are marked `#[ignore = "integration: …"]` and run with
//! `cargo ti` (nextest's `integration` profile).

use std::str::FromStr;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use object_store::ObjectStore;
use object_store::aws::AmazonS3Builder;
use object_store::prefix::PrefixStore;
use sqlx::ConnectOptions;
use sqlx::postgres::{PgConnectOptions, PgPool};

fn env(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| {
        panic!("{name} is not set: run integration tests from the dev shell with `devenv up` running")
    })
}

/// A name no other test in this run, or a concurrent run, will pick.
pub fn unique_name(prefix: &str) -> String {
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.subsec_nanos());
    format!(
        "{prefix}_{}_{}_{}",
        std::process::id(),
        nanos,
        COUNTER.fetch_add(1, Ordering::Relaxed)
    )
}

/// A fresh, empty database on the test PostgreSQL (with `pg_search`).
pub async fn database() -> PgPool {
    let admin = PgConnectOptions::from_str(&env("MOTET_TEST_DATABASE_URL"))
        .unwrap_or_else(|e| panic!("MOTET_TEST_DATABASE_URL is not a PostgreSQL URL: {e}"));
    let name = unique_name("motet_test");
    let mut conn = admin
        .connect()
        .await
        .unwrap_or_else(|e| panic!("can't reach the test database: {e}"));
    // The name is ours (unique_name), not input: safe to interpolate.
    let create = sqlx::AssertSqlSafe(format!(r#"CREATE DATABASE "{name}""#));
    sqlx::raw_sql(create)
        .execute(&mut conn)
        .await
        .unwrap_or_else(|e| panic!("can't create test database {name}: {e}"));
    PgPool::connect_with(admin.database(&name))
        .await
        .unwrap_or_else(|e| panic!("can't connect to test database {name}: {e}"))
}

/// A key prefix of its own in the test bucket (SeaweedFS under devenv).
pub fn bucket() -> Arc<dyn ObjectStore> {
    let store = AmazonS3Builder::new()
        .with_endpoint(env("MOTET_TEST_S3_ENDPOINT"))
        .with_bucket_name(env("MOTET_TEST_S3_BUCKET"))
        .with_access_key_id(env("MOTET_TEST_S3_ACCESS_KEY"))
        .with_secret_access_key(env("MOTET_TEST_S3_SECRET_KEY"))
        .with_region("us-east-1")
        .with_allow_http(true)
        .with_conditional_put(object_store::aws::S3ConditionalPut::ETagMatch)
        .build()
        .unwrap_or_else(|e| panic!("bad test bucket configuration: {e}"));
    Arc::new(PrefixStore::new(store, unique_name("t")))
}
