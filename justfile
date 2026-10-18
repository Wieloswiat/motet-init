# Motet tasks. Run from the dev shell (`nix develop --no-pure-eval` or direnv).
# CI calls these same recipes, so they are the definition of "passes CI".

set shell := ["bash", "-euo", "pipefail", "-c"]

default:
    @just --list --unsorted

# --- everyday -----------------------------------------------------------------

# Fast checks: formatting, lints and the default test suites (no services).
check: fmt-check lint test

# Default Rust tests (pure, property and in-process) plus web unit/property tests.
test:
    cargo nextest run --workspace
    pnpm -r --if-present test

# Integration tests against PostgreSQL and S3 (SeaweedFS). Needs `devenv up`.
test-integration:
    cargo nextest run --workspace --profile integration --run-ignored only
    pnpm --filter @motet/web test:e2e

# Format everything.
fmt:
    cargo fmt --all
    nixfmt flake.nix nix/*.nix
    taplo fmt
    pnpm -r --if-present format

fmt-check:
    cargo fmt --all --check
    nixfmt --check flake.nix nix/*.nix
    taplo fmt --check
    pnpm -r --if-present format:check

lint:
    cargo clippy --workspace --all-targets --all-features -- --deny warnings
    typos
    pnpm -r --if-present lint
    pnpm -r --if-present check

# --- code generation ----------------------------------------------------------

# Regenerate Rust and TypeScript types from lexicons/.
lexgen:
    cargo run -q -p motet-lexgen -- generate --lexicons lexicons --rust crates/lexicons/src/generated --ts packages/lexicons/src/generated
    pnpm --filter @motet/lexicons format

# Fail if generated code is stale.
lexgen-check:
    cargo run -q -p motet-lexgen -- check --lexicons lexicons --rust crates/lexicons/src/generated --ts packages/lexicons/src/generated

# --- running ------------------------------------------------------------------

# The web app against a local AppView (start services with `devenv up` first).
web:
    pnpm --filter @motet/web dev

appview:
    cargo run -p motet-appview -- serve

tenor:
    cargo run -p motet-tenor -- serve

# The deployment-shaped stack: AppView, tenor, Tap, a Spaces PDS, PostgreSQL, S3 storage.
stack:
    nix build .#appview-image -o result-appview && ./result-appview | podman load
    nix build .#tenor-image -o result-tenor && ./result-tenor | podman load
    podman-compose -f deploy/compose.yaml up

# --- slower assurance ---------------------------------------------------------

# Dependency advisories (needs network; nightly in CI).
audit:
    cargo deny check advisories

# Mutation testing: which code can change without any test noticing?
mutants *args:
    cargo mutants --workspace --test-tool nextest {{args}}

# Line coverage of the default suites, as lcov.info.
coverage:
    cargo llvm-cov nextest --workspace --lcov --output-path lcov.info

# Property tests with many more cases than the default run.
soak cases="20000":
    PROPTEST_CASES={{cases}} cargo nextest run --workspace --release

# --- CI entry points ----------------------------------------------------------

ci-rust:
    cargo nextest run --workspace --profile ci

ci-web:
    pnpm install --frozen-lockfile
    pnpm -r --if-present lint
    pnpm -r --if-present check
    pnpm -r --if-present format:check
    pnpm -r --if-present test
    pnpm -r --if-present build

ci-integration:
    cargo nextest run --workspace --profile integration --run-ignored only
