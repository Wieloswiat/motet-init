# 0024. Durable dice results and single-use opportunities

Status: Proposed

## Context and checked implementation

First World needs durable signed dice, not a browser animation. The plan's
[Dice contract](../plan.md) distinguishes issuer authenticity
from fair randomness: the tenor sees results first and remains trusted.

The current implementation provides these parts, not an issuance flow:

- `crates/dice` parses integer expressions and evaluates through `Source`.
  `GRAMMAR_VERSION` is `motet-dice/1`; `Limits` bounds generated values and die
  sides. Explosions count toward the value bound; arithmetic overflow is an error.
- `tenor::dice::OsSource` uses OS randomness and rejection sampling, not biased
  remainder sampling. Its entropy failure currently panics, and `Source` is
  infallible. Neither behavior is a durable issuance policy.
- `rules::world` has admission idempotency and epoch-scoped places, but **no roll
  commands, opportunities or slot state**. Generic operation retries in
  `world_properties.rs` do not establish restricted-roll retries; the acceptance
  table now explicitly marks roll slots as unmodelled.
- Draft `roll.request` pins an optional opportunity but has no opportunity
  lexicon. Draft `roll.result` records faces, kept flags, total or failure, but
  lacks audience, slot, recovery and signature fields. Generic World writes to
  roll results are already forbidden by `tenor::authorization::protected`.
- `tenor::shard` logs effects before SlateDB and before returning outcomes.
  `publication.rs` captures the audience, refuses stale Space generations and
  commits publication receipts atomically. Its current intents only cover
  admissions/closings; dice must extend them, not claim existing support.
- atproto-crates at `09a868cdde938c789326f1db16ffc30bbf38c15e` supplies
  `atproto-attestation`. Its CID-first inline signature binds record content,
  metadata and repository DID. `verify_record` alone does not establish that
  the claimed issuer owns the signing key; issuer/key binding is a separate check.

This builds on ADRs [0014](0014-tenor-state-in-slatedb.md),
[0018](0018-world-service-auth.md), [0021](0021-single-use-member-tokens.md)
and proposed [0022](0022-private-space-ingestion.md). No runtime behavior is
changed or claimed tested by this proposal.

## Decision

### Requests, opportunities and authority

The signed-in member asks through the AppView; independent clients use the same
tenor procedures. The AppView obtains a fresh exact-method, World-audience
member service token. The tenor takes the actor from the verified issuer and
consumes its JTI in the shard transaction, as ADR 0021 requires. A retry obtains
a new token but retains the request identity and operation ID.

A request is an immutable record in the author's public repo or the scene's
Space repo. Extend its draft lexicon with creation epoch and explicit audience
identity; these must be in the read-back record, not supplied only as RPC hints. The AppView writes it create-only and reconciles ambiguous PDS
responses before asking the tenor to issue it. The tenor reads back the exact
CID and checks author, World, scene and audience. A stream or notification may
also discover requests, but uses the same verified read-back and issuance
path, never a separate roller. Private writes depend on ADR 0022's conditional
personal-PDS write prerequisite and verified private ingestion.

Add a World-authored opportunity record, opened only by a permitted GM procedure
in First World. Versioned game rules may open opportunities later through the
same transaction. An opportunity binds World, scene/audience, recovery epoch,
subject action's strong reference, actor, slot name, canonical expression,
grammar, rules version and character-sheet strong reference (or explicit
absence). One opportunity represents one slot. The unique restricted-slot key
is `(World, action identity, actor, slot name)`; an epoch is provenance, not a
way to reopen that same action after recovery. A new legitimate attempt needs
an explicit new action or adjudication record linked to the old one.

The opportunity is immutable. Changing its expression or context requires an
explicit cancellation/supersession decision, never an in-place edit or deletion
that resets the slot. A request cannot open an opportunity. Its pinned opportunity
must match every bound field. Free-form rolls have no opportunity, are labelled
free-form, and cannot satisfy a restricted action later. In First World, any
current, unsuspended cast member may request a free-form roll in an open scene;
only the bound actor can consume a restricted slot. Check these facts in the
same shard transaction that reserves the request, not just before PDS I/O.

Bind the stable request key to `(audience identity, author DID, collection,
rkey)` and freeze its first accepted CID. Rewriting that rkey or changing an
operation's payload is a conflict, not a fresh roll. Deleting and recreating a
request does not reset its receipt. Preserve request/slot receipts through
compaction for as long as replay or action references can reach them. Bound
admission rate and queue size; reject overload before acceptance rather than
evicting deduplication state.

For a restricted slot, the first eligible request reserves it. A different
request cannot draw again: return a durable refusal pointing to the winning
request when the caller may read it. Thus every accepted request has one terminal
result, and every restricted slot has at most one accepted roll outcome, either
rolled or failed. Unauthorized, malformed-envelope and competing requests receive
refusals, not invented dice results. A refusal never exposes private slot details
to an unauthorized caller.

### Durable state machine and timeout semantics

Keep the deterministic transition policy in a small `rules::dice` module; the
tenor owns randomness, signing and persistence. Persist dice state as shard
`Write` effects together with its indexes and publication intents. Never put an
OS draw in a replayed `Entry::Apply` command.

The state machine is:

1. **Accepted:** atomically reserve the request, restricted slot, fixed result
   rkey, audience/generation and input versions. Store the operation receipt.
   No faces have been drawn. A restart can resume this state.
2. **Drawing:** the one fenced worker durably claims the accepted request before
   invoking entropy. No other worker or retry may draw it. Evaluate once with
   bounded limits; append the exact complete outcome or failure as effects.
3. **Outcome recorded:** preserve all result fields, timestamps and generated
   faces. Signing and publication may retry these bytes; evaluation may not.
4. **Published:** use the World signing key to attest the fixed result and commit
   it to the captured public repo or private Space. Store signed bytes, repo/Space
   effects and publication receipt atomically in the shard. Only after that log
   commit may an endpoint return a terminal result and its exact reference.

A request stranded in `Drawing` after process loss has an unknown computation,
not permission to roll again. After fenced takeover proves the old worker cannot
commit, record a terminal `interrupted` failure without sampling. The slot stays
consumed. This conservative policy trades an occasional lost roll for no retry
fishing. A client timeout does not itself produce that failure or cancel work;
it returns **pending** and queries the same request until the server resolves it.
A live worker must not be declared lost merely because an HTTP request expired.

Reconcile ambiguous log appends with `motet-log` before takeover or further work.
If durability is unknown, stop the shard; never assume the draw was not recorded.
If signing, storage or publication is unavailable after evaluation, keep the exact
outcome pending publication. Do not publish a new failure to replace a successful
but unpublished outcome. Audience deletion or incompatible recovery can leave
publication blocked; expose only a non-content status to authorized callers, not
a public fallback. Membership removal after acceptance does not erase the
reserved outcome, but current audience authorization still controls its reads.

### Signed result, publication and verification

Use one inline World attestation for **both** public and private results. Public
repo commits already authenticate published results; the extra result signature
makes exported copies verifiable without carrying an MST inclusion proof. Private
Space commits are deniable MACs, so they do not supply portable issuer proof.
This intentionally makes the World's private dice statements non-deniable to
someone holding the result; it does not add author signatures to private posts.
Never publish a private attestation or its contents into the public repo.

Extend the result lexicon to include World, scene, exact audience/Space identity,
request strong reference and author, opportunity/slot if restricted, authority
revision, recovery epoch, result rkey, grammar, canonical expression, algorithm
version, limits policy version, complete outcome and inline signatures. Bind
these fields in the content signed by the pinned CID-first attestation format;
metadata names the World issuer and explicit public signing key. Use the World
repository DID as the attestation repository. The signing key is the tenor-held
World account key (`keys::Purpose::Signing`), sealed under the tenor master key.
It is not the owner's PLC rotation key. The owner never sends that rotation key
to the tenor or signs each roll; it authorizes account control and key rotation.
The AppView holds neither World key. Do not sign a total alone or an
unversioned JSON serialization. Specify a new Motet attestation type and its
allowed schema, and regenerate lexicons rather than hiding fields in extras.

The full signed record's repo CID differs from the attestation's content CID.
Preserve both correctly; never make the signature depend on its own final CID.
Include explosion provenance alongside faces and kept flags so a verifier can
recompute the trace and total. The existing draft result already has `kept`;
it lacks `exploded` provenance.
Success/failure shapes must be exclusive, with bounded groups and values.

Extend `publication.rs` with a dice intent using its existing captured-destination,
receipt and fenced-transaction pattern. Do not disguise a roll as an admission.
Use the rkey allocated at acceptance and the stored outcome, not a new timestamp
or expression at each attempt. An unknown or stale audience publishes nowhere.
Keep request/outcome provenance even if the author's request later disappears.
Readers order dice by a durable World event sequence within the recovery epoch,
not client time. Do not silently consume post-only admission place numbers;
define a scene event sequence/cursor when adding interleaved dice to the UI.

AppView ingestion verifies the exact record CID, the inline signature, World
issuer/key binding and audience before archiving. Require a verified World
signature, not merely a nonempty signatures array or an arbitrary valid key.
Recompute grammar/trace/kept decisions/total independently; reject unknown
algorithm/grammar versions as unsupported, never reinterpret them with defaults.
A failure contains a typed code and no fabricated total. Repository provenance
and opportunity matching still matter: a copied valid signature does not grant
access, create a second slot result or move a roll to another scene.

At live ingestion, resolve the World and verify its authorized signing key.
Retain the resolved identity/key provenance and exact signed record in the
archive. Current-key lookup alone will fail after migration rotates that key.
Offline rebuild trusts previously verified archive evidence, as ADR 0007 does;
it must not pretend the result timestamp proves historical key ownership.
The tenor installs the World signing key as PLC's `#atproto` verification method
in `plc_genesis.rs`. Starting from a stated trusted checkpoint, the accepted PLC
audit history establishes the key's authorization interval. A historical verifier
checks the attestation's signed creation time against that interval and verifies
the World/key binding. Directory time and accepted operation history are trusted
inputs; a signer-controlled timestamp is not independent evidence of signing
time and cannot settle a compromised old key's backdated claims.

Independent clients can fetch the signed record through ordinary public sync
or an authorized Space read and verify current issuer binding locally. Rotation
or migration does not change old attestation bytes or invalidate their signature
math. Preserve old public keys and the verified provenance already observed at
ingestion; new results use the newly appointed tenor's World signing key. Never
silently replace old signatures with new ones. “Signature valid, historical
issuer binding unavailable” is a fallback for missing directory/history evidence,
not the normal migration outcome; an embedded `did:key` alone is insufficient.

First World preserves and restores exact committed results and their trusted
archive provenance. A self-contained export verification bundle, packaged PLC
history/checkpoints, external profile fixtures and independent historical verifier
are post-First-World work in slice 5. Do not imply that a milestone backup is
already a third-party historical verification bundle.

A signature authenticates the World's statement. It proves neither unbiased
operator behavior nor that the operator never discarded unseen draws. Keep
`os-csprng` and a versioned evaluation/sampling policy explicit; do not advertise
verifiable randomness. No random seed is exported or used to regenerate results.

### Limits and failures

Validate HTTP/body and record-size bounds before accepting a request. Once an
authorized, well-formed request is accepted, invalid notation, unsupported dice,
exceeded face/side/group/output limits and arithmetic overflow produce one signed
terminal failure. For a restricted opportunity, validate its expression at
creation too; runtime explosion exhaustion can still fail and consumes the slot.
A corrected attempt requires an explicit GM decision and new action, not a retry.

Persist the effective limits policy with acceptance. Enforce a total generated
value cap, result group/byte bounds, parser depth/length and integer wire limits,
not just the existing per-array lexicon limits. The effective total generated
value cap must be no greater than both the enforced `Limits::max_values` and
the result lexicon's faces/kept `maxLength`: First World caps all three at 1,000.
Require at most 64 groups and equal faces/kept/exploded lengths per group.
These are cross-layer invariants, not permission to allocate 1,000 values for
each of 64 groups. Test boundaries through the encoded result consumer.
Add a bounded entropy-word budget
for rejection sampling; never replace rejected samples with modulo bias. A
fallible source/evaluator path must report OS entropy failure and budget
exhaustion without panic or partial successful results. This requires changing
the current infallible `Source` seam deliberately and retaining unbiased-sampling
tests. Infrastructure failure before durable acceptance remains retryable; failure
after drawing starts must never create another entropy attempt for that request.
This deliberately includes an entropy error before the first accepted face: it
permanently consumes a restricted slot. Such an error does not itself enable
fishing, but retaining one terminal rule avoids relying on volatile evidence of
whether any face was drawn before a crash. A bounded zero-face retry would need
a separate durable attempt counter and precisely specified crash semantics.
For First World, choose the simpler conservative rule and disclose its cost.

### AppView, UI and recovery

Add request/create-status procedures, an AppView outbox kind and session-scoped
result queries. The outbox keeps request rkey, CID, operation ID and pending
status across lost responses. A retry button resumes that request; “new free-form
roll” is a separate deliberate action. Persist pending state in the browser so
navigation or reload does not issue again. Render pending, rolled, failed,
publication-blocked and recovery-unresolved states distinctly. Show expression,
faces/kept trace, total, grammar and World verification status. Do not animate a
new outcome locally or mark an unverified HTTP response as final.

Index only World-authored exact results, joining requests/opportunities by
Space-aware identity. Public results travel through public ingestion; private
results require ADR 0022's verified archive and per-reader gate, including
historical versions and any later downloaded proof bundles. Projection changes bump
`PROJECTION_VERSION`; outbox/service changes get migrations. Rebuild performs no
entropy calls, issuance, signing or notifications.

Export all opportunities, slot reservations, immutable request identities,
Drawing markers, exact outcomes/signatures, publication intents/receipts,
identity evidence and recovery epoch. A complete planned move transfers this
state before the new writer starts and returns identical committed results.
An old client's query uses its original identity even after migration.

An older export cannot establish that an apparently open slot was unused.
Recovery declares a new epoch and freezes every pre-checkpoint opportunity whose
post-checkpoint fate is not evidenced, including “open” ones in the export.
Reconcile the old log, published World records, permitted AppView archive copies
and submitted signed receipts. A recovered authenticated result consumes the
original slot; never sample it again or re-sign it as if newly rolled. Publish a
recovery decision referencing it if the new epoch needs a record. Conflicting
signed outcomes stay disputed; do not pick the more favorable one.

With no surviving evidence, keep the old slot `recovery-unresolved`. A GM may
adjudicate the game consequence with an attributed decision, not silently clear
the slot or turn uncertainty into a new RNG result. New post-recovery actions
are explicit and linked to that adjudication. Old-epoch requests absent from the
export remain unresolved rather than becoming fresh free-form rolls. The API
must bind requests to their creation epoch; a recovery epoch is not a deduplication
reset. There is no promise to reconstruct an unexported result when every copy
is gone.

## Ordered implementation slices

Each test must name its property and plausible bug, following
[testing.md](../testing.md). These are obligations, not executed evidence.

### 1. Pure request/slot transitions (one to two agent-days)

Add `crates/rules/src/dice.rs` and focused property tests. Model reserve, begin,
record outcome, publication receipt and recovery freeze without RNG, HTTP or
storage. Do not expand the general admission command enum merely to inject
randomness. Test that many distinct requests for one slot yield at most one
accepted outcome, retries preserve the receipt, changed payloads conflict and
an interrupted/recovered slot cannot reopen. Bugs: deduplication only by operation
ID, or failure clears the slot. No running stack. Supplies the core of “Many
requests for one restricted roll slot”; does not close the end-to-end row.

### 2. Durable issuance and signed publication

Own tenor dice operations, store keys, shard effects, authorization, publication,
new opportunity/procedure/attestation lexicons and updated request/result
lexicons; run `lexgen`. Add a fallible bounded source seam in `crates/dice`.
Test real signed member tokens and recorded effects through the shard: concurrent
requests, removal before acceptance, forced entropy error, process loss in
Drawing, log-append uncertainty and publication retry. Assert no second source
invocation after a durable Drawing claim and identical bytes after replay.
Bug: replay rerolls or retries regenerate result metadata. Add public/private
signature tamper, fake-issuer key and wrong-audience tests. Uses real S3 for fault
cases; pure crypto/transition tests need no stack. Closes the tenor portion of
the restricted-slot row; private serving still waits on ADR 0022.

### 3. AppView and visible roll journey

Own AppView auth scopes, writes/outbox, API, ingestion/archive and projections;
web composer/status/result UI; production Playwright journey. Test a lost HTTP
response and page reload while a roll completes: the reader sees the same signed
result, not a second request. Bug: client generates a new rkey on retry. Two
clients race one opportunity and see one winner. Private results remain
inaccessible after reader revocation; exported proof-bundle checks belong to
slice 5. Tampered outcomes never
appear as verified. Use PostgreSQL/S3 and the full TLS/PDS/tenor/web stack.
Closes the full restricted-slot row once both public and private paths run.

### 4. Export, migration and older-checkpoint recovery

Extend the owner export/recovery format, not the browser key-backup file.
Add isolated acceptance tests for “Return a roll, lose the client, migrate” and
“Restore an export older than a completed roll”. Stop the old tenor after a
result was shown; restore current and old exports separately. The first returns
the identical result. The second recovers the old signed result if evidence
survives, otherwise leaves the original slot unresolved. Rebuild with network
and entropy disabled. Bugs: resetting a slot on new epoch, omitting pending
publication from export, or replacing an old result's signature after migration.
Needs the running stack and isolated fresh storage; never destructive tests
against real Worlds. Owner export work is a dependency, not part of the
one-to-two-day slice. No independent third-party key-history bundle is required
to close these milestone rows.

### 5. Portable historical verification bundles (after First World)

Package PLC operation history, trusted checkpoints, signing-key intervals,
exact records and profile fixtures in an optional export verification bundle.
Add independent historical verification instructions/tooling and an authorized
AppView download path. Test old signatures across key rotation with a separate
consumer, including a claimed creation time outside the key's PLC interval,
incomplete history and a mismatched checkpoint. Bug: accepting any valid
`did:key` as the World or requiring its current key for an old result. Test
private bundle denial after revocation through the real API. Signature validity
must not be presented as independently established signing time or fairness.
Crypto/fixture tests need no stack; download and migration tests need it. This
extends portability beyond the milestone's identical committed-result promise.

## Rejected alternatives

- **Client rolls or optimistic totals:** cannot provide World-issued durable
  outcomes and allow retries to select a favorable result.
- **One result per operation ID only:** changing the ID bypasses restricted slots
  or rerolls an edited request. Preserve request identity and slot uniqueness too.
- **Randomness inside replayed rules commands:** recovery would change outcomes.
- **Commit a sealed seed at acceptance, then evaluate deterministically:** this
  is a viable alternative, not equivalent to rerolling. It removes Drawing and
  interrupted failures and can replay without fresh entropy. It also introduces
  a pinned seeded PRNG/byte-consumption algorithm, versioned evaluator and limit
  semantics, golden compatibility fixtures, and recovery custody for a long-lived
  seed until an exact outcome commits. Seed custody must survive key rotation
  and export for pending requests without exposing future outcomes. The current
  direct-OS Source supplies none of that contract. First World instead stores
  exact outcomes and accepts rare interrupted failures, keeping historical
  recovery independent of executable evaluator compatibility. Revisit this
  trade-off if interrupted failures are operationally significant; committing a
  seed alone is not proof of fair seed selection.
- **Return after drawing or shard reservation:** durable reservation is not a
  published result. Return pending until signed repository publication commits.
- **Reroll after timeout or interrupted execution:** converts uncertainty into
  another chance. Interrupted execution consumes a failure instead.
- **Public commit proof only:** needs retained inclusion proofs for portable
  exports and supplies no signature for private Space results.
- **Separate public proof record for a private roll:** leaks private context;
  use an inline attestation in the same audience.
- **Commit/reveal, external beacon or VRF now:** broader trust and availability
  design than First World needs. A signature alone is not any of these.

## Owner decisions before acceptance

1. Approve GM-opened opportunities first, free-form cast rolls separately, and
   explicit versioned game-rule automation later. Which concrete action/sheet
   references should the initial game UI expose?
2. Approve failed/interrupted rolls permanently consuming their restricted slot,
   including OS entropy failure before any face is drawn. This conservative
   policy chooses a simpler crash contract over zero-face retries or committed
   seeds. A GM may adjudicate, but “try again” cannot erase a failure or unknown
   prior outcome.
3. Approve portable World signatures for private dice: a recipient can show
   the signed result to outsiders. Ordinary private posts retain Spaces'
   deniable authentication model.
4. Choose effective limits, per-member queue/rate quotas and a UI wait budget.
   Suggested UI budget: ten seconds before switching to pending polling; it is
   not a cancellation deadline. Measure before claiming a latency objective.
5. Confirm older-export recovery stays unresolved without evidence, even when
   that delays play. Choose the GM adjudication workflow and export frequency.

## Revisit when

A game needs automatic opportunity issuance; measured shard latency requires an
external evaluation worker; or users require independently verifiable randomness.
Any worker must preserve the durable Drawing claim and single outcome. Revisit
the signature profile on an atproto-attestation upgrade, with fixed fixtures and
historical issuer-key verification tests before changing the pin.
