# 0016. A local atproto network in the dev shell, with patched indigo

Status: Accepted

## Context

The tenor must be crawled by a relay and read through Tap, players need PDS
accounts with Spaces, and Worlds need a PLC directory. Tests against the public
network aren't hermetic and can't run in CI. Upstream, only Tap is in nixpkgs;
the PLC server, the relay, the reference PDS's Spaces alpha and atproto-crates'
PDS aren't packaged or published as images. Indigo's relay and Tap refuse
loopback addresses, and any port other than 80 or 443, through their SSRF
transports, so out of the box they can't crawl or backfill a local host.

## Decision

- `devenv up` runs a PLC directory (in memory), indigo's relay and Tap from
  one pinned indigo revision, the reference PDS from the Spaces alpha (the
  2026-10-22 `permissioned-data` merge), and atproto-crates' PDS at the same
  revision as the crates. They run beside PostgreSQL and SeaweedFS, and the
  compose stack runs the same builds.
- Players get accounts on both PDSes, so interop with a second Spaces
  implementation is tested from the start.
- Relay and Tap carry a small patch, kept in `nix/patches/`, that allows
  private hosts only when an explicit development flag is set. The flag is
  off by default, and the patch is reviewed whenever the indigo pin moves.
- Everything is built by nix from pinned sources. Nothing is fetched at
  runtime.

## Consequences

- The dev shell and CI build Go and Node workspaces; first builds are slow
  and cached afterwards.
- The patch is Motet's to carry until upstream has an equivalent option.

## Revisit when

Indigo gains a development mode for private hosts, the Spaces alpha merges to
`main` with published images, or atproto-crates' PDS is packaged upstream.
