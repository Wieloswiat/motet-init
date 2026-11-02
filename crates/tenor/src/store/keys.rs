//! Every key the shard's database holds is built here, so no two parts of
//! the tenor can write under the same prefix by accident.
//!
//! | Prefix      | Holds                                                  |
//! | ----------- | ------------------------------------------------------ |
//! | `meta/`     | the applied marker, the rules checkpoint, event seq    |
//! | `w/{did}/`  | per-World info and its rules snapshot                  |
//! | `k/{did}/`  | the World's keys, sealed under the master key          |
//! | `b/{did}/`  | the World's repo blocks, by CID bytes                  |
//! | `h/`, `r/`  | repo heads and record indexes (the repo module)        |
//! | `e/`        | firehose events by sequence number                     |
//! | `n/`        | handles (the identity module)                          |
//! | `q/`        | reservations, their sealed keys and creation receipts   |
//! | `v/`, `x/`  | reserved handles and reservation expiry order           |
//! | `o/`        | owner-scoped reservation operations                     |
//! | `s/`        | Spaces (the spaces module)                             |
//! | `p/`        | admission publication queue and receipts               |
//! | `c/`        | typed scene operation receipts                         |
//! | `t/`        | spent member service tokens and their expiry order     |
//!
//! DIDs contain no `/`, so they can be a path component as they are. A
//! component that can contain `/` is followed by `\0` instead.

use atproto_space::SpaceUri;
use motet_rules::Did;

use crate::keys::Purpose;

/// The last log segment written to the database.
pub const APPLIED: &[u8] = b"meta/applied";
/// The log segment the rules snapshots under `w/{did}/rules` were taken at.
pub const RULES_AT: &[u8] = b"meta/rules_at";
/// The next firehose sequence number to assign.
pub const EVENT_SEQ: &[u8] = b"meta/event_seq";

/// Every World's rules snapshot; `w/{did}/rules`.
pub const WORLDS: &[u8] = b"w/";
const RULES_SUFFIX: &[u8] = b"/rules";

/// Firehose events, ordered by sequence number.
pub const EVENTS: &[u8] = b"e/";

/// Unsigned reservations in expiry order; submitting intents never expire.
pub const RESERVATION_EXPIRIES: &[u8] = b"x/";

/// Unpublished admissions and closings, captured with their rules decisions.
pub const PUBLICATIONS: &[u8] = b"p/queue/";

/// The next per-shard publication decision sequence.
pub const PUBLICATION_SEQ: &[u8] = b"p/next";

/// Spent member service tokens, ordered by expiration for bounded cleanup.
pub const SERVICE_TOKEN_EXPIRIES: &[u8] = b"t/expiry/";
/// Greatest time used to expire a spent token, persisted across clock rollback.
pub const SERVICE_TOKEN_CLOCK: &[u8] = b"t/clock";
/// Pending Space delegation origins, ordered by their expiration.
pub const SPACE_DELEGATION_EXPIRY: &[u8] = b"s/delegation-origin-expiry/";
/// Cross-World credential reverse entries, ordered by credential expiration.
pub const SPACE_DELEGATED_CREDENTIAL_EXPIRY: &[u8] = b"s/delegated-credential-expiry/";

/// The issuer and token ID identify a token regardless of its request body.
#[must_use]
pub fn service_token(issuer: &Did, jti: &str) -> Vec<u8> {
    join(&[b"t/token/", issuer.as_str().as_bytes(), b"/", jti.as_bytes()])
}

#[must_use]
pub fn service_token_expiry(expires: u64, issuer: &Did, jti: &str) -> Vec<u8> {
    join(&[
        SERVICE_TOKEN_EXPIRIES,
        &expires.to_be_bytes(),
        issuer.as_str().as_bytes(),
        b"/",
        jti.as_bytes(),
    ])
}

#[must_use]
pub fn service_token_expiry_time(key: &[u8]) -> Option<u64> {
    let bytes = key.strip_prefix(SERVICE_TOKEN_EXPIRIES)?.get(..8)?;
    Some(u64::from_be_bytes(bytes.try_into().ok()?))
}

#[must_use]
pub fn space_delegation_origins(world: &Did) -> Vec<u8> {
    space_delegation_origin(world, "")
}

/// A World-issued Space delegation is bound to its requesting member.
#[must_use]
pub fn space_delegation_origin(world: &Did, jti: &str) -> Vec<u8> {
    join(&[
        b"s/delegation-origin/",
        world.as_str().as_bytes(),
        b"\0",
        jti.as_bytes(),
    ])
}

#[must_use]
pub fn space_delegation_expiry(expires: u64, world: &Did, jti: &str) -> Vec<u8> {
    join(&[
        SPACE_DELEGATION_EXPIRY,
        &expires.to_be_bytes(),
        world.as_str().as_bytes(),
        b"\0",
        jti.as_bytes(),
    ])
}

/// Credentials obtained through a locally hosted World's delegations.
#[must_use]
pub fn space_delegated_credentials(world: &Did) -> Vec<u8> {
    space_delegated_credential(world, &[])
}

#[must_use]
pub fn space_delegated_credential(world: &Did, credential: &[u8]) -> Vec<u8> {
    join(&[b"s/delegated-credential/", world.as_str().as_bytes(), b"\0", credential])
}

#[must_use]
pub fn space_delegated_credential_expiry(expires: u64, world: &Did, credential: &[u8]) -> Vec<u8> {
    join(&[
        SPACE_DELEGATED_CREDENTIAL_EXPIRY,
        &expires.to_be_bytes(),
        world.as_str().as_bytes(),
        b"\0",
        credential,
    ])
}

#[must_use]
pub fn publication(sequence: u64) -> Vec<u8> {
    join(&[b"p/receipt/", &sequence.to_be_bytes()])
}

/// Big-endian sequences preserve decision order, independent of post CIDs.
#[must_use]
pub fn publication_queue(sequence: u64) -> Vec<u8> {
    join(&[PUBLICATIONS, &sequence.to_be_bytes()])
}

/// Operation identities deduplicate retries, not later identical post bytes.
#[must_use]
pub fn publication_operation(world: &Did, operation: &motet_rules::OperationId) -> Vec<u8> {
    join(&[
        b"p/operation/",
        &(world.as_str().len() as u64).to_be_bytes(),
        world.as_str().as_bytes(),
        operation.as_str().as_bytes(),
    ])
}

/// Idempotency receipts are isolated by World and verified actor.
#[must_use]
pub fn scene_operation(world: &Did, actor: &Did, operation: &str) -> Vec<u8> {
    join(&[
        b"c/scene/",
        world.as_str().as_bytes(),
        b"/",
        actor.as_str().as_bytes(),
        b"/",
        operation.as_bytes(),
    ])
}

/// The last publication decision for a post, including closings.
#[must_use]
pub fn publication_latest(world: &Did, post: &motet_rules::RecordUri) -> Vec<u8> {
    join(&[
        b"p/latest/",
        &(world.as_str().len() as u64).to_be_bytes(),
        world.as_str().as_bytes(),
        post.as_str().as_bytes(),
    ])
}

fn join(parts: &[&[u8]]) -> Vec<u8> {
    let mut key = Vec::with_capacity(parts.iter().map(|p| p.len()).sum());
    for part in parts {
        key.extend_from_slice(part);
    }
    key
}

#[must_use]
pub fn world_info(world: &Did) -> Vec<u8> {
    join(&[b"w/", world.as_str().as_bytes(), b"/info"])
}

#[must_use]
pub fn world_rules(world: &Did) -> Vec<u8> {
    join(&[b"w/", world.as_str().as_bytes(), RULES_SUFFIX])
}

/// The World a `w/{did}/rules` key belongs to, or `None` for any other key
/// under `w/`.
#[must_use]
pub fn rules_world(key: &[u8]) -> Option<&str> {
    let did = key.strip_prefix(WORLDS)?.strip_suffix(RULES_SUFFIX)?;
    std::str::from_utf8(did).ok().filter(|d| !d.contains('/'))
}

#[must_use]
pub fn sealed_key(world: &Did, purpose: Purpose) -> Vec<u8> {
    join(&[b"k/", world.as_str().as_bytes(), b"/", purpose.as_str().as_bytes()])
}

/// The prefix of one World's repo blocks.
#[must_use]
pub fn blocks(world: &Did) -> Vec<u8> {
    join(&[b"b/", world.as_str().as_bytes(), b"/"])
}

#[must_use]
pub fn block(world: &Did, cid: &[u8]) -> Vec<u8> {
    join(&[b"b/", world.as_str().as_bytes(), b"/", cid])
}

/// Zero-padded so byte order is numeric order.
#[must_use]
pub fn event(seq: u64) -> Vec<u8> {
    join(&[EVENTS, format!("{seq:020}").as_bytes()])
}

/// The sequence number of an `e/` key.
#[must_use]
pub fn event_seq(key: &[u8]) -> Option<u64> {
    std::str::from_utf8(key.strip_prefix(EVENTS)?).ok()?.parse().ok()
}

/// Current repo heads and retained revisions. Only keys without a second
/// slash are current heads.
pub const REPO_HEADS: &[u8] = b"h/";

#[must_use]
pub fn repo_head(world: &Did) -> Vec<u8> {
    join(&[REPO_HEADS, world.as_str().as_bytes()])
}

#[must_use]
pub fn repo_revision(world: &Did, rev: &str) -> Vec<u8> {
    join(&[REPO_HEADS, world.as_str().as_bytes(), b"/", rev.as_bytes()])
}

#[must_use]
pub fn repo_records(world: &Did, collection: Option<&str>) -> Vec<u8> {
    collection.map_or_else(
        || join(&[b"r/", world.as_str().as_bytes(), b"/"]),
        |collection| join(&[b"r/", world.as_str().as_bytes(), b"/", collection.as_bytes(), b"/"]),
    )
}

#[must_use]
pub fn repo_record(world: &Did, collection: &str, rkey: &str) -> Vec<u8> {
    join(&[
        b"r/",
        world.as_str().as_bytes(),
        b"/",
        collection.as_bytes(),
        b"/",
        rkey.as_bytes(),
    ])
}

#[must_use]
pub fn handle(handle: &str) -> Vec<u8> {
    join(&[b"n/", handle.as_bytes()])
}

#[must_use]
pub fn reserved_handle(handle: &str) -> Vec<u8> {
    join(&[b"v/", handle.as_bytes()])
}

#[must_use]
pub fn reservation(reservation: &str) -> Vec<u8> {
    join(&[b"q/", reservation.as_bytes(), b"/info"])
}

#[must_use]
pub fn reservation_op(owner: &Did, op: &str) -> Vec<u8> {
    join(&[b"o/", owner.as_str().as_bytes(), b"/", op.as_bytes()])
}

#[must_use]
pub fn reserved_key(reservation: &str, purpose: Purpose) -> Vec<u8> {
    join(&[b"q/", reservation.as_bytes(), b"/", purpose.as_str().as_bytes()])
}

#[must_use]
pub fn reservation_expiry(reservation: &str, expires: u64) -> Vec<u8> {
    join(&[
        RESERVATION_EXPIRIES,
        format!("{expires:020}").as_bytes(),
        b"/",
        reservation.as_bytes(),
    ])
}

/// Every issued credential of a World, including secret and deleted Spaces.
#[must_use]
pub fn space_credentials(world: &Did) -> Vec<u8> {
    join(&[b"s/credential/", world.as_str().as_bytes(), b"/"])
}

#[must_use]
pub fn space_credential(space: &str, world: &Did, jti: &str) -> Vec<u8> {
    join(&[
        b"s/credential/",
        world.as_str().as_bytes(),
        b"/",
        space.as_bytes(),
        b"\0",
        jti.as_bytes(),
    ])
}

/// Replay identities are issuer-scoped; a length prefix prevents ambiguous joins.
#[must_use]
pub fn space_delegation_replay(issuer: &str, jti: &str) -> Vec<u8> {
    join(&[
        b"s/delegation-replay/",
        &(issuer.len() as u64).to_be_bytes(),
        issuer.as_bytes(),
        jti.as_bytes(),
    ])
}

#[must_use]
pub fn space_attestation_replay(issuer: &str, jti: &str) -> Vec<u8> {
    join(&[
        b"s/attestation-replay/",
        &(issuer.len() as u64).to_be_bytes(),
        issuer.as_bytes(),
        jti.as_bytes(),
    ])
}

// The repo owner and Space authority are separate identities. Keeping both
// prevents two authors in the same Space from sharing a commitment.
fn space_store_key(owner: &Did, space: &SpaceUri, kind: &[u8], suffix: &[&[u8]]) -> Vec<u8> {
    let prefix: &[&[u8]] = &[
        b"s/",
        kind,
        b"/",
        owner.as_str().as_bytes(),
        b"/",
        space.space_did.as_bytes(),
        b"/",
        space.space_type.as_str().as_bytes(),
        b"/",
        space.space_key.as_str().as_bytes(),
        b"/",
    ];
    let mut key = Vec::with_capacity(prefix.iter().chain(suffix).map(|part| part.len()).sum());
    for part in prefix.iter().chain(suffix) {
        key.extend_from_slice(part);
    }
    key
}

/// Full record-set hash state and revision, not the wire commitment digest.
#[must_use]
pub fn space_repo_state(owner: &Did, space: &SpaceUri) -> Vec<u8> {
    space_store_key(owner, space, b"repo", &[b"state"])
}

/// All record rows, or one collection's rows in record-key order.
#[must_use]
pub fn space_records(owner: &Did, space: &SpaceUri, collection: Option<&str>) -> Vec<u8> {
    collection.map_or_else(
        || space_store_key(owner, space, b"repo", &[b"records/"]),
        |collection| space_store_key(owner, space, b"repo", &[b"records/", collection.as_bytes(), b"/"]),
    )
}

#[must_use]
pub fn space_record(owner: &Did, space: &SpaceUri, collection: &str, rkey: &str) -> Vec<u8> {
    space_store_key(
        owner,
        space,
        b"repo",
        &[b"records/", collection.as_bytes(), b"/", rkey.as_bytes()],
    )
}

/// Reads a collection from a row key under a `space_records(_, _, None)` prefix.
#[must_use]
pub fn space_record_collection<'a>(prefix: &[u8], key: &'a [u8]) -> Option<&'a str> {
    let path = std::str::from_utf8(key.strip_prefix(prefix)?).ok()?;
    Some(path.split_once('/')?.0)
}

/// Record operations in `(rev, idx)` order, retained indefinitely.
#[must_use]
pub fn space_repo_oplog(owner: &Did, space: &SpaceUri) -> Vec<u8> {
    space_store_key(owner, space, b"repo", &[b"oplog/"])
}

/// A big-endian index keeps batches larger than ten operations in order.
#[must_use]
pub fn space_repo_op(owner: &Did, space: &SpaceUri, rev: &str, idx: u32) -> Vec<u8> {
    space_store_key(
        owner,
        space,
        b"repo",
        &[b"oplog/", rev.as_bytes(), b"/", &idx.to_be_bytes()],
    )
}

/// Full member-set hash state and revision, separate from the record set.
#[must_use]
pub fn space_member_state(owner: &Did, space: &SpaceUri) -> Vec<u8> {
    space_store_key(owner, space, b"members", &[b"state"])
}

/// Member rows in DID order.
#[must_use]
pub fn space_members(owner: &Did, space: &SpaceUri) -> Vec<u8> {
    space_store_key(owner, space, b"members", &[b"rows/"])
}

#[must_use]
pub fn space_member(owner: &Did, space: &SpaceUri, did: &str) -> Vec<u8> {
    space_store_key(owner, space, b"members", &[b"rows/", did.as_bytes()])
}

/// Member operations retained with the member commitment.
#[must_use]
pub fn space_member_oplog(owner: &Did, space: &SpaceUri) -> Vec<u8> {
    space_store_key(owner, space, b"members", &[b"oplog/"])
}

#[must_use]
pub fn space_member_op(owner: &Did, space: &SpaceUri, rev: &str, idx: u32) -> Vec<u8> {
    space_store_key(
        owner,
        space,
        b"members",
        &[b"oplog/", rev.as_bytes(), b"/", &idx.to_be_bytes()],
    )
}

/// Durable Space notifications, ordered by their next delivery time.
pub const SPACE_NOTIFY_OUTBOX: &[u8] = b"s/notify/outbox/";

#[must_use]
pub fn space_notify_outbox(due_ms: u64, id: &str) -> Vec<u8> {
    join(&[SPACE_NOTIFY_OUTBOX, &due_ms.to_be_bytes(), b"/", id.as_bytes()])
}

/// The next delivery time encoded in an outbox key.
#[must_use]
pub fn space_notify_due(key: &[u8]) -> Option<u64> {
    let bytes = key.strip_prefix(SPACE_NOTIFY_OUTBOX)?.get(..8)?;
    Some(u64::from_be_bytes(bytes.try_into().ok()?))
}

#[must_use]
pub fn space_notify_writers(space: &SpaceUri) -> Vec<u8> {
    join(&[b"s/notify/writer/", space.to_string().as_bytes(), b"\0"])
}

#[must_use]
pub fn space_notify_writer(space: &SpaceUri, writer: &str) -> Vec<u8> {
    join(&[&space_notify_writers(space), writer.as_bytes()])
}

#[must_use]
pub fn space_notify_revisions(space: &SpaceUri) -> Vec<u8> {
    join(&[b"s/notify/revision/", space.to_string().as_bytes(), b"\0"])
}

#[must_use]
pub fn space_notify_revision(space: &SpaceUri, rev: &str) -> Vec<u8> {
    join(&[&space_notify_revisions(space), rev.as_bytes()])
}

#[must_use]
pub fn space_notify_sequence(space: &SpaceUri) -> Vec<u8> {
    join(&[b"s/notify/sequence/", space.to_string().as_bytes()])
}

#[must_use]
pub fn space_notify_registrations(space: &SpaceUri) -> Vec<u8> {
    join(&[b"s/notify/registration/", space.to_string().as_bytes(), b"\0"])
}

#[must_use]
pub fn space_notify_registration(space: &SpaceUri, service: &str) -> Vec<u8> {
    join(&[&space_notify_registrations(space), service.as_bytes()])
}

#[must_use]
pub fn space_notify_revoked(space: &SpaceUri, jti: &str) -> Vec<u8> {
    join(&[
        b"s/notify/revoked/",
        space.to_string().as_bytes(),
        b"\0",
        jti.as_bytes(),
    ])
}

/// Host-private inventory, including secret and deleted Spaces.
#[must_use]
pub fn space_inventory(world: &Did) -> Vec<u8> {
    join(&[b"s/authority/", world.as_str().as_bytes(), b"/"])
}

#[must_use]
pub fn space_info(space: &SpaceUri) -> Vec<u8> {
    join(&[
        b"s/authority/",
        space.space_did.as_bytes(),
        b"/",
        space.space_type.as_str().as_bytes(),
        b"/",
        space.space_key.as_str().as_bytes(),
    ])
}
