# 0014. Tenor state in SlateDB, with the shard log as its only WAL

Status: Accepted. Builds on [0006](0006-storage.md), which deferred SlateDB.

## Context

0006 made each shard's fenced log the tenor's durable record and kept state in
memory, rebuilt by replaying from the start, with "SlateDB for current state
and replay checkpoints" to come. Hosting World accounts
([0013](0013-tenor-hosts-worlds.md)) makes that state large: repo blocks,
record indexes, firehose history, sealed keys, Space repos and members. Replay
from zero on every start no longer works, and a repo commit can't be replayed
by re-running it, since signing and revisions aren't functions of the command.

## Decision

- **One SlateDB (`=0.17.0`) per shard**, under `state/{shard}/` in the same
  bucket and through the same object-store client as the log, with SlateDB's
  own WAL disabled (`wal_disable`). The log stays the only write-ahead log, as
  in vlpds.
- **The log records effects.** Rules commands are still logged as commands
  and replayed by re-running them, since they are deterministic. Every other
  operation (World creation, key custody, repo commits, Space changes, OAuth
  state) is logged as the key-value mutations and firehose events it
  produced, and replay re-applies them.
- **One writer runs every operation**, in order, against a staged overlay;
  the batch is appended to the log as one segment, then written to SlateDB as
  one batch with the applied marker and the segment's position as SlateDB's
  sequence number, and only then are the outcomes returned. Acknowledged
  still means durable in the log; SlateDB can only be behind it, never ahead.
- **Checkpoints** write the rules snapshots and the applied marker, then
  flush the memtable, every ~10 seconds when anything changed, which bounds
  replay.
- **Both fences stop the shard**: the log's create-only segment and SlateDB's
  writer epoch. A SlateDB error after a durable append is fail-stop, and
  replay repairs it.
- Sealed key material lives in the same store, sealed under the tenor's
  master key with XChaCha20-Poly1305. The master key is kept outside the
  bucket.

## Consequences

- Upgrading SlateDB is stepwise: it only promises storage compatibility
  between adjacent versions, so the pin is exact and an upgrade needs a
  format fixture test.
- SeaweedFS isn't in SlateDB's tested store list. The integration tests
  (open, write, checkpoint, reopen; takeover) are what establish that it
  works for us.
- Log compaction becomes possible below the applied marker; it isn't built
  yet.

## Revisit when

SlateDB's pluggable WAL (RFC 0030) is stable and the log could be that WAL
directly, or multi-node tenors need vlpds's lease and span machinery.
