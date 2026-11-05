# 0022. Private Space ingestion with member-bound reader checks

Status: Proposed

## Context

The First World milestone needs private posts, not just private storage.
The tenor already hosts Spaces, mints credentials, tracks writers and publishes
private admissions without putting them in the public repo. The AppView only
reads public data. Its `SpaceRecordEvent` explicitly means a public snapshot;
putting private records through that variant would violate its contract.

This proposal extends [0006](0006-storage.md) and [0007](0007-projections.md).
It preserves member attribution from [0018](0018-world-service-auth.md) and
[0021](0021-single-use-member-tokens.md), and the audience rules in
[the plan](../plan.md#audience-lifecycle). It supersedes ADR 0018's statement that the tenor “does not mint a World token
for downstream services”: `getDelegationToken` already issues `iss=World`
tokens. Local origin-bound issuance is the retained exception; issuance to a
remote authority becomes an explicit refusal. It does not introduce World OAuth
sessions or end-to-end encryption. Space hosts, the tenor and the AppView can
read private content. Space MACs provide authenticated, deniable reads, not
publicly verifiable author signatures.

### Evidence and upstream boundary

The inspected atproto-crates revision in `Cargo.lock` is
`09a868cdde938c789326f1db16ffc30bbf38c15e`.

- `atproto-space-client::{credential,attestation}` provides member-PDS
  delegation, client attestation, key-bound credentials and notification
  registration. `SpaceHosts` separates the member's PDS from the authority.
- `sync` provides `SpaceCheckpoint`, `list_repos`, `list_repo_ops`, STAR reads
  and `OplogGap`. `listRepos` is a notification-derived hint, not a complete
  membership inventory. `catch_up` retains all pages; use bounded paging instead
  for a durable worker.
- Credential-based `get_repo` verifies the commit MAC and full set digest.
  `get_repo_streaming` is not complete until its records stream finishes. Its
  configured response byte limit does not bound the full stream; Motet must.
- `list_repo_ops` authenticates a supplied terminal commit, not every operation
  independently. Motet must verify the resulting set against that commit.
  Blob fetches do not verify their CID; Motet must hash them before use.
- `tenor::spaces::credentials` remembers the member behind a locally issued
  World delegation. Another authority cannot consult that binding.
  `tenor::spaces::http::Delegation` currently permits remote targets.
- `appview::{archive,indexer,projection}` already enforces archive, project,
  then acknowledge. It does not yet validate every record against its lexicon,
  implement private retention, or authorize private queries.

## Decision

### Ownership and credentials

The tenor owns authoritative access and admission. `motet-spaces` owns pure
identity and serving decisions. A new `appview::private_spaces` worker owns
credential renewal, discovery and verified ingestion. It uses `motet-net` for
all outbound requests. The browser continues to talk only to the AppView.

Obtain each private read credential through the **person's own PDS**, using
that person's OAuth grant for `com.atproto.space.getDelegationToken`. Present
that delegation and an AppView client attestation to the World's Space
authority at `getSpaceCredential`. The delegation issuer must be the signed-in
person, not the World. The authority applies current opensocial roles and
Space grants to that issuer. App identity satisfies the application's access
policy; it never substitutes for membership.

Add the required Space OAuth scopes explicitly and obtain renewed consent
where an existing session lacks them. Do not forward the OAuth bearer to the
tenor. Existing World mutations retain their exact-method service tokens and
single-use semantics. The SDK's credential helper must be adapted to the
AppView's serialized session refresh and nonce handling, not given a second
refresh loop.

Verify the returned credential's authority signature, Space, expiry and
binding key; constructing `SpaceCredentialGrant` alone is not signature
verification. Generate a new binding key per mint. Seal credentials and keys
in service storage with identity and purpose binding, outside the archive.
Persist the delegating member, session reference, credential JTI, expiry and
subscription expiry. Never log tokens, keys or private record bodies.

One usable member may sponsor ingestion of a Space. Renewal may choose another
currently authorized member with a live, consented session. That substitution
never refreshes anyone else's reader access. Renew before expiry, use fresh
single-use delegation and attestation tokens, and refresh notification
registration separately. On ambiguous mint failure, mint with new assertions;
do not replay spent assertions. Losing the last usable session stops sync and
serving for that Space, even while cached credentials remain unexpired.
Retaining a cached copy is a separate retention decision.

### Discovery and generations

A supplied World DID or invite is a discovery hint, not authorization. A private
World must work without a public declaration. Bootstrap the known `members`
Space through a member credential, then read its verified Space index,
memberships, roles and permissions. Read each scene's own access facts only
with a credential for that Space. Resolve each author's current PDS from its
DID, with the same destination validation used by `public_space`.

Maintain a private inventory of candidates from current and historical
membership/access facts, observed writers, World admissions and previous syncs.
It includes members who have never written and former writers. `listRepos`
adds candidates and revisions but cannot remove them from this inventory.
The tenor needs its own durable inventory for admission and owner recovery;
it must not depend on the AppView remaining available. Privileged inventory
must not be published into the members-visible index.

Use `(Space URI, generation, repo DID, collection, rkey)` as record identity;
include CID for a version. Keep operations distinct from content. The same CID
in two Spaces must not share access, deletion or cache state. Bind every job,
subscription and verification to its generation. For First World, the tenor
must never reuse a deleted Space key. Private scene creation allocates a fresh
scene rkey and therefore a fresh Space URI. Retain an irreversible retired-key
inventory across restart, export and recovery; client-supplied creation keys
cannot bypass it. The standard reference needs no new lifecycle descriptor.
The internal generation remains defense in depth and does not change the
reader-lease contract.

Well-known `meta` and `members` Spaces use `self`, so they cannot be deleted and
recreated under this rule. Refuse that lifecycle operation in First World;
update their records instead. Restoring the same logical Space from an export
is not recreation: preserve its identity and retirements, declare the recovery
epoch separately, and invalidate old leases. Recovery must reconcile missing
retirement evidence before creating Spaces; uncertain keys remain unavailable.
A future requirement to reuse a URI needs an authenticated lifecycle descriptor
and a separate decision, not an implicit relaxation.

A deleted Space stays denied. Delayed deletion notifications refer only to its
retired URI and cannot erase a replacement scene's new URI. No implicit public/private moves or audience widening are allowed.
Ordinary private scenes reveal their cast through roles. Secret scenes remain
unsupported until their separate access extension exists.

### Conditional personal-PDS writes are a prerequisite

The pinned atproto-pds does not implement the required Space write swap
conditions. Private posting must wait for atomic `swapRecord`/`swapCommit`
semantics on the applicable `createRecord`, `putRecord` and `applyWrites`
endpoints, with a documented capability contract and real concurrent-client
tests. The AppView must refuse hosts lacking it. Plan slice 4a owns this work;
a temporary PDS patch needs its upstream provenance recorded and a new ADR
superseding ADR 0017's relay-only patch scope. Prefer an upstream fix and a
coordinated pin update over a permanent private protocol fork.

### Verification, archive and projection

Start with bounded full STAR snapshots. Authenticate HTTP requests to the
actual repo owner, obtain its current KEM key, verify the commit's Space, repo,
revision, MAC, every record and final set digest. Validate record bytes, CIDs,
lexicons and cross-record audience references. Stage incomplete snapshots in
non-serving temporary storage; never advance a cursor or expose a partial
snapshot. Reject revision rollback or inconsistent same-revision contents and
mark the repo unresolved. A verified current snapshot establishes current
state, not every operation that happened during an outage.

Later, apply paginated operations to a staged last-verified set, checking
previous CIDs and record hashes and comparing the final set digest with the
authenticated terminal commit. Fall back to STAR on a gap, missing values or
mismatch. Do not silently replace an admitted version with today's content.
Missing dependencies stay unresolved; only exact World-authored admissions
in the same Space and generation make posts visible, ordered by epoch/place.
Former members' admitted posts remain, but their new edits require admission.

Keep private payloads **out of the public archive log and public tables**.
Use a separate private object-store prefix with restricted credentials and
application-layer authenticated encryption. A prefix alone is not encryption.
The existing fenced AppView archive writer remains the one ordering point:
its versioned log envelopes reference encrypted private evidence and payload
objects. Write those objects durably before appending their reference; project
only committed envelopes. Envelopes must not contain private prose, member
names or scene titles. Private evidence objects remain access-restricted too.

Use distinct encryption keys per erasable payload object, wrapped by a
persistent archive master key outside PostgreSQL and the bucket. Bind object
identity, Space and generation as authenticated data. Store the removable key
references separately from immutable evidence. Do not deduplicate plaintext or
keys across audiences. PostgreSQL private projections and backups need encrypted
disks and restricted access; object encryption does not protect plaintext SQL.

Keep admissions, version identities, observed deletions and policy decisions
as replay evidence, apart from erasable prose and media. Default deletion removes
content and leaves its place tombstoned. Retaining a World-authored copy requires
the explicit retention policy shown before joining. A removal of membership is
not a content erasure. Space deletion removes its retained data and derived
copies according to the protocol; preserve only minimal permitted erasure
receipts, not a hidden recoverable content archive.

A monotonic erasure ledger dominates every replay and restore. Store its
append-only hash-chained entries in a dedicated versioned object-store bucket,
in a separate administrative account from the AppView archive and PostgreSQL
backups. Give the application append/read access, not rollback or deletion.
Each entry carries a deployment identity, ledger epoch, increasing sequence,
previous-entry hash and the erased payload/generation identities, without prose.

A separate off-box witness service durably accepts only compare-and-swap advances
to `(deployment, epoch, sequence, head hash)`. Its account and signing key are
outside the application's backup/restore boundary. A new epoch requires explicit
operator authorization and a link to the previous witnessed head; resetting a
database cannot create an epoch. Acknowledge an erasure only after its ledger
entry and witness advance are durable. Uncertain advances stop acknowledgement
and serving until reconciled; they never roll the witness back.

Before restoring traffic, query the witness directly over authenticated TLS and
read the ledger through that witnessed head. Verify its hash chain and apply all
erasures before exposing projections or unwrapping restored payload keys. Missing
entries, an older head, a mismatched epoch/hash or an unreachable witness blocks
traffic. PostgreSQL may cache the ledger but cannot establish its freshness. A
previously signed witness receipt alone is replayable and does not establish the
current head. Offline rebuild may use a fixed verified ledger cut, but enabling
serving still requires this online restore barrier. Test restoring old data
**and an old ledger** while the witness retains a later erasure: restore must
refuse until the missing entries are fetched and applied. Erasure removes live payload keys, objects, renders, search rows
and cached copies; storage versions and backups need an explicit purge schedule.
Deleting a live key is not cryptographic erasure if an old backup can restore
it. Until backup purge completes, disclose that window and require the current
ledger before any restore serves traffic. Do not call a separate prefix or
soft deletion erasure.

Private events go through `projection::apply` for both live ingestion and
rebuild. Add audience-aware private tables, bump `PROJECTION_VERSION`, and
version the archive format with compatibility fixtures. Credentials, worker
leases and refresh state require service migrations. Derived inventory and
verified checkpoints must be reconstructible from committed archive evidence;
service scheduling may be rebuilt from those checkpoints, not vice versa.

### Per-reader serving and revocation

A sync credential says nothing about the current reader. Authenticate the
reader from the AppView session, never a query DID. Maintain a separate lease
keyed by `(reader DID, Space URI, generation)`, with authority identity,
verification start time, local invalidation generation, credential JTI and
credential expiry. The first implementation establishes a lease by a fresh
member-specific delegation and credential mint, which checks current access at
the authority. Reusing an hour-long credential is not a fresh check.

For each actively reading reader/Space pair, this costs one new binding key,
one OAuth-authenticated PDS delegation round trip and one tenor mint round trip
per renewal, at least once per 60 seconds while continuously served. Session
refresh, nonce challenges and registration add work. At 1,000 active pairs,
that is approximately 16.7 mints and 33.3 base HTTP requests per second, before
retries. These are arithmetic estimates, not measured capacity. Instrument the
whole renewal latency and authority request rate. Revisit with a dedicated
authenticated access-check query if renewal p95 exceeds 500 ms or mint traffic
exceeds 10 requests/second per tenor for 15 minutes under representative load.
Do not lengthen the freshness bound to hide cost. The lease adapter consumes a
verified result and start time, not a fixed number of network hops; another
verification transport must preserve the same invalidation and freshness rules.

Capture the serving clock at verification **start**, not response arrival;
network delay must not extend permission age. Before installing the result,
compare the invalidation generation captured at start with the current one.
A removal observed during verification must defeat that older successful
response. Discard restored leases at startup and on clock discontinuity.

At query time, first require a live local session, a usable Space sponsor and
the current generation. Check credential expiry. Then feed this reader's lease
as `Verified { at_millis, revoked }` to `spaces::serving::decide`, with the
current serving clock and `DEFAULT_BOUND_MILLIS` (60 seconds). Only `Serve`
permits output. Missing, future, expired or revoked verification denies output;
a failed renewal never extends the old timestamp. Coalesce concurrent renewals
per reader/Space, with bounded waiters and timeouts.

Known authoritative denial, membership/access removal, Space deletion or
credential revocation invalidates affected leases immediately. An uncertain
policy change invalidates all potentially affected leases until checked again.
Do not infer reader revocation solely from another sponsor's JTI. The standard
credential-revocation notification targets repo hosts; it is not a guaranteed
AppView reader-ACL feed. Verified authority changes and AppView-mediated changes
provide early invalidation; fresh reader checks enforce the bound when notices
are lost. Bob's ongoing sync must never preserve Alice's access.

Persist invalidation generations centrally and check them at output time across
instances; an in-process cache alone is insufficient. Serialize a lease install
against invalidation. Recheck before each streamed chunk or queued delivery,
and close live subscriptions on invalidation. Bytes already delivered cannot
be recalled; the guarantee concerns later output after observed revocation.

Filter authorized audiences in SQL **before** counting, ranking or pagination.
Apply the same gate to historical versions, media, exports, downloads, search,
SSE replay and queued notifications. Public endpoints remain public-only.
Private responses use `Cache-Control: private, no-store`; no public CDN or
bearer media URL that outlives access. Switch reader identity on every account
change. Return a non-enumerating refusal for unknown/denied private resources;
report stale authorization as unavailable without disclosing their contents.

### Reconciliation and failure bounds

Notifications are authenticated, contentless hints. Persist a bounded dirty
repo task before acknowledging receipt; no receipt promises content was
projected. Verified sync checkpoints advance only after encrypted archive
objects, log envelopes and projections commit. A crash resumes from archived
checkpoints, with idempotent event identities.

Reconcile at startup, after notification gaps or outages, on membership changes
and periodically. Page through the durable candidate inventory, including
never-writers and former writers, independently of `listRepos`. The tenor runs
the equivalent discovery and verified read path to admit independent clients'
posts; the AppView cannot invent admissions. A first post during a 24-hour
notification outage must be found even if no notification ever registered its
writer. Current permission still governs admission after the outage.

A former writer's verified deletion removes the displayed post without waiting
for a tenor closing record. Verified full-state absence can tombstone a known
record. Timeout, access refusal, `RepoNotFound` or account migration ambiguity
is not verified deletion; retain the last verified state as stale and retry.
A delete and byte-identical recreation hidden entirely inside a lost oplog
cannot be reconstructed from a snapshot. Mark that history gap unresolved;
do not assert that the old admission authenticates an unobserved recreation.

Bound concurrent fetches, bytes, staging disk, pages per job and retry rate.
Persist continuation and use fair scheduling across Worlds. On quota exhaustion,
pause affected sync visibly rather than dropping inventory or acknowledging
unpersisted work. Never remove a former writer merely because a retry budget
expired. Retire inventory only when no surviving contribution or retained
history depends on it, with a durable retention/erasure decision. Sweep abandoned
staging objects only after excluding objects referenced by committed envelopes.

### Rebuild, recovery and the delegation-origin limitation

Rebuild from a fixed archive cut, format/rules version and current erasure
ledger, with no network and no side effects. Build a fresh private projection
alongside public projections and swap only when complete. Rebuild restores
content and historical access facts, **not** usable credentials or fresh reader
leases. Removed readers remain denied after rebuild. Without live checks,
private content can be rebuilt offline but cannot be served offline.

Owner recovery is not an AppView rebuild. Export the tenor's privileged Space
and historical-writer inventory, audience generations, recovery epoch,
authoritative decisions and policy-permitted retained copies under the owner's
export encryption. The AppView may supply archived evidence to an explicitly
authorized recovery flow, never treat its cache as a new World authority.
Recovery on a fresh tenor rotates keys, reacquires credentials and reconciles
surviving repos. Old leases are invalid. An older export declares a new recovery
epoch; unresolved admissions remain unresolved and possibly used roll slots
must not reopen. With both the old tenor and an author's PDS gone, recovery
can return only policy-permitted copies included in a surviving export.

Direct member delegation avoids losing the member identity on the AppView path.
It does **not** fix World-issued delegations to another authority. Before private
release, refuse issuance of those remote World-origin delegations in the tenor,
and retain local origin-bound issuance only where the binding is enforceable.
A self-hosted tenor that is both World authority and Space host remains supported;
this restriction concerns cross-authority delegation, not self-hosting.

Portable group-to-group delegation needs an upstream-compatible authenticated
origin and revocation mechanism. Neither a request-body member DID nor a local
AppView allowlist fixes a remote host. Supporting that topology is deferred,
with an explicit refusal rather than best-effort revocation.

## Rejected alternatives

- **App-only, owner-wide or World-wide read credentials:** bypass member expiry,
  expand compromise scope and make Alice's access depend on Bob's activity.
- **Trust cached membership projections for live access:** rebuilds and delayed
  sync would restore permissions removed at the authority.
- **Use notifications or `listRepos` as the complete inventory:** misses first
  posts during outages and deletions by former writers.
- **Publish private envelopes as public `SpaceRecordEvent`:** existing consumers
  assume those snapshots are safe to serve anonymously.
- **Separate prefix without encryption or erasure-ledger support:** object
  backups could silently recover content that was meant to disappear.
- **Treat STAR MACs as permanent public signatures:** contradicts Spaces'
  deniable authentication model.
- **Implement incremental sync first:** increases verification and crash-state
  complexity before there is a private end-to-end path.

## Owner decisions before acceptance

1. Approve direct member delegation and fail-closed service when the final
   usable session disappears. Existing members must re-consent with Space scopes;
   this is a visible product change, not a transparent session upgrade.
2. Approve the cross-authority World-delegation restriction. Do not promise
   secret casts or portable group-to-group delegation in First World.
3. Confirm the 60-second maximum unknown-revocation window. Shorter bounds
   increase authority traffic; future dedicated access-check queries need their
   own authenticated, versioned contract.
4. Choose archive key custody, backup purge deadlines, quotas and retention
   disclosure. Decide whether cached copies survive final-session loss, for how
   long, and who may request their deletion. Default to tombstones, not retained
   deleted prose. The full erasure design costs a separate bucket/account,
   witness service, keys, restore barrier and purge operations. An alternative
   closed-alpha default is to retain deletion tombstones and defer that ledger
   machinery until before open beta. That alternative cannot promise erasure
   from historical backups or safe automatic rollback restores. It requires
   explicit retention disclosure, no automatic restoration to serving, and a
   reviewed manual purge/restore process for required erasures. Legal erasure
   duties do not wait for beta; if that process cannot meet them, real private
   data must wait for the full design. This alternative does not mark slice 6's
   compaction/erasure acceptance row complete.
5. Choose a measured reconciliation interval and outage recovery objective.
   Suggested starting interval: five minutes, not a demonstrated service level.
6. Confirm release gates: tested encrypted owner export/restore and breaking-alpha
   migration support remain prerequisites for real private play, alongside risk
   disclosure. Completing ingestion alone does not satisfy those gates.

## Revisit when

Spaces reaches its reviewed production release; upstream exposes portable
origin-bound delegation; repeated per-reader minting becomes a measured cost;
or private snapshot sizes justify verified incremental sync. Revisit storage
when compaction, erasure rules or multi-node deployment change its assumptions.

Implementation order and test obligations are in
[the private Spaces plan](../architecture/private-spaces-plan.md).
