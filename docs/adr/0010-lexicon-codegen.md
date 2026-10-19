# 0010. Our own lexicon code generator

Status: Accepted

## Context

Lexicons are the contract between Motet, tenors and independent clients, and
both Rust and TypeScript need types from them. Generated types must keep
unknown fields (records written by newer clients) and refuse dangling
references.

## Decision

- `motet-lexgen` generates Rust (`crates/lexicons`) and TypeScript
  (`packages/lexicons`) from `lexicons/`. Generated code is committed so it can
  be reviewed; `lexgen-check` and a flake check fail when it is stale.
- Generated types carry unknown fields through and write them back unchanged.
- Motet's lexicons live under `at.motet.pilot.*` until they are stable; nothing
  is promoted in place to a stable NSID.
- Authorization is not in lexicons; it is in the XRPC registry.

## Revisit when

atproto-crates' lexicon tooling generates what we need, including TypeScript.
