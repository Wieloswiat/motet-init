# Testing

Motet's correctness lives in a few promises: every consumer agrees on what a
World admitted, a replay gives back exactly what was acknowledged, nothing
unauthorised reaches a handler, projections rebuild from the archive. Tests
state those promises directly and try hard to break them.

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
  change without any test noticing; the scheduled CI job runs it on the core
  crates.

## Suites

| Suite       | Runs                                                                                 | Needs                                |
| ----------- | ------------------------------------------------------------------------------------ | ------------------------------------ |
| default     | `tests`; every change; `nix flake check`                                             | nothing                              |
| integration | `tests-integration`; `devenv test`; CI                                               | `devenv up` (PostgreSQL, S3)         |
| web         | `pnpm test` (vitest in Chromium), `pnpm test:e2e` (Playwright on a production build) | Chromium (the dev shell provides it) |
| soak        | `soak [cases]`                                                                       | nothing; slow                        |
| mutation    | `mutants`; weekly in CI                                                              | nothing; slow                        |

Integration tests are ordinary `#[test]`s marked
`#[ignore = "integration: …"]`, so `cargo test` stays fast and a missing
service is a skip with a reason, never a pass. Their names start with
`fault_` when they inject faults; those run one at a time
(`.config/nextest.toml`).

## Acceptance tests

The backend plan's acceptance table, and where each stands. Tests that need
more than one part of Motet live in `crates/acceptance`.

| Test                                                                     | Status                                                                               |
| ------------------------------------------------------------------------ | ------------------------------------------------------------------------------------ |
| Two consumers, different cross-repo orders, same posts in the same order | `rules/tests/view_convergence.rs`                                                    |
| Remove a player while their post is in flight                            | Public scene: `acceptance/tests/removal_in_flight.rs`. Private scene waits on Spaces |
| Stop the tenor while players post, then restart                          | Replay half: `tenor/tests/shard.rs`. Outbox half waits on the API                    |
| Admission for a post whose PDS write fails                               | `tenor/tests/admission.rs`                                                           |
| Edit after leaving the cast                                              | Rules: `rules/tests/world_properties.rs`                                             |
| Delete and recreate byte for byte                                        | To do                                                                                |
| Two edits out of order become a conflict                                 | To do (outbox)                                                                       |
| First private post during a 24-hour notification outage                  | To do (Spaces reconciliation)                                                        |
| Former member deletes a private post                                     | To do (Spaces reconciliation)                                                        |
| Typst emits hostile markup or malformed vector data                      | Sandbox and limits: `render/tests`. Browser half waits on typst.ts                   |
| Restore from an export after readers saw lost admissions                 | To do                                                                                |
| Remove a member, then rebuild                                            | To do                                                                                |
| Two clients edit a private record on the hosted PDS                      | To do (Spaces PDS)                                                                   |
| Publication timeout while another client edits                           | To do (outbox)                                                                       |
| Reuse a GM's session after their role is removed                         | To do                                                                                |
| Many requests for one restricted roll slot                               | Retries: `rules/tests/world_properties.rs`. Rolls through the tenor to do            |
| Return a roll, lose the client, migrate                                  | To do                                                                                |
| Restore an export older than a completed roll                            | To do                                                                                |
| Recover a private scene with former members' posts                       | To do                                                                                |
| Recover with the old tenor and an author's PDS gone                      | To do                                                                                |
| Rebuild with no network after compaction and erasure                     | Rebuild: `appview/tests/rebuild.rs`. Compaction and erasure to do                    |
| Stop the API and the indexer                                             | To do                                                                                |
| Revoke Alice while Bob keeps the credential alive                        | Serving decision: `spaces::serving`. End to end to do                                |
| Change a cast role after a lore subtree exists                           | To do                                                                                |
| The same CID in two Spaces                                               | Types keep Space identity in references (`spaces`). End to end to do                 |
| Report a GM who can read the GM Space                                    | To do                                                                                |
| Search with matching private documents                                   | To do                                                                                |
| A second writer against the same bucket                                  | `log/tests/fencing.rs`, `tenor/tests/shard.rs`                                       |
