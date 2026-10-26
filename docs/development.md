# Development standards

These rules apply to production code, tests and tooling. Read
[testing.md](testing.md) for test admission and verification. Architectural
changes follow the [ADRs](adr/README.md).

## Style and configuration

Use [Google's Rust style guide](https://google.github.io/styleguide/rustguide.html),
the [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/) and
idiomatic Rust. Prefer a direct design over an abstraction without a caller.

The committed formatter configurations decide formatting. Do not reformat
code by hand to satisfy a preference. Motet keeps these local conventions:

- Standard Cargo layout, with crates separated by responsibility. Chromium's
  flat-directory and build-system rules do not apply.
- Tokio and async I/O in services. Rules, dice and access decisions stay pure.
- `Arc` for state shared by tasks and handlers. Prefer borrowing otherwise.
  Use `Arc::clone(&value)` to distinguish reference counting from deep copies.
- Exhaustive internal enums. Do not add `#[non_exhaustive]` to unpublished
  APIs without an external compatibility requirement.
- Stable rustfmt and the existing 120-column Rust/TOML width. Nightly-only
  Google formatting options are not required. Prettier formats web code.
- nextest, proptest and Vitest rather than a Google-internal test framework.

### Rust design

- Borrow by default. Take ownership when code keeps or consumes the value.
  Do not clone or allocate merely to silence the borrow checker.
- Avoid intermediate collections and repeated copies. Reserve known capacity.
  Use an iterator when the caller consumes a sequence once.
- Use newtypes for values that are easy to confuse. Include units in names
  or types. Prefer a named decision enum over an unclear boolean parameter.
- Make ordering explicit when it affects output, state or replay. Do not
  depend on hash-map iteration order.
- Return typed errors from libraries. Binaries add context with `anyhow`.
  An expected refusal is a domain outcome, not an infrastructure error.
- Panic only for an invariant violation. Each `expect` must say why the
  invariant holds. Public APIs document reachable panic conditions.
- Keep blocking I/O and CPU-heavy work off Tokio workers. Do not hold a
  synchronous lock across `.await`. Explain cancellation-sensitive state.
- Prefer standard traits and derives. Add traits and generic interfaces when
  a real caller needs them, not for hypothetical reuse.

### Rust lint policy

Every crate inherits `[workspace.lints]` from `Cargo.toml`. `lint` and the
hermetic Clippy check reject warnings. Rustdoc warnings also fail `lint`.

Clippy enables `all`, `pedantic`, `nursery` and `cargo`. The
[`restriction` category](https://doc.rust-lang.org/clippy/usage.html) is not
enabled wholesale: some rules contradict each other or prohibit valid code.
Selected restrictions cover unchecked indexing and string slicing, accidental
panics, shared ownership, resource leaks, nondeterministic iteration, stray
output and test-only production paths. See `Cargo.toml` for the exact list.
`clippy.toml` also bans blocking sleeps, `catch_unwind` and lossy UTF-8
conversion. Tests may unwrap and index; production invariants remain explicit.

Use `#[expect(clippy::lint_name, reason = "…")]` on the smallest applicable
item. The reason must explain why the lint is unsuitable there. An unfulfilled
expectation fails CI, so obsolete exemptions do not accumulate. Use `allow`
only when an expectation cannot work, such as generated documentation whose
content varies; give a reason. Do not suppress a whole crate to make CI green.
Workspace exceptions belong beside their rationale in `Cargo.toml`.

## TypeScript and Svelte

`pnpm lint` runs Oxlint and ESLint with warnings treated as errors.
`pnpm check` runs TypeScript and svelte-check. Prettier owns formatting.

[Anti-slop](https://github.com/dmmulroy/anti-slop) is vendored under
`tools/oxlint/anti-slop`, with its source revision and licences. All generic
rules run at error, together with Oxlint correctness rules and
`oxc/no-accumulating-spread`. Effect-specific rules are not enabled because
Motet does not use Effect.

These rules reject unsafe widening/assertion chains, opaque dictionary
contracts, module mocking, repeated accumulator copies and low-signal
patterns. A non-const type assertion needs a `SAFETY:` comment that states
the evidence for its type. Do not replace a lint error with a cast or a
fallback that hides invalid input.

Anti-slop examines syntax and lexical scope, not cross-file TypeScript types.
Oxlint checks Svelte script blocks; ESLint also checks Svelte markup. Neither
proves that a UI interaction changes what a reader sees. Browser tests do.

Generated output and vendored code are excluded deliberately. Update the
source generator or the recorded upstream copy; do not hand-edit their
output to satisfy a linter. Keep Oxlint and `@oxlint/plugins` pinned together.

## Comments and documentation

A comment adds information the code cannot express clearly:

- Why this design was chosen, including a rejected alternative when relevant.
- An invariant, protocol constraint or security boundary.
- A surprising dependency behaviour, failure mode or cancellation consequence.
- A reference to the specification or ADR that requires the behaviour.

Do not narrate assignments, loops or obvious names. Public API documentation
states the contract, errors and panic conditions; implementation comments
explain reasoning. Update or remove comments when their assumptions change.

```rust
// The archive must contain the batch before projections expose it.
// Otherwise a restart could lose records that readers already saw.
```

Use [ASD-STE100-inspired](https://www.asd-ste100.org/) clear technical English:
short sentences, active voice, one instruction per sentence, and conditions
before actions. Aim for about 20 words per instruction and 25 per description.
Use one term consistently: World, tenor, AppView, admission, Space, scene,
cast. Prefer specific verbs over "handle" and remove filler. Motet does not
claim STE100 certification or require its controlled dictionary. Reviewers
check clarity and truth; a word-count linter cannot check reasoning.

## Local commands and CI

Commands live in `nix/devenv.nix`, with descriptions shown by `devenv info`.
Multi-command scripts stop on failure. A later successful command must never
hide a failed lint or Rust test.
The shell puts Motet's `fmt` script before the unrelated GNU coreutils command
of the same name, including when direnv loads it.

| Command                  | Required evidence                                                                              |
| ------------------------ | ---------------------------------------------------------------------------------------------- |
| `fmt`, `fmt-check`       | Canonical Rust, Nix, TOML, web and documentation formatting                                    |
| `lint`                   | Clippy, rustdoc, cargo-deny policy, spelling, workflow lint, Oxlint, ESLint and type checks    |
| `lexgen-check`           | Checked-in generated code matches the lexicons                                                 |
| `tests`                  | Rust nextest suites and Vitest, including property and browser component tests                 |
| `tests-integration`      | Real PostgreSQL/S3 boundaries and production-build Playwright journeys                         |
| `network-smoke`          | Two Rust PDS accounts publish pilot profiles through PLC, relay and Tap's acknowledged channel |
| `devenv test`            | Local service-backed CI gate                                                                   |
| `mutants`, `mutants-web` | Rust and TypeScript/Svelte mutation analysis                                                   |
| `soak [cases]`           | Larger Rust property runs; failures retain replay seeds                                        |
| `audit`                  | Rust advisories and production npm dependency advisories                                       |

CI runs the service-backed gate and hermetic `nix flake check`. Only after
both succeed can a push to `main` publish images. Pull requests do not receive
package-write permissions. Scheduled jobs run audits and mutation analysis,
retain reports and fail on actionable findings. A test that passes only on
its diagnostic retry still fails CI ([ADR 0011](adr/0011-flaky-tests-fail.md)).

Use CI failures as evidence. Fix the cause; do not add retries, weaken a
threshold, or skip a test to make the badge green. Branch protection must
require the `checks` and `gate` jobs; repository files cannot configure that
hosting-side setting by themselves.

## Review checklist

- Does the change preserve the domain invariants and accepted ADRs?
- Is each allocation, copy, shared owner and abstraction justified?
- Are failure, cancellation and authorization boundaries explicit?
- Do comments explain why, with accurate assumptions and stable terminology?
- Does each test protect a useful property and catch a plausible bug?
- Are generators, reference models and assertions independent of the code?
- Do browser tests observe behavior, not CSS classes or internal markers?
- Are lint/mutation exceptions narrow and supported by evidence?
- Did the relevant commands and real changed path run? What remains unverified?
- Are docs and the acceptance table accurate? Formal proof claims require an
  executed proof of the production function, not a copied model.
