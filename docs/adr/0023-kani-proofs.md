# 0023. Use Kani for selected pure-function proofs

Status: Proposed.

## Context

Property tests explore sampled inputs. Some pure decisions have small input
spaces at the bit level but large value domains, where a verifier can check
symbolic inputs more broadly. Kani 0.68.0 uses CBMC and requires a matching
nightly Rust compiler. Its compiler interface adds version-coupling and CI cost.
A proof gate must stay limited to properties whose benefit exceeds that cost.

## Decision

Use the `proofs` devenv command to run Kani 0.68.0 for selected pure-function
harnesses. `nix/pkgs/kani.nix` pins Kani, CBMC and its private nightly toolchain;
production builds continue to use `rust-toolchain.toml`. The CI `proofs` job
runs that same command and gates image publication.

The initial scope is `spaces::serving::decide`. Its harnesses check revocation,
freshness and clock boundaries across symbolic full-width `u64` inputs. The
function has no loops, so no unwinding bound applies. CBMC checks all executions
of the harnesses under their assertions. This is partial correctness, not a
proof that callers provide valid or synchronized clocks, and it does not prove
private AppView serving or network behavior. Keep property and boundary tests.

## Consequences

The CI job adds verifier time and storage for `target/kani/proofs.json`. Kani's
compiler components couple the verifier to its private nightly ABI; update the
Kani release and nightly together. The package currently defines x86_64-linux,
aarch64-linux and aarch64-darwin releases. Other platforms are unsupported until
Nix packaging and the upstream release provide them.

## Revisit when

Revisit if the nightly ABI becomes unreliable, CI cost exceeds the value of the
checked properties, a required platform lacks a Kani release, or a target
property needs specifications Kani cannot express. Consider another verifier
for that property rather than widening this gate to the async application.
