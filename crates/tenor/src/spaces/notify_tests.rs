//! Notification properties through the real shard transaction and replay boundary.

use bytes::Bytes;
use object_store::ObjectStore;
use object_store::memory::InMemory;

use super::*;
use crate::keys::MasterKey;
use crate::shard::{self, Options, Shard, TxnError};
use crate::spaces::{self, AccessRecord, GMS, ProfileInput};

const WORLD: &str = "did:plc:aaaaaaaaaaaaaaaaaaaaaaaa";
const OWNER: &str = "did:plc:bbbbbbbbbbbbbbbbbbbbbbbb";
const OUTSIDER: &str = "did:plc:cccccccccccccccccccccccc";
const SYNCER: &str = "did:web:syncer.example#atproto_space_syncer";

fn space() -> SpaceUri {
    spaces::world_space(&Did::new(WORLD), GMS).unwrap()
}

async fn open(store: &Arc<dyn ObjectStore>) -> Shard {
    Shard::open(
        Arc::clone(store),
        "notifications",
        Arc::new(MasterKey::from_bytes(&[19; 32])),
    )
    .await
    .unwrap()
}

async fn fixture(subscribe: bool) -> (Arc<dyn ObjectStore>, Handle) {
    let store: Arc<dyn ObjectStore> = Arc::new(InMemory::new());
    let handle = shard::spawn(
        open(&store).await,
        Options {
            max_batch: 16,
            checkpoint_every: Duration::from_secs(3600),
        },
    );
    handle.run(Setup { subscribe }).await.unwrap().unwrap();
    (store, handle)
}

struct Setup {
    subscribe: bool,
}

impl Op for Setup {
    type Output = ();
    type Error = SpaceError;

    async fn run(self, txn: &mut Txn<'_>) -> Result<(), SpaceError> {
        let world = Did::new(WORLD);
        txn.begin_world(world.clone(), 1)
            .map_err(|error| SpaceError::Invalid(error.to_string()))?;
        txn.generate_key(&world, Purpose::Signing).map_err(TxnError::from)?;
        txn.generate_key(&world, Purpose::Kem).map_err(TxnError::from)?;
        crate::repo::genesis(txn, &world).await?;
        spaces::bootstrap(txn, &world, &Did::new(OWNER), ProfileInput::default()).await?;
        if self.subscribe {
            subscribe(txn).await?;
        }
        Ok(())
    }
}

async fn subscribe(txn: &mut Txn<'_>) -> Result<(), SpaceError> {
    let registration = Registration {
        service: SYNCER.into(),
        endpoint: "https://syncer.example".into(),
        generation: info_txn(txn, &space()).await?.generation,
        expires: i64::try_from(now_ms()? / 1000).unwrap() + DEFAULT_REGISTER_NOTIFY_TTL_SECS,
    };
    txn.put(
        keys::space_notify_registration(&space(), SYNCER),
        encode(&registration)?,
    );
    Ok(())
}

fn payload(writer: &str, rev: &str, data: u8) -> NotifyWrite {
    NotifyWrite {
        space: space().to_string(),
        repo: writer.into(),
        repo_rev: rev.into(),
        data: LexBytes(vec![data; 32]),
        // A sender cannot choose the authority's sequence, even on the first notification.
        space_rev: Some("2222222222222".into()),
        prev_space_rev: Some("2222222222222".into()),
    }
}

struct Receive(NotifyWrite);

impl Op for Receive {
    type Output = ();
    type Error = SpaceError;

    async fn run(self, txn: &mut Txn<'_>) -> Result<(), SpaceError> {
        let issuer = self.0.repo.clone();
        receive_write(txn, &space(), &issuer, WORLD, self.0).await
    }
}

async fn pending(reader: &Reader) -> Vec<(Bytes, Bytes)> {
    reader.scan(keys::SPACE_NOTIFY_OUTBOX, None, usize::MAX).await.unwrap()
}

/// Refusing write access must leave both tracking and forwarding untouched.
/// This catches sequencing or enqueueing before the authority checks the writer.
#[tokio::test]
async fn denied_notify_write_records_nothing() {
    let (_store, handle) = fixture(true).await;
    let before = list_repos(handle.reader(), &space(), None, 100).await.unwrap();
    let result = handle
        .run(Receive(payload(OUTSIDER, &Tid::new().to_string(), 9)))
        .await
        .unwrap();
    assert!(matches!(result, Err(SpaceError::NotAuthorized)));
    assert!(
        handle
            .reader()
            .get(&keys::space_notify_writer(&space(), OUTSIDER))
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(pending(handle.reader()).await, []);
    let after = list_repos(handle.reader(), &space(), None, 100).await.unwrap();
    assert_eq!(
        serde_json::to_value(before).unwrap(),
        serde_json::to_value(after).unwrap()
    );
}

/// Reordered deliveries preserve each writer's greatest revision and enqueue each advance once.
/// Removing the monotonic guard would overwrite the digest and send three notifications.
#[tokio::test]
async fn duplicate_and_older_revisions_do_not_enqueue_again() {
    let (_store, handle) = fixture(true).await;
    let rev = Tid::new();
    handle
        .run(Receive(payload(WORLD, &rev.to_string(), 7)))
        .await
        .unwrap()
        .unwrap();
    let before = pending(handle.reader()).await;
    assert_eq!(before.len(), 1);
    let older = Tid::from_parts(rev.timestamp_micros() - 1, 0);
    for repeated in [rev, older, rev] {
        handle
            .run(Receive(payload(WORLD, &repeated.to_string(), 99)))
            .await
            .unwrap()
            .unwrap();
    }
    assert_eq!(pending(handle.reader()).await, before);
    let page = list_repos(handle.reader(), &space(), None, 1).await.unwrap();
    assert_eq!(page.repos.len(), 1);
    assert_eq!(page.repos[0].repo_rev, rev.to_string());
    assert_eq!(page.repos[0].data.0, vec![7; 32]);
    assert_eq!(page.cursor.as_deref(), Some(page.repos[0].space_rev.as_str()));

    let next = Tid::next_after(&rev);
    handle
        .run(Receive(payload(WORLD, &next.to_string(), 8)))
        .await
        .unwrap()
        .unwrap();
    let pending = pending(handle.reader()).await;
    assert_eq!(pending.len(), 2);
    let first: Intent = decode(&pending[0].1).unwrap();
    let second: Intent = decode(&pending[1].1).unwrap();
    let (Notification::Write(first), Notification::Write(second)) = (first.notification, second.notification) else {
        panic!("writes must queue write notifications");
    };
    assert_eq!(second.prev_space_rev, first.space_rev);
    assert!(second.space_rev > first.space_rev);
    assert_ne!(first.space_rev.as_deref(), Some("2222222222222"));
    let page = list_repos(handle.reader(), &space(), page.cursor.as_deref(), 100)
        .await
        .unwrap();
    assert_eq!(
        page.repos.len(),
        1,
        "a previously seen writer is visible after its next advance"
    );
    assert_eq!(page.repos[0].repo_rev, next.to_string());
}

/// A crash after acknowledging a write cannot lose its undelivered notification.
/// This catches an in-memory queue or an enqueue performed after the durable Op.
#[tokio::test]
async fn reopening_preserves_pending_notifications_and_writer_revisions() {
    let (store, handle) = fixture(true).await;
    let rev = Tid::new().to_string();
    handle.run(Receive(payload(WORLD, &rev, 3))).await.unwrap().unwrap();
    let before = pending(handle.reader()).await;
    assert_eq!(before.len(), 1);
    drop(handle);
    let reopened = open(&store).await;
    assert_eq!(pending(&reopened.reader()).await, before);
    let page = list_repos(&reopened.reader(), &space(), None, 100).await.unwrap();
    assert_eq!(page.repos[0].repo_rev, rev);
}

/// A queued request may not be sent after its registration expires, even if it was due earlier.
/// This catches checking only `due_ms`, or using the live credential's expiry instead of registration expiry.
#[tokio::test]
async fn registration_deadline_retires_a_queued_write_without_delivery() {
    let (_store, handle) = fixture(true).await;
    handle
        .run(Receive(payload(WORLD, &Tid::new().to_string(), 1)))
        .await
        .unwrap()
        .unwrap();
    let rows = pending(handle.reader()).await;
    let intent: Intent = decode(&rows[0].1).unwrap();
    let result = handle
        .run(Prepare {
            key: rows[0].0.to_vec(),
            now_ms: intent.deadline_ms,
        })
        .await
        .unwrap()
        .unwrap();
    assert!(result.is_none());
    assert_eq!(pending(handle.reader()).await, []);
}

struct Delete;

impl Op for Delete {
    type Output = ();
    type Error = SpaceError;

    async fn run(self, txn: &mut Txn<'_>) -> Result<(), SpaceError> {
        spaces::delete(txn, &space()).await
    }
}

struct Recreate;

impl Op for Recreate {
    type Output = ();
    type Error = SpaceError;

    async fn run(self, txn: &mut Txn<'_>) -> Result<(), SpaceError> {
        let access: AccessRecord = serde_json::from_value(serde_json::json!({ "public": false, "grants": [] }))?;
        spaces::create(txn, &space(), access, false).await?;
        subscribe(txn).await
    }
}

/// Deletion delivery belongs to one lifetime, not merely to the URI.
/// This catches an old attempt deleting a new queue row or notifying after recreation.
#[tokio::test]
async fn recreation_cancels_old_deletion_and_late_completion_keeps_new_deletion() {
    let (_store, handle) = fixture(true).await;
    handle.run(Delete).await.unwrap().unwrap();
    let first = pending(handle.reader()).await;
    assert_eq!(first.len(), 1);
    let first_key = first[0].0.to_vec();
    let old: Intent = decode(&first[0].1).unwrap();
    assert!(matches!(old.notification, Notification::Deleted));
    handle.run(Recreate).await.unwrap().unwrap();
    let current = info(handle.reader(), &space()).await.unwrap();
    assert!(!current.deleted);
    assert_ne!(current.generation, old.generation);
    let canceled = handle
        .run(Prepare {
            key: first_key.clone(),
            now_ms: now_ms().unwrap(),
        })
        .await
        .unwrap()
        .unwrap();
    assert!(canceled.is_none());

    handle.run(Delete).await.unwrap().unwrap();
    let new = pending(handle.reader()).await;
    assert_eq!(new.len(), 1);
    let newer: Intent = decode(&new[0].1).unwrap();
    assert_eq!(newer.generation, current.generation);
    handle
        .run(Finish {
            key: first_key,
            intent: old,
            result: Delivery::Retry("late transport failure".into()),
            now_ms: now_ms().unwrap(),
        })
        .await
        .unwrap()
        .unwrap();
    assert_eq!(pending(handle.reader()).await, new);
}

/// A temporary refusal retains the intent, with bounded backoff; a terminal refusal removes it.
/// This catches losing queued retries or a stale completion scheduling an attempt after the deadline.
#[tokio::test]
async fn retries_back_off_without_outliving_the_registration() {
    let (_store, handle) = fixture(true).await;
    handle
        .run(Receive(payload(WORLD, &Tid::new().to_string(), 1)))
        .await
        .unwrap()
        .unwrap();
    let rows = pending(handle.reader()).await;
    let intent: Intent = decode(&rows[0].1).unwrap();
    let now = now_ms().unwrap();
    handle
        .run(Finish {
            key: rows[0].0.to_vec(),
            intent,
            result: Delivery::Retry("HTTP 503".into()),
            now_ms: now,
        })
        .await
        .unwrap()
        .unwrap();
    let retried = pending(handle.reader()).await;
    assert_eq!(retried.len(), 1);
    let intent: Intent = decode(&retried[0].1).unwrap();
    assert!((30_000..=60_000).contains(&(intent.due_ms - now)));
    assert_eq!(intent.attempts, 1);
    let deadline = intent.deadline_ms;
    handle
        .run(Finish {
            key: retried[0].0.to_vec(),
            intent,
            result: Delivery::Retry("HTTP 429".into()),
            now_ms: deadline - 1,
        })
        .await
        .unwrap()
        .unwrap();
    let last = pending(handle.reader()).await;
    let intent: Intent = decode(&last[0].1).unwrap();
    assert_eq!(intent.due_ms, deadline);
    handle
        .run(Finish {
            key: last[0].0.to_vec(),
            intent,
            result: Delivery::Terminal("HTTP 501".into()),
            now_ms: deadline - 1,
        })
        .await
        .unwrap()
        .unwrap();
    assert_eq!(pending(handle.reader()).await, []);
}

struct Revoke {
    issuer: &'static str,
}

impl Op for Revoke {
    type Output = ();
    type Error = SpaceError;

    async fn run(self, txn: &mut Txn<'_>) -> Result<(), SpaceError> {
        receive_revocation(txn, &space(), self.issuer, &["same-jti".into()]).await
    }
}

/// Only the issuing authority can revoke, and a JTI is scoped to one Space.
/// This catches treating a claimed body identity as authority or a JTI as globally unique.
#[tokio::test]
async fn revocation_is_authority_authenticated_and_space_scoped() {
    let (_store, handle) = fixture(false).await;
    assert!(matches!(
        handle.run(Revoke { issuer: OUTSIDER }).await.unwrap(),
        Err(SpaceError::NotAuthorized)
    ));
    assert!(!is_revoked(handle.reader(), &space(), "same-jti").await.unwrap());
    handle.run(Revoke { issuer: WORLD }).await.unwrap().unwrap();
    assert!(is_revoked(handle.reader(), &space(), "same-jti").await.unwrap());
    let other = spaces::world_space(&Did::new(WORLD), spaces::META).unwrap();
    assert!(!is_revoked(handle.reader(), &other, "same-jti").await.unwrap());
}

struct ReceiveDeletion {
    issuer: &'static str,
}

impl Op for ReceiveDeletion {
    type Output = ();
    type Error = SpaceError;

    async fn run(self, txn: &mut Txn<'_>) -> Result<(), SpaceError> {
        receive_deletion(txn, &space(), self.issuer).await
    }
}

/// A deletion notification cannot erase the repo host's own records, even after recreation.
/// This catches the pre-alpha behavior that tombstoned a member-owned Space on notification.
#[tokio::test]
async fn receiving_deletion_preserves_member_owned_repo_data() {
    let (_store, handle) = fixture(false).await;
    let prefix = keys::space_records(&Did::new(WORLD), &space(), None);
    let before = handle.reader().scan(&prefix, None, usize::MAX).await.unwrap();
    assert_ne!(before, []);
    assert!(matches!(
        handle.run(ReceiveDeletion { issuer: OUTSIDER }).await.unwrap(),
        Err(SpaceError::NotAuthorized)
    ));
    handle.run(ReceiveDeletion { issuer: WORLD }).await.unwrap().unwrap();
    assert_eq!(handle.reader().scan(&prefix, None, usize::MAX).await.unwrap(), before);
    assert!(!info(handle.reader(), &space()).await.unwrap().deleted);
}

struct QueueRevocation {
    expires: i64,
}

impl Op for QueueRevocation {
    type Output = ();
    type Error = SpaceError;

    async fn run(self, txn: &mut Txn<'_>) -> Result<(), SpaceError> {
        queue_revocation(txn, &space(), "revoked-jti", self.expires).await
    }
}

/// An unresolved repo host remains owed a revocation until that credential expires.
/// This catches dropping an unresolved destination or retrying against the longer subscription lifetime.
#[tokio::test]
async fn revocations_keep_unresolved_hosts_until_the_credential_deadline() {
    let (_store, handle) = fixture(false).await;
    handle
        .run(Receive(payload(OWNER, &Tid::new().to_string(), 4)))
        .await
        .unwrap()
        .unwrap();
    let expires = i64::try_from(now_ms().unwrap() / 1000).unwrap() + 600;
    handle.run(QueueRevocation { expires }).await.unwrap().unwrap();
    let rows = pending(handle.reader()).await;
    assert_eq!(rows.len(), 1);
    let intent: Intent = decode(&rows[0].1).unwrap();
    assert_eq!(intent.audience, OWNER);
    assert!(intent.endpoint.is_none(), "endpoint resolution belongs outside the Op");
    assert!(matches!(intent.notification, Notification::Revoked { .. }));
    assert_eq!(intent.deadline_ms, u64::try_from(expires).unwrap() * 1000);
    let result = handle
        .run(Prepare {
            key: rows[0].0.to_vec(),
            now_ms: intent.deadline_ms,
        })
        .await
        .unwrap()
        .unwrap();
    assert!(result.is_none());
    assert_eq!(pending(handle.reader()).await, []);
    assert!(is_revoked(handle.reader(), &space(), "revoked-jti").await.unwrap());
}

/// A future revision must not pin the writer beyond every honest update.
/// This catches applying the monotonic upsert before checking the five-minute clock allowance.
#[tokio::test]
async fn future_revision_is_refused_before_it_can_pin_the_writer() {
    let (_store, handle) = fixture(true).await;
    let future = Tid::from_parts((now_ms().unwrap() + 86_400_000) * 1000, 0);
    let result = handle
        .run(Receive(payload(WORLD, &future.to_string(), 5)))
        .await
        .unwrap();
    assert!(matches!(result, Err(SpaceError::FutureRev)));
    assert_eq!(pending(handle.reader()).await, []);
    let honest = Tid::new().to_string();
    handle.run(Receive(payload(WORLD, &honest, 6))).await.unwrap().unwrap();
    let page = list_repos(handle.reader(), &space(), None, 100).await.unwrap();
    assert_eq!(page.repos[0].repo_rev, honest);
}
