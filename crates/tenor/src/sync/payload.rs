//! Lexicon-shaped bodies for `com.atproto.sync.subscribeRepos`.
//!
//! Ported from atproto-pds/src/sequencer/payload.rs (MIT), atproto-crates
//! revision 09a868cdde938c789326f1db16ffc30bbf38c15e. See UPSTREAM.md.
//! Local changes: borrow shard events, decode stored CID strings as links,
//! and serialize the sequence/time envelope directly rather than splice a
//! second encoded body. Storage remains the shard's sequenced event JSON.

use atproto_dasl::Cid;
use serde::ser::{Error as _, SerializeSeq as _};
use serde::{Serialize, Serializer};

use crate::event::{Action, Event, RepoOp, Sequenced};

use super::{Error, internal};

#[derive(Serialize)]
struct Envelope<'a, T> {
    seq: i64,
    time: &'a str,
    #[serde(flatten)]
    body: T,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CommitBody<'a> {
    rebase: bool,
    too_big: bool,
    repo: &'a str,
    commit: Cid,
    rev: &'a str,
    since: Option<&'a str>,
    #[serde(serialize_with = "serialize_bytes")]
    blocks: &'a [u8],
    ops: Operations<'a>,
    blobs: &'a [Cid],
    #[serde(skip_serializing_if = "Option::is_none")]
    prev_data: Option<Cid>,
}

#[derive(Serialize)]
struct Operation<'a> {
    action: Action,
    path: &'a str,
    // A deletion's cid is required and nullable; prev is optional.
    cid: Option<Cid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    prev: Option<Cid>,
}

impl<'a> TryFrom<&'a RepoOp> for Operation<'a> {
    type Error = Error;

    fn try_from(op: &'a RepoOp) -> Result<Self, Error> {
        Ok(Self {
            action: op.action,
            path: &op.path,
            cid: cid(op.cid.as_deref())?,
            prev: cid(op.prev.as_deref())?,
        })
    }
}

struct Operations<'a>(&'a [RepoOp]);

impl Serialize for Operations<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut sequence = serializer.serialize_seq(Some(self.0.len()))?;
        for operation in self.0 {
            let operation = Operation::try_from(operation).map_err(S::Error::custom)?;
            sequence.serialize_element(&operation)?;
        }
        sequence.end()
    }
}

#[derive(Serialize)]
struct SyncBody<'a> {
    did: &'a str,
    #[serde(serialize_with = "serialize_bytes")]
    blocks: &'a [u8],
    rev: &'a str,
}

#[derive(Serialize)]
struct IdentityBody<'a> {
    did: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    handle: Option<&'a str>,
}

#[derive(Serialize)]
struct AccountBody<'a> {
    did: &'a str,
    active: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    status: Option<&'a str>,
}

fn serialize_bytes<S: Serializer>(bytes: &[u8], serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_bytes(bytes)
}

fn cid(value: Option<&str>) -> Result<Option<Cid>, Error> {
    value.map(str::parse).transpose().map_err(internal)
}

fn encode<T: Serialize>(out: &mut Vec<u8>, seq: i64, time: &str, body: T) -> Result<(), Error> {
    atproto_dasl::to_writer(out, &Envelope { seq, time, body }).map_err(internal)
}

pub(super) fn event(out: &mut Vec<u8>, sequenced: &Sequenced) -> Result<(), Error> {
    let seq = i64::try_from(sequenced.seq).map_err(internal)?;
    match &sequenced.event {
        Event::Commit(commit) => encode(
            out,
            seq,
            &commit.time,
            CommitBody {
                rebase: false,
                too_big: false,
                repo: commit.repo.as_str(),
                commit: commit.commit.parse().map_err(internal)?,
                rev: &commit.rev,
                since: commit.since.as_deref(),
                blocks: &commit.blocks,
                ops: Operations(&commit.ops),
                blobs: &[],
                prev_data: cid(commit.prev_data.as_deref())?,
            },
        ),
        Event::Sync(sync) => encode(
            out,
            seq,
            &sync.time,
            SyncBody {
                did: sync.repo.as_str(),
                blocks: &sync.blocks,
                rev: &sync.rev,
            },
        ),
        Event::Identity { did, handle, time } => encode(
            out,
            seq,
            time,
            IdentityBody {
                did: did.as_str(),
                handle: handle.as_deref(),
            },
        ),
        Event::Account {
            did,
            active,
            status,
            time,
        } => encode(
            out,
            seq,
            time,
            AccountBody {
                did: did.as_str(),
                active: *active,
                status: status.as_deref(),
            },
        ),
    }
}
