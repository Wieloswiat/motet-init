# 0017. The local network starts with atproto-crates' PDS only

Status: Accepted. Supersedes [0016](0016-local-atproto-network.md)'s two-PDS
requirement and its patch scope; the rest of 0016 stands.

## Context

0016 put both the reference TypeScript PDS (the 2026-10-22 Spaces alpha) and
atproto-crates' PDS in the local network, and planned to patch both indigo's
relay and Tap for private hosts. Building the reference PDS's pnpm monorepo
from pinned source in nix reached its OAuth UI message extraction, then
exhausted the build host's memory; no binary was verified. At the pinned
indigo revision, Tap's HTTP and WebSocket clients already allow private
hosts.

## Decision

- The native and compose environments run PLC, the relay, Tap and
  atproto-crates' PDS. The reference PDS's package, process and image are not
  shipped until its build and runtime are verified. No runtime `npx` or
  unpinned image stands in for it.
- Only the relay carries the default-off private-host patch. A local first
  subscription replays from cursor zero, so a crawl can't miss records
  written before it started.
- `network-smoke` creates two accounts on the Rust PDS and checks both pilot
  profiles through PLC, the relay's crawl and Tap's acknowledged channel.

## Consequences

The smoke checks local ingestion, not compatibility between independent PDS
implementations or private Spaces; that coverage waits for the reference PDS.

## Revisit when

The reference PDS builds reproducibly within the dev machines' memory, or the
Spaces alpha publishes an image that can be pinned by digest.
