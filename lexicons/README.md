# Lexicons

The schemas are the contract. Records, Space types and every XRPC method
Motet exposes are defined here first; Rust and TypeScript types are
generated from them (`lexgen` in the dev shell), and CI fails if the generated code is
stale (`lexgen-check`, `nix flake check`).

| Path                | Owner                    | Status                                                                                                                                                                                                           |
| ------------------- | ------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `at/motet/pilot/`   | Motet                    | **Draft.** The settled pilot namespace; exact names and fields are still an open gate (backend plan, "Data model and lexicons"). Nothing here is stable, and nothing is ever promoted in place to a stable NSID. |
| `com/atproto/`      | Bluesky; exception below | Vendored, unmodified dependencies. `com.atproto.label.label` is an opensocial.group proposal-owned strawman, not an adopted Bluesky lexicon.                                                                     |
| `group/opensocial/` | opensocial.group         | All 31 proposal lexicons at `2b41a9bf9171c03a1aa5ee26c6e39823ba27c79d` (2026-10-19). See [source and licence status](group/opensocial/UPSTREAM).                                                                 |

The proposal has no LICENSE file at this pin. Its maintainers intend it to be
public; Motet's maintainer will ask for an explicit licence. The source note also
records the revisions of the referenced label and moderation definitions.

`group.opensocial.invites` is a user's invite inbox, not a World audience.
It therefore has a Space-type constant but no `Audience` variant: assigning it
the World's authority would put invitations in the wrong account.

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

The tenor publishes `at.motet.pilot.admission` after a durable rules decision.
Its `place` retains the epoch and sequence across edits; `replaces` names the
previous post CID. The draft `at.motet.pilot.closing` pins the admission closed
by deletion. A byte-identical recreation needs a new admission at a new place.
Each admission captures the scene record's unique location at decision time:
the World's public repo or a specific Space generation. Later scene moves
and Space recreation cannot redirect queued records. A missing or ambiguous
location stays unknown and queued; it is never inferred public later.
Closings retain the admission's destination. Publication follows decision
order for each post; a later accepted reversion to earlier bytes gets its
own record, while retries of the same operation do not.

## Generated wire types

- Records and objects keep unknown fields (`extra` in Rust; an index signature
  in TypeScript). Open unions dispatch on `$type` and keep unknown variants,
  including their tags and nested data. Closed unions omit that fallback.
- Queries and procedures generate `Params`, `Input` and `Output` types where
  the schema supplies them. The current proposal uses JSON objects and refs.
  Other body shapes and subscriptions are refused, not silently omitted.
- String definitions, including `knownValues`, stay open string aliases.
  Schema validation at ingest enforces formats, bounds and closed string enums;
  known values are suggestions, not a closed enumeration. Tokens expose their
  full lexicon identifiers.
- `bytes` and `cid-link` use the protocol's `$bytes` and `$link` JSON objects.
  `unknown` preserves arbitrary JSON data.
- Space and permission-set definitions generate identifier constants, not
  wire structs. These describe host policy, not data values. Their generated
  modules explain the omission, and Rust's `SCHEMAS` keeps the full definitions.
