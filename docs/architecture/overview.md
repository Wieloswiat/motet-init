# Architecture overview

This is the map from Motet's design to its code. The design itself, with the
reasoning and the references behind it, is the backend plan ("Motet backend:
research and plan"); the decisions that shaped the code are in
[the ADRs](../adr/README.md).

## The parts

```
          people's PDSes  ──(relay, Tap)──┐        ┌── Spaces (sync, notifications)
                 ▲                        ▼        ▼
 web app ──► AppView API ──admission──► tenor ── World's repo and Spaces
   (SvelteKit)   │  indexer ◄──────────────────────┘
                 ▼
     archive (bucket) ──► projections (PostgreSQL)
```

- **The AppView** (`crates/appview`), which Motet runs. Its indexer verifies
  every record, appends it to the **archive** in object storage, then applies
  it to the PostgreSQL **projections** pages are built from. Its API serves
  Motet's XRPC methods; its **outbox** carries writes to people's PDSes and
  admission requests to tenors until each has a final outcome.
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
| Spaces are part of the design                                     | `spaces`: one Space per audience, per-reader serving decisions with a freshness bound                                                     |
| Buckets for what must last, PostgreSQL for what must be queried   | `log`: group commit, fencing, read-back reconciliation; the tenor's shard and the AppView's archive both use it                           |

## Contracts and their code

The backend plan lists implementation contracts. Each has one home, so
handlers can't forget it:

- **Endpoint authorization**: `xrpc::Registry` declares every method with its
  access rule; `xrpc::server` checks verb, caller and access before reading a
  body. A method can't be served without being declared.
- **Service auth**: `xrpc::service_auth` verifies the token against the
  issuer's `#atproto` key, the audience, the method (`lxm`) and a lifetime cap.
  A DID inside a request body is never the caller.
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
  a tenor recovery key. A creation retry uses the same reservation and signed
  genesis. Later World-scoped calls resolve the World's `#motet_tenor`
  service and check its destination. The open-scene API generates an
  operation ID for each request, so a retry after a lost response may create
  a second scene. Posting first publishes to the author's PDS. The outbox
  retries pending or uncertain admissions with the same operation ID and
  exact post version; a failed PDS write cannot trigger admission.
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
  World write tokens name the hosted World's DID, bare or with its
  `#motet_tenor` fragment, and the exact XRPC method. World creation instead
  names the tenor's bare service DID. The tenor issues no World OAuth
  sessions. Typed procedures create public scenes and change their casts; the
  World publishes the cast with the scene.
- **Serialized World commands** and **operations**: `rules::world::World::apply`
  with an `OperationId`; a retried operation returns its first outcome, a
  reused one is refused. `tenor::shard` makes outcomes durable before
  returning them.
- **Outbound HTTP**: `net::Fetcher`, the only client: HTTPS only, no redirects, non-public
  addresses refused after DNS, bounded time and size.
- **Rendering**: `render::world::Sandbox` sees only the post, bundled fonts and the
  package mirror, with a fixed date; `render::supervisor::Supervisor` runs the worker with
  a clean environment, rlimits, a wall-clock timeout and an output cap.
- **Per-reader authorization**: `spaces::serving::decide`.
- **Tap ingestion**: `appview::tap` batches verified records and identity changes.
  It acknowledges each delivery only after the archive and projections commit.
  `motet-appview serve` starts the indexer and API together; set
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

Next, in rough order: private Spaces ingestion and World themes; dice through
the tenor; recovery of the owner's exported key onto a fresh tenor; typst.ts
vector artifacts instead of static SVG
([ADR 0008](../adr/0008-rendering.md)).

The [local network](../local-network.md) runs PLC, relay, Tap, an
atproto-crates PDS, the tenor and the AppView under `devenv up`.
`network-smoke` writes two PDS profiles and reads their exact versions through
the AppView after Tap delivers them.
The compose stack has the same services; its images build and its
configuration validates, but it hasn't been run as a stack yet. The reference
TypeScript Spaces-alpha PDS and cross-implementation coverage remain deferred
([ADR 0017](../adr/0017-rust-pds-local-network.md)).
