# Architecture overview

This is the map from Motet's design to its code. The design itself, with the
reasoning and the references behind it, is the backend plan ("Motet backend:
research and plan"); the decisions that shaped the code are in
[the ADRs](../adr/README.md).

## The parts

```text
 people's PDSes and World's public repo ──► relay ──► Tap ──► indexer
                                                              │
                                               projections ◄── archive
 web app ◄── HTTPS ──► AppView API ──► outbox worker ──► PDSes / tenor
                            └── member service auth ──► tenor ──► World's repo and Spaces
```

- **The AppView** (`crates/appview`), which Motet runs. Its indexer verifies
  records, appends them to the **archive** in object storage, then applies
  them to the PostgreSQL **projections** pages are built from. Its API serves
  Motet's XRPC methods. Its outbox worker carries writes to people's PDSes
  and admission requests to tenors until each has a final outcome.
- **The tenor** (`crates/tenor`), which anyone can run. It hosts each World as
  a group account ([ADR 0013](../adr/0013-tenor-hosts-worlds.md)): the
  World's `did:plc` (the owner holds the first rotation key), its signed
  public repo and firehose, and its Spaces as authority and host. Members act
  as the World using service tokens from their own PDS
  ([ADR 0018](../adr/0018-world-service-auth.md)). Every change for a World
  runs on its shard's one writer, is made durable in a fenced log in the
  tenor's bucket, then lands in the shard's SlateDB
  ([ADR 0014](../adr/0014-tenor-state-in-slatedb.md)).
- **Workers**: Typst rendering (`crates/render`) runs in a separate,
  resource-limited process because its input is untrusted. An image proxy,
  email and exports come later.
- **The web app** (`apps/web`) talks only to the AppView. Sign-in, World
  creation and public scenes use real data; the remaining design-export
  studies are labelled on `/design` ([ADR 0020](../adr/0020-appview-backed-web-screens.md)).
  Real screens use Motet's default appearance until World theme records are
  projected; the design engine can compile those records into custom properties.
- **The Theme Creator** (`apps/theme-creator`) edits a theme record. It is
  the exported tool, not yet connected to anything.

## Five ideas, and where each lives

| Idea                                                              | Where it is enforced                                                                                                                      |
| ----------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------- |
| Repositories hold the truth; projections rebuild from the archive | `appview::projection::apply` is the only writer of projections, for the live indexer and for a rebuild alike; `tests/rebuild.rs`          |
| A post counts once the World admits it                            | `rules::world` decides; `tenor::admission` reads the record back from its author's repo first; `rules::view` shows only what was admitted |
| A World is a group account on its tenor                           | `tenor::{identity,keys,repo,sync,spaces,authorization}`: hosted repos, role-backed Spaces and member-attributed service writes            |
| Spaces are part of the design                                     | `spaces` defines audiences and per-reader serving decisions; private AppView ingestion and serving remain future work                     |
| Buckets for what must last, PostgreSQL for what must be queried   | `log`: group commit, fencing, read-back reconciliation; the tenor's shard and the AppView's archive both use it                           |

## Contracts and their code

The backend plan lists implementation contracts. Each has one home, so
handlers can't forget it:

- **Endpoint authorization**: `xrpc::Registry` declares every method with its
  access rule; `xrpc::server` checks verb, caller and access before reading a
  body. A method can't be served without being declared.
- **Service auth**: `xrpc::service_auth` checks audience, method (`lxm`) and
  lifetime before resolving the issuer, then verifies its `#atproto` key.
  DID documents are cached for at most five seconds; a DID inside a request
  body is never the caller. Resolver failures return 503 `UpstreamFailure`
  without consuming the token; invalid credentials remain 401.
- **AppView sessions**: `appview::auth` is the confidential atproto OAuth
  client for the reader's PDS. The web app and AppView share one HTTPS
  origin, with `/oauth/*` and `/xrpc/*` routed to the AppView. The browser
  receives an opaque, host-only, HttpOnly, Secure, SameSite=Lax cookie;
  tokens, DPoP keys and pending PKCE state are sealed before they reach
  PostgreSQL. Session refresh is serialized across instances. Sign-out first
  revokes the local session and also requests remote revocation when the PDS
  advertises it.
  Sign-in and signup start with a same-origin, form-encoded POST; the AppView
  checks `Origin` and returns `authorizeUrl` as JSON. The browser navigates to
  that URL separately, so the web app keeps `form-action 'self'`. A callback
  consumes browser-bound state and revokes any session it replaces.
- **AppView write grants**: OAuth requests `atproto`, repo access for Motet
  posts, profiles and characters, and one `rpc:` permission per tenor method,
  each with `aud=*`. The [permission specification](https://atproto.com/specs/permission#rpc)
  permits a wildcard audience when the method is specific. The pinned Rust
  PDS checks the requested method and audience when minting
  `com.atproto.server.getServiceAuth` tokens. The AppView requests the World
  or tenor DID audience and exact method required by the endpoint.
- **AppView write routing**: `MOTET_APPVIEW_TENOR_URL` and
  `MOTET_APPVIEW_TENOR_DID` select the default tenor for World creation.
  The browser supplies the sole rotation key; the AppView does not opt in to
  a tenor recovery key. An owner-scoped operation ID recovers the same live
  reservation after a lost response; creation retries use the signed genesis.
  Later World-scoped calls resolve the World's `#motet_tenor`
  service and check its destination. The open-scene API forwards a
  client-chosen operation ID, so retries of the same request return the
  original scene. Posting uses an author-scoped client TID as the create-only
  PDS record key. The outbox compares a read-back after an
  uncertain PDS response before requesting admission of that exact version.
  It retries admissions with the same operation ID and fresh service tokens.
  After six unsuccessful attempts, admission fails visibly and later posts
  can proceed. A recorded outcome remains retrievable after membership
  changes; a failed PDS write cannot trigger admission. Only the signed-in
  author can query a published post's final admission outcome.
- **OAuth key custody**: `MOTET_APPVIEW_PUBLIC_ORIGIN` is the canonical public
  HTTPS origin, and `MOTET_APPVIEW_DEFAULT_PDS` selects the PDS for signup.
  `MOTET_APPVIEW_OAUTH_SIGNING_KEY_FILE` names a P-256 private scalar, and
  `MOTET_APPVIEW_SESSION_KEY_FILE` names a separate 32-byte sealing key. Each
  file accepts raw bytes or standard base64. Keep both keys private, persistent
  and backed up outside PostgreSQL. Losing or replacing the sealing key
  invalidates stored sessions and pending logins.
- **World authorization**: `tenor::authorization` checks the verified member
  issuer's current World roles and opensocial grants in the shard transaction,
  and appends the member-attributed audit record in the GM Space atomically.
  World mutations and creation consume a service-token `jti` once in that
  transaction; tokens live at most 60 seconds and retries use fresh tokens
  ([ADR 0021](../adr/0021-single-use-member-tokens.md)). World write tokens
  name the hosted World's DID, bare or with `#motet_tenor`, and the exact
  XRPC method. World creation instead names the tenor's bare service DID.
  The tenor issues no World OAuth sessions. Typed procedures create public
  scenes and change their casts; the World publishes the cast with the scene.
  Locally issued delegations for hosted Spaces retain their requesting member
  for revocation. A remote Space host cannot read that local origin binding.
- **Serialized World commands** and **operations**: `rules::world::World::apply`
  with an `OperationId`; a retried operation returns its first outcome, a
  reused one is refused. `tenor::shard` makes outcomes durable before
  returning them.
- **Outbound HTTP**: `net::Fetcher` uses HTTPS, refuses redirects and non-public
  addresses after DNS, and bounds time and size. The local network opts into
  private addresses and HTTP for development only.
- **Rendering**: `render::world::Sandbox` sees only the post, bundled fonts and the
  package mirror, with a fixed date; `render::supervisor::Supervisor` runs the worker with
  a clean environment, rlimits, a wall-clock timeout and an output cap.
- **Per-reader authorization**: `spaces::serving::decide` defines the
  freshness decision; private AppView serving does not yet use it.
- **Tap ingestion**: `appview::tap` batches verified records and identity changes.
  It acknowledges each delivery only after the archive and projections commit.
  `motet-appview serve` starts the indexer, API and outbox worker; set
  `MOTET_APPVIEW_TAP_URL` to enable ingestion and optionally
  `MOTET_APPVIEW_TAP_ADMIN_PASSWORD` for authentication. HTTPS selects TLS.
  Without a Tap URL, the API still serves after archive catch-up.

## Still to come

The AppView projects public World declarations and scenes, and shows posts
only when the World admits their exact version. It reads a public World's
current meta Space profile through a credential-bound Space request and
archives the snapshot before projecting it. A previously public name can
remain cached until Tap delivers the declaration's removal; private profile
updates are not read. Private Spaces and private scenes are not indexed yet.
A World with neither a public declaration nor a public scene cannot be
looked up from its public repo. A declaration identifies a group but not
necessarily a Motet World.

Next, in rough order: private scene and Space ingestion in the AppView; World
themes; dice through the tenor; World export and recovery of the owner's
exported key onto a fresh tenor; and typst.ts vector artifacts instead of
static SVG ([ADR 0008](../adr/0008-rendering.md)). Deferred Space security
findings also need resolution before private scenes can be relied on; a remote
Space host cannot read the local delegation origin binding noted above.

The [local network](../local-network.md) runs PostgreSQL, SeaweedFS, DNS,
the HTTPS proxy, PLC, relay, Tap, an atproto-crates PDS, the tenor and the
AppView under `devenv up`. The web server runs separately.
`network-smoke` writes two PDS profiles and reads their exact versions through
the AppView after Tap delivers them.
The reference TypeScript Spaces-alpha PDS and cross-implementation coverage
remain deferred ([ADR 0017](../adr/0017-rust-pds-local-network.md)).
