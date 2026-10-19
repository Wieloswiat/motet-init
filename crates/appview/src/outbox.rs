//! The outbox: writes Motet makes on people's behalf, retried safely.
//!
//! A post goes to its author's PDS with a create-only or compare-and-swap
//! condition, and its admission request to the World's tenor, both under one
//! operation ID. Workers lease jobs with `FOR UPDATE SKIP LOCKED`; a crashed
//! worker's lease runs out and another worker takes the job. Taking a job
//! over is safe only because an ambiguous outcome is reconciled by reading
//! back before any retry, never by sending again.

use std::time::Duration;

use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq, sqlx::FromRow)]
pub struct Job {
    pub id: Uuid,
    pub account: String,
    pub op_id: String,
    pub kind: String,
    pub payload: Value,
    pub attempts: i32,
}

/// Queues a job. Queuing the same operation ID twice is a no-op that returns
/// the existing job's ID, so a client retrying a request never doubles it.
pub async fn enqueue(pool: &PgPool, account: &str, op_id: &str, kind: &str, payload: &Value) -> sqlx::Result<Uuid> {
    sqlx::query_scalar(
        "WITH inserted AS ( \
             INSERT INTO service.outbox (id, account, op_id, kind, payload) VALUES ($1, $2, $3, $4, $5) \
             ON CONFLICT (op_id) DO NOTHING RETURNING id) \
         SELECT id FROM inserted UNION ALL SELECT id FROM service.outbox WHERE op_id = $3 LIMIT 1",
    )
    .bind(Uuid::now_v7())
    .bind(account)
    .bind(op_id)
    .bind(kind)
    .bind(payload)
    .fetch_one(pool)
    .await
}

/// Leases up to `limit` jobs for `owner`. A job is ready when it is queued
/// and unleased (or its lease ran out), and no earlier job of the same
/// account is still queued: one account's writes go out in order.
pub async fn claim(pool: &PgPool, owner: &str, limit: i64, lease: Duration) -> sqlx::Result<Vec<Job>> {
    sqlx::query_as(
        "UPDATE service.outbox o SET lease_owner = $1, lease_until = now() + $2, attempts = attempts + 1 \
         WHERE o.id IN ( \
             SELECT id FROM service.outbox j \
             WHERE j.state = 'queued' AND (j.lease_until IS NULL OR j.lease_until < now()) \
               AND NOT EXISTS ( \
                   SELECT 1 FROM service.outbox e \
                   WHERE e.account = j.account AND e.state = 'queued' AND e.created_at < j.created_at) \
             ORDER BY j.created_at LIMIT $3 FOR UPDATE SKIP LOCKED) \
         RETURNING o.id, o.account, o.op_id, o.kind, o.payload, o.attempts",
    )
    .bind(owner)
    .bind(lease)
    .bind(limit)
    .fetch_all(pool)
    .await
}

/// Records a job's final outcome. Only the lease holder can finish it, so a
/// worker whose lease ran out can't overwrite its successor's result.
pub async fn finish(pool: &PgPool, id: Uuid, owner: &str, failed: bool, outcome: &Value) -> sqlx::Result<bool> {
    let done = sqlx::query(
        "UPDATE service.outbox SET state = $3, outcome = $4, lease_owner = NULL, lease_until = NULL \
         WHERE id = $1 AND lease_owner = $2 AND state = 'queued'",
    )
    .bind(id)
    .bind(owner)
    .bind(if failed { "failed" } else { "done" })
    .bind(outcome)
    .execute(pool)
    .await?;
    Ok(done.rows_affected() == 1)
}
