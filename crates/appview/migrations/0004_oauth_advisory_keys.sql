-- Session hashes are random, but truncating them to advisory-lock keys could
-- couple unrelated accounts. Assign each coordinator a unique durable key.
ALTER TABLE service.oauth_session_locks
    ADD COLUMN advisory_key bigint GENERATED ALWAYS AS IDENTITY;
ALTER TABLE service.oauth_session_locks
    ADD CONSTRAINT oauth_session_locks_advisory_key_unique UNIQUE (advisory_key);
