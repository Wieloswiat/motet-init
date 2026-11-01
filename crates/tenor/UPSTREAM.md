# Ported atproto-pds modules

Source: [atproto-crates](https://tangled.org/ngerakines.me/atproto-crates), revision
`09a868cdde938c789326f1db16ffc30bbf38c15e` (0.15.0-rc.6), MIT.
The tenor uses the protocol crates from that same revision. It does not depend
on the unpublished SQLite-backed `atproto-pds` server.

| Local module | Upstream source | Local changes |
| --- | --- | --- |
| `src/repo/commit_car.rs` | `crates/atproto-pds/src/repo/commit_car.rs` | Uses the transaction's existing recorded blocks instead of `RecordingBlockStorage`; keeps covering proofs separate so unchanged nodes are not stored again; streams selected blocks into the CAR writer instead of collecting a second block vector; deterministic visited set; follows typed structural edges rather than attempting to decode records as MST nodes; returns protocol CAR errors directly. Sync still carries only the signed commit. |
| `src/sync/frame.rs` | `crates/atproto-pds/src/sequencer/frame.rs` | Keeps `xrpc.v0.cbor` only; borrows the header and writes a shard event directly into the frame buffer; propagates encoding errors instead of emitting empty frames. Omits JSON/v1 encodings and info/OutdatedCursor frames because this shard never prunes history. |
| `src/sync/payload.rs` | `crates/atproto-pds/src/sequencer/payload.rs` | Borrows shard events, parses stored CID strings into link types, serializes raw bytes without a new dependency, and writes sequence/time in the flattened envelope instead of decoding and splicing a stored body. Reuses the shard action enum and converts its operations; retains the protocol's nullability, optional fields and required rebase/tooBig/blobs fields. |
| `src/plc_genesis.rs` | `crates/atproto-pds/src/plc.rs`, `prepare_genesis` / `build_genesis_operation` | Requires the owner's signature, places the owner's rotation key first, and adds a tenor rotation key only by opt-in. Builds an unsigned operation with PLC `UpdateBuilder` validation because `DidBuilder` requires a signer. Publishes `MotetTenor` beside `atproto_pds`. Key reservation and adoption use the tenor's transaction and sealed-key custody. |
| `src/spaces/credentials.rs` | `crates/atproto-pds/src/space/{mint_authz,issued,revoke}.rs`, `src/http/{space_auth,space_mac}.rs` | Verifies network assertions through `motet-net` before entering the shard. Rechecks current role-derived access, assertion replay identities and credential generations inside the transaction. Stores one revocation decision per issued JTI and queues revocation delivery atomically. Uses the World's sealed Signing and Kem keys. |
| `src/spaces/notify.rs` | `crates/atproto-pds/src/space/{notify,sequencer,write_delivery,deletion_delivery,revocation,revoke,recipient}.rs` | Replaces SQL with shard transactions. Writes writer revisions, registrations and delivery intents atomically; resolves recipients and sends outside the writer through `motet-net`. Signs each delivery attempt afresh. Retries until the registration or credential deadline and rejects obsolete deletion generations. |
| `src/spaces/http.rs` | `crates/atproto-pds/src/http/space_handlers.rs`, `src/http/space_mac.rs`, `src/space/reader.rs` | Registers methods through `motet-xrpc`; admits HTTP Message Signature credentials as a separate caller class. Reads coherent repo exports through shard operations and derives MACs from the addressed World's Kem key. Pages across collections with one limit and collection/record-key cursors. Management writes opensocial access and membership records rather than a second ACL. |

The repository writer is Motet code over `atproto-repo::Mst` and
`atproto-identity::key::sign`, following the upstream writer's commit and
covering-proof sequence. Revisions use `Tid::next_after` against the durable
head, including after restart or a clock rollback. All known record schemas
resolve through an offline `atproto-lexicon::BaseCatalog` built from
`motet_lexicons::SCHEMAS`; no network resolution or shape-only fallback is used.

The Space storage adapters retain full 2048-byte LtHash states and complete
oplogs under `s/repo/` and `s/members/`. Deleting a Space records its record
deletions before retaining a tombstone; recreating its URI advances the old
revision instead of silently truncating history. Authority inventory lives
under `s/authority/`, including Spaces omitted from the members-visible index.
The credential ledger, replay guards, writer set and notification outbox also
live under `s/` and are replayed from the shard log.

World creation writes meta, members and GM Spaces, the owner and GM roles,
owner membership, permissions, profile, access records and discovery entries.
The public declaration follows meta's public flag, including later changes.
Space records reject blob references until a blob upload/storage boundary
exists; authenticated blob reads consequently report no stored blobs.

The role-backed host serves opensocial create/delete, and simplespace
get/delete/listMembers. Simplespace create/update/putMember/removeMember are
not declared: their independent ACL/policy model would conflict with the
opensocial records. `getDelegationToken` requires service authentication by a
member with a current whole-Space read grant. Repo and Space writes recheck
the member's current roles and grants inside the write operation, append an
`at.motet.pilot.audit` record in the GM Space atomically, and enforce
opensocial assignable-role and grant-escalation limits. Removing a role affects
the next write without waiting for token expiry.

When updating the pin, compare CAR selection, firehose encoding and PLC
ordering with those source files, and run consumer-verification and event-replay tests. The
covering-proof tests exercise the producer contract; they are not a claim of
formal proof or an implementation of a general Sync 1.1 verifier.

## Upstream licence

MIT License

Copyright (c) 2024 Nick Gerakines

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in
all copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN
THE SOFTWARE.
