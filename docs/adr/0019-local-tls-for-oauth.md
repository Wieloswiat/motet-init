# 0019. Local TLS for PDS OAuth

Status: Accepted. Supersedes only the plain-HTTP local endpoint assumptions in
[0016](0016-local-atproto-network.md) and
[0017](0017-rust-pds-local-network.md). Their other decisions remain in force,
including 0017's PDS coverage and relay patch scope.

## Context

The AppView is a confidential atproto OAuth client of the user's PDS. Its
client metadata, PDS authorization server and callback need stable HTTPS
origins. The pinned atproto service validators reject `.localhost` service
names but accept `.test` names. HTTP loopback endpoints and a port-bearing
`did:web:localhost` therefore cannot represent this OAuth configuration.

## Decision

Use a locally generated development CA and HTTPS on port 443 for the canonical
origins: `https://motet.test`, `https://pds.motet.test`,
`https://tenor.motet.test`, `https://plc.motet.test`,
`https://relay.motet.test` and `https://tap.motet.test`. Scoped wildcard DNS
maps World and PDS account handle names to the local proxy. It routes World
handles under `*.motet.test` to the tenor, and account handles under
`*.pds.motet.test` to the PDS. The proxy routes `/xrpc/*` and `/oauth/*` on
`motet.test` to the AppView and other web paths to SvelteKit. The Motet UI
and AppView consequently share one origin; the PDS authorization page remains
on the PDS origin. Only the AppView exchanges OAuth tokens with the PDS.
Local service readiness checks may still use HTTP loopback.

The local network opts into private-network access explicitly. That exception
is for isolated development services, not production. Production outbound
requests retain HTTPS, address filtering and certificate verification. Local
clients and Chromium trust the development CA rather than disabling TLS
verification. The PDS uses `pds.motet.test` and `did:web:pds.motet.test`,
with the reverse proxy as its single trusted proxy hop.

## Consequences

Setup changes the host's local DNS, port and trust configuration and creates
private keys under the dev environment's state directory. The local CA can
issue certificates trusted on that host, so neither the CA nor server keys
belong in the repository or production. Port 443 must be available. The
local-network guide describes the setup, trust, state reset and cleanup.

This decision does not establish that the OAuth journey or compose deployment
has passed an end-to-end test.
