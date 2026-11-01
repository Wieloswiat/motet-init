//! Projections: the PostgreSQL tables pages are built from.
//!
//! Everything in the `projection` schema is derived from the archive and can
//! be thrown away. There are no projection migrations: a change to
//! `schema.sql` bumps [`PROJECTION_VERSION`], and the AppView rebuilds into a
//! fresh schema from the archive and swaps it in. Service state that can't be
//! rebuilt (the outbox, read positions, sessions) lives in the `service`
//! schema, under ordinary migrations.
//!
//! [`apply`] is the only code that writes projections, and it is the same
//! code for the live indexer and for a rebuild, which is what makes "rebuild
//! gives the same projections" hold by construction rather than by care.
//! Each record keeps the newest repository revision applied, so a late or
//! redelivered event can't roll the display back.

use motet_log::Seq;
use sqlx::{Connection, PgConnection, PgPool};

use crate::archive::{Archive, ArchiveError};
use crate::event::{Action, Event, RecordEvent};

/// Bump with any change to `schema.sql` or to what [`apply`] writes.
pub const PROJECTION_VERSION: i32 = 2;

/// The schema the API reads.
pub const LIVE: &str = "projection";
/// Where a rebuild happens before it is swapped in.
const NEXT: &str = "projection_next";

const SCHEMA: &str = include_str!("schema.sql");

#[derive(Debug, thiserror::Error)]
pub enum ProjectionError {
    #[error(transparent)]
    Db(#[from] sqlx::Error),
    #[error(transparent)]
    Archive(#[from] ArchiveError),
    #[error("archive segment {seq} has a {action:?} without {missing}")]
    Malformed {
        seq: Seq,
        action: Action,
        missing: &'static str,
    },
}

/// Brings the live projections up to date with the archive.
///
/// Rebuilds them if they are missing or from another [`PROJECTION_VERSION`],
/// otherwise applies whatever the archive holds past the last applied
/// segment (an indexer that stopped between archiving and applying).
pub async fn ensure(pool: &PgPool, archive: &Archive) -> Result<(), ProjectionError> {
    let current: Option<(i32, Option<i64>)> = match sqlx::query_as("SELECT version, archive_seq FROM projection.meta")
        .fetch_optional(pool)
        .await
    {
        Ok(row) => row,
        Err(sqlx::Error::Database(e)) if e.code().as_deref() == Some("42P01") => None, // undefined_table
        Err(e) => return Err(e.into()),
    };
    match current {
        Some((version, applied)) if version == PROJECTION_VERSION => {
            let from = applied.map_or(0, |s| u64::try_from(s).unwrap_or(0) + 1);
            replay(pool, archive, LIVE, from).await
        }
        _ => rebuild(pool, archive).await,
    }
}

/// Rebuilds the projections from the archive alone, with no network access,
/// then swaps them in. Readers keep the old tables until the swap.
pub async fn rebuild(pool: &PgPool, archive: &Archive) -> Result<(), ProjectionError> {
    create(pool, NEXT).await?;
    replay(pool, archive, NEXT, 0).await?;
    let mut tx = pool.begin().await?;
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
        "DROP SCHEMA IF EXISTS {LIVE} CASCADE; ALTER SCHEMA {NEXT} RENAME TO {LIVE};"
    )))
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(())
}

/// A fresh, empty projection schema named `schema`.
async fn create(pool: &PgPool, schema: &'static str) -> Result<(), ProjectionError> {
    let mut tx = pool.begin().await?;
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
        "DROP SCHEMA IF EXISTS {schema} CASCADE; CREATE SCHEMA {schema}; SET LOCAL search_path TO {schema};"
    )))
    .execute(&mut *tx)
    .await?;
    sqlx::raw_sql(SCHEMA).execute(&mut *tx).await?;
    sqlx::query("INSERT INTO meta (version) VALUES ($1)")
        .bind(PROJECTION_VERSION)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}

async fn replay(pool: &PgPool, archive: &Archive, schema: &'static str, from: Seq) -> Result<(), ProjectionError> {
    let mut conn = pool.acquire().await?;
    for (seq, events) in archive.read_from(from).await? {
        apply(&mut conn, schema, seq, &events).await?;
    }
    Ok(())
}

/// Applies one archive segment to the projections in `schema`, atomically
/// with recording that it was applied.
///
/// # Panics
///
/// Panics if `seq` exceeds `i64::MAX`: PostgreSQL stores archive positions
/// as signed `bigint`, so callers must keep positions within that range.
pub async fn apply(
    conn: &mut PgConnection,
    schema: &'static str,
    seq: Seq,
    events: &[Event],
) -> Result<(), ProjectionError> {
    let mut tx = conn.begin().await?;
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!("SET LOCAL search_path TO {schema}")))
        .execute(&mut *tx)
        .await?;
    for event in events {
        match event {
            Event::Record(record) => apply_record(&mut tx, seq, record).await?,
            // Account and identity events carry no repo revision. Ordering
            // them against a redelivery is open (Tap's event IDs are the
            // candidate); until then the last one delivered wins.
            Event::Identity { did, handle } => {
                sqlx::query(
                    "INSERT INTO accounts (did, handle) VALUES ($1, $2) \
                     ON CONFLICT (did) DO UPDATE SET handle = excluded.handle",
                )
                .bind(did)
                .bind(handle)
                .execute(&mut *tx)
                .await?;
            }
            Event::Account { did, active, status } => {
                sqlx::query(
                    "INSERT INTO accounts (did, active, status) VALUES ($1, $2, $3) \
                     ON CONFLICT (did) DO UPDATE SET active = excluded.active, status = excluded.status",
                )
                .bind(did)
                .bind(active)
                .bind(status)
                .execute(&mut *tx)
                .await?;
            }
        }
    }
    sqlx::query("UPDATE meta SET archive_seq = $1")
        .bind(i64::try_from(seq).expect("archive positions fit in i64"))
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}

async fn apply_record(conn: &mut PgConnection, seq: Seq, e: &RecordEvent) -> Result<(), ProjectionError> {
    let uri = e.uri();
    let (cid, record) = match e.action {
        Action::Delete => (None, None),
        Action::Create | Action::Update => {
            let malformed = |missing| ProjectionError::Malformed {
                seq,
                action: e.action,
                missing,
            };
            let cid = e.cid.as_deref().ok_or_else(|| malformed("a CID"))?;
            let record = e.record.as_ref().ok_or_else(|| malformed("a record"))?;
            sqlx::query(
                "INSERT INTO record_versions (uri, cid, rev, record) VALUES ($1, $2, $3, $4) \
                 ON CONFLICT (uri, cid) DO NOTHING",
            )
            .bind(&uri)
            .bind(cid)
            .bind(&e.rev)
            .bind(record)
            .execute(&mut *conn)
            .await?;
            (Some(cid), Some(record))
        }
    };
    // Only a newer revision replaces what is there: a redelivered or late
    // event changes nothing.
    sqlx::query(
        "INSERT INTO records AS r (uri, did, collection, rkey, rev, cid, record) \
         VALUES ($1, $2, $3, $4, $5, $6, $7) \
         ON CONFLICT (uri) DO UPDATE SET rev = excluded.rev, cid = excluded.cid, record = excluded.record \
         WHERE r.rev < excluded.rev",
    )
    .bind(&uri)
    .bind(&e.did)
    .bind(&e.collection)
    .bind(&e.rkey)
    .bind(&e.rev)
    .bind(cid)
    .bind(record)
    .execute(&mut *conn)
    .await?;
    Ok(())
}
