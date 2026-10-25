# 0011. A test that passes only on its retry fails CI

Status: Accepted. Supersedes the retry result in
[0005](0005-testing.md); the rest of 0005 stands.

## Context

[ADR 0005](0005-testing.md) gave nextest's `ci` profile one retry and said
the report makes flakiness visible. By default nextest counts a test that
fails and then passes on its retry as flaky, and a flaky test doesn't fail
the run. The report is in the summary and in `junit.xml`, but the run is
green, so nothing makes anyone read it. A flaky test can hide a race in the
code as easily as a fault in the test.

## Decision

- The `ci` profile keeps its one retry. The retry shows whether a failure
  repeats, which helps to find the cause.
- A test that passes only on its retry fails the run
  (`flaky-result = "fail"`, nextest 0.9.131 or later). JUnit reports it as a
  failure and marks it flaky.
- The cause gets fixed, in the test or in the code. More retries, sleeps,
  longer timeouts, skipped tests or quarantine don't count as a fix.

## Consequences

A flaky test blocks merging until it is fixed, like any other failing test.
The integration profile has no retries, so a failure there already fails the
run.
