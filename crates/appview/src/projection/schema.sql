-- Projections: everything here is rebuilt from the archive, never migrated.
-- A change to this file bumps PROJECTION_VERSION in projection/mod.rs, and
-- the AppView rebuilds into a fresh schema and swaps it in.
--
-- Runs with search_path set to the schema being built, so names are
-- unqualified.

CREATE TABLE meta (
    singleton boolean PRIMARY KEY DEFAULT true CHECK (singleton),
    version integer NOT NULL,
    -- The last archive segment applied.
    archive_seq bigint
);

-- The current state of every record seen. A deletion leaves a row with no
-- CID, so an older create delivered late can't bring the record back.
CREATE TABLE records (
    uri text PRIMARY KEY,
    did text NOT NULL,
    collection text NOT NULL,
    rkey text NOT NULL,
    -- Revisions are TIDs: byte order is commit order.
    rev text COLLATE "C" NOT NULL,
    cid text,
    record jsonb
);
CREATE INDEX records_by_collection ON records (collection, did);

-- Every version seen, current or not: replies, admissions and reports pin
-- exact versions, and show "this version isn't available" only when it was
-- never seen.
CREATE TABLE record_versions (
    uri text NOT NULL,
    cid text NOT NULL,
    rev text COLLATE "C" NOT NULL,
    record jsonb NOT NULL,
    PRIMARY KEY (uri, cid)
);

CREATE TABLE accounts (
    did text PRIMARY KEY,
    handle text,
    active boolean NOT NULL DEFAULT true,
    status text
);
