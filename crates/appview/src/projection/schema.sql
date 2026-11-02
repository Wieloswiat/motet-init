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

-- A Space repository has its own revision history, separate from the
-- author's public repository. Keep tombstones to reject stale deliveries.
CREATE TABLE space_records (
    space text NOT NULL,
    did text NOT NULL,
    collection text NOT NULL,
    rkey text NOT NULL,
    rev text COLLATE "C" NOT NULL,
    cid text,
    record jsonb,
    PRIMARY KEY (space, did, collection, rkey)
);

CREATE TABLE accounts (
    did text PRIMARY KEY,
    handle text,
    active boolean NOT NULL DEFAULT true,
    status text
);

-- Only a declaration in the World's current public repo makes its canonical
-- meta Space eligible for public ingestion and display.
CREATE VIEW public_world_meta AS
SELECT did, record->>'meta' AS space
FROM records
WHERE collection = 'group.opensocial.declaration' AND rkey = 'self' AND cid IS NOT NULL
    AND record->>'meta' = 'at://' || did || '/space/group.opensocial.meta/self';

-- A declaration identifies a group with public metadata. A public scene also
-- identifies its author as a World, even when its metadata Space is private.
-- A freshly created World with no public records is not distinguishable from
-- a person's account on the public firehose.
CREATE VIEW worlds AS
SELECT w.did, a.handle,
       CASE WHEN jsonb_typeof(p.record->'displayName') = 'string'
            THEN p.record->>'displayName' END AS display_name
FROM (
    SELECT DISTINCT did
    FROM records
    WHERE cid IS NOT NULL AND (
        (collection = 'group.opensocial.declaration' AND rkey = 'self'
            AND jsonb_typeof(record->'meta') = 'string')
        OR (collection = 'at.motet.pilot.scene' AND jsonb_typeof(record) = 'object')
    )
) w
LEFT JOIN accounts a ON a.did = w.did
LEFT JOIN public_world_meta m ON m.did = w.did
LEFT JOIN space_records p ON p.space = m.space AND p.did = w.did
    AND p.collection = 'group.opensocial.profile' AND p.rkey = 'self' AND p.cid IS NOT NULL;

CREATE VIEW scenes AS
SELECT did AS world_did, rkey, uri, cid, record,
       CASE WHEN jsonb_typeof(record->'cast') = 'array' THEN record->'cast' END AS cast_dids,
       CASE WHEN jsonb_typeof(record->'openedBy') = 'string' THEN record->>'openedBy' END AS opened_by
FROM records
WHERE collection = 'at.motet.pilot.scene' AND cid IS NOT NULL
    AND jsonb_typeof(record) = 'object';

-- A World admits a version, not the author's current record. Historical
-- versions remain available even after an edit or a delete.
CREATE VIEW posts AS
SELECT r.did AS author, v.uri, v.cid, v.record
FROM records r
JOIN record_versions v ON v.uri = r.uri
WHERE r.collection = 'at.motet.pilot.post';

-- Lexicon validation is a separate ingestion stage. A bad place must not
-- make every read of a scene fail or turn an invalid value into a position.
CREATE VIEW admissions AS
SELECT p.did AS world_did, p.uri, p.cid, p.rev,
       p.record->>'scene' AS scene,
       p.record#>>'{post,uri}' AS post_uri,
       p.record#>>'{post,cid}' AS post_cid,
       p.record->>'author' AS author,
       p.epoch, p.place_seq
FROM (
    SELECT r.*,
           CASE WHEN jsonb_typeof(r.record#>'{place,epoch}') = 'number' THEN
               CASE WHEN (r.record#>>'{place,epoch}')::numeric BETWEEN 0 AND 9223372036854775807
                         AND trunc((r.record#>>'{place,epoch}')::numeric) = (r.record#>>'{place,epoch}')::numeric
                    THEN (r.record#>>'{place,epoch}')::numeric::bigint END
           END AS epoch,
           CASE WHEN jsonb_typeof(r.record#>'{place,seq}') = 'number' THEN
               CASE WHEN (r.record#>>'{place,seq}')::numeric BETWEEN 0 AND 9223372036854775807
                         AND trunc((r.record#>>'{place,seq}')::numeric) = (r.record#>>'{place,seq}')::numeric
                    THEN (r.record#>>'{place,seq}')::numeric::bigint END
           END AS place_seq
    FROM records r
    WHERE r.collection = 'at.motet.pilot.admission' AND r.cid IS NOT NULL
) p
WHERE p.epoch IS NOT NULL AND p.place_seq IS NOT NULL
    AND jsonb_typeof(p.record->'scene') = 'string'
    AND jsonb_typeof(p.record->'author') = 'string'
    AND jsonb_typeof(p.record#>'{post,uri}') = 'string'
    AND jsonb_typeof(p.record#>'{post,cid}') = 'string';

CREATE VIEW closings AS
SELECT r.did AS world_did, r.record#>>'{admission,uri}' AS admission_uri,
       r.record#>>'{admission,cid}' AS admission_cid,
       r.record#>>'{post,uri}' AS post_uri, r.record#>>'{post,cid}' AS post_cid,
       r.record->>'scene' AS scene, r.record->'place' AS place
FROM records r
WHERE r.collection = 'at.motet.pilot.closing' AND r.cid IS NOT NULL;

-- Select the newest admission before joining the post: an edit whose new
-- version has not arrived must not expose the superseded version again.
-- Closings pin the admission itself, so identical bytes admitted later at a
-- different place remain visible.
CREATE VIEW admitted_posts AS
WITH ranked AS (
    SELECT a.*,
           row_number() OVER (
               PARTITION BY a.world_did, a.scene, a.epoch, a.place_seq
               ORDER BY a.rev DESC, a.uri COLLATE "C" DESC
           ) AS rank
    FROM admissions a
)
SELECT a.world_did, a.scene, a.uri AS admission_uri, a.cid AS admission_cid,
       p.uri AS post_uri, p.cid AS post_cid, p.author,
       a.epoch, a.place_seq, p.record
FROM ranked a
JOIN scenes s ON s.world_did = a.world_did AND s.rkey = a.scene
JOIN posts p ON p.uri = a.post_uri AND p.cid = a.post_cid AND p.author = a.author
    AND jsonb_typeof(p.record->'world') = 'string'
    AND jsonb_typeof(p.record->'scene') = 'string'
    AND p.record->>'world' = a.world_did AND p.record->>'scene' = a.scene
WHERE a.rank = 1
    AND NOT EXISTS (
        SELECT 1 FROM closings c
        WHERE c.world_did = a.world_did
            AND c.admission_uri = a.uri AND c.admission_cid = a.cid
            AND c.post_uri = a.post_uri AND c.post_cid = a.post_cid
            AND c.scene = a.scene
            AND c.place->'epoch' = to_jsonb(a.epoch) AND c.place->'seq' = to_jsonb(a.place_seq)
    );
