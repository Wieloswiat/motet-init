# 0002. One nix flake for the toolchain, services, checks and CI

Status: Accepted; shell entry point superseded in part by [0025](0025-native-devenv.md)

## Context

Motet needs Rust, Node, PostgreSQL with pg_search, S3-compatible storage and
a handful of tools, at the same versions on every machine and in CI. CI that
runs different commands from the ones developers run drifts.

## Decision

- `flake.nix` is the single entry point. `rust-toolchain.toml` is read by the
  flake (rust-overlay), so the shell, the build and CI use one toolchain.
- The dev shell is [devenv](https://devenv.sh) as a flake-parts module
  (`nix/devenv.nix`): packages, PostgreSQL and SeaweedFS as processes, git
  hooks, and every command as a devenv **script**, with a description shown by
  `devenv info`. No justfile or Makefile: one place defines commands.
- `nix flake check --no-pure-eval` (`nix/rust.nix`, crane) runs everything hermetic: fmt,
  clippy, the default tests, cargo-deny and lexicon freshness.
- `devenv test` runs `enterTest`, the whole gate with services. CI runs these
  two and nothing else.
- Container images are built by nix (`dockerTools.streamLayeredImage`); there
  is no Dockerfile to keep in step.

## Consequences

Contributors need nix. Anyone without it can still use `cargo` and `pnpm`
directly with the versions pinned in the repository, but CI is the reference.
devenv's flake integration requires `--no-pure-eval`.
