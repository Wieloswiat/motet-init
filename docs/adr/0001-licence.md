# 0001. Motet's licence

Status: Proposed (open)

## Context

The licence decides what self-hosters of a tenor may do, whether ParadeDB's
AGPL-3.0 community edition fits, and what the legal review in Running Worlds
must cover (hosting, rendering, migration and archives of people's writing).
None of the code yet depends on the answer.

## Decision

Not made. Until it is, every crate and package is `publish = false` /
`"private": true`, and `deny.toml` lists the licences our dependencies may
use, so whichever licence is chosen won't be blocked by something already
pulled in.

## Revisit when

Before the closed alpha, or before anyone outside the project runs a tenor.
