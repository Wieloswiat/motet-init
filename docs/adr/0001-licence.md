# 0001. Motet's licence

Status: Accepted

## Context

The licence decides what self-hosters of a tenor may do, whether ParadeDB's
AGPL-3.0 community edition fits, and what the legal review in Running Worlds
must cover (hosting, rendering, migration and archives of people's writing).
Until this decision, no code depended on the answer.

## Decision

Motet's own code is MIT. The repository root holds the standard MIT text in
[LICENSE](../../LICENSE), copyright 2026 the Motet contributors.

- Crates and packages stay `publish = false` / `"private": true` until a
  release decision. Publishing is a separate decision from licensing.
- `deny.toml` keeps its allow list. Permissive licences and weak copyleft such
  as MPL-2.0 are all compatible with distributing Motet under MIT.
- ParadeDB's AGPL-3.0 community edition runs as a separate database service.
  That does not change Motet's licence. An operator who modifies ParadeDB must
  meet the AGPL obligations for that modified ParadeDB.
- Vendored third-party code keeps its own licence. `tools/oxlint/anti-slop` is
  MIT and keeps its LICENSE files.
- The vendored opensocial.group lexicons still have no upstream licence
  ([lexicons/README.md](../../lexicons/README.md)). This item stays open.
- The legal review in Running Worlds still covers users' writing: hosting,
  rendering, migration and archives. The code licence does not license
  people's content.

## Revisit when

A contributor agreement or a relicensing need makes these terms insufficient.
