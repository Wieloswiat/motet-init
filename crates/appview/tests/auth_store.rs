//! OAuth grants must remain confidential, single-use and revocable across instances.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use base64::Engine as _;
use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use chrono::{Duration, Utc};
use motet_appview::MIGRATOR;
use motet_appview::auth::store::{SessionRef, SessionStore, StoreError};
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};
use sqlx::PgPool;
use sqlx::postgres::PgListener;
use tokio::sync::{Barrier, Notify, mpsc, oneshot};

const KEY: [u8; 32] = [91; 32];

async fn setup() -> (PgPool, SessionStore) {
    let pool = motet_testkit::services::database().await;
    MIGRATOR.run(&pool).await.expect("migrations");
    let store = SessionStore::new(pool.clone(), KEY);
    (pool, store)
}

fn grant(refresh: &str) -> Value {
    json!({
        "did": "did:plc:alice",
        "access_token": "access-token-secret",
        "refresh_token": refresh,
        "dpop_private_key": "private-dpop-key-secret"
    })
}

fn row_id(secret: &str) -> Vec<u8> {
    Sha256::digest(secret.as_bytes()).to_vec()
}

// A pair of instances must not send the same rotating refresh token twice.
// Without a cross-instance advisory lock and generation reread, both would send R0.
#[tokio::test]
#[ignore = "integration: PostgreSQL"]
async fn concurrent_refresh_uses_each_rotating_token_once() {
    let (pool, first_store) = setup().await;
    let second_store = SessionStore::new(pool, KEY);
    let cookie = first_store
        .create_session(&grant("R0"), Utc::now() + Duration::hours(1))
        .await
        .expect("create session");
    let reference = SessionRef::from_cookie(&cookie).expect("cookie reference");
    let uses = Arc::new(AtomicUsize::new(0));
    let release = Arc::new(Notify::new());
    let (entered_tx, entered_rx) = oneshot::channel();
    let first = {
        let cookie = cookie.clone();
        let uses = Arc::clone(&uses);
        let release = Arc::clone(&release);
        tokio::spawn(async move {
            first_store
                .refresh(&cookie, 0, move |mut old: Value| async move {
                    assert_eq!(old["refresh_token"], "R0");
                    uses.fetch_add(1, Ordering::SeqCst);
                    entered_tx.send(()).expect("refresh observer");
                    release.notified().await;
                    old["refresh_token"] = json!("R1");
                    Ok::<_, ()>(old)
                })
                .await
        })
    };
    entered_rx.await.expect("first refresh entered PDS");
    let second = {
        let uses = Arc::clone(&uses);
        let store = second_store.clone();
        tokio::spawn(async move {
            store
                .refresh_by_ref(&reference, 0, move |old: Value| async move {
                    uses.fetch_add(1, Ordering::SeqCst);
                    Ok::<_, ()>(old)
                })
                .await
        })
    };
    let reader = {
        let store = second_store.clone();
        tokio::spawn(async move { store.get_session_by_ref::<Value>(&reference).await })
    };
    release.notify_one();
    let first = first
        .await
        .expect("first task")
        .expect("first refresh")
        .expect("live session");
    let second = second
        .await
        .expect("second task")
        .expect("second refresh")
        .expect("live session");
    let reader = reader.await.expect("reader task").expect("read").expect("live session");
    assert_eq!(uses.load(Ordering::SeqCst), 1);
    for session in [first, second, reader] {
        assert_eq!(session.generation, 1);
        assert_eq!(session.grant["refresh_token"], "R1");
    }
}

// A stalled refresh must not monopolize the lock pool. If waiters held
// connections while blocked on A, unrelated B could not refresh.
#[tokio::test]
#[ignore = "integration: PostgreSQL"]
async fn stalled_session_waiters_do_not_block_another_session() {
    const WAITERS: usize = 12;
    let (_, store) = setup().await;
    let expires = Utc::now() + Duration::hours(1);
    let cookie_a = store.create_session(&grant("A0"), expires).await.expect("session A");
    let cookie_b = store.create_session(&grant("B0"), expires).await.expect("session B");
    let reference_a = SessionRef::from_cookie(&cookie_a).expect("reference A");
    let reference_b = SessionRef::from_cookie(&cookie_b).expect("reference B");
    let release = Arc::new(Notify::new());
    let (entered_tx, entered_rx) = oneshot::channel();
    let active = {
        let store = store.clone();
        let release = Arc::clone(&release);
        tokio::spawn(async move {
            store
                .refresh_by_ref(&reference_a, 0, move |mut old: Value| async move {
                    entered_tx.send(()).expect("refresh observer");
                    release.notified().await;
                    old["refresh_token"] = json!("A1");
                    Ok::<_, ()>(old)
                })
                .await
        })
    };
    entered_rx.await.expect("session A refresh started");
    let started = Arc::new(Barrier::new(WAITERS + 1));
    let mut waiters = Vec::with_capacity(WAITERS);
    for _ in 0..WAITERS {
        let store = store.clone();
        let started = Arc::clone(&started);
        waiters.push(tokio::spawn(async move {
            let request = store.refresh_by_ref(&reference_a, 0, |old: Value| async move { Ok::<_, ()>(old) });
            tokio::pin!(request);
            tokio::select! {
                biased;
                result = &mut request => {
                    started.wait().await;
                    result
                }
                () = std::future::ready(()) => {
                    started.wait().await;
                    request.await
                }
            }
        }));
    }
    // Every waiter has polled its lock request before B starts.
    started.wait().await;
    let refreshed_b = tokio::time::timeout(
        std::time::Duration::from_secs(3),
        store.refresh_by_ref(&reference_b, 0, |mut old: Value| async move {
            old["refresh_token"] = json!("B1");
            Ok::<_, ()>(old)
        }),
    )
    .await
    .expect("session B was not blocked by A")
    .expect("refresh B")
    .expect("live session B");
    assert_eq!(refreshed_b.grant["refresh_token"], "B1");

    // Marker cleanup must not remove either live session's coordinator.
    let grant_c = grant("C0");
    let (created, expired) = tokio::join!(store.create_session(&grant_c, expires), store.expire());
    let cookie_c = created.expect("concurrent session C");
    assert_eq!(expired.expect("concurrent cleanup"), 0);
    let reference_c = SessionRef::from_cookie(&cookie_c).expect("reference C");
    assert!(
        store
            .refresh_by_ref(&reference_c, 0, |old: Value| async move { Ok::<_, ()>(old) })
            .await
            .expect("session C refresh")
            .is_some()
    );

    release.notify_one();
    assert!(active.await.expect("active task").expect("refresh A").is_some());
    for waiter in waiters {
        let session = waiter
            .await
            .expect("waiter task")
            .expect("waiter refresh")
            .expect("session A survived cleanup");
        assert_eq!(session.grant["refresh_token"], "A1");
    }
}

// Expiring a session during its remote refresh must not make cleanup wait on
// the pinned coordinator, and a completed refresh must not revive the session.
#[tokio::test]
#[ignore = "integration: PostgreSQL"]
async fn cleanup_skips_an_expired_session_with_a_stalled_refresh() {
    let (pool, store) = setup().await;
    let expires = Utc::now() + Duration::hours(1);
    let cookie_a = store.create_session(&grant("A0"), expires).await.expect("session A");
    let cookie_b = store.create_session(&grant("B0"), expires).await.expect("session B");
    let reference_a = SessionRef::from_cookie(&cookie_a).expect("reference A");
    let reference_b = SessionRef::from_cookie(&cookie_b).expect("reference B");
    let release = Arc::new(Notify::new());
    let (entered_tx, entered_rx) = oneshot::channel();
    let active = {
        let store = store.clone();
        let release = Arc::clone(&release);
        tokio::spawn(async move {
            store
                .refresh_by_ref(&reference_a, 0, move |old: Value| async move {
                    entered_tx.send(()).expect("refresh observer");
                    release.notified().await;
                    Ok::<_, ()>(old)
                })
                .await
        })
    };
    entered_rx.await.expect("refresh started");
    sqlx::query("UPDATE service.oauth_sessions SET expires_at = clock_timestamp() - interval '1 second' WHERE id = $1")
        .bind(reference_a.as_bytes())
        .execute(&pool)
        .await
        .expect("expire active session");
    let removed = tokio::time::timeout(std::time::Duration::from_secs(3), store.expire())
        .await
        .expect("cleanup did not wait for the remote PDS")
        .expect("cleanup");
    assert_eq!(removed, 1);
    let (marker_exists,): (bool,) =
        sqlx::query_as("SELECT EXISTS(SELECT 1 FROM service.oauth_session_locks WHERE id = $1)")
            .bind(reference_a.as_bytes())
            .fetch_one(&pool)
            .await
            .expect("pinned marker");
    assert!(marker_exists);
    assert!(
        store
            .refresh_by_ref(&reference_b, 0, |old: Value| async move { Ok::<_, ()>(old) })
            .await
            .expect("independent refresh")
            .is_some()
    );
    release.notify_one();
    assert!(active.await.expect("active task").expect("expired refresh").is_none());
    assert!(
        store
            .get_session_by_ref::<Value>(&reference_a)
            .await
            .expect("expired session")
            .is_none()
    );
    store.expire().await.expect("remove released marker");
    let (marker_exists,): (bool,) =
        sqlx::query_as("SELECT EXISTS(SELECT 1 FROM service.oauth_session_locks WHERE id = $1)")
            .bind(reference_a.as_bytes())
            .fetch_one(&pool)
            .await
            .expect("released marker");
    assert!(!marker_exists);
}

// A stalled refresh must not admit unbounded local waiters. Cancelling a
// waiter must release its slot instead of permanently exhausting that bound.
#[tokio::test]
#[ignore = "integration: PostgreSQL"]
async fn stalled_session_bounds_waiters_and_cancellation_releases_a_slot() {
    const CONTENDERS: usize = 40;
    let (_, store) = setup().await;
    let cookie = store
        .create_session(&grant("R0"), Utc::now() + Duration::hours(1))
        .await
        .expect("session");
    let reference = SessionRef::from_cookie(&cookie).expect("reference");
    let release = Arc::new(Notify::new());
    let (entered_tx, entered_rx) = oneshot::channel();
    let active = {
        let store = store.clone();
        let release = Arc::clone(&release);
        tokio::spawn(async move {
            store
                .refresh_by_ref(&reference, 0, move |mut old: Value| async move {
                    entered_tx.send(()).expect("refresh observer");
                    release.notified().await;
                    old["refresh_token"] = json!("R1");
                    Ok::<_, ()>(old)
                })
                .await
        })
    };
    entered_rx.await.expect("refresh started");
    let (started_tx, mut started_rx) = mpsc::unbounded_channel();
    let mut tasks = Vec::with_capacity(CONTENDERS);
    for index in 0..CONTENDERS {
        let store = store.clone();
        let started = started_tx.clone();
        tasks.push(Some(tokio::spawn(async move {
            let request = store.refresh_by_ref(&reference, 0, |old: Value| async move { Ok::<_, ()>(old) });
            tokio::pin!(request);
            tokio::select! {
                biased;
                result = &mut request => {
                    started.send((index, false)).expect("waiter observer");
                    result
                }
                () = std::future::ready(()) => {
                    started.send((index, true)).expect("waiter observer");
                    request.await
                }
            }
        })));
    }
    drop(started_tx);
    let mut cancellable = None;
    let mut refused = 0;
    for _ in 0..CONTENDERS {
        let (index, pending) = started_rx.recv().await.expect("each contender was polled");
        if pending {
            cancellable.get_or_insert(index);
        } else {
            refused += 1;
        }
    }
    assert!(refused > 0, "a stalled session must bound its waiters");
    let canceled = tasks
        .get_mut(cancellable.expect("at least one waiter"))
        .and_then(Option::take)
        .expect("cancellable waiter");
    canceled.abort();
    assert!(canceled.await.expect_err("cancelled waiter").is_cancelled());

    let replacement = store.refresh_by_ref(&reference, 0, |old: Value| async move { Ok::<_, ()>(old) });
    tokio::pin!(replacement);
    tokio::select! {
        biased;
        result = &mut replacement => panic!("cancelled waiter did not free a slot: {result:?}"),
        () = std::future::ready(()) => {}
    }
    release.notify_one();
    assert!(active.await.expect("active task").expect("refresh").is_some());
    let recovered = replacement.await.expect("replacement refresh").expect("live session");
    assert_eq!(recovered.grant["refresh_token"], "R1");
    for task in tasks.into_iter().flatten() {
        match task.await.expect("waiter task") {
            Ok(Some(session)) => assert_eq!(session.grant["refresh_token"], "R1"),
            Err(StoreError::Busy) => {}
            result => panic!("unexpected waiter outcome: {result:?}"),
        }
    }
}

// A process cancellation after sending R0 is ambiguous. Releasing the
// advisory lock must not roll back the durable marker and make R0 usable again.
#[tokio::test]
#[ignore = "integration: PostgreSQL"]
async fn abandoned_refresh_never_returns_the_possibly_consumed_grant() {
    let (_, store) = setup().await;
    let cookie = store
        .create_session(&grant("R0"), Utc::now() + Duration::hours(1))
        .await
        .expect("create session");
    let (entered_tx, entered_rx) = oneshot::channel();
    let task = {
        let store = store.clone();
        let cookie = cookie.clone();
        tokio::spawn(async move {
            store
                .refresh(&cookie, 0, move |_old: Value| async move {
                    entered_tx.send(()).expect("refresh observer");
                    std::future::pending::<Result<Value, ()>>().await
                })
                .await
        })
    };
    entered_rx.await.expect("PDS request started");
    task.abort();
    assert!(task.await.expect_err("cancelled refresh").is_cancelled());
    assert!(store.get_session::<Value>(&cookie).await.expect("read").is_none());
    let calls = Arc::new(AtomicUsize::new(0));
    let result = store
        .refresh(&cookie, 0, {
            let calls = Arc::clone(&calls);
            move |old: Value| async move {
                calls.fetch_add(1, Ordering::SeqCst);
                Ok::<_, ()>(old)
            }
        })
        .await
        .expect("refresh after cancellation");
    assert!(result.is_none());
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

// A failed remote call may have consumed the refresh token before the response
// was lost. A second request must not be allowed to reuse it.
#[tokio::test]
#[ignore = "integration: PostgreSQL"]
async fn ambiguous_remote_failure_invalidates_the_session() {
    let (_, store) = setup().await;
    let cookie = store
        .create_session(&grant("R0"), Utc::now() + Duration::hours(1))
        .await
        .expect("create session");
    let result = store
        .refresh(&cookie, 0, |_old: Value| async { Err::<Value, _>("connection lost") })
        .await;
    assert!(matches!(result, Err(StoreError::Refresh)));
    assert!(store.get_session::<Value>(&cookie).await.expect("read").is_none());
}

// If PostgreSQL rejects a replacement after the PDS consumed R0, the old
// grant must not become usable when the refresh transaction ends.
#[tokio::test]
#[ignore = "integration: PostgreSQL"]
async fn fault_failed_replacement_write_never_reuses_the_old_refresh_token() {
    let (pool, store) = setup().await;
    let cookie = store
        .create_session(&grant("R0"), Utc::now() + Duration::hours(1))
        .await
        .expect("session");
    sqlx::raw_sql(
        "CREATE FUNCTION service.refuse_replacement() RETURNS trigger LANGUAGE plpgsql AS $$ \
         BEGIN IF OLD.refreshing AND NOT NEW.refreshing THEN RAISE EXCEPTION 'replacement failed'; \
         END IF; RETURN NEW; END $$; \
         CREATE TRIGGER refuse_replacement BEFORE UPDATE ON service.oauth_sessions \
         FOR EACH ROW EXECUTE FUNCTION service.refuse_replacement();",
    )
    .execute(&pool)
    .await
    .expect("replacement failure trigger");
    let result = store
        .refresh(&cookie, 0, |mut old: Value| async move {
            old["refresh_token"] = json!("R1");
            Ok::<_, ()>(old)
        })
        .await;
    assert!(matches!(result, Err(StoreError::Database(_))));
    assert!(store.get_session::<Value>(&cookie).await.expect("read").is_none());
    let retried = store
        .refresh(&cookie, 0, |old: Value| async move { Ok::<_, ()>(old) })
        .await
        .expect("subsequent attempt");
    assert!(retried.is_none());
}

// A database reader must not obtain the cookie or private OAuth material;
// swapping a ciphertext between rows or purposes must not open it.
#[tokio::test]
#[ignore = "integration: PostgreSQL"]
async fn ciphertext_requires_its_key_row_and_purpose_and_does_not_reveal_the_cookie() {
    let (pool, store) = setup().await;
    let expires = Utc::now() + Duration::hours(1);
    let first = store
        .create_session(&grant("R0"), expires)
        .await
        .expect("first session");
    let second = store
        .create_session(&grant("R1"), expires)
        .await
        .expect("second session");
    let wrong_key = SessionStore::new(pool.clone(), [92; 32]);
    assert!(matches!(
        wrong_key.get_session::<Value>(&first).await,
        Err(StoreError::Unseal)
    ));
    store
        .store_pending("independent-state", "browser-secret", &grant("pending"), expires)
        .await
        .expect("pending state");
    assert!(matches!(
        wrong_key
            .consume_pending::<Value>("independent-state", "browser-secret")
            .await,
        Err(StoreError::Unseal)
    ));
    assert!(
        store
            .consume_pending::<Value>("independent-state", "browser-secret")
            .await
            .expect("failed decryption consumed the callback")
            .is_none()
    );

    let (stored_id, sealed): (Vec<u8>, Vec<u8>) =
        sqlx::query_as("SELECT id, sealed FROM service.oauth_sessions WHERE id = $1")
            .bind(row_id(&first))
            .fetch_one(&pool)
            .await
            .expect("stored ciphertext");
    assert!(
        !sealed
            .windows(b"private-dpop-key-secret".len())
            .any(|part| part == b"private-dpop-key-secret")
    );
    assert!(
        !sealed
            .windows(b"access-token-secret".len())
            .any(|part| part == b"access-token-secret")
    );
    let exposed_id_as_cookie = URL_SAFE_NO_PAD.encode(&stored_id);
    assert!(
        store
            .get_session::<Value>(&exposed_id_as_cookie)
            .await
            .expect("database identifier is not the cookie")
            .is_none()
    );

    sqlx::query("UPDATE service.oauth_sessions SET sealed = $1 WHERE id = $2")
        .bind(&sealed)
        .bind(row_id(&second))
        .execute(&pool)
        .await
        .expect("swap row ciphertext");
    assert!(matches!(
        store.get_session::<Value>(&second).await,
        Err(StoreError::Unseal)
    ));

    // Use exactly the same row ID in both tables to isolate purpose binding.
    store
        .store_pending(&first, "browser-binding", &grant("pending"), expires)
        .await
        .expect("pending state");
    let (pending_sealed,): (Vec<u8>,) = sqlx::query_as("SELECT sealed FROM service.oauth_pending_logins WHERE id = $1")
        .bind(row_id(&first))
        .fetch_one(&pool)
        .await
        .expect("pending ciphertext");
    sqlx::query("UPDATE service.oauth_sessions SET sealed = $1 WHERE id = $2")
        .bind(pending_sealed)
        .bind(row_id(&first))
        .execute(&pool)
        .await
        .expect("swap purpose ciphertext");
    assert!(matches!(
        store.get_session::<Value>(&first).await,
        Err(StoreError::Unseal)
    ));
}

// Expiry and logout must take effect without a cleanup worker; the login
// state belongs to one browser and cannot be used by two callbacks.
#[tokio::test]
#[ignore = "integration: PostgreSQL"]
async fn expired_and_revoked_sessions_and_replayed_logins_are_unavailable() {
    let (_, store) = setup().await;
    let future = Utc::now() + Duration::hours(1);
    let past = Utc::now() - Duration::hours(1);
    let expired = store
        .create_session(&grant("old"), past)
        .await
        .expect("expired session");
    let revoked = store
        .create_session(&grant("live"), future)
        .await
        .expect("live session");
    assert!(
        store
            .get_session::<Value>(&expired)
            .await
            .expect("expired read")
            .is_none()
    );
    store.revoke(&revoked).await.expect("revoke");
    assert!(
        store
            .get_session::<Value>(&revoked)
            .await
            .expect("revoked read")
            .is_none()
    );
    let expired_refresh = store
        .refresh(&expired, 0, |old: Value| async move { Ok::<_, ()>(old) })
        .await
        .expect("expired refresh");
    assert!(expired_refresh.is_none());
    let revoked_refresh = store
        .refresh(&revoked, 0, |old: Value| async move { Ok::<_, ()>(old) })
        .await
        .expect("revoked refresh");
    assert!(revoked_refresh.is_none());

    store
        .store_pending("oauth-state", "right-browser", &grant("pending"), future)
        .await
        .expect("store pending login");
    assert!(
        store
            .consume_pending::<Value>("oauth-state", "wrong-browser")
            .await
            .expect("wrong browser")
            .is_none()
    );
    let (one, two) = tokio::join!(
        store.consume_pending::<Value>("oauth-state", "right-browser"),
        store.consume_pending::<Value>("oauth-state", "right-browser"),
    );
    let one = one.expect("first callback");
    let two = two.expect("second callback");
    assert_ne!(one.is_some(), two.is_some());
    assert_eq!(one.or(two).expect("one callback succeeds")["refresh_token"], "pending");
    assert!(
        store
            .consume_pending::<Value>("oauth-state", "right-browser")
            .await
            .expect("replay")
            .is_none()
    );
    store
        .store_pending("expired-state", "right-browser", &grant("pending"), past)
        .await
        .expect("store expired pending login");
    assert!(
        store
            .consume_pending::<Value>("expired-state", "right-browser")
            .await
            .expect("expired callback")
            .is_none()
    );
    assert_eq!(store.expire().await.expect("expiry cleanup"), 2);
}

// Logout must deny reads as soon as it starts, while a concurrent rotation
// finishes so remote revocation receives R1 rather than the spent R0.
#[tokio::test]
#[ignore = "integration: PostgreSQL"]
async fn logout_during_refresh_revokes_locally_then_takes_the_rotated_grant() {
    let (pool, store) = setup().await;
    sqlx::raw_sql(
        "CREATE FUNCTION service.notify_logout() RETURNS trigger LANGUAGE plpgsql AS $$ \
         BEGIN IF NEW.revoked AND NOT OLD.revoked THEN PERFORM pg_notify('oauth_logout', 'revoked'); \
         END IF; RETURN NEW; END $$; \
         CREATE TRIGGER notify_logout AFTER UPDATE ON service.oauth_sessions \
         FOR EACH ROW EXECUTE FUNCTION service.notify_logout();",
    )
    .execute(&pool)
    .await
    .expect("logout notification trigger");
    let mut listener = PgListener::connect_with(&pool).await.expect("notification listener");
    listener.listen("oauth_logout").await.expect("listen");
    let cookie = store
        .create_session(&grant("R0"), Utc::now() + Duration::hours(1))
        .await
        .expect("session");
    let reference = SessionRef::from_cookie(&cookie).expect("cookie reference");
    let release = Arc::new(Notify::new());
    let (entered_tx, entered_rx) = oneshot::channel();
    let refresh = {
        let store = store.clone();
        let release = Arc::clone(&release);
        tokio::spawn(async move {
            store
                .refresh_by_ref(&reference, 0, move |mut old: Value| async move {
                    entered_tx.send(()).expect("refresh observer");
                    release.notified().await;
                    old["refresh_token"] = json!("R1");
                    Ok::<_, ()>(old)
                })
                .await
        })
    };
    entered_rx.await.expect("refresh started");
    let queued_calls = Arc::new(AtomicUsize::new(0));
    let (queued_tx, queued_rx) = oneshot::channel();
    let queued_refresh = {
        let store = store.clone();
        let calls = Arc::clone(&queued_calls);
        tokio::spawn(async move {
            queued_tx.send(()).expect("queue observer");
            store
                .refresh_by_ref(&reference, 0, move |old: Value| async move {
                    calls.fetch_add(1, Ordering::SeqCst);
                    Ok::<_, ()>(old)
                })
                .await
        })
    };
    queued_rx.await.expect("queued reference started");
    let logout = {
        let cookie = cookie.clone();
        let store = store.clone();
        tokio::spawn(async move { store.take_session::<Value>(&cookie).await })
    };
    listener.recv().await.expect("revocation committed");
    assert!(
        store
            .get_session_by_ref::<Value>(&reference)
            .await
            .expect("revoked read")
            .is_none()
    );
    release.notify_one();
    assert!(refresh.await.expect("refresh task").expect("refresh").is_none());
    assert!(
        queued_refresh
            .await
            .expect("queued refresh task")
            .expect("queued refresh")
            .is_none()
    );
    assert_eq!(queued_calls.load(Ordering::SeqCst), 0);
    let taken = logout
        .await
        .expect("logout task")
        .expect("logout")
        .expect("latest grant");
    assert_eq!(taken.grant["refresh_token"], "R1");
    assert!(
        store
            .get_session_by_ref::<Value>(&reference)
            .await
            .expect("deleted read")
            .is_none()
    );
    assert!(
        store
            .take_session::<Value>(&cookie)
            .await
            .expect("repeat logout")
            .is_none()
    );
}

// Logout may time out after committing revocation. Retrying must be able to
// take the grant once for remote revocation without making it usable locally.
#[tokio::test]
#[ignore = "integration: PostgreSQL"]
async fn fault_interrupted_logout_can_finish_without_reactivating_the_session() {
    let (pool, store) = setup().await;
    let cookie = store
        .create_session(&grant("R0"), Utc::now() + Duration::hours(1))
        .await
        .expect("session");
    let reference = SessionRef::from_cookie(&cookie).expect("reference");
    sqlx::query("UPDATE service.oauth_sessions SET revoked = true WHERE id = $1")
        .bind(reference.as_bytes())
        .execute(&pool)
        .await
        .expect("interrupted logout committed revocation");
    assert!(
        store
            .get_session_by_ref::<Value>(&reference)
            .await
            .expect("revoked read")
            .is_none()
    );
    let taken = store
        .take_session::<Value>(&cookie)
        .await
        .expect("retry logout")
        .expect("grant for remote revocation");
    assert_eq!(taken.grant["refresh_token"], "R0");
    assert!(
        store
            .take_session::<Value>(&cookie)
            .await
            .expect("repeat logout")
            .is_none()
    );
}

// Queued work persists only a hash of the browser cookie. Reopening the
// store must preserve its grant without turning that hash into a bearer token,
// and expiry and logout must still block work through a saved reference.
#[tokio::test]
#[ignore = "integration: PostgreSQL"]
async fn persisted_session_reference_survives_reopen_without_bypassing_revocation() {
    let (pool, store) = setup().await;
    let future = Utc::now() + Duration::hours(1);
    let past = Utc::now() - Duration::hours(1);
    let cookie = store.create_session(&grant("R0"), future).await.expect("live session");
    let expired = store
        .create_session(&grant("old"), past)
        .await
        .expect("expired session");
    let revoked = store
        .create_session(&grant("revoked"), future)
        .await
        .expect("revoked session");
    let reference = SessionRef::from_cookie(&cookie).expect("live reference");
    let expired_ref = SessionRef::from_cookie(&expired).expect("expired reference");
    let revoked_ref = SessionRef::from_cookie(&revoked).expect("revoked reference");
    store.revoke(&revoked).await.expect("revoke");

    let (persisted,): (Vec<u8>,) = sqlx::query_as("SELECT id FROM service.oauth_sessions WHERE id = $1")
        .bind(reference.as_bytes())
        .fetch_one(&pool)
        .await
        .expect("stored reference");
    assert!(SessionRef::from_db(&[0; 31]).is_none());
    assert!(SessionRef::from_db(&[0; 33]).is_none());
    assert!(SessionRef::from_cookie(&"!".repeat(43)).is_none());
    let restored = SessionRef::from_db(&persisted).expect("restore reference");
    assert_eq!(restored, reference);
    let reopened_pool = PgPool::connect_with((*pool.connect_options()).clone())
        .await
        .expect("reopen database");
    drop(store);
    drop(pool);
    let reopened = SessionStore::new(reopened_pool, KEY);
    let session = reopened
        .get_session_by_ref::<Value>(&restored)
        .await
        .expect("reference lookup")
        .expect("live grant");
    assert_eq!(session.grant["refresh_token"], "R0");
    assert!(
        reopened
            .get_session_by_ref::<Value>(&expired_ref)
            .await
            .expect("expired reference")
            .is_none()
    );
    assert!(
        reopened
            .get_session_by_ref::<Value>(&revoked_ref)
            .await
            .expect("revoked reference")
            .is_none()
    );
    let encoded_hash = URL_SAFE_NO_PAD.encode(restored.as_bytes());
    assert!(
        reopened
            .get_session::<Value>(&encoded_hash)
            .await
            .expect("database hash is not the browser cookie")
            .is_none()
    );
}

// An operator may provide either documented key-file encoding. Both must
// recover the same session; a malformed file must not silently choose a key.
#[tokio::test]
#[ignore = "integration: PostgreSQL"]
async fn key_file_encodings_open_the_same_grant_and_invalid_keys_fail_closed() {
    let (pool, store) = setup().await;
    let cookie = store
        .create_session(&grant("R0"), Utc::now() + Duration::hours(1))
        .await
        .expect("session");
    let path = std::env::temp_dir().join(motet_testkit::services::unique_name("motet_key"));
    std::fs::write(&path, KEY).expect("write raw key");
    let raw = SessionStore::from_file(pool.clone(), &path).expect("raw key");
    assert!(raw.get_session::<Value>(&cookie).await.expect("raw read").is_some());
    std::fs::write(&path, format!(" {}\n", STANDARD.encode(KEY))).expect("write base64 key");
    let base64 = SessionStore::from_file(pool.clone(), &path).expect("base64 key");
    assert!(
        base64
            .get_session::<Value>(&cookie)
            .await
            .expect("base64 read")
            .is_some()
    );
    std::fs::write(&path, b"bad key").expect("write invalid key");
    assert!(matches!(
        SessionStore::from_file(pool, &path),
        Err(StoreError::InvalidKey)
    ));
    std::fs::remove_file(path).expect("remove temporary key");
}
