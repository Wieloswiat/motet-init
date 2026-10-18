# Lexicons

The schemas are the contract. Records, Space types and every XRPC method
Motet exposes are defined here first; Rust and TypeScript types are
generated from them (`just lexgen`), and CI fails if the generated code is
stale (`just lexgen-check`, `nix flake check`).

| Path | Owner | Status |
| --- | --- | --- |
| `at/motet/pilot/` | Motet | **Draft.** The settled pilot namespace; exact names and fields are still an open gate (backend plan, "Data model and lexicons"). Nothing here is stable, and nothing is ever promoted in place to a stable NSID. |
| `com/atproto/` | Bluesky | Vendored, unmodified, only what Motet's lexicons reference. |
| `group/opensocial/` | opensocial.group | Not vendored yet. Pin a revision of the proposal here when Groundwork decides which records Motet adopts. |

Rules for Motet's own lexicons (from the backend plan's modelling rules):

- Refer to a World by its DID, never by a record URI.
- Pin versions with `com.atproto.repo.strongRef` wherever an approval or
  answer depends on the exact version; a plain `at-uri` means "whatever is
  current".
- One record describes one thing. No World mega-record, no command log.
- Typst source goes in the record; rendered output never does.
- Extend someone else's record with a sidecar in our namespace, never by
  adding fields to their schema.
- Integers only (records have no floats).
- Unknown fields survive: generated types keep them and write them back.

Authorization is not in lexicons. Who may call each XRPC method lives in the
fail-closed policy registry (`crates/xrpc`), next to the generated method
types.
