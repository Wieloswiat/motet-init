-- A retry references the sealed OAuth session by its non-bearer hash. The
-- cookie and tokens never enter the outbox. Revocation remains authoritative.
ALTER TABLE service.outbox
    ADD COLUMN session_id bytea CHECK (session_id IS NULL OR octet_length(session_id) = 32),
    ADD COLUMN sequence bigint GENERATED ALWAYS AS IDENTITY,
    ADD COLUMN next_attempt_at timestamptz NOT NULL DEFAULT now(),
    ADD COLUMN paused boolean NOT NULL DEFAULT false;

-- Sequence breaks timestamp ties and keeps later jobs behind all earlier jobs,
-- including ones waiting for their retry time or for an expired lease.
CREATE UNIQUE INDEX outbox_sequence ON service.outbox (sequence);
CREATE INDEX outbox_account_order ON service.outbox (account, sequence) WHERE state = 'queued';
CREATE INDEX outbox_due ON service.outbox (next_attempt_at, sequence) WHERE state = 'queued';
