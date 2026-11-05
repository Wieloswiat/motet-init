//! Tap input: bounded batches, acknowledged only after archive and projection commit.
//!
//! The client owns connection retries and TLS selection from the URL scheme.
//! Fatal client errors and rejected events stop ingestion without acknowledging them.
//! Shutdown finishes the current batch and flushes queued acknowledgements.

use std::sync::Arc;
use std::time::Duration;

use atproto_tap::{RecordAction, TapConfig, TapError, TapEvent, TapStream};
use futures::StreamExt as _;
use tokio::sync::Mutex;

use crate::event::{Action, Event, RecordEvent};
use crate::indexer::{IndexError, Indexer};

const BATCH_SIZE: usize = 100;
const BATCH_WAIT: Duration = Duration::from_millis(25);

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Tap(#[from] TapError),
    #[error(transparent)]
    Index(#[from] IndexError),
}

/// Runs until shutdown or an unrecoverable input/indexing error.
///
/// Cancellation by dropping this future can leave a durable batch unacknowledged.
/// Tap redelivers it; record revisions make its projection application idempotent.
/// Use `shutdown` to finish the current batch and close the connection instead.
///
/// # Errors
///
/// Returns fatal Tap errors, malformed input, or archive/projection failures.
pub async fn run(
    indexer: Arc<Mutex<Indexer>>,
    config: TapConfig,
    shutdown: impl Future<Output = ()>,
) -> Result<(), Error> {
    let mut stream = atproto_tap::connect(config);
    let result = ingest(&indexer, &mut stream, shutdown).await;
    stream.close().await;
    result
}

async fn ingest(
    indexer: &Mutex<Indexer>,
    stream: &mut TapStream,
    shutdown: impl Future<Output = ()>,
) -> Result<(), Error> {
    tokio::pin!(shutdown);
    // An identity delivery carries both a handle and an account status.
    let mut events = Vec::with_capacity(BATCH_SIZE * 2);
    let mut ids = Vec::with_capacity(BATCH_SIZE);
    loop {
        let deadline = tokio::time::sleep(BATCH_WAIT);
        tokio::pin!(deadline);
        let stopping = loop {
            tokio::select! {
                biased;
                () = &mut shutdown => break true,
                () = &mut deadline, if !ids.is_empty() => break false,
                item = stream.next() => match item {
                    Some(Ok(event)) => {
                        if ids.is_empty() {
                            deadline.as_mut().reset(tokio::time::Instant::now() + BATCH_WAIT);
                        }
                        ids.push(event.id());
                        normalize(Arc::unwrap_or_clone(event), &mut events);
                        if ids.len() == BATCH_SIZE {
                            break false;
                        }
                    }
                    Some(Err(error)) if error.is_connection_error() => {
                        tracing::warn!(%error, "Tap connection interrupted; client will reconnect");
                    }
                    Some(Err(error)) => return Err(error.into()),
                    None => return Err(TapError::StreamClosed.into()),
                }
            }
        };
        if !ids.is_empty() {
            // Never cancel ingest on shutdown: append can already be durable.
            indexer.lock().await.ingest(&events).await?;
            for &id in &ids {
                stream.ack(id)?;
            }
            ids.clear();
            events.clear();
        }
        if stopping {
            return Ok(());
        }
    }
}

fn normalize(event: TapEvent, events: &mut Vec<Event>) {
    match event {
        TapEvent::Record { record, .. } => events.push(Event::Record(RecordEvent {
            did: record.did.into_string(),
            rev: record.rev.into_string(),
            collection: record.collection.into_string(),
            rkey: record.rkey.into_string(),
            action: match record.action {
                RecordAction::Create => Action::Create,
                RecordAction::Update => Action::Update,
                RecordAction::Delete => Action::Delete,
            },
            cid: record.cid.map(Into::into),
            record: record.record,
        })),
        TapEvent::Identity { identity, .. } => {
            let did = identity.did.into_string();
            events.push(Event::Identity {
                did: did.clone(),
                handle: Some(identity.handle.into_string()),
            });
            events.push(Event::Account {
                did,
                active: identity.is_active,
                status: Some(identity.status.to_string()),
            });
        }
    }
}
