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
use motet_appview::indexer::Indexer;
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
