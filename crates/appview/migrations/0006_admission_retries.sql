-- Claim generations also include publication and ordering waits. Count only
-- admission failures recorded while the worker still owns its lease.
ALTER TABLE service.outbox
    ADD COLUMN admission_failures integer NOT NULL DEFAULT 0 CHECK (admission_failures >= 0);
