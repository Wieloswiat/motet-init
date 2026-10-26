//! `xrpc.v0.cbor`: one binary message containing header CBOR then body CBOR.
//!
//! Ported from atproto-pds/src/sequencer/frame.rs (MIT), atproto-crates
//! revision 09a868cdde938c789326f1db16ffc30bbf38c15e. See UPSTREAM.md.
//! Local changes: only v0 CBOR, borrowed headers, direct shard-event encoding
//! into one buffer, and propagated encoding failures instead of empty frames.
//! No `OutdatedCursor` notices: this shard does not prune its event history.

use serde::Serialize;

use crate::event::{Event, Sequenced};

use super::{Error, internal, payload};

#[derive(Serialize)]
struct Header {
    op: i8,
    #[serde(skip_serializing_if = "Option::is_none")]
    t: Option<&'static str>,
}

pub(super) fn event(sequenced: &Sequenced) -> Result<Vec<u8>, Error> {
    let tag = match &sequenced.event {
        Event::Commit(_) => "#commit",
        Event::Sync(_) => "#sync",
        Event::Identity { .. } => "#identity",
        Event::Account { .. } => "#account",
    };
    let mut out = atproto_dasl::to_vec(&Header { op: 1, t: Some(tag) }).map_err(internal)?;
    payload::event(&mut out, sequenced)?;
    Ok(out)
}

pub(super) fn error(error: &str, message: &str) -> Result<Vec<u8>, Error> {
    #[derive(Serialize)]
    struct Body<'a> {
        error: &'a str,
        message: &'a str,
    }
    let mut out = atproto_dasl::to_vec(&Header { op: -1, t: None }).map_err(internal)?;
    atproto_dasl::to_writer(&mut out, &Body { error, message }).map_err(internal)?;
    Ok(out)
}
