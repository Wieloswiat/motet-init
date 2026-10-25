# 0005. Properties, integration and acceptance tests; no tautological unit tests

Status: Accepted. The retry result is superseded by
[0011](0011-flaky-tests-fail.md).

## Context

Motet's failures that matter are disagreements: two consumers showing
different scenes, a replay that differs from what was acknowledged, a token
that opens the wrong method. Example-based unit tests that restate the code
don't find those.

## Decision

- Tests state properties of the system and use generators (`proptest`) to
  attack them: orders, retries, failures, hostile inputs.
- Boundaries are tested for real: PostgreSQL, S3 storage, signatures, an HTTP
  router. Those tests are `#[ignore = "integration: …"]` and run against the
  devenv services, locally and in CI.
- The backend plan's acceptance table is tracked in `docs/testing.md`; tests
  spanning several parts live in `crates/acceptance`.
- Mutation testing (`cargo mutants`) measures whether the tests pin the code
  down, weekly in CI on the core crates.
- nextest runs everything; the `ci` profile retries once and reports the
  retry, so flakiness is visible, not hidden.

## Consequences

A change to the rules or the log comes with the property it preserves. Code
that only a mock could exercise is a smell: restructure so the logic is pure
or test it against the real service.
