//! Durable, account-ordered writes to people’s PDSes and World tenors.
//!
//! Each claim has a generation as well as a lease owner. A worker that lost
//! its lease cannot publish a result, even if the same owner claims it again.
//! Admission retries use the original operation and strong reference.

use std::sync::Arc;
use std::time::Duration;

use serde_json::Value;
use sqlx::PgPool;
use tokio::sync::watch;
use uuid::Uuid;

use crate::auth::{self, store::SessionRef};
use crate::writes::{client::Client, post};

const LEASE: Duration = Duration::from_secs(180);
const POLL: Duration = Duration::from_secs(1);
pub(crate) const POST_KIND: &str = "create-post";

#[derive(Clone, Debug, PartialEq, Eq, sqlx::FromRow)]
pub struct Job {
    pub id: Uuid,
    pub account: String,
    pub op_id: String,
    pub kind: String,
    pub payload: Value,
    pub attempts: i32,
    pub session_id: Option<Vec<u8>>,
}

/// Queues an operation once. Account insertion is serialized so a concurrent
/// insert cannot become visible ahead of an earlier uncommitted operation.
pub async fn enqueue(pool: &PgPool, account: &str, op_id: &str, kind: &str, payload: &Value) -> sqlx::Result<Uuid> {
    insert(pool, account, op_id, kind, payload, None).await
}

pub(crate) async fn enqueue_post(
    pool: &PgPool,
    account: &str,
    op_id: &str,
    payload: &Value,
    session: &SessionRef,
) -> sqlx::Result<Uuid> {
    insert(pool, account, op_id, POST_KIND, payload, Some(session.as_bytes())).await
}

async fn insert(
    pool: &PgPool,
    account: &str,
    op_id: &str,
    kind: &str,
    payload: &Value,
    session: Option<&[u8]>,
) -> sqlx::Result<Uuid> {
    let mut tx = pool.begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
        .bind(account)
        .execute(&mut *tx)
        .await?;
    let id = sqlx::query_scalar(
        "INSERT INTO service.outbox (id, account, op_id, kind, payload, session_id) \
         VALUES ($1, $2, $3, $4, $5, $6) \
         ON CONFLICT (op_id) DO UPDATE SET op_id = EXCLUDED.op_id RETURNING id",
    )
    .bind(Uuid::now_v7())
    .bind(account)
    .bind(op_id)
    .bind(kind)
    .bind(payload)
    .bind(session)
    .fetch_one(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(id)
}

/// Claims due jobs, preserving PDS write and tenor admission order per account.
///
/// A published post waiting on admission does not block later PDS writes.
/// `attempts` is the claim generation, not a retry count alone.
pub async fn claim(pool: &PgPool, owner: &str, limit: i64, lease: Duration) -> sqlx::Result<Vec<Job>> {
    claim_matching(pool, owner, limit, lease, None, None).await
}

async fn claim_matching(
    pool: &PgPool,
    owner: &str,
    limit: i64,
    lease: Duration,
    kind: Option<&str>,
    id: Option<Uuid>,
) -> sqlx::Result<Vec<Job>> {
    sqlx::query_as(
        "UPDATE service.outbox o SET lease_owner = $1, lease_until = clock_timestamp() + $2, attempts = attempts + 1 \
         WHERE o.id IN ( \
             SELECT j.id FROM service.outbox j \
             WHERE j.state = 'queued' AND NOT j.paused AND j.next_attempt_at <= clock_timestamp() \
               AND (j.lease_until IS NULL OR j.lease_until <= clock_timestamp()) \
               AND ($4::text IS NULL OR j.kind = $4) AND ($5::uuid IS NULL OR j.id = $5) \
               AND NOT EXISTS ( \
                   SELECT 1 FROM service.outbox e \
                   WHERE e.account = j.account AND e.state = 'queued' AND e.sequence < j.sequence \
                     AND NOT (j.kind = 'create-post' AND j.payload->>'stage' IN ('publish', 'publishing') \
                              AND e.kind = 'create-post' AND e.payload->>'stage' = 'admission')) \
             ORDER BY j.sequence LIMIT $3 FOR UPDATE OF j SKIP LOCKED) \
         RETURNING o.id, o.account, o.op_id, o.kind, o.payload, o.attempts, o.session_id",
    )
    .bind(owner)
    .bind(lease)
    .bind(limit)
    .bind(kind)
    .bind(id)
    .fetch_all(pool)
    .await
}

pub(crate) async fn claim_post(pool: &PgPool, owner: &str, id: Option<Uuid>) -> sqlx::Result<Option<Job>> {
    Ok(claim_matching(pool, owner, 1, LEASE, Some(POST_KIND), id)
        .await?
        .into_iter()
        .next())
}

/// Records a final outcome only while this exact claim is still live.
pub async fn finish(pool: &PgPool, job: &Job, owner: &str, failed: bool, outcome: &Value) -> sqlx::Result<bool> {
    let done = sqlx::query(
        "UPDATE service.outbox SET state = $4, outcome = $5, lease_owner = NULL, lease_until = NULL \
         WHERE id = $1 AND lease_owner = $2 AND attempts = $3 AND state = 'queued' \
           AND lease_until > clock_timestamp()",
    )
    .bind(job.id)
    .bind(owner)
    .bind(job.attempts)
    .bind(if failed { "failed" } else { "done" })
    .bind(outcome)
    .execute(pool)
    .await?;
    Ok(done.rows_affected() == 1)
}

/// Marks the create-only PDS call as in flight before it can be sent.
pub(crate) async fn mark_publishing(pool: &PgPool, job: &Job, owner: &str) -> sqlx::Result<bool> {
    let updated = sqlx::query(
        "UPDATE service.outbox SET payload = jsonb_set(payload, '{stage}', '\"publishing\"'::jsonb) \
         WHERE id = $1 AND lease_owner = $2 AND attempts = $3 AND state = 'queued' \
           AND lease_until > clock_timestamp() AND payload->>'stage' = 'publish'",
    )
    .bind(job.id)
    .bind(owner)
    .bind(job.attempts)
    .execute(pool)
    .await?;
    Ok(updated.rows_affected() == 1)
}

/// Makes a stage transition durable before the corresponding outbound call.
pub(crate) async fn advance(pool: &PgPool, job: &Job, owner: &str, payload: &Value) -> sqlx::Result<bool> {
    let updated = sqlx::query(
        "UPDATE service.outbox SET payload = $4 \
         WHERE id = $1 AND lease_owner = $2 AND attempts = $3 AND state = 'queued' \
           AND lease_until > clock_timestamp()",
    )
    .bind(job.id)
    .bind(owner)
    .bind(job.attempts)
    .bind(payload)
    .execute(pool)
    .await?;
    Ok(updated.rows_affected() == 1)
}

/// An admission must not overtake any earlier unfinished account operation.
pub(crate) async fn may_submit(pool: &PgPool, job: &Job, owner: &str) -> sqlx::Result<bool> {
    sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM service.outbox j \
         WHERE j.id = $1 AND j.lease_owner = $2 AND j.attempts = $3 AND j.state = 'queued' \
           AND j.lease_until > clock_timestamp() AND j.payload->>'stage' = 'admission' \
           AND NOT EXISTS (SELECT 1 FROM service.outbox e WHERE e.account = j.account \
                           AND e.state = 'queued' AND e.sequence < j.sequence))",
    )
    .bind(job.id)
    .bind(owner)
    .bind(job.attempts)
    .fetch_one(pool)
    .await
}

/// Releases the lease and delays the next try. A pending earlier job still
/// blocks later admissions, but not later PDS writes.
pub(crate) async fn reschedule(pool: &PgPool, job: &Job, owner: &str, outcome: &Value) -> sqlx::Result<bool> {
    let exponent = u32::try_from(job.attempts.clamp(1, 6)).unwrap_or(6);
    let delay = Duration::from_secs(1_u64 << exponent);
    let updated = sqlx::query(
        "UPDATE service.outbox SET outcome = $4, next_attempt_at = clock_timestamp() + $5, \
         lease_owner = NULL, lease_until = NULL \
         WHERE id = $1 AND lease_owner = $2 AND attempts = $3 AND state = 'queued' \
           AND lease_until > clock_timestamp()",
    )
    .bind(job.id)
    .bind(owner)
    .bind(job.attempts)
    .bind(outcome)
    .bind(delay)
    .execute(pool)
    .await?;
    Ok(updated.rows_affected() == 1)
}

/// Pauses admission until the same author signs in again. If a new session
/// already replaced this claim's reference, release it for an immediate retry.
pub(crate) async fn pause(pool: &PgPool, job: &Job, owner: &str, outcome: &Value) -> sqlx::Result<bool> {
    let updated = sqlx::query(
        "UPDATE service.outbox SET outcome = $4, paused = session_id IS NOT DISTINCT FROM $5, \
         next_attempt_at = clock_timestamp(), lease_owner = NULL, lease_until = NULL \
         WHERE id = $1 AND lease_owner = $2 AND attempts = $3 AND state = 'queued' \
           AND lease_until > clock_timestamp()",
    )
    .bind(job.id)
    .bind(owner)
    .bind(job.attempts)
    .bind(outcome)
    .bind(job.session_id.as_deref())
    .execute(pool)
    .await?;
    Ok(updated.rows_affected() == 1)
}

/// Binds unfinished work to a newly authenticated session for the same DID.
///
/// The caller must verify the session and DID before invoking this function.
/// A worker already in flight keeps its lease and is fenced on completion.
pub async fn resume_account(pool: &PgPool, did: &str, session: &SessionRef) -> sqlx::Result<()> {
    sqlx::query(
        "UPDATE service.outbox SET session_id = $2, \
         next_attempt_at = CASE WHEN paused THEN clock_timestamp() ELSE next_attempt_at END, paused = false \
         WHERE account = $1 AND kind = $3 AND state = 'queued'",
    )
    .bind(did)
    .bind(session.as_bytes())
    .bind(POST_KIND)
    .execute(pool)
    .await?;
    Ok(())
}

/// Runs until shutdown, finishing the current attempt before returning.
///
/// Expired leases are taken over after a restart; idle and failed database
/// polls wait rather than spinning. Start one worker per AppView instance.
pub async fn run(pool: PgPool, auth: Arc<auth::Server>, client: Arc<Client>, mut shutdown: watch::Receiver<bool>) {
    loop {
        if *shutdown.borrow() {
            return;
        }
        let owner = Uuid::now_v7().to_string();
        match claim_post(&pool, &owner, None).await {
            Ok(Some(job)) => {
                if let Err(error) = post::process(&pool, &auth, &client, &job, &owner).await {
                    tracing::error!(%error, job = %job.id, "post outbox attempt failed");
                }
            }
            Ok(None) => {
                tokio::select! {
                    () = tokio::time::sleep(POLL) => {},
                    changed = shutdown.changed() => if changed.is_err() || *shutdown.borrow() { return; },
                }
            }
            Err(error) => {
                tracing::error!(%error, "claiming post outbox work failed");
                tokio::select! {
                    () = tokio::time::sleep(POLL) => {},
                    changed = shutdown.changed() => if changed.is_err() || *shutdown.borrow() { return; },
                }
            }
        }
    }
}
