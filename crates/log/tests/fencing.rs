//! Two writers can never both acknowledge the same position, and what readers
//! see is exactly what was acknowledged. (Acceptance test: "start a second
//! writer against the same bucket".)

use std::sync::Arc;

use bytes::Bytes;
use motet_log::{AppendError, Log};
use object_store::ObjectStore;
use object_store::memory::InMemory;
use proptest::prelude::*;

/// Two writers take over the same log, then append in the given interleaving.
async fn race(store: Arc<dyn ObjectStore>, schedule: &[bool]) {
    let log = Log::new(store, "shard-0");
    let mut writers = [log.writer().await.expect("writer a"), log.writer().await.expect("writer b")];
    let mut fenced = [false, false];
    let mut acknowledged = Vec::new();
    for (step, &second) in schedule.iter().enumerate() {
        let w = usize::from(second);
        if fenced[w] {
            continue;
        }
        let batch = Bytes::from(format!("writer {w} step {step}"));
        match writers[w].append(batch.clone()).await {
            Ok(seq) => acknowledged.push((seq, batch)),
            Err(AppendError::Fenced(_)) => fenced[w] = true,
            Err(e) => panic!("unexpected {e}"),
        }
    }
    assert_eq!(log.read_from(0).await.expect("read"), acknowledged);
}

proptest! {
    #[test]
    fn readers_see_exactly_the_acknowledged_appends(schedule in proptest::collection::vec(any::<bool>(), 0..40)) {
        tokio::runtime::Builder::new_current_thread().build().expect("runtime")
            .block_on(race(Arc::new(InMemory::new()), &schedule));
    }
}

/// The same race against the real bucket: this is also what checks that the
/// development and CI object store implements create-only PUTs.
#[tokio::test]
#[ignore = "integration: needs S3 (devenv up)"]
async fn fault_second_writer_against_real_bucket() {
    race(motet_testkit::services::bucket(), &[false, true, true, false, false, true, false]).await;
}
