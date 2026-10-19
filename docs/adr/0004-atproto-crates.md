# 0004. Build on atproto-crates

Status: Accepted

## Context

The backend plan starts from scratch on
[atproto-crates](https://tangled.org/ngerakines.me/atproto-crates): identity,
lexicons, OAuth, records, and the Spaces and Tap clients that Motet needs and
that exist nowhere else in Rust. Some of those crates are released on
crates.io, Tap's client (`atproto-tap`) among them; the Spaces and testkit
crates are not yet.

## Decision

- The released crates come from crates.io, all at one version, bumped
  together (Renovate groups them).
- The unreleased crates are added from git at one pinned revision by the
  change that first uses them, and the released crates move to that same
  revision then, so two copies of the core types never meet. `deny.toml`'s
  `allow-git` gets that repository at the same time.
- Where a released crate lacks something (service-auth verification, at
  0.14.5), Motet implements it in the crate that owns the boundary
  (`motet-xrpc::service_auth`) against atproto-crates' key and JWT
  primitives, with tests using real keys, and drops it when upstream has it.
- `reqwest` is pinned to the major atproto-crates uses, so their clients can
  take `motet-net`'s hardened client.

## Checked from a machine that can reach tangled.org (2026-10-19)

- Cargo resolves the git crates at a pinned `rev` through Tangled's redirect
  to its knot, with `net.git-fetch-with-cli = true` (already set in
  `.cargo/config.toml`).
- Nix's `builtins.fetchGit` (what crane uses to vendor git dependencies)
  fetches the same revision, so `nix flake check` and the image builds need
  no `outputHashes`.
- Mixing them really does duplicate the core types: with the git Spaces crates
  at 0.15.0-rc.6 next to `atproto-identity = "0.14.5"` from crates.io, the
  lockfile holds both versions of `atproto-identity`. The change that adds
  Spaces must move every atproto crate to the git revision at once, as above.

## Consequences

Upstream churn in Spaces lands in `motet-spaces` and nowhere else; that crate
is the seam. CI environments need network access to tangled.org once the
git dependencies arrive.
