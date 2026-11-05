# Private Spaces implementation plan

This is a proposed sequence, not implemented functionality or a passing
acceptance report. It implements [ADR 0022](../adr/0022-private-space-ingestion.md).
Accept that ADR's owner decisions before enabling private data. The current
public journey must remain unchanged throughout this work.

Each slice includes a useful behavior through its owning boundary. Features
stay disabled until their security dependencies exist. Acceptance rows below
use the names in [testing.md](../testing.md#acceptance-tests); update their
status only after the named real tests run. A partial row is not closed.

## 1. Bind a reader lease to its audience and invalidation generation

**Size:** about one agent-day. No credentials, SQL schema or new public API.

**Scope:** add the AppView's pure access-lease adapter around
`spaces::serving::decide`. Bind reader, Space generation, expiry and local
invalidation generation. Model verification start/install, denial, expiry and
restart explicitly. Integrate it with an in-process private response boundary
that cannot return content without a permit; keep production private routes
unavailable. Do not add a generic authorization framework. The adapter consumes
a verified result and its start time; do not hard-code network hops per renewal.
A future authenticated access-check query must preserve the same lease contract.

**Files:** new `crates/appview/src/private_spaces/access.rs` and module export;
new `crates/appview/tests/private_access.rs`; reuse
`crates/spaces/src/{audience,serving}.rs` without duplicating their decision.

**Acceptance:** supplies the local contract for “Revoke Alice while Bob keeps
the credential alive” and “The same CID in two Spaces”. Neither full row closes
until slices 2–4 exercise credentials, storage and real queries.

**Property test:** any sequence of checks, invalidations and delayed results
cannot install an older grant after revocation. Bug: a successful request
started before removal clears the sticky denial. Generate different readers,
Space generations, credential expiries and future clocks; Bob's events must
not change Alice's verdict.

**Integration test:** send requests through the in-process response boundary
as Alice, Bob and an anonymous reader. Only the exact permitted reader/Space
receives bytes. Bug: the boundary accepts a permit for a different audience or
uses the sync sponsor as the caller. This tests output, not method wiring.

**Risks:** accidentally creating a parallel freshness algorithm or a permit
that can be reused after invalidation. **Running stack:** no; ordinary Rust
and in-process tests only. This slice does not claim live private serving.

## 2. Obtain a member-bound credential and discover a private World

**Scope:** use the session's serialized OAuth client for direct member-PDS
delegation and client attestation. Seal credential/key pairs, retain sponsor
provenance, renew registration, and verify signatures and binding keys. Discover
`members` and its Space index without a public declaration. Enforce permanent
Space-key non-reuse: replacements get fresh scene rkeys/URIs; retain retired keys
across recovery. Refuse deletion/recreation of well-known `self` Spaces in First
World. No new lifecycle descriptor is needed.
Refuse tenor World-origin delegations to remote authorities before enabling this
path. No private prose enters the archive yet.

**Files:** new `appview/src/private_spaces/{credentials,discovery}.rs`;
`appview/src/auth/{pds,store,mod}.rs`, config and service migrations;
`tenor/src/spaces/{http,credentials}.rs`, authorization as needed;
tenor Space creation/store paths for durable retired-key checks;
AppView and tenor credential integration tests. Paths without `crates/` here
and below are under `crates/`.

**Acceptance:** extends “Revoke Alice while Bob keeps the credential alive”,
“Use a fresh member service token after their role is removed” and “A client's
credential and KEM share authenticate the World's exact STAR records, never
another Space”. The existing tenor cases remain; the AppView journey closes
only in slice 4. Add a distinct acceptance row for private-World discovery.

**Property test:** randomized role removal and mint preparation orders never
mint after removal wins the authority transaction. Bug: network preparation
freezes an earlier grant. Model independently from transaction internals.

**Integration tests:** with real PDS OAuth and tenor keys, discover a World with
no declaration, mint through Alice, remove Alice, and renew through Bob. Alice's
fresh mint fails while Bob's works. Bug: World identity replaces member identity.
Use separate authority/repo hosts to catch single-host routing assumptions.
Attempt a remote World-origin delegation and observe refusal. Delete a scene
Space and create its replacement with a fresh URI; old credentials and delayed
deletion notices must not affect it. Retry the retired key through typed and
generic creation, then after restart; each attempt must fail. Bug: one creation
path bypasses permanent retirement. Measure renewal latency and mint traffic:
ADR 0022 triggers access-check-query review at p95 above 500 ms or more than
10 mints/second per tenor for 15 minutes. At 1,000 active reader/Space pairs,
60-second renewal alone costs about 16.7 mints and 33.3 base requests/second,
plus one binding key per mint; these are estimates, not measurements.

**Risks:** existing members must re-consent, app metadata/JWKS delivery,
remote-host revocation limitations. **Running stack:** yes for the
PDS/OAuth/tenor path and PostgreSQL sealing; pure mint tests need none.

## 3. Archive a verified private snapshot without exposing partial data

**Scope:** pull full STAR snapshots from the World's and a member's Space repos.
Verify the whole stream before commit; validate lexicons and record identity.
Add encrypted private evidence/payload objects, an archive envelope format and
private projection tables. Publish no API results yet. Establish checkpoint,
staging cleanup and bounded resource behavior. Preserve public event semantics.

**Files:** new `appview/src/private_spaces/{sync,archive}.rs`;
`appview/src/{archive,event,indexer}.rs`, `projection/{mod.rs,schema.sql}`;
service migrations for keys/jobs; `appview/tests/private_ingestion.rs` and archive
format fixtures. Bump `PROJECTION_VERSION`. Reuse `motet-log` fencing rather than
introducing an unfenced second writer.

**Acceptance:** closes the storage part, not the query part, of “The same CID in
two Spaces”. Extend “Redelivery after an ingest-before-ack crash leaves projected
records and versions unchanged” with a private sync-checkpoint case, clearly
separate from the existing Tap acknowledgement test. Add a row for invalid
private snapshots never becoming visible.

**Property tests:** permute complete verified batches and duplicates; final
versions and unresolved dependencies converge. Bug: ordering different repos
by one shared revision or deduplicating operations by CID. Tamper with one byte,
path, Space or final digest; no completed snapshot may result. Bug: accepting
verified metadata without draining the records stream.

**Integration tests:** inject process-boundary failures after payload put,
after log append and after projection commit using real PostgreSQL/S3. Restart
and observe one complete snapshot or none, never a partial one. Bug: advancing
the checkpoint before durable archive. Put the same CID under different
Spaces, erase one payload and verify the other's bytes remain available only
through its own archive context. Bug: shared key/payload deletion across audiences.

**Risks:** key loss, backward archive compatibility, orphan staging growth,
stream limits not provided by upstream. **Running stack:** yes for storage
faults and real STAR HTTP; verifier/property tests need none.

## 4. Read and write one private scene end to end

### 4a. Prerequisite: conditional Space writes on the personal PDS

**Owner:** a dedicated PDS integration agent, before the scene-journey agent
starts private writes. This is an implementation prerequisite, not a risk to
accept. The pinned atproto-pds `space_handlers.rs` does not implement the
required `swapRecord`/`swapCommit` conditions for its Space write endpoints.
The tenor's conditional World writes do not repair that personal-PDS boundary.

**Scope/files:** implement and atomically enforce create-only/expected-record
and expected-commit conditions across Space `createRecord`, `putRecord` and
`applyWrites`, defining applicable fields and conflict responses in an explicit
wire contract. Patch the pinned PDS storage transaction and handlers together;
never perform a separate read/check followed by an unconditional write. Include
corresponding schema/client changes and capability advertisement; the AppView
must refuse unsupported hosts rather than silently drop preconditions.

Prefer an upstream fix followed by one coordinated atproto-crates revision bump,
as ADR 0013 requires. If a temporary patch is necessary, own it under
`nix/patches/`, apply it in `nix/pkgs/pds-rs.nix`, and add an `UPSTREAM.md` beside
it naming the exact source revision, licence, upstream issue and every local
change. Obtain an ADR explicitly superseding ADR 0017's “only the relay” patch
scope before shipping a PDS patch. Native and compose must use the same patched
build; do not depend on or embed the PDS crate into the tenor.

**Acceptance/tests:** closes the personal-PDS portion of “Two clients edit a
private record on the hosted PDS”. A property test generates concurrent
conditional writes and asserts at most one succeeds against a given predecessor;
bug: checking the precondition outside the write transaction. Integration tests
race two OAuth clients through each deployed endpoint, including batched writes,
null/create-only conditions, stale commit/record conditions and lost responses.
Assert the loser cannot overwrite any winning record and a failed batch changes
nothing. Bug: an endpoint ignores a supplied condition or partially commits.
**Running stack:** yes. **Risks:** changing draft wire shapes and mismatched
client/server versions; pin and document the temporary capability contract.

### 4b. Private scene journey, after 4a passes

**Scope:** create one Space per cast with explicit access grants and scene
records. Write a post to its author's private PDS repo with real conditional
writes; read it back at the tenor, commit admission, and publish it only in that
Space. Match exact admitted versions in private projections and show them through
session-bound AppView routes. Production routes must reuse slice 1's
response-boundary type, not duplicate its authorization checks. Fresh
member-specific mints establish reader leases; SQL filters access before paging/counting. Keep public version routes
public-only. No secret casts, private media, search or notification emails yet;
unsupported paths must refuse private data.

**Files:** `tenor/src/{admission,admission_service,publication}.rs` and scene
procedures; `tenor/src/spaces/{access,http,mod}.rs`; `appview/src/{api,outbox}.rs`,
`writes/`, private access/discovery, projection schema; private scene lexicons
and generated types; web scene route/composer and new private e2e journey.
Confirm exact current module names before editing. Coordinate with public
scene-control work rather than replacing its methods.

**Acceptance:** closes private portions of “Remove a player while their post is
in flight”, “Edit after leaving the cast”, “Two clients edit a private record
on the hosted PDS”, “Revoke Alice while Bob keeps the credential alive” and
“The same CID in two Spaces”. Extend “Public scenes show only exact
World-authored admissions, in place order, even when deliveries arrive out of
order” with a separately named private counterpart. Existing public evidence
must not be relabeled as private coverage.

**Property tests:** independent delivery orders produce the same admitted
versions/places within each audience. Bug: joining admissions by unscoped URI
or letting current cast membership erase historical admissions. Generate removal
before/after commit; only the former prevents admission. Bug: checking cast only
before asynchronous PDS read-back.

**Integration tests:** two real clients race conditional private updates; one
wins and one gets a conflict, without overwrite. Through the browser, Alice
and Bob read a private scene while an outsider gets no title, count or prose.
Remove Alice while Bob continues sync; Alice's next known-revocation read fails,
and an unknown removal stops serving within the clock bound. Simulate a delayed
successful check racing invalidation across two AppView instances. Bug: a stale
lease resurrection or local-only invalidation. Assert no private admission
appears in the public firehose or public API.

**Risks:** deployment must preserve prerequisite 4a's tested PDS capability;
server-side rendering and historical-version endpoints can leak. **Running
stack:** yes, including the production web build and independent PDS accounts.

## 5. Reconcile missed first writes and former writers' deletions

**Scope:** build a durable candidate inventory at the tenor and AppView from
membership/access history, admissions and observed repos. Schedule fair bounded
sweeps independently of writer notifications. Persist dirty hints before receipt
acknowledgement. Perform startup, gap, membership-change and periodic sweeps.
Use full verified snapshots initially; add incremental sync only with final
set verification and STAR fallback. Keep former writers enrolled while their
contributions can change.

**Files:** new `tenor/src/spaces/reconcile.rs` and AppView private reconciliation
worker; tenor store keys/shard effects, `spaces/notify.rs`, admission service;
AppView service migrations, event/checkpoint projection;
`acceptance/tests/private_reconciliation.rs` and private ingestion tests.

**Acceptance:** closes “First private post during a 24-hour notification outage”
and “Former member deletes a private post”. Extend “Stop the tenor while players
post, then restart” to private independent-client posts. “Delete and recreate
byte for byte” gains private observed-op coverage, not a claim to reconstruct
unobserved operations lost to compaction.

**Property tests:** delete, edit, removal, notification loss, retry and restart
schedules converge to verified current state and the same admitted history.
Bug: inventory sourced only from `listRepos`, or removal deletes the polling
candidate. Generate transport failures separately from verified absence; only
the latter deletes. Bug: interpreting an unreachable PDS as an empty repo.

**Integration tests:** suppress every write notification for a logical 24-hour
period using an injected clock, create a first private post from an independent
client, and run reconciliation through real repo hosts and the tenor admission
path. The post appears exactly once. Remove its author, delete it on their PDS,
and reconcile to a tombstone while other cast members retain access. Test
`OplogGap`, malformed final digest, restarted cursor and unreachable PDS. Bugs:
notification dependence, stale-cursor loops, unverified deletions.

**Risks:** historical inventory is sensitive and grows; no silent eviction at
quota. Snapshot gaps cannot reveal a deleted-and-recreated identical record's
unobserved intermediate lifecycle. **Running stack:** yes for outage/restart and
independent PDS tests; scheduler model tests need none. No real 24-hour sleep.

## 6. Enforce revocation and erasure on every output, including rebuild

**Scope:** attach the common reader gate to any enabled private search, media,
download, SSE and queued-delivery boundary. Unsupported features remain denied.
Keep authorized filtering ahead of ranking/counting. Implement the monotonic
erasure ledger in its separately administered versioned bucket, off-box
monotonic head witness, physical/key cleanup and restore barrier. PostgreSQL
holds only a cache; it cannot vouch for the ledger's current head. Rebuild private
projections from archived evidence without network, side effects or restored
live permissions. Add compaction only with retained evidence and erasure tests.

**Files:** private access/archive modules, `appview/src/{api,archive,event}.rs`,
projection code/schema, output workers as introduced, service migrations;
`appview/tests/private_rebuild.rs`, output-boundary integration tests and
browser account-switch coverage. Key/backup configuration belongs in existing
deployment/devenv configuration, with described commands if new ones are needed.

**Acceptance:** closes “Remove a member, then rebuild”, “Rebuild with no network
after compaction and erasure” for the implemented archive format, and “Search
with matching private documents” when actual private search is implemented.
Extend “Revoke Alice while Bob keeps the credential alive” to enabled output
channels. Do not mark search done merely because its absent route refuses.

**Property tests:** replaying any older archive cut with the current erasure
ledger never resurrects erased content, and does not erase an unrelated Space's
same CID. Bug: replay ignores the independent ledger or keys erasure by CID.
Removing unauthorized documents must not change an authorized reader's counts,
ranking or page boundaries. Bug: filtering after search rank/limit.

**Integration tests:** remove Alice, retain Bob, drop projections, disable
network and rebuild using real S3/PostgreSQL. Bob's content reconstructs but
neither reader gains a fresh lease offline. Reconnect and verify only Bob can
read. Restore an older database/bucket snapshot under the current erasure ledger;
erased prose must not reach SQL, render caches or output. Also restore an old
ledger alongside that old data while the off-box witness retains a newer head:
traffic must stay blocked until the full witnessed chain is fetched and applied.
Repeat with an unreachable witness and mismatched epoch/hash; neither may serve.
Bug: trusting the restored PostgreSQL ledger or replaying an old signed receipt.
Exercise old media
links, SSE replay, queued delivery and account switching. Bug: a secondary path
bypasses authorization or rebuild replays side effects.

**Risks:** old backups may retain payload keys until purge; SQL backups contain
plaintext projections. Operational purge evidence is required, not just a unit
test of a tombstone. **Running stack:** yes for rebuild/restore and output tests;
small replay/authorization properties need none.

## 7. Recover a private World without the old tenor

**Scope:** implement the owner's encrypted export with signed manifests,
privileged generation/writer inventory, authoritative outcomes, recovery epoch
and policy-permitted retained author copies. Import on a fresh tenor, rotate
keys and reacquire credentials without requiring former authors to sign in.
Reconcile surviving PDSes and permitted archived evidence. Preserve tombstones,
unknown admissions and possibly spent roll slots. This is distinct from the
already implemented browser rotation-key backup.

**Files:** new tenor export/recovery modules and commands, shard/store/publication
integration; owner export lexicons and generated code; AppView authorized
recovery evidence path; web owner recovery journey;
`acceptance/tests/private_recovery.rs`. Specify a separate export-format ADR
before coding this larger slice; do not overload the ingestion envelope format.

**Acceptance:** closes “Recover a private scene with former members' posts” and
“Recover with the old tenor and an author's PDS gone” for copies the World may
retain. Closes “Restore from an export after readers saw lost admissions” only
when epoch declaration and pending outcomes are exercised. Roll-specific rows
remain open until the live roll path exists and its recovery tests pass.

**Property tests:** restoring an older checkpoint never reuses an epoch/place
identity, resurrects erased content or converts an unknown outcome to success.
Bug: resetting allocation counters or interpreting missing history as unused.

**Integration tests:** create and admit a private post, remove its author,
export, then permanently remove the old tenor and that author's PDS fixtures.
Restore onto a fresh tenor and read the policy-permitted retained copy as a
current cast member; otherwise show its tombstone/unavailable version. Verify
the removed reader and old credentials cannot read. Restore an earlier export
after an admission was shown and assert the new epoch and unresolved state,
not silent rewriting of the visible place. Bug: export includes only current
members or recovery treats the AppView's cache as authoritative admission.

**Risks:** export size, custody, alpha format changes and unavailable author
copies. No design can recover bytes no surviving permitted copy contains.
**Running stack:** yes, with isolated fresh storage and destructive failure
fixtures; never run this against a person's real accounts.

## Owner choices and release gates

The owner must approve ADR 0022's six questions: member-backed continuity,
cross-authority restrictions, freshness bound, retention/key/backup policy,
reconciliation target and private-alpha release requirements. Existing members
must re-consent with Space scopes. Decide how long cached copies survive loss
of the last usable session and who can request deletion. In particular:

- Owner decision 4 may choose a closed-alpha tombstone-only default and defer
  the independently witnessed erasure ledger until before open beta. That saves
  a separate bucket/account, witness service and automated restore barrier, but
  cannot promise erasure from historical backups. Disclose retention, prohibit
  automatic restoration to serving, and require a reviewed manual purge/restore
  process for required erasures. If it cannot meet legal duties, real private
  data must wait. Slice 6's erasure/compaction row stays open under this option.

- A five-minute reconciliation interval and a 60-second reader bound concern
  different promises. Measure both; do not advertise them as established yet.
- New members see a Space's available history. Removal stops future access;
  it cannot recall copies already delivered. Secret cast membership is not part
  of these slices.
- Private ingestion and the UI can be tested with synthetic accounts before
  recovery ships. Real private alpha play still requires tested protected
  backups, owner export/restore, migration across alpha changes and invitation
  risk disclosure. First World readiness must state this dependency.

No runtime tests accompany this document-only proposal. Each slice's test list
is an obligation, not evidence. Start with slice 1 while the remaining owner
choices are reviewed; do not start the running stack until host TLS setup has
been completed by the owner.
