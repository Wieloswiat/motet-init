# 0006. Fenced logs in object storage; SeaweedFS for development

Status: Accepted

## Context

Tenors must be easy to self-host and their nodes disposable; the AppView's
projections must rebuild without re-crawling the network. Both need a
durable, ordered record of what was decided, in storage a self-hoster can
rent (R2, S3).

## Decision

- `motet-log`: segments in a bucket, written with conditional puts so exactly
  one writer can append at a given position. A second writer fences the
  first. A write whose outcome is unknown is reconciled by reading it back,
  never retried blindly. Writers batch (group commit), and nothing is
  acknowledged before its segment is durable.
- The tenor keeps each shard's World commands in such a log
  (`tenor::shard`); state in memory is what replaying it gives. SlateDB for
  current state and replay checkpoints comes later and doesn't change this
  contract.
- The AppView archives every verified record to a log before applying it to
  PostgreSQL.
- Development and CI use SeaweedFS's S3 gateway. MinIO was abandoned upstream
  and nixpkgs marks it insecure. The fencing tests double as a check that the
  store honours conditional writes.

## Revisit when

Multi-node tenors (takeover between nodes on one bucket), and log compaction
with erasure requests.
