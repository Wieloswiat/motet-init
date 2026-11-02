//! "Rebuild gives the same projections", from the first week.
//!
//! Records arrive interleaved across repositories and redelivered; the live
//! indexer applies them as they come. The projections must be what the
//! records' commit order implies, whatever the delivery, and rebuilding from
//! the archive alone must give exactly the same tables.

use std::collections::BTreeMap;
use std::sync::Arc;

use motet_appview::archive::Archive;
use motet_appview::event::{Action, Event, RecordEvent};
use motet_appview::indexer::{IndexError, Indexer};
use motet_appview::{MIGRATOR, projection};
use object_store::memory::InMemory;
use proptest::prelude::*;
use sqlx::PgPool;

const COLLECTION: &str = "at.motet.pilot.post";

/// One repository's operations in commit order, with increasing revisions.
fn repo(did: &'static str) -> impl Strategy<Value = Vec<Event>> {
    proptest::collection::vec((0..3u8, 0..4u8), 0..12).prop_map(move |ops| {
        ops.into_iter()
            .enumerate()
            .map(|(n, (rkey, action))| {
                let action = match action {
                    0 => Action::Delete,
                    1 => Action::Create,
                    _ => Action::Update,
                };
                let live = action != Action::Delete;
                Event::Record(RecordEvent {
                    did: did.into(),
                    rev: format!("3l{n:011}"),
                    collection: COLLECTION.into(),
                    rkey: format!("k{rkey}"),
                    action,
                    cid: live.then(|| format!("bafy-{did}-{n}")),
                    record: live.then(|| serde_json::json!({ "n": n })),
                })
            })
            .collect()
    })
}

fn streams() -> impl Strategy<Value = Vec<Vec<Event>>> {
    (repo("did:plc:alice"), repo("did:plc:bob"), repo("did:plc:carol")).prop_map(|(a, b, c)| vec![a, b, c])
}

/// What the records table must hold: each record as its newest operation
/// left it.
fn expected(streams: &[Vec<Event>]) -> Vec<(String, Option<String>)> {
    let mut latest = BTreeMap::new();
    for event in streams.iter().flatten() {
        if let Event::Record(r) = event {
            latest.insert(r.uri(), r.cid.clone());
        }
    }
    latest.into_iter().collect()
}

async fn records(pool: &PgPool) -> Vec<(String, Option<String>)> {
    sqlx::query_as("SELECT uri, cid FROM projection.records ORDER BY uri")
        .fetch_all(pool)
        .await
        .expect("records")
}

async fn versions(pool: &PgPool) -> Vec<(String, String, String, serde_json::Value)> {
    sqlx::query_as("SELECT uri, cid, rev, record FROM projection.record_versions ORDER BY uri, cid")
        .fetch_all(pool)
        .await
        .expect("versions")
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(16))]

    #[test]
    #[ignore = "integration: needs PostgreSQL (devenv up)"]
    fn live_and_rebuilt_projections_match_commit_order(
        (streams, delivery) in streams().prop_flat_map(|s| (Just(s.clone()), motet_testkit::delivery::deliveries(s, 6))),
        batch in 1..6usize,
    ) {
        tokio::runtime::Builder::new_current_thread().enable_all().build().expect("runtime").block_on(async {
            let pool = motet_testkit::services::database().await;
            MIGRATOR.run(&pool).await.expect("migrations");
            let archive = Archive::new(Arc::new(InMemory::new()), "archive");

            let mut indexer = Indexer::start(pool.clone(), &archive).await.expect("indexer");
            for chunk in delivery.chunks(batch) {
                indexer.ingest(chunk).await.expect("ingest");
            }
            let live = (records(&pool).await, versions(&pool).await);
            prop_assert_eq!(&live.0, &expected(&streams));

            projection::rebuild(&pool, &archive).await.expect("rebuild");
            prop_assert_eq!((records(&pool).await, versions(&pool).await), live);
            Ok(())
        })?;
    }
}

/// An indexer that stopped after archiving but before applying catches up
/// on start, so projections never lag the archive for good.
#[tokio::test]
#[ignore = "integration: needs PostgreSQL (devenv up)"]
async fn a_restart_applies_what_was_archived_but_not_applied() {
    let pool = motet_testkit::services::database().await;
    MIGRATOR.run(&pool).await.expect("migrations");
    let archive = Archive::new(Arc::new(InMemory::new()), "archive");
    let event = Event::Record(RecordEvent {
        did: "did:plc:alice".into(),
        rev: "3l00000000001".into(),
        collection: COLLECTION.into(),
        rkey: "k".into(),
        action: Action::Create,
        cid: Some("bafy".into()),
        record: Some(serde_json::json!({})),
    });
    drop(Indexer::start(pool.clone(), &archive).await.expect("first start"));
    // The crash window: durable in the archive, never applied.
    archive
        .writer()
        .await
        .expect("writer")
        .append(&[event])
        .await
        .expect("archived");

    Indexer::start(pool.clone(), &archive).await.expect("restart");
    assert_eq!(
        records(&pool).await,
        vec![(
            "at://did:plc:alice/at.motet.pilot.post/k".to_owned(),
            Some("bafy".to_owned())
        )]
    );
}

/// An unapplied durable segment cannot be skipped by a later successful batch.
/// Without poisoning, advancing the watermark would make restart lose that segment.
#[tokio::test]
#[ignore = "integration: needs PostgreSQL and S3 (devenv up)"]
async fn fault_incomplete_batch_cannot_be_skipped_and_restart_replays_it() {
    let pool = motet_testkit::services::database().await;
    MIGRATOR.run(&pool).await.expect("migrations");
    let archive = Archive::new(motet_testkit::services::bucket(), "archive");
    let event = |key: &str, rev: u32| {
        Event::Record(RecordEvent {
            did: "did:plc:alice".into(),
            rev: format!("3l{rev:011}"),
            collection: COLLECTION.into(),
            rkey: key.into(),
            action: Action::Create,
            cid: Some(format!("cid-{key}")),
            record: Some(serde_json::json!({"text": key})),
        })
    };
    let first = event("first", 1);
    let interrupted = event("interrupted", 2);
    let later = event("later", 3);
    let mut indexer = Indexer::start(pool.clone(), &archive).await.expect("start");
    let first_seq = indexer.ingest(std::slice::from_ref(&first)).await.expect("first batch");

    // The next archive append succeeds, but PostgreSQL rejects its projection.
    sqlx::query("ALTER TABLE projection.records ADD CONSTRAINT injected_failure CHECK (rkey <> 'interrupted')")
        .execute(&pool)
        .await
        .expect("inject transient database failure");
    assert!(matches!(
        indexer.ingest(std::slice::from_ref(&interrupted)).await,
        Err(IndexError::Projection(projection::ProjectionError::Db(_)))
    ));
    sqlx::query("ALTER TABLE projection.records DROP CONSTRAINT injected_failure")
        .execute(&pool)
        .await
        .expect("restore database");

    assert!(matches!(
        indexer.ingest(std::slice::from_ref(&later)).await,
        Err(IndexError::Poisoned)
    ));
    let applied: Option<i64> = sqlx::query_scalar("SELECT archive_seq FROM projection.meta")
        .fetch_one(&pool)
        .await
        .expect("watermark");
    assert_eq!(applied, Some(i64::try_from(first_seq).expect("sequence fits")));
    let archived = archive.read_from(0).await.expect("archive");
    assert_eq!(archived.len(), 2);
    assert_eq!(archived[0].1, vec![first]);
    assert_eq!(archived[1].1, vec![interrupted.clone()]);
    assert_eq!(
        records(&pool).await,
        vec![(
            "at://did:plc:alice/at.motet.pilot.post/first".to_owned(),
            Some("cid-first".to_owned())
        )]
    );

    drop(indexer);
    let mut restarted = Indexer::start(pool.clone(), &archive)
        .await
        .expect("restart catches up");
    assert_eq!(
        records(&pool).await,
        vec![
            (
                "at://did:plc:alice/at.motet.pilot.post/first".to_owned(),
                Some("cid-first".to_owned())
            ),
            (
                "at://did:plc:alice/at.motet.pilot.post/interrupted".to_owned(),
                Some("cid-interrupted".to_owned())
            )
        ]
    );
    restarted.ingest(&[later]).await.expect("new batch after catch-up");
    assert_eq!(
        records(&pool).await,
        vec![
            (
                "at://did:plc:alice/at.motet.pilot.post/first".to_owned(),
                Some("cid-first".to_owned())
            ),
            (
                "at://did:plc:alice/at.motet.pilot.post/interrupted".to_owned(),
                Some("cid-interrupted".to_owned())
            ),
            (
                "at://did:plc:alice/at.motet.pilot.post/later".to_owned(),
                Some("cid-later".to_owned())
            )
        ]
    );
}
