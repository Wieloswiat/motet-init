# 0018. Authorize World actions with member service tokens

Status: Accepted. Supersedes the World-session and tenor OAuth-server parts of
[0013](0013-tenor-hosts-worlds.md), and the group OAuth-server assumption in
[0015](0015-opensocial-group.md).

## Context

The tenor hosts a World's identity, repositories and Spaces, but an action as
the World must still identify the person who took it. ADR 0013 chose OAuth
sessions issued by the tenor for the World's DID, after the GM signed in at
their own PDS. That requires a second OAuth server, a second login and a
session whose authority can outlive the role which created it. Tangled's
[knot architecture](https://tangled.org/tangled.org/core/blob/9934837f0a9153d87a9558c938b1e654109556cf/docs/src/content/docs/architecture/data-flow.mdx)
instead uses short-lived service tokens signed by a person's PDS when its
AppView calls a knot.

## Decision

- The AppView authenticates the person at their own PDS. For each World action
  it obtains a service-auth JWT from that PDS for the exact XRPC method. Other
  clients can obtain and submit the same kind of token. The tenor verifies the
  issuer's `#atproto` key, lifetime, audience and `lxm`, and takes the acting
  member only from the verified `iss` claim. A request-body DID never asserts
  the caller's identity.
- A World-scoped method's audience is the hosted World's DID, either bare or
  with its `#motet_tenor` service fragment. The tenor selects the World from
  that audience and refuses requests for unhosted Worlds or inconsistent
  World, repo or Space targets. This also supports standard atproto-proxy
  calls, for which a PDS mints the proxied DID as the audience. The exact
  `lxm` must equal the called method. Before a World exists, `reserveWorld`
  and `createWorld` instead require the tenor's bare service DID as audience;
  for example, `did:web:tenor.motet.test` locally. A port in a `did:web`
  authority is percent-encoded as `%3A`.
- Each World mutation reads the actor's current membership, surviving roles and
  opensocial grants within the shard transaction. It also writes an audit
  record in the GM Space with the member DID in the same transaction. A token
  cannot preserve authority after a role is removed. Author-driven admission
  is additionally governed by the current scene cast and sanctions.
- A public scene is opened with `at.motet.pilot.play.openScene`. Its opener is
  always in its cast, and the World publishes that cast in the public scene
  record. Scene changes go through typed methods, not generic writes.
  The default owner and GM grants include the scene procedures. To permit a
  player role to open scenes, add `rpc:at.motet.pilot.play.openScene` to that
  role's grant in the World's `group.opensocial.permissions` record in its
  members Space. Grant the cast and close procedures separately if desired.
- The tenor does not issue World OAuth sessions or authenticate a member at
  their PDS. It does not mint a World token for downstream services.

## Owner signature

The owner supplies a public P-256 or K-256 `did:key` as the first PLC rotation
key. `reserveWorld` returns `signingBytes`, the unpadded base64url encoding of
the canonical DAG-CBOR serialization of the unsigned PLC genesis operation,
including `prev: null` and excluding `sig`. For P-256, sign the decoded bytes
using ECDSA with SHA-256. The signature is the 64-byte compact IEEE P1363
encoding: 32-byte big-endian `r` followed by 32-byte big-endian `s`, not DER.
Normalize `s` to the lower half of the P-256 group order, encode the signature
as unpadded base64url, and add it as `sig` to the otherwise unchanged
operation. The tenor verifies the first rotation key and rejects a high-S
signature. WebCrypto hashes the supplied message itself; do not prehash it.

## Consequences

Role changes take effect on the next write, and audit attribution does not
rely on a shared World credential. The caller or AppView must obtain a new
method-bound token from the member's PDS. Tokens are bearer credentials for
their short lifetime and must be protected accordingly. Unlike the group-host
OAuth model, this decision does not offer third-party clients OAuth sessions
whose subject is the World; such clients must use the member-token procedures.
The AppView's ordinary sign-in session is separate and is not a tenor-issued
World session.
