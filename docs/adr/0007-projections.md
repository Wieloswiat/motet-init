# 0007. Rebuildable projections, runtime-checked SQL

Status: Accepted

## Context

Everything pages are built from is derived from the archive, so it can be
rebuilt; some service state (outbox, sessions, read positions) can't.

## Decision

- Two schemas. `projection` is derived and has no migrations: a change to
  `projection/schema.sql` bumps `PROJECTION_VERSION`, and the AppView rebuilds
  into a fresh schema from the archive and swaps it in. `service` holds
  state that can't be rebuilt and uses ordinary sqlx migrations.
- `projection::apply` is the only writer of projections, for live indexing
  and rebuilds alike, so "rebuild gives the same projections" holds by
  construction and is tested.
- Each record keeps the newest repo revision applied, so a late event can't
  roll the display back.
- SQL is checked at runtime against the integration database (`sqlx::query`),
  not at compile time: compile-time checks would need a live database or a
  committed query cache for every build, and the projection schema is
  replaced rather than migrated. The integration tests are the check.
- The outbox is a PostgreSQL table with leases and `SKIP LOCKED`; a broker
  only after measurements.

## Consequences

Every query needs an integration test that runs it. Search (pg_search) joins
the projection schema and is rebuilt the same way.
