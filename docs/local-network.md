# Local atproto network

`devenv up` starts PLC, relay, Tap, the atproto-crates PDS, AppView and tenor
beside PostgreSQL and SeaweedFS. No account, DID or record is sent to the
public atproto network. The four network binaries come from pinned Nix
derivations, not runtime `npx` or mutable container tags. `stack` builds OCI
images from those same derivations.

The native AppView and tenor processes build with Cargo in the dev shell.
They share the incremental build cache and environment with tests, so a local
edit does not require rebuilding the Nix packages. Restart the corresponding
process after changing its code.

## Endpoints

The dev shell exports these URLs for tests and clients:

| Environment variable        | URL                        | Service                      |
| --------------------------- | -------------------------- | ---------------------------- |
| `DATABASE_URL`              | `postgres://…:54329/motet` | AppView development database |
| `MOTET_APPVIEW_S3_ENDPOINT` | `http://127.0.0.1:9010`    | SeaweedFS S3                 |
| `MOTET_PLC_DIRECTORY`       | `http://localhost:2582`    | In-memory DID PLC directory  |
| `MOTET_RELAY_URL`           | `http://localhost:2470`    | Relay; metrics on port 2471  |
| `MOTET_TAP_URL`             | `http://localhost:2480`    | Tap's acknowledged channel   |
| `MOTET_PDS_RS_URL`          | `http://localhost:4800`    | atproto-crates PDS           |
| `MOTET_APPVIEW_URL`         | `http://localhost:2583`    | AppView                      |
| `MOTET_TENOR_PUBLIC_URL`    | `http://localhost:2584`    | tenor                        |

The AppView uses the `motet` development database and `motet-dev` archive
bucket. Integration tests create separate databases and use `motet-test`.
The tenor uses the separate `motet-tenor` bucket. Its 32-byte master key is
created once at `$DEVENV_STATE/tenor/master-key` with mode `0600`.
Keep that file with the tenor state: losing it prevents the tenor from
using existing Worlds' keys.

The shell and compose set `MOTET_TENOR_PUBLIC_URL=http://localhost:2584`,
`MOTET_TENOR_HANDLE_DOMAINS=.motet.test` and
`MOTET_TENOR_CRAWLERS=http://localhost:2470`. Worlds can therefore use handles
such as `demo.motet.test`. `MOTET_ALLOW_PRIVATE_NETWORK=true` permits Motet's
outbound client to reach these local services; never use it in production.

These are development services, not an internet-facing deployment. Signup is
open, SSRF protection is explicitly relaxed for local crawling, and credentials
are public fixtures. The upstream PLC server binds all interfaces; keep this
network behind a local firewall. Compose publishes ports only on `127.0.0.1`.

## Smoke check

In one dev shell, run `devenv up`. In another, run:

```sh
network-smoke
```

The check creates fresh Alice and Bob accounts on the Rust PDS, writes each
account's `at.motet.pilot.profile/self` record, and requests a relay crawl of
the PDS. It waits for the AppView's `getRecordVersion` query to return both
exact record URIs, CIDs and values. Fresh accounts and values ensure old
projections or PLC identities cannot make a later run pass.

This protects the cross-service ingestion property: a private-address dialer,
missed crawl, wrong collection filter or stalled AppView consumer must not
silently lose a pilot's profile. Tap's acknowledged channel is shared rather
than broadcast, so only the AppView consumes it. The smoke check never opens
the channel itself. It does not test a second PDS implementation, OAuth
sign-in or private Space access.

The local stack registers the Rust PDS and tenor through the relay's
authenticated admin endpoint. `network-smoke` also requests the PDS crawl.
For another local PDS, use its full HTTP URL in the same call:

```sh
curl --fail-with-body \
  -u "admin:$MOTET_RELAY_ADMIN_PASSWORD" \
  -H 'Content-Type: application/json' \
  -d '{"hostname":"http://localhost:4800"}' \
  "$MOTET_RELAY_URL/admin/pds/requestCrawl"
```

The PDS and tenor also announce to `http://localhost:2470`, but these
announcements do not replace administrator registration of local hosts. A
public relay's public crawl endpoint is not intended for localhost URLs.

Tap runs in full-network mode on this isolated network. It enrolls every
repository seen on the relay, including Worlds' group repos, and backfills it
through `com.atproto.sync.getRepo`. A pilot-profile signal alone would miss
Worlds because their repos need not contain that collection. The output
collection filters remain `at.motet.pilot.*,group.opensocial.*`. Full-network
mode is suitable for this small local network, not for a public relay.

`TAP_DISABLE_ACKS=false` keeps acknowledgments enabled. Tap health, `/channel`
and `/info/:did` require Basic auth with user `admin` and the dev password in
`MOTET_TAP_ADMIN_PASSWORD` (`motet-network-dev`).

## State and restart

Native state lives under `$DEVENV_STATE/network`: `relay`, `tap` and `pds-rs`.
The Rust PDS stores blob bytes with each actor's SQLite repository. Its local
lexicon catalogue comes from `lexicons/`, avoiding remote schema discovery for
Motet records.

PLC deliberately has no `DATABASE_URL`: it keeps DIDs in memory. Stopping PLC
therefore loses identity state, while the other services retain their files.
Existing accounts and Worlds cannot resolve against the new directory. For a
fresh network after stopping **all** processes, remove only its generated
state, then start `devenv up` again:

```sh
: "${DEVENV_STATE:?Enter the dev shell first}"
rm -rf -- "$DEVENV_STATE/network"
```

Do not remove this directory while processes are running. PostgreSQL and
SeaweedFS state are outside it and are not affected. Re-running `network-smoke`
without restarting PLC needs no reset.

The compose services share PLC's network namespace. This preserves canonical
`http://localhost:<port>` DID service endpoints across all network services,
the AppView and tenor, while keeping PostgreSQL and S3 on the compose bridge.
Do not start compose and native services together: they use the same ports.
After stopping compose, remove its three network volumes before starting a new
PLC instance (`motet_relay`, `motet_tap`, `motet_pds-rs`). Compose keeps the
tenor's master key in the `motet_tenor-key` volume. Keep that volume with the
tenor's S3 data; removing the key loses access to existing Worlds' keys. Do not
remove the PostgreSQL or S3 volumes unless their data should also be discarded.

## Reproducible sources and compatibility

| Package  | Pinned source                                                                                                                                                                              |
| -------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `plc`    | [did-method-plc `ed532fadf23c524c0dfddc2e7c2b7da85b432ef5`](https://github.com/did-method-plc/did-method-plc/commit/ed532fadf23c524c0dfddc2e7c2b7da85b432ef5), `packages/server`           |
| `indigo` | [Indigo `4f47add43060c27e8a37d9d76482ecddf001fcd8`](https://github.com/bluesky-social/indigo/commit/4f47add43060c27e8a37d9d76482ecddf001fcd8), `cmd/relay` and `cmd/tap` (`tap-v0.1.10`)   |
| `pds-rs` | [atproto-crates `09a868cdde938c789326f1db16ffc30bbf38c15e`](https://tangled.org/ngerakines.me/atproto-crates/commit/09a868cdde938c789326f1db16ffc30bbf38c15e), `atproto-pds`, binary `pds` |

PLC uses the flake's `pnpm_11` and a fixed dependency closure.
`pnpm_config_pm_on_fail=ignore` prevents the upstream package-manager pin from
self-downloading in the sandbox. Containers reuse the native builds without a
separate dependency graph or external image fallback.

`nix/patches/indigo-private-hosts.patch` changes relay behavior only when
`RELAY_ALLOW_PRIVATE_HOSTS=true`. It permits local authorities and removes the
public-only dialer's address restriction. For a new local host, it also starts
at cursor zero: retained events must not disappear in a race with the first
crawl. Saved positive cursors, default host checks and admin authentication
stay intact. At this exact Indigo revision, Tap's HTTP and
WebSocket transports already permit private hosts, so it needs no patch or
ineffective `TAP_ALLOW_PRIVATE_HOSTS` setting.

The Rust PDS uses the Dockerfile's `clap,smtp,metrics,hickory-dns` features. Its
`PDS_HOSTNAME` must be `localhost:4800`: it does not append `PDS_PORT` to
advertised account endpoints. Its service DID is
`did:web:localhost%3A4800`. This fixes PLC account endpoints and relay crawling;
upstream's service DID/OAuth metadata still assumes HTTPS and is not a working
local HTTP OAuth setup.

Build just the network packages without the workspace gates:

```sh
nix build --no-pure-eval .#plc .#indigo .#pds-rs
```

`nix flake check` does not build these packages. The service-backed CI gate
builds them to start the local network. `stack` builds and loads `plc-image`,
`relay-image`, `tap-image` and `pds-rs-image`, as well as the AppView and
tenor images.

## Deferred reference TypeScript PDS

The reference Spaces-alpha PDS is not shipped or started yet
([ADR 0017](adr/0017-rust-pds-local-network.md)). Its intended source is atproto
merge `5bfe38fa0a3a5e214db42058b6906099c64a466a` from October 22, 2026.
The npm release `@atproto/pds@0.0.0-spaces-alpha-20261022141228` is attested to
release commit `ac0c9f15392f6f0298d71dc52aa88c05036150be`; package sources and the
dependency lock match that merge, with release metadata and Dockerfile changes.

The attempted source build fetched its pinned dependency closure and reached
OAuth UI message extraction, but exhausted the build host's memory. Its final
binary and runtime interoperability were not verified. Adding a reproducible,
verified reference PDS and restoring two-implementation smoke coverage remain
follow-up work. No unverified package, process, image output or runtime `npx`
fallback is included.
