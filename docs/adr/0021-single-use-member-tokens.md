# 0021. Consume member service tokens once

Status: Accepted. Narrows the bearer-token assumptions of
[0018](0018-world-service-auth.md).

## Context

A method-bound bearer token does not bind the request body. A recipient who
obtains it could replay it with different arguments and create an audit entry
attributing another action to its issuer. A short expiry limits the window but
does not make that attribution reliable.

## Decision

For a member-authorized World mutation, including World reservation and
creation, the service token must contain a nonempty `jti`. The tenor records
the verified issuer and `jti` in the same shard transaction as the mutation
and refuses their reuse until the token expires. An aborted transaction does
not spend the token. Service tokens live for at most 60 seconds. Expired
replay entries can be removed after expiry.

A client that retries an operation obtains a fresh service token, even when
it keeps the same operation ID and body. The operation ID still gives an
idempotent result; the token identifies one authenticated attempt. Read-only
queries and Space protocol notifications do not consume this mutation token.

## Consequences

The ledger survives shard replay alongside the decisions it protects. It
prevents a used token from authorizing a second mutation, including one with
a changed body. It does not bind a token to a body or prevent someone who
steals an unused bearer token from spending it first. Clients and services
must still protect tokens in transit and avoid logging them.
