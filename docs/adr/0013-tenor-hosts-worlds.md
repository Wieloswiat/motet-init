# 0013. The tenor hosts Worlds itself, on atproto-crates' protocol crates

Status: Accepted. Supersedes the crates.io half of [0004](0004-atproto-crates.md).

## Context

A World is a group account on its tenor (the backend plan, "Worlds and
tenors"): its own `did:plc`, a public repo with signed commits and a firehose
relays crawl, Space authority and host for its Spaces, and sessions that let
GMs act as the World with their own accounts. Groundwork asked whether to
build that from vlpds or from atproto-crates (checked 2026-10-26):

- **vlpds** (v1.2.3, MIT) is a complete PDS whose only durable store is a
  bucket. Its storage design is the closest match to the tenor's, but it is
  one server crate over its own atproto implementation (`vlatproto`, with its
  own CID, MST and CAR types), its Spaces follow the October 1 alpha (no STAR
  repos, no KEM keys, no MAC headers), and it has no notion of one account
  acting as another. Embedding it means adopting its whole object graph, and
  its types can't meet atproto-crates' anywhere in Motet.
- **atproto-crates** (0.15.0-rc.6 at `09a868cd`, MIT, the 2026-10-22 Spaces
  alpha) splits into protocol crates with no storage or server attached
  (`atproto-repo`'s MST and commits, `atproto-space`'s authority and repo
  primitives behind storage traits, `atproto-identity`'s PLC builder,
  `atproto-oauth`'s DPoP, JWT and scope primitives, `atproto-xrpcs`,
  `atproto-tap`) and a PDS (`atproto-pds`, unpublished) whose repo writer,
  sequencer, Space services and OAuth server need its SQLite account database
  and its HTTP state.
- Neither has group accounts. opensocial.group says the group host is the
  OAuth authorization server for the group's DID and checks each member's
  roles on every write ([0015](0015-opensocial-group.md)).

## Decision

- **The tenor is its own account host**, assembled from atproto-crates'
  protocol crates over the tenor's own storage
  ([0014](0014-tenor-state-in-slatedb.md)). It serves the World's repo and
  sync methods, its firehose, its Spaces as authority and host, handle
  resolution, and an OAuth authorization server for World DIDs.
- **vlpds is the reference for storage**, not a dependency: SlateDB with its
  write-ahead log off, the node's log as the only WAL, an applied marker per
  batch, checkpoints by memtable flush.
- **Small pure modules of `atproto-pds` are ported, not depended on**: the
  `#commit` CAR builder, firehose frame and payload encoding, and the PLC
  genesis ordering. They go into the tenor with attribution and an
  `UPSTREAM.md` naming the source revision and every local change, as
  `packages/design` does. Depending on `atproto-pds` would pull in its
  mandatory SQLite and HTTP features and an API its README calls unstable.
  The Space-host and OAuth server logic is written in Motet on the protocol
  crates, following `atproto-pds`'s handlers as the reference.
- **Every atproto crate comes from one git revision** of atproto-crates,
  pinned in the workspace and allowed in `deny.toml`. 0.15 isn't on
  crates.io, and mixing 0.14.5 from crates.io with 0.15 from git would
  duplicate the core types. reqwest follows atproto-crates to 0.13, so their
  clients still take `motet-net`'s hardened client.
- **Owner-held rotation key first.** A World's genesis lists the owner's
  rotation key first and, only if the owner opts in, a tenor-held key after
  it. The owner signs the genesis on their device: the tenor reserves the
  World's signing and KEM keys, returns the unsigned operation, and submits
  it to PLC once the owner has signed it.
- **GMs act as the World** through service auth for Motet's typed methods,
  and through OAuth sessions the tenor issues for the World's DID after the
  GM authenticates at their own PDS. Each session records the GM, and each
  write is checked against the GM's current roles.

## Consequences

- The tenor owns several thousand lines that a PDS library would otherwise
  supply: commit pipeline, sync endpoints, Space host handlers, an OAuth
  server. In exchange there is one atproto type system and one storage design
  in Motet.
- Ported modules can drift from upstream; `UPSTREAM.md` is the checklist when
  the pinned revision moves.
- CI and image builds need network access to tangled.org.
- Sync 1.1 inductive verification exists nowhere in atproto-crates; the
  tenor publishes `prevData` and `since`, and verifying them is a consumer's
  problem.

## Revisit when

atproto-crates publishes a storage-agnostic repo writer, sequencer, Space
host or OAuth server (drop the port or the Motet code for it), or vlpds moves
to the current Spaces alpha and splits out libraries over shared types.
