-- OAuth credentials are service state; a projection rebuild must not remove them.
-- Browser secrets are represented only by hashes. The sealed values include the
-- row ID and purpose as authenticated data, so ciphertext cannot be moved.
CREATE TABLE service.oauth_sessions (
    id bytea PRIMARY KEY CHECK (octet_length(id) = 32),
    sealed bytea NOT NULL,
    generation bigint NOT NULL DEFAULT 0 CHECK (generation >= 0),
    refreshing boolean NOT NULL DEFAULT false,
    revoked boolean NOT NULL DEFAULT false,
    expires_at timestamptz NOT NULL
);
CREATE INDEX oauth_sessions_expiry ON service.oauth_sessions (expires_at);

-- This row is locked separately from the grant. A refresh commits its poison
-- marker before calling the PDS, while holding this lock across both commits.
CREATE TABLE service.oauth_session_locks (
    id bytea PRIMARY KEY CHECK (octet_length(id) = 32)
);

CREATE TABLE service.oauth_pending_logins (
    id bytea PRIMARY KEY CHECK (octet_length(id) = 32),
    browser_binding bytea NOT NULL CHECK (octet_length(browser_binding) = 32),
    sealed bytea NOT NULL,
    expires_at timestamptz NOT NULL
);
CREATE INDEX oauth_pending_logins_expiry ON service.oauth_pending_logins (expires_at);
