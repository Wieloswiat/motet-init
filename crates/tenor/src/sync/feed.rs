//! Joins a committed history prefix to live notifications without a gap.

use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket};
use axum::response::Response;
use bytes::Bytes;
use futures::SinkExt as _;
use motet_xrpc::server::Input;
use tokio::sync::broadcast;

use crate::event::Sequenced;
use crate::shard::Handle;
use crate::store::{Reader, keys};

use super::{Error, frame, internal, optional};

const PAGE: usize = 256;

#[derive(Debug)]
pub(super) struct Feed {
    reader: Reader,
    live: broadcast::Receiver<Arc<Sequenced>>,
    next: u64,
    until: u64,
    page: std::vec::IntoIter<(Bytes, Bytes)>,
    catch_up: bool,
    closed: bool,
}

#[derive(Debug)]
pub(super) enum StartError {
    FutureCursor,
    Storage(Error),
}

async fn watermark(reader: &Reader) -> Result<u64, Error> {
    let Some(value) = reader.get(keys::EVENT_SEQ).await.map_err(internal)? else {
        return Ok(0);
    };
    let bytes: [u8; 8] = value.as_ref().try_into().map_err(internal)?;
    Ok(u64::from_be_bytes(bytes))
}

/// Captures live notifications before any asynchronous history read.
#[derive(Debug)]
pub(super) struct Subscription {
    reader: Reader,
    live: broadcast::Receiver<Arc<Sequenced>>,
}

impl Subscription {
    pub(super) fn new(handle: &Handle) -> Self {
        let live = handle.subscribe();
        Self {
            reader: handle.reader().clone(),
            live,
        }
    }

    pub(super) async fn start(self, cursor: Option<i64>) -> Result<Feed, StartError> {
        let until = watermark(&self.reader).await.map_err(StartError::Storage)?;
        let next = cursor.map_or(until, |cursor| u64::try_from(cursor).map_or(0, |seq| seq + 1));
        if next > until {
            return Err(StartError::FutureCursor);
        }
        Ok(Feed {
            reader: self.reader,
            live: self.live,
            next,
            until,
            page: Vec::new().into_iter(),
            catch_up: false,
            closed: false,
        })
    }
}

impl Feed {
    /// Cancellation-safe: after a lag or sequence gap, the catch-up flag is
    /// set before awaiting storage. A socket ping cannot discard that work.
    pub(super) async fn next(&mut self) -> Result<Option<Arc<Sequenced>>, Error> {
        loop {
            if self.catch_up {
                self.until = watermark(&self.reader).await?;
                self.catch_up = false;
            }
            if self.next < self.until {
                if let Some((_, bytes)) = self.page.next() {
                    let event: Sequenced = serde_json::from_slice(&bytes).map_err(internal)?;
                    if event.seq != self.next {
                        return Err(internal("stored event history has a sequence gap"));
                    }
                    self.next += 1;
                    return Ok(Some(Arc::new(event)));
                }
                let after = self.next.checked_sub(1).map(keys::event);
                let remaining = usize::try_from(self.until - self.next).unwrap_or(usize::MAX);
                let page = self
                    .reader
                    .scan(keys::EVENTS, after.as_deref(), PAGE.min(remaining))
                    .await
                    .map_err(internal)?;
                if page.is_empty() {
                    return Err(internal("committed event watermark has no stored event"));
                }
                self.page = page.into_iter();
                continue;
            }
            if self.closed {
                return Ok(None);
            }
            match self.live.recv().await {
                Ok(event) if event.seq < self.next => {}
                Ok(event) if event.seq == self.next => {
                    self.next += 1;
                    return Ok(Some(event));
                }
                Ok(_) | Err(broadcast::error::RecvError::Lagged(_)) => self.catch_up = true,
                Err(broadcast::error::RecvError::Closed) => {
                    self.closed = true;
                    self.catch_up = true;
                }
            }
        }
    }
}

pub(super) async fn subscribe(handle: &Handle, input: Input) -> Result<Response, Error> {
    let cursor = optional(&input.params, "cursor")?
        .map(|value| {
            value
                .parse::<i64>()
                .map_err(|_| Error::InvalidRequest("cursor must be an integer".into()))
        })
        .transpose()?;
    let upgrade = input
        .websocket
        .ok_or_else(|| Error::InvalidRequest("WebSocket upgrade required".into()))?;
    let feed = Subscription::new(handle).start(cursor).await;
    Ok(upgrade
        .protocols(["xrpc.v0.cbor"])
        .on_upgrade(move |socket| send(socket, feed)))
}

async fn send(mut socket: WebSocket, feed: Result<Feed, StartError>) {
    let mut feed = match feed {
        Ok(feed) => feed,
        Err(StartError::FutureCursor) => {
            terminal(
                &mut socket,
                "FutureCursor",
                "cursor is ahead of the committed event history",
            )
            .await;
            return;
        }
        Err(StartError::Storage(error)) => {
            terminal(&mut socket, "InternalServerError", &error.to_string()).await;
            return;
        }
    };
    loop {
        tokio::select! {
            incoming = socket.recv() => match incoming {
                Some(Ok(Message::Ping(bytes))) => {
                    if socket.send(Message::Pong(bytes)).await.is_err() {
                        break;
                    }
                }
                None | Some(Err(_) | Ok(Message::Close(_))) => break,
                Some(Ok(_)) => {}
            },
            event = feed.next() => {
                let bytes = match event {
                    Ok(Some(event)) => frame::event(&event),
                    Ok(None) => break,
                    Err(error) => Err(error),
                };
                match bytes {
                    Ok(bytes) => {
                        if socket.send(Message::Binary(bytes.into())).await.is_err() {
                            break;
                        }
                    }
                    Err(error) => {
                        terminal(&mut socket, "InternalServerError", &error.to_string()).await;
                        return;
                    }
                }
            }
        }
    }
    let _ = socket.close().await;
}

async fn terminal(socket: &mut WebSocket, name: &str, message: &str) {
    if let Ok(bytes) = frame::error(name, message) {
        let _ = socket.send(Message::Binary(bytes.into())).await;
    }
    let _ = socket.send(Message::Close(None)).await;
}
