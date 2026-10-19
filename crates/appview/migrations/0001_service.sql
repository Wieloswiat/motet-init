-- Service state: what the archive can't rebuild. Backed up like any
-- database; migrated with sqlx migrations, never dropped by a rebuild.
CREATE SCHEMA IF NOT EXISTS service;

-- Writes Motet makes for people (posts to their PDS, admission requests to
-- tenors), with the operation ID that makes retries safe. A worker leases a
-- job; a crashed worker's lease expires and another takes it over, which is
-- safe because every operation is idempotent by its ID and an ambiguous
-- outcome is read back before any retry.
CREATE TABLE service.outbox (
    id uuid PRIMARY KEY,
    account text NOT NULL,
    op_id text NOT NULL UNIQUE,
    kind text NOT NULL,
    payload jsonb NOT NULL,
    state text NOT NULL DEFAULT 'queued' CHECK (state IN ('queued', 'done', 'failed')),
    -- Jobs of one account run in creation order.
    created_at timestamptz NOT NULL DEFAULT now(),
    lease_owner text,
    lease_until timestamptz,
    attempts integer NOT NULL DEFAULT 0,
    outcome jsonb
);
CREATE INDEX outbox_ready ON service.outbox (created_at) WHERE state = 'queued';

-- Where each reader is in each scene: a place in the admission sequence,
-- with its recovery epoch so a place from before a recovery is never read as
-- one after it.
CREATE TABLE service.read_positions (
    account text NOT NULL,
    world text NOT NULL,
    scene text NOT NULL,
    epoch integer NOT NULL,
    seq bigint NOT NULL,
    PRIMARY KEY (account, world, scene)
);
