//! Two writers can never both acknowledge the same position, and what readers
//! see is exactly what was acknowledged. Readers reject gaps and replay only
//! the requested suffix. (Acceptance test: "start a second writer against the
//! same bucket".)

use std::sync::Arc;

use bytes::Bytes;
use motet_log::{AppendError, Log, ReadError};
use object_store::memory::InMemory;
use object_store::path::Path;
use object_store::{ObjectStore, ObjectStoreExt};
use proptest::prelude::*;

/// Two writers take over the same log, then append in the given interleaving.
async fn race(store: Arc<dyn ObjectStore>, schedule: &[bool]) {
    let log = Log::new(store, "shard-0");
    let mut writers = [
        log.writer().await.expect("writer a"),
        log.writer().await.expect("writer b"),
    ];
    let mut fenced = [false, false];
    let mut acknowledged = Vec::new();
    for (step, &second) in schedule.iter().enumerate() {
        let w = usize::from(second);
        let is_fenced = fenced.get_mut(w).expect("a boolean selects one of the two writers");
        if *is_fenced {
            continue;
        }
        let batch = Bytes::from(format!("writer {w} step {step}"));
        match writers
            .get_mut(w)
            .expect("a boolean selects one of the two writers")
            .append(batch.clone())
            .await
        {
            Ok(seq) => acknowledged.push((seq, batch)),
            Err(AppendError::Fenced(_)) => *is_fenced = true,
            Err(e) => panic!("unexpected {e}"),
        }
    }
    assert_eq!(log.read_from(0).await.expect("read"), acknowledged);
}

async fn append_history(log: &Log, batches: Vec<Vec<u8>>) -> Vec<(u64, Bytes)> {
    let mut writer = log.writer().await.expect("writer");
    let mut acknowledged = Vec::with_capacity(batches.len());
    for batch in batches {
        let batch = Bytes::from(batch);
        let seq = writer.append(batch.clone()).await.expect("append");
        acknowledged.push((seq, batch));
    }
    acknowledged
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    #[test]
    fn readers_see_exactly_the_acknowledged_appends(schedule in proptest::collection::vec(any::<bool>(), 0..40)) {
        tokio::runtime::Builder::new_current_thread().build().expect("runtime")
            .block_on(race(Arc::new(InMemory::new()), &schedule));
    }

    /// Readers report the first absent position, not a truncated or gap-skipping replay.
    /// Removing the gap check or ignoring the requested cursor violates this property.
    #[test]
    fn readers_report_the_first_hole_after_the_cursor(
        layout in proptest::collection::vec(
            (any::<bool>(), proptest::collection::vec(any::<u8>(), 0..65)),
            2..32,
        ),
        hole in 0_usize..31,
        from in prop_oneof![Just(0_u64), 0_u64..33, any::<u64>(), Just(u64::MAX)],
    ) {
        tokio::runtime::Builder::new_current_thread().build().expect("runtime").block_on(async {
            let store: Arc<dyn ObjectStore> = Arc::new(InMemory::new());
            let log = Log::new(Arc::clone(&store), "read-layout");
            let (mut occupied, batches): (Vec<_>, Vec<_>) = layout.into_iter().unzip();
            let last = occupied.len() - 1;
            occupied[hole % last] = false;
            occupied[last] = true;
            let acknowledged = append_history(&log, batches).await;
            // `Log` cannot delete a segment, so this test builds the path itself and
            // deliberately couples to the layout in `Log::key`.
            for ((seq, _), present) in acknowledged.iter().zip(&occupied) {
                if !present {
                    store.delete(&Path::from(format!("read-layout/{seq:020}"))).await.expect("delete segment");
                }
            }

            // The retained final append bounds the holes; an absent tail is not a gap.
            let missing = acknowledged.iter().zip(&occupied)
                .find(|((seq, _), present)| *seq >= from && !**present)
                .map(|((seq, _), _)| *seq);
            match (missing, log.read_from(from).await) {
                (Some(expected), Err(ReadError::Gap(actual))) => assert_eq!(actual, expected),
                (None, Ok(actual)) => {
                    let expected: Vec<_> = acknowledged.into_iter().filter(|(seq, _)| *seq >= from).collect();
                    assert_eq!(actual, expected);
                }
                (expected, actual) => panic!("expected first hole {expected:?}, got {actual:?}"),
            }
        });
    }

    /// A resumed replay returns exactly its acknowledged byte suffix, including empty segments.
    /// Ignoring the cursor or listing another log's prefix leaks records into the replay.
    #[test]
    fn resumed_reads_return_only_the_acknowledged_suffix(
        batches in proptest::collection::vec(proptest::collection::vec(any::<u8>(), 0..65), 0..32),
        from in prop_oneof![Just(0_u64), 0_u64..33, any::<u64>(), Just(u64::MAX)],
    ) {
        tokio::runtime::Builder::new_current_thread().build().expect("runtime").block_on(async {
            let store: Arc<dyn ObjectStore> = Arc::new(InMemory::new());
            let log = Log::new(Arc::clone(&store), "read-target");
            // A prefix-colliding neighbor fails a listing that matches by string prefix.
            let neighbor = Log::new(store, "read-target-extra");
            neighbor.writer().await.expect("neighbor writer")
                .append(Bytes::from_static(b"unrelated log")).await.expect("append neighbor");
            let acknowledged = append_history(&log, batches).await;
            let expected: Vec<_> = acknowledged.into_iter().filter(|(seq, _)| *seq >= from).collect();
            assert_eq!(log.read_from(from).await.expect("read suffix"), expected);
        });
    }
}

/// The same race against the real bucket: this is also what checks that the
/// development and CI object store implements create-only PUTs.
#[tokio::test]
#[ignore = "integration: needs S3 (devenv up)"]
async fn fault_second_writer_against_real_bucket() {
    race(
        motet_testkit::services::bucket(),
        &[false, true, true, false, false, true, false],
    )
    .await;
}
