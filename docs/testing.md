# Testing

Motet's correctness lives in a few promises: every consumer agrees on what a
World admitted, a replay gives back exactly what was acknowledged, nothing
unauthorised reaches a handler, projections rebuild from the archive. Tests
state those promises directly and try hard to break them. This document puts
[ADR 0005](adr/0005-testing.md) and [ADR 0011](adr/0011-flaky-tests-fail.md)
into practice.

## What a good test here looks like

- **A property, not an example of the implementation.** "Any delivery order
  shows the same scenes" rather than "`apply` inserts a row". If a test would
  pass for any implementation that compiles, or only restates the code, it
  doesn't belong.
- **Through the real boundary.** Service auth is tested with real keys and
  signatures; the XRPC dispatch order through a real router; the log against
  a real bucket; projections against real PostgreSQL.
- **Adversarial inputs from generators.** `proptest` explores orders,
  retries, failures and inputs; `motet-testkit` provides delivery orders and
  service fixtures so each crate doesn't reinvent them.
- **Mutation testing checks the tests.** `mutants` reports code that can
  change without any test noticing.

## Admitting a test

A test must be able to catch a bug that could really happen. Before you add
a test or an assertion, write down two things in its name or doc comment:

1. The property it protects: a promise that users, another part of Motet or
   an ADR relies on.
2. A plausible bug or mutation that makes it fail. For example: "if `apply`
   decided a retried operation again, the retry could get a different
   outcome".

If you can't name such a bug, don't add the test. Reviewers ask for both.

These tests are not admitted:

- **Source-text assertions**: reading a source file and checking that it
  contains a call, comment or declaration. Generated code also needs a
  consumer contract or semantic property, not snapshots of formatting.
- **Restated defaults and constants**: asserting that a constant or a default
  equals its literal. If other systems rely on the value, test the behaviour
  at that value instead.
- **Wiring**: asserting that a router has a route, that a struct has a field,
  or that one function calls another. Test what a caller sees through the
  wiring: `xrpc/tests/server.rs` sends requests through a real router and
  counts the handler calls.
- **Mock echoes**: telling a test double to return a value and asserting that
  the value comes back. A double may stand in for a boundary only when the
  assertion is about Motet's own decision. In `tenor/tests/admission.rs`, a
  fake repository stands in for the author's PDS, and the tests assert what
  the shard admits.
- **Tautologies**: assertions that pass for every implementation that
  compiles.

An exact expected value is fine when the value is the contract: the
canonical form of a dice expression, a wire format, the error variant that a
caller matches on.

## Property tests

Low-level code gets property tests: parsers, the rules' state machine,
ordering and merge logic, bounds arithmetic, access decisions. A property
test needs three things.

**Generators that cover the input domain.**

- Boundaries: generate the whole domain of the input type, not only what a
  well-behaved caller sends. Include zero, the maximum, empty input,
  duplicates, and timestamps in the past and in the future. If the code has a
  precondition, generate values on both sides of it, and check that the code
  refuses the wrong side.
- State: generate sequences of operations and apply them to the real
  implementation, as `rules/tests/world_properties.rs` does with commands.
- Order: generate delivery orders, retries, redelivery and interleavings.
  `motet-testkit::delivery` has the strategies.

**An oracle that is independent of the code under test.** Use one of these:

- a round trip: the canonical form of a dice expression parses back to the
  same expression;
- an invariant: every roll total falls inside the computed bounds;
- a metamorphic relation: any delivery order shows the same scenes;
- a small model in the test: `world_properties.rs` tracks the authority facts
  from the commands alone and checks each outcome against them.

Don't compute the expected value with the function under test, and don't
copy its algorithm into the test.

**Reproducible failures.** proptest shrinks a failure to a small case and
saves its seed. Integration-test sources use `*.proptest-regressions` files
beside the test; unit tests use the crate's `proptest-regressions/` directory.
Commit these files. proptest replays the saved cases first on every run.
For sources under `tests/`, the default `SourceParallel` policy reports that
it cannot find `lib.rs` or `main.rs`, then uses the beside-source fallback.
That warning does not disable saving or replay.

**Case counts.** Each suite sets its default with
`ProptestConfig::with_cases(n)`. If `PROPTEST_CASES` is set, it replaces that
default for every `proptest!` test. `nix flake check` sets it to 256, so every
property runs 256 cases there, with a new random seed each time. `soak` runs
the default suites in release mode with 20000 cases, or the number you give
it.

### TypeScript properties

Use [fast-check](https://github.com/dubzzz/fast-check) with Vitest for pure
TypeScript logic. Generate inputs at boundaries and assert a mathematical,
domain or metamorphic property. Do not copy the implementation into an oracle.
The design package checks contrast symmetry, bounds and reference values,
alongside the real themes' accessibility requirements.

Normal runs vary the seed. On failure, retain the reported seed and shrink
path so the smallest counterexample can be replayed. Mutation runs use a
fixed seed: each mutant must face the same inputs. Do not use only a fixed
seed in ordinary CI or add generators to a test that merely echoes its input.

The design suite runs 500 cases per property. To replay one failing case:

```sh
FC_SEED=<seed> FC_PATH='<shrink path>' pnpm --filter @motet/design exec vitest run src/contrast.test.ts -t '<test name>'
```

Unset both variables for ordinary runs. The mutation script pins `FC_SEED`
only for its own process.

### Web testing layers

Follow the [Svelte testing guidance](https://svelte.dev/docs/svelte/testing)
and [Sveltest's setup](https://sveltest.dev/docs/getting-started), with this
document's stricter test-admission rules:

- **Vitest Node tests:** pure functions and server boundaries. Use real
  `Request`, `Response` and `FormData` values. Do not mock a module merely to
  assert that its configured return value came back.
- **Vitest Browser Mode:** Svelte components in real Chromium through
  `vitest-browser-svelte`. Use accessible roles, real keyboard/pointer input,
  awaited assertions and automatic cleanup. Keep client and server test
  projects separate.
- **Playwright:** complete journeys against a production build, including
  CSP, hydration and visible behavior. Assert computed appearance when
  appearance is the requirement, not `data-theme` or generated class names.
  A passing component test does not establish full-page integration.

The public World and scene journey uses the AppView. The other design-export
screens remain demos; theme, CSP and demo-load checks do not establish
authentication, posting or admission. The Playwright journeys exercise those
behaviors against the HTTPS stack.

## Regression tests

A bug fix comes with a test that fails without the fix. The test states the
observable behaviour that the bug broke: what a reader saw, what a replay
returned, what a caller was allowed to do. It doesn't pin the line that
changed.

If a property test could have found the bug, widen its generator so that it
finds it. The saved seed then keeps the case. Add an example test only when
the case is worth reading on its own. A bug that can't come back, such as a
typo in a log message, needs no test.

## Suites

| Suite       | Runs                                                                                 | Needs                                            |
| ----------- | ------------------------------------------------------------------------------------ | ------------------------------------------------ |
| default     | `tests`; every change; `nix flake check`                                             | nothing                                          |
| integration | `tests-integration`; `devenv test`; CI                                               | `devenv up` (PostgreSQL, S3)                     |
| network     | `network-smoke`; `devenv test`; CI                                                   | `devenv up` (PLC, relay, Tap, Rust PDS, AppView) |
| web         | `pnpm test` (vitest in Chromium), `pnpm test:e2e` (Playwright on a production build) | Chromium (the dev shell provides it)             |
| soak        | `soak [cases]`                                                                       | nothing; slow                                    |
| mutation    | `mutants` (Rust), `mutants-web` (TypeScript/Svelte); weekly in CI                    | Chromium for browser mutations                   |

Integration tests are ordinary `#[test]`s marked
`#[ignore = "integration: …"]`, so `cargo test` stays fast and a missing
service is a skip with a reason, never a pass. Their names start with
`fault_` when they inject faults; those run one at a time
(`.config/nextest.toml`).

nextest runs every Rust suite, in `tests`, `tests-integration`, `soak`,
`mutants`, `coverage` and `nix flake check`. Its profiles are in
`.config/nextest.toml`.

## Flaky tests

The `ci` profile retries a failed test once. The retry shows whether the
failure repeats. A test that passes only on its retry is flaky, and it fails
the run ([ADR 0011](adr/0011-flaky-tests-fail.md)); `junit.xml` marks it
flaky. The integration profile doesn't retry.

A flaky test is a bug, in the test or in the code. Fix the cause. Don't add
retries, sleeps or longer timeouts to make it pass, and don't skip or
quarantine tests to get a green run. To reproduce an intermittent failure,
run the test many times without retries:
`cargo nextest run --stress-count 100 <test name>`.

## Mutation testing

`mutants` runs cargo-mutants with nextest. cargo-mutants changes the code one
mutation at a time, for example by replacing an operator or a return value,
and runs the tests against each change. A mutant that no test catches has
survived.

The weekly Scheduled workflow runs it on the core crates (`motet-dice`,
`motet-rules`, `motet-log`, `motet-xrpc` and `motet-spaces`) with the default
suites. It keeps `mutants.out/` as an artifact, where `missed.txt` lists the
survivors. A survivor or a timeout fails the job. Locally,
`mutants --in-diff <file>` tests only the mutants in the code that a diff
changes.

Triage every survivor:

1. If a bug could hide there, add or strengthen a test that states a property
   and fails on the mutant.
2. If the mutant is equivalent, because the program behaves the same and no
   test can tell, add it to `exclude_re` in `.cargo/mutants.toml` with the
   reason.
3. If the code has no observable effect, delete it.

Do not use a mutation percentage as a quality claim. A high score can reward
tests that pin implementation details. The evidence is the list of survivors
and the reasoning or behavior test for each.

### TypeScript and Svelte mutations

`mutants-web` runs `pnpm test:mutation`. Each package keeps its Stryker
configuration beside its tests and writes HTML/JSON reports under
`reports/mutation/`; CI retains both packages' reports. After every successful
Stryker run, the package command automatically checks the JSON report.
The gate accepts only `Killed`, `CompileError` (unviable) and `Ignored`
(documented exclusions), rejects every unresolved or unknown status, and
requires at least one `Killed` behavioral mutant. Empty, all-unviable and
all-ignored reports therefore cannot claim mutation evidence. This extra gate
is necessary because Stryker counts timeouts toward its percentage and excludes
runtime errors: even a 100% score can hide a run that did not catch mutants.

The design package uses
[`@stryker-mutator/vitest-runner`](https://stryker-mutator.io/docs/stryker-js/vitest-runner/)
for pure TypeScript. That runner does **not** support Vitest Browser Mode.
The web package therefore uses Stryker's command runner to execute the real
Vitest browser component tests. Its test-only bridge passes the active mutant
to the browser. Never count a run as mutation evidence unless a known
behavior-changing mutant actually reaches the browser and is caught.

The web scope is the component instance script's initial selection and
value-change handler. Stryker 10.1.1 interprets Svelte ranges relative to each
script and skips the catalogue's `as const` expression as a TypeScript type
node; this gate does not claim module-catalogue or template mutation coverage.
Panda style declarations stay outside the range so instrumentation cannot
change static CSS extraction. A client test renders the real home page and
checks its painted colours: removing the handler body must fail that test,
not merely change a callback count or a `data-theme` marker.

Local state and paint assertions use an explicit one-second polling budget,
in ordinary client tests and mutation runs alike. Installed Browser Mode
otherwise extends a failed assertion to the remaining 15-second test
deadline, which can outlast the mutation runner before the assertion is
reported. These updates have no network or animation to wait for; this does
not increase Stryker's timeouts or change the asserted behavior.

The command runner uses `--bail 1` to stop after the first failing browser
test, without dropping tests from the baseline. A `Timeout` status is not
proof that an assertion caught a mutant, even when Stryker counts it toward
the score. Require an actual `Killed` result for the handler-body mutation
and treat timeouts as failures needing investigation.

[Stryker supports Svelte source](https://stryker-mutator.io/docs/stryker-js/guides/svelte/)
through the installed compiler. Mutate owned production logic, not generated
files, tests, configuration or vendored dependencies. The configured break
threshold is a failure gate, not a quality claim. Triage survivors using the
same rules as Rust. Equivalent mutants need a precise, documented exclusion;
do not lower the threshold or exclude a whole mutator to hide a real gap.

## Formal proofs

Motet has no formal proofs. Do not call a property proved or formally
verified until an executed proof establishes it.

[Creusot](https://creusot.rs/) is a candidate for pure, synchronous code with
small, stable specifications. Good targets include access-freshness bounds
and admission ordering/idempotency. The current future-timestamp defect in
`spaces::serving::decide` must be resolved before claiming its intended
freshness property. A proof of the existing expression alone would prove the
wrong contract.

The [installation guide](https://guide.creusot.rs/installation.html) requires
a compatible compiler, Why3 and solvers. Those tools are not in Motet's dev
shell. Adding them is justified only for a concrete property whose benefit
exceeds its proof and maintenance cost. Do not impose a proof gate on the
whole async application.

For any adopted proof:

1. Specify and verify the production function, not a copied implementation.
2. Record preconditions, trusted assumptions, overflow/target assumptions,
   and whether the proof covers termination or only partial correctness.
3. Pin the compiler, Creusot and solver dependencies through the flake.
4. Add a described devenv command and CI job that fail on unproved goals.
5. Keep property and boundary tests. A proof does not establish real network,
   PostgreSQL, S3, browser or operating-system behavior.
6. Name the executed proof job when making a verification claim.

## Acceptance tests

The backend plan's acceptance table, and where each stands. Tests that need
more than one part of Motet live in `crates/acceptance`.

| Test                                                                                                                                                  | Status                                                                                                                                                                   |
| ----------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Two consumers, different cross-repo orders, same posts in the same order                                                                              | `rules/tests/view_convergence.rs`; public AppView admission and rebuild: `appview/tests/public_reads.rs`                                                                 |
| Remove a player while their post is in flight                                                                                                         | Public scene: `acceptance/tests/removal_in_flight.rs`; removal during read-back: `tenor/tests/admission_service.rs`. Private scene waits on Spaces                       |
| Stop the tenor while players post, then restart                                                                                                       | Replay and publication: `tenor/tests/shard.rs`, `tenor/tests/publication.rs`; outbox ordering and lost-response recovery: `appview/tests/post_writes.rs`                 |
| Admission for a post whose PDS write fails                                                                                                            | `tenor/tests/admission.rs`, `tenor/tests/admission_service.rs`, `appview/tests/post_writes.rs`                                                                           |
| Edit after leaving the cast                                                                                                                           | Rules: `rules/tests/world_properties.rs`                                                                                                                                 |
| Delete and recreate byte for byte                                                                                                                     | `tenor/tests/publication.rs`: the deletion publishes a closing and the recreation gets its own admission; `appview/tests/public_reads.rs`: the closed place stays absent |
| Two edits out of order become a conflict                                                                                                              | To do (outbox)                                                                                                                                                           |
| First private post during a 24-hour notification outage                                                                                               | To do (Spaces reconciliation)                                                                                                                                            |
| Former member deletes a private post                                                                                                                  | To do (Spaces reconciliation)                                                                                                                                            |
| Typst emits hostile markup or malformed vector data                                                                                                   | Sandbox and limits: `render/tests`. Browser half waits on typst.ts                                                                                                       |
| Restore from an export after readers saw lost admissions                                                                                              | To do                                                                                                                                                                    |
| Remove a member, then rebuild                                                                                                                         | To do                                                                                                                                                                    |
| Two clients edit a private record on the hosted PDS                                                                                                   | Conditional World writes: `tenor/tests/world_spaces.rs`. Personal PDS boundary remains to do                                                                             |
| Publication timeout while another client edits                                                                                                        | To do (outbox)                                                                                                                                                           |
| Use a fresh member service token after their role is removed                                                                                          | `tenor/tests/world_service_auth.rs`; AppView reports a removed member's explicit refusal in `appview/tests/post_writes.rs`                                               |
| Many requests for one restricted roll slot                                                                                                            | Retries: `rules/tests/world_properties.rs`. Rolls through the tenor to do                                                                                                |
| Return a roll, lose the client, migrate                                                                                                               | To do                                                                                                                                                                    |
| Restore an export older than a completed roll                                                                                                         | To do                                                                                                                                                                    |
| Recover a private scene with former members' posts                                                                                                    | To do                                                                                                                                                                    |
| Recover with the old tenor and an author's PDS gone                                                                                                   | To do                                                                                                                                                                    |
| Rebuild with no network after compaction and erasure                                                                                                  | `appview/tests/rebuild.rs`, `appview/tests/public_reads.rs`: public rebuild and incomplete-batch restart. Compaction and erasure to do                                   |
| Tap acknowledges only archived and projected batches, including live and backfill records                                                             | `appview/tests/tap.rs` (PostgreSQL and S3)                                                                                                                               |
| Redelivery after an ingest-before-ack crash leaves projected records and versions unchanged                                                           | `appview/tests/tap.rs` (PostgreSQL and S3)                                                                                                                               |
| Tap identity changes update both handles and account status before acknowledgement                                                                    | `appview/tests/tap.rs` (PostgreSQL and S3)                                                                                                                               |
| Public scenes show only exact World-authored admissions, in place order, even when deliveries arrive out of order                                     | `appview/tests/public_reads.rs` (PostgreSQL and S3)                                                                                                                      |
| Public scene and admitted-post queries page without skipping places and refuse invalid bounds                                                         | `appview/tests/public_reads.rs` (PostgreSQL and S3)                                                                                                                      |
| Stop the API and the indexer                                                                                                                          | To do                                                                                                                                                                    |
| Revoke Alice while Bob keeps the credential alive                                                                                                     | Authority ledger, delegation replay and HTTP proof binding: `tenor/tests/space_credentials.rs`; serving decision: `spaces::serving`. Full AppView journey remains to do  |
| Change a cast role after a lore subtree exists                                                                                                        | To do                                                                                                                                                                    |
| The same CID in two Spaces                                                                                                                            | Types keep Space identity in references (`spaces`). End to end to do                                                                                                     |
| Report a GM who can read the GM Space                                                                                                                 | To do                                                                                                                                                                    |
| Search with matching private documents                                                                                                                | To do                                                                                                                                                                    |
| A second writer against the same bucket                                                                                                               | `log/tests/fencing.rs`, `tenor/tests/shard.rs`                                                                                                                           |
| A World's genesis lists the owner's rotation key first, the tenor's only by opt-in, and is accepted only signed by the owner                          | `tenor/tests/identity.rs`: owner priority and compact low-S P-256 signatures over the reserved signing bytes                                                             |
| Browser owner keys produce independently verified low-S signatures and encrypted backups reject tampering or a different World                        | `apps/web/src/lib/owner-key/owner-key.svelte.test.ts`; re-import and request secrecy: `apps/web/e2e/journey.e2e.ts`                                                      |
| Service-auth audiences bind World creation to the tenor and admissions to the named World                                                             | `tenor/tests/identity.rs`, `tenor/tests/admission_service.rs`, `tenor/tests/world_service_auth.rs`                                                                       |
| The tenor's public DID document identifies its configured service, not an attacker-supplied Host                                                      | `tenor/tests/identity.rs`                                                                                                                                                |
| A World repo built by random writes verifies against its signing key, and replaying its firehose gives the same records                               | `tenor/tests/repo.rs`, `tenor::sync::tests`                                                                                                                              |
| A firehose subscribed at an empty head or current head receives the next event; future cursors are rejected                                           | `tenor::sync::tests`                                                                                                                                                     |
| A deleted record's sync proof verifies its absence                                                                                                    | `tenor::sync::tests`                                                                                                                                                     |
| A sealed World key opens only for its World and purpose, under its master key                                                                         | `tenor/tests/keys.rs`                                                                                                                                                    |
| Signed service tokens use one clock and preserve the exact verified issuer, including any service fragment                                            | `xrpc/tests/service_auth.rs`: expired lifetime and fragment-preserving real-signature checks                                                                             |
| Space records page completely across collections, and protocol revocation notices revoke only the named credentials                                   | `tenor/tests/space_http.rs`                                                                                                                                              |
| A private scene's admission never publishes publicly, and a scene with no known audience publishes nowhere                                            | `tenor/tests/publication.rs`                                                                                                                                             |
| Lexicon references resolve through nested arrays, union members and RPC bodies                                                                        | `lexgen/tests/references.rs`                                                                                                                                             |
| Choose themes by keyboard and pointer; rendered text stays legible and nested theme scopes stay isolated                                              | `apps/web/src/lib/components/ThemeSwitch.svelte.test.ts`, `apps/web/e2e/design.e2e.ts`                                                                                   |
| The design page hydrates and changes its painted theme under the production Content Security Policy                                                   | `apps/web/e2e/design.e2e.ts`                                                                                                                                             |
| Every remaining demo screen loads under the production Content Security Policy without errors; a gallery World paints its own art                     | `apps/web/e2e/screens.e2e.ts`                                                                                                                                            |
| Two Rust PDS accounts deliver their exact pilot profile versions through relay, Tap and the AppView                                                   | `network-smoke` (`nix/network-smoke.py`, needs `devenv up`)                                                                                                              |
| Create a World with a complete opensocial creation set, then reopen the shard                                                                         | `tenor/tests/world_spaces.rs`, `tenor/tests/identity.rs`: public declaration and named profile, private visibility, owner-key signature and replay                       |
| Persist full Space set-hash state, paginate one atomic batch and reject partial failed writes                                                         | `tenor/tests/space_storage.rs`                                                                                                                                           |
| Refuse a non-writer's notification, deduplicate revisions and retain pending deliveries after replay                                                  | `tenor::spaces::notify::tests`                                                                                                                                           |
| A Space deletion expires with its registration and cannot delete a recreated Space's copy                                                             | `tenor::spaces::notify::tests`                                                                                                                                           |
| A client's credential and KEM share authenticate the World's exact STAR records, never another Space                                                  | `tenor/tests/space_http.rs`: independent `verify_repo_star_full`, forged/revoked/wrong-Space refusals                                                                    |
| AppView-session XRPC methods reject account, service and Space credentials; cookie responses and preflights cannot use wildcard CORS                  | `xrpc::tests::session_methods_exclude_every_other_credential`, `xrpc/tests/server.rs`, `xrpc/tests/space_auth.rs`                                                        |
| Undeclared XRPC methods and wrong verbs never authenticate or poll bodies; verifiers receive the original HTTP context                                | `xrpc/tests/server.rs`: dispatch order, HTTP-bound credentials, preflight validation and readable nonce challenges                                                       |
| Current role grants, assignable roles and grant ceilings constrain World writes                                                                       | `tenor/tests/world_service_auth.rs`: collection/action grants, confined Space grants and membership or permission escalation refusals                                    |
| A World write and its member-attributed audit either both persist or neither does                                                                     | `tenor/tests/world_service_auth.rs`, `tenor/tests/scenes.rs`, `tenor/tests/admission_service.rs`: audit-failure rollback and replay                                      |
| A service-auth member obtains a Space delegation only with current whole-Space read permission                                                        | `tenor/tests/world_service_auth.rs`: independent delegation verification and read_self/write-only refusals                                                               |
| World service tokens bind the verified issuer, audience and exact XRPC method                                                                         | `tenor/tests/world_service_auth.rs`, `tenor/tests/admission_service.rs`                                                                                                  |
| Generic writes cannot change rules-backed scene, admission or closing records                                                                         | `tenor/tests/world_service_auth.rs`                                                                                                                                      |
| Only a permitted member opens a scene; the opener stays in its published cast                                                                         | `tenor/tests/scenes.rs`; `apps/web/e2e/journey.e2e.ts` checks the browser and projected cast                                                                             |
| Published cast and closure govern admission before and after shard replay                                                                             | `tenor/tests/scenes.rs`, `tenor/tests/admission_service.rs`, `rules/tests/world_properties.rs`                                                                           |
| Fetcher GETs and SDK destination checks refuse non-public IP literals before connecting, while allowing public literals and explicit local access     | `net::tests`: IPv4/IPv6 special-use classes, mapped IPv6 and alternate IPv4 encodings; raw SDK clients must call `check_destination` on untrusted URLs                   |
| Cross-origin login starts, unbound callbacks and unsafe cookie-authenticated requests are refused | `appview/tests/auth_http.rs` (PostgreSQL and the real XRPC router) |
| An AppView callback rejects a different state or issuer, and login refuses external or ambiguous return paths                                         | `appview/tests/auth_http.rs`, `appview::auth::login::tests`                                                                                                              |
| AppView OAuth session grants and pending state require the correct sealing key, row and purpose; opaque cookies cannot be recovered from the database | `appview/tests/auth_store.rs`                                                                                                                                            |
| Concurrent refreshes use a rotating token only once; cancellation or uncertain refresh cannot resurrect it                                            | `appview/tests/auth_store.rs`: concurrent, cancelled, remote-failure and replacement-write-failure cases                                                                 |
| Pending logins bind to one browser and are consumed once; expiry and logout prevent further session access, including a logout racing refresh         | `appview/tests/auth_store.rs`                                                                                                                                            |
| DPoP PDS calls handle nonce challenges, bounded responses and confidential assertions; service tokens remain bound to the member, audience and method | `appview::auth::pds::tests`                                                                                                                                              |
| Real PDS signup persists the authenticated identity; signing in as another account revokes the replaced session | `appview/tests/oauth_e2e.rs`; signup, sign-out and sign-in: `apps/web/e2e/journey.e2e.ts` |
| AppView OAuth metadata publishes a public key matching its confidential signer without revealing the private scalar                                   | `appview/tests/auth_http.rs`: client metadata and JWKS checked through the router                                                                                        |
| A port-bearing did:web resolves on its named host without accepting URL syntax smuggled into the authority                                            | `net::identity::tests`: encoded ports and malformed authority refusals                                                                                                   |
| Durable outbox session references never reveal or replace browser cookies and stop working after expiry or revocation                                 | `appview/tests/auth_store.rs`: reference restoration, mixed refresh and logout race                                                                                      |
| Public World names and scene casts survive redelivery and rebuild; profile tombstones and private-only profiles stay private                          | `appview/tests/public_reads.rs`, `appview/tests/public_space.rs`                                                                                                         |
| Outbound World service and public-Space credential requests cannot be redirected to untrusted destinations or unbounded responses                     | `appview/tests/write_client.rs`, `appview::public_space::tests`                                                                                                          |
| A revoked session pauses an outbox write; an expired lease cannot finish another worker's generation                                                  | `appview/tests/post_writes.rs`                                                                                                                                           |
| A non-cast or nonmember post is refused and never appears in the public scene                                                                         | `appview/tests/refused_post.rs`, `apps/web/e2e/journey.e2e.ts`                                                                                                           |
| Invalid public scene records and non-string post bodies do not become rendered content                                                                | `apps/web/src/routes/w/[world]/play/records.test.ts`                                                                                                                     |
| A member signs up, creates and backs up a World, opens a scene and sees the exact admitted post at its first place                                    | `appview/tests/live_journey.rs`, `apps/web/e2e/journey.e2e.ts`                                                                                                           |
