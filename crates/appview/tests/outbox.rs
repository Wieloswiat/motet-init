//! The outbox's leases: no job is worked twice at once, one account's jobs go
//! out in order, and a worker whose lease ran out can't record an outcome.

use std::time::Duration;

use motet_appview::MIGRATOR;
use motet_appview::outbox::{claim, enqueue, finish};
use serde_json::json;
use sqlx::PgPool;

async fn pool() -> PgPool {
    let pool = motet_testkit::services::database().await;
    MIGRATOR.run(&pool).await.expect("migrations");
    pool
}

#[tokio::test]
#[ignore = "integration: needs PostgreSQL (devenv up)"]
async fn the_same_operation_is_queued_once() {
    let pool = pool().await;
    let a = enqueue(&pool, "did:plc:alice", "op-1", "post", &json!({}))
        .await
        .expect("enqueue");
    let b = enqueue(&pool, "did:plc:alice", "op-1", "post", &json!({}))
        .await
        .expect("enqueue again");
    assert_eq!(a, b);
}

#[tokio::test]
#[ignore = "integration: needs PostgreSQL (devenv up)"]
async fn concurrent_workers_never_share_a_job() {
    let pool = pool().await;
    for i in 0..40 {
        enqueue(
            &pool,
            &format!("did:plc:user{i}"),
            &format!("op-{i}"),
            "post",
            &json!({}),
        )
        .await
        .expect("enqueue");
    }
    let lease = Duration::from_secs(60);
    let workers = (0..8).map(|w| {
        let pool = pool.clone();
        tokio::spawn(async move { claim(&pool, &format!("worker-{w}"), 10, lease).await.expect("claim") })
    });
    let mut claimed: Vec<_> = Vec::new();
    for worker in workers {
        claimed.extend(worker.await.expect("worker").into_iter().map(|j| j.id));
    }
    let total = claimed.len();
    claimed.sort();
    claimed.dedup();
    assert_eq!(claimed.len(), total, "a job was claimed twice");
    assert_eq!(total, 40);
}

#[tokio::test]
#[ignore = "integration: needs PostgreSQL (devenv up)"]
async fn an_accounts_jobs_go_out_in_order() {
    let pool = pool().await;
    enqueue(&pool, "did:plc:alice", "op-1", "post", &json!({}))
        .await
        .expect("enqueue");
    enqueue(&pool, "did:plc:alice", "op-2", "post", &json!({}))
        .await
        .expect("enqueue");
    let lease = Duration::from_secs(60);

    let jobs = claim(&pool, "w", 10, lease).await.expect("claim");
    assert_eq!(jobs.iter().map(|j| j.op_id.as_str()).collect::<Vec<_>>(), ["op-1"]);
    assert!(
        claim(&pool, "w", 10, lease).await.expect("claim").is_empty(),
        "op-2 waits for op-1"
    );

    assert!(finish(&pool, &jobs[0], "w", false, &json!({})).await.expect("finish"));
    let jobs = claim(&pool, "w", 10, lease).await.expect("claim");
    assert_eq!(jobs.iter().map(|j| j.op_id.as_str()).collect::<Vec<_>>(), ["op-2"]);
}

#[tokio::test]
#[ignore = "integration: needs PostgreSQL (devenv up)"]
async fn an_expired_lease_passes_the_job_on() {
    let pool = pool().await;
    enqueue(&pool, "did:plc:alice", "op-1", "post", &json!({}))
        .await
        .expect("enqueue");
    let stale = claim(&pool, "slow", 1, Duration::from_millis(1)).await.expect("claim");
    assert_eq!(stale.len(), 1);
    tokio::time::sleep(Duration::from_millis(20)).await;

    let taken = claim(&pool, "fast", 1, Duration::from_secs(60)).await.expect("claim");
    assert_eq!(taken.first().map(|j| j.attempts), Some(2));
    assert!(
        !finish(&pool, &stale[0], "slow", false, &json!({"by": "slow"}))
            .await
            .expect("finish")
    );
    assert!(
        finish(&pool, &taken[0], "fast", false, &json!({"by": "fast"}))
            .await
            .expect("finish")
    );
}
