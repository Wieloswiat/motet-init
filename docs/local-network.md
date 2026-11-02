# Local atproto network

The local network runs PLC, the relay, Tap, the atproto-crates PDS, the
AppView and tenor beside PostgreSQL and SeaweedFS. A TLS reverse proxy gives
them stable local HTTPS origins. No account, DID or record needs to go to the
public atproto network. The network binaries come from pinned Nix derivations,
not runtime `npx` or mutable container tags. `stack` builds OCI images from
those same derivations. [ADR 0019](adr/0019-local-tls-for-oauth.md) records
why the network uses local HTTPS origins.

The native AppView and tenor processes build with Cargo in the dev shell.
They share the incremental build cache and environment with tests, so a local
edit does not require rebuilding the Nix packages. Restart the corresponding
process after changing its code.

## Endpoints and OAuth

The dev shell exports these canonical URLs for network clients:

| Environment variable        | URL                        | Service                      |
| --------------------------- | -------------------------- | ---------------------------- |
| `DATABASE_URL`              | `postgres://…:54329/motet` | AppView development database |
| `MOTET_APPVIEW_S3_ENDPOINT` | `http://127.0.0.1:9010`    | SeaweedFS S3                 |
| `MOTET_PLC_DIRECTORY`       | `https://plc.motet.test`   | In-memory DID PLC directory  |
| `MOTET_RELAY_URL`           | `https://relay.motet.test` | Relay; metrics on port 2471  |
| `MOTET_TAP_URL`             | `https://tap.motet.test`   | Tap's acknowledged channel   |
| `MOTET_PDS_RS_URL`          | `https://pds.motet.test`   | atproto-crates PDS           |
| `MOTET_APPVIEW_URL`         | `https://motet.test`       | Web app and AppView          |
| `MOTET_TENOR_PUBLIC_URL`    | `https://tenor.motet.test` | tenor                        |

Each HTTPS origin uses the default port, 443. On the host, the proxy exposes
only loopback. It routes `motet.test`'s `/xrpc/*` and `/oauth/*` to the AppView
on port 2583; other paths go to SvelteKit on port 4174. It routes the PDS, tenor,
PLC, relay and Tap origins to their respective local ports 4800, 2584, 2582,
2470 and 2480. The proxy reads `MOTET_TLS_CERT` and `MOTET_TLS_KEY`; its native
web upstream defaults to `127.0.0.1:4174` via `MOTET_WEB_UPSTREAM`. Service
health checks may use HTTP loopback directly. Those backend ports are not the
canonical DID service endpoints.

The Motet UI and AppView share `https://motet.test`. The AppView is the
confidential OAuth client: it redirects to the user's PDS, retains tokens on
the server and receives the callback at the same web origin. Its client ID is
`https://motet.test/oauth/client-metadata.json`, its JWKS URL is
`https://motet.test/oauth/jwks.json`, and its callback is
`https://motet.test/oauth/callback`. The client ID has a different hostname
from the local PDS. Signup uses the PDS authorization flow with
`prompt=create`, rather than a separate local password login. The PDS has
`PDS_HOSTNAME=pds.motet.test`,
`PDS_SERVICE_DID=did:web:pds.motet.test` and `PDS_TRUSTED_PROXY_HOPS=1`.
The tenor's `MOTET_TENOR_HANDLE_DOMAINS=.motet.test` serves World handles;
`PDS_SERVICE_HANDLE_DOMAINS=.pds.motet.test` serves PDS account handles.
These names need local DNS as well as proxy routes and certificate coverage;
see [Host setup](#host-setup).

The native setup configures `MOTET_APPVIEW_PUBLIC_ORIGIN=https://motet.test`
and `MOTET_APPVIEW_DEFAULT_PDS=https://pds.motet.test`.
`MOTET_APPVIEW_OAUTH_SIGNING_KEY_FILE` and `MOTET_APPVIEW_SESSION_KEY_FILE`
identify the AppView's persistent secret files described below.

The AppView uses the `motet` development database and `motet-dev` archive
bucket. Integration tests create separate databases and use `motet-test`.
The tenor uses the separate `motet-tenor` bucket. Its 32-byte master key is
created once at `$DEVENV_STATE/tenor/master-key` with mode `0600`.
Keep that file with the tenor state: losing it prevents the tenor from
using existing Worlds' keys.

The native setup creates two separate 32-byte AppView files,
`$DEVENV_STATE/appview/oauth-signing-key` and
`$DEVENV_STATE/appview/session-key`. Keep them private and back them up
securely outside PostgreSQL. They must persist across AppView restarts.
Replacing the session key makes old sessions and pending logins unreadable;
replacing the signing key changes the client JWKS and may invalidate cached
authorizations. The signing key must be a valid P-256 scalar; startup checks
it. Do not silently regenerate existing keys.

The dev shell explicitly sets `MOTET_ALLOW_PRIVATE_NETWORK=true` so Motet
can contact the local services. This opt-in permits HTTP and private network
destinations broadly; it is not an allowlist of `.test` hosts. Never enable it
in production. The relay's separate `RELAY_ALLOW_PRIVATE_HOSTS=true` patch is
also for the local network only. Without these development opt-ins, production
outbound policy retains HTTPS, address filtering and TLS verification.

These are development services, not an internet-facing deployment. PDS signup
is open, email verification is disabled, and credentials are public fixtures.
The upstream PLC server binds all interfaces; keep this network behind a local
firewall. Compose publishes ports only on `127.0.0.1`.

## Host setup

The local network requires Linux with active `systemd-resolved`, a system
resolver configured to use its stub, `sudo`, available TCP port 443 and
UDP/TCP port 53535 on loopback, and control of the local `.test` names. The
browser must use the system resolver for these names. Enter the dev shell and
record the current unprivileged-port setting before setup, so you can restore
it later:

```sh
direnv allow  # or: nix develop --no-pure-eval
motet_prior_port=$(sysctl -n net.ipv4.ip_unprivileged_port_start)
printf 'Previous unprivileged port start: %s\n' "$motet_prior_port"
tls-setup
```

`tls-setup` requires `/etc/resolv.conf` to list `127.0.0.53` as its only
nameserver and checks that the resolved stub is listening. It fails before
changing the host when these prerequisites are absent; configure the host
resolver separately.
The CI gate runs the same setup and fails on an error. Its runner-specific
resolver configuration has not been verified here.

`tls-setup` creates `$DEVENV_STATE/tls/rootCA.crt`, `rootCA.key`, `server.crt`,
`server.key` and `ca-bundle.crt`. The CA signs the local service certificate;
the bundle includes both the development CA and public roots. The CA and
server private keys are local secrets: do not commit, share or deploy them.
Trusting this CA allows it to issue certificates for other names too.

Setup makes these host changes:

- It manages a block between `# BEGIN motet local development` and
  `# END motet local development` in `/etc/hosts`, mapping the six service
  names above to `127.0.0.1`.
- It creates the dummy interface `motetdns` with `169.254.253.253/32`, and
  gives `systemd-resolved` a route-only `~motet.test` domain on that interface
  with DNS server `127.0.0.1:53535`. This does not replace `/etc/resolv.conf`
  or redirect DNS for other domains. The interface and resolver settings
  disappear on reboot; rerun `tls-setup` afterward.
- On Debian/Ubuntu, it installs the CA as
  `/usr/local/share/ca-certificates/motet-dev-root.crt` and runs
  `update-ca-certificates`. On Fedora-family systems, it uses
  `/etc/pki/ca-trust/source/anchors/motet-dev-root.crt` and
  `update-ca-trust extract`. The system trust change persists.
- It imports the CA under the nickname `Motet Local Development CA` into the
  current user's Chromium NSS database: `$HOME/.pki/nssdb` if that directory
  exists, otherwise `$HOME/.local/share/pki/nssdb`. Restart Chromium after
  setup. Other browser profiles and other users may need their own CA import.
- If the current `net.ipv4.ip_unprivileged_port_start` is greater than 443,
  it sets it to 443 for the current network namespace. This allows
  unprivileged processes to bind ports 443 and above, not just Caddy. The
  setting is runtime-only and resets on reboot; run `tls-setup` again as
  needed. It does not preserve the previous value.

`devenv up` starts a loopback-only dnsmasq on port 53535. It resolves
`motet.test` and its subdomains to `127.0.0.1`, including arbitrary World and
PDS account handles. The proxy routes `*.motet.test` to the tenor and
`*.pds.motet.test` to the PDS; both wildcard domains are covered by the local
server certificate. `/etc/hosts` alone does not provide wildcard resolution.
When neither the native nor compose DNS process is running, those dynamic
names do not resolve; the route-only domain remains configured until the
interface is removed or the host reboots. A browser using external secure DNS
instead of the host resolver needs an exception for this local zone.

The development shell sets `SSL_CERT_FILE`, `CURL_CA_BUNDLE` and
`REQUESTS_CA_BUNDLE` to `ca-bundle.crt`, and `NODE_EXTRA_CA_CERTS` to
`rootCA.crt`. This supplies local CA trust to Go, Python, curl, Node and the
Rust reqwest clients without disabling certificate verification. Start them
in the configured shell, or pass the equivalent trust settings to other
clients. `NO_PROXY` and `no_proxy` keep local names off configured HTTP
proxies. Certificates and host configuration are development-only.

Run the services and web upstream in separate dev shells:

```sh
devenv up
# In another dev shell:
pnpm --filter @motet/web dev --host 127.0.0.1 --port 4174 --strictPort
```

Open `https://motet.test`, not the HTTP backend or Vite port. The OAuth
journey is configured for this origin but has not been established by the
network smoke check. A fresh clone has its own CA, but the installed system
anchor and NSS nickname are shared: rerun `tls-setup` when switching clones.

### Removing host changes

After stopping the network and any other clone using this local setup, remove
the managed block from `/etc/hosts`. Remove the runtime DNS route and dummy
interface with `tls-teardown`:

```sh
tls-teardown
```

`tls-teardown` uses sudo and only removes the scoped DNS setup. It does not
change `/etc/hosts`, CA trust or the unprivileged-port setting.

Remove the installed system CA anchor and refresh the trust store. On
Debian/Ubuntu, run:

```sh
sudo rm -f /usr/local/share/ca-certificates/motet-dev-root.crt
sudo update-ca-certificates
```

On Fedora-family systems, run:

```sh
sudo rm -f /etc/pki/ca-trust/source/anchors/motet-dev-root.crt
sudo update-ca-trust extract
```

In the dev shell, remove the Chromium NSS certificate from the database setup
selected. Remove it separately from any other browser stores where you
imported it:

```sh
if [ -d "$HOME/.pki/nssdb" ]; then
  nss_db="$HOME/.pki/nssdb"
else
  nss_db="$HOME/.local/share/pki/nssdb"
fi
certutil -D -d "sql:$nss_db" -n "Motet Local Development CA"
```

To restore the runtime port setting in the shell where you saved it, run:

```sh
sudo sysctl -w "net.ipv4.ip_unprivileged_port_start=$motet_prior_port"
```

If that shell is gone, use the value recorded before setup or the host's
configured value; reboot also resets this runtime-only change. Delete the
local `$DEVENV_STATE/tls` directory only when no services need its certificates.

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
For another local PDS, use its full HTTPS URL in the same call:

```sh
curl --fail-with-body \
  -u "admin:$MOTET_RELAY_ADMIN_PASSWORD" \
  -H 'Content-Type: application/json' \
  -d '{"hostname":"https://pds.motet.test"}' \
  "$MOTET_RELAY_URL/admin/pds/requestCrawl"
```

The PDS and tenor also announce to `https://relay.motet.test`, but these
announcements do not replace administrator registration of local hosts. A
public relay's public crawl endpoint is not intended for local URLs.

Tap runs in full-network mode on this isolated network. It enrolls every
repository seen on the relay, including Worlds' group repos, and backfills it
through `com.atproto.sync.getRepo`. A pilot-profile signal alone would miss
Worlds because their repos need not contain that collection. The output
collection filters remain `at.motet.pilot.*,group.opensocial.*`. Full-network
mode is suitable for this small local network, not for a public relay.

`TAP_DISABLE_ACKS=false` keeps acknowledgments enabled. Tap health, `/channel`
and `/info/:did` require Basic auth with user `admin` and the dev password in
`MOTET_TAP_ADMIN_PASSWORD` (`motet-network-dev`).

## Compose

`stack` builds and starts the deployment-shaped compose configuration. Run
`tls-setup` first, and stop native `devenv up`: both configurations use port
443 and the same service ports. Compose mounts the generated certificates and
server key individually, read-only, and configures its proxy for the same
canonical HTTPS origins. After replacing certificate files, recreate the
compose containers so their bind mounts see the new files. The proxy reaches
the web server on the host at `host.docker.internal:4174`; there is no web
container. In a separate dev shell, make the host web server reachable from the
container:

```sh
pnpm --filter @motet/web dev --host 0.0.0.0 --port 4174 --strictPort
# In another dev shell:
stack
```

Compose configures a separate dnsmasq in the shared PLC network namespace.
It listens on container port 53, and compose publishes it on host loopback
port 53535 for both UDP and TCP. The container clients use it for wildcard
local names; other names go to the container engine's resolver. This is a
development configuration, not a public DNS service.

Binding to `0.0.0.0` exposes that web server on host interfaces, so restrict
access with a firewall. The compose configuration is intended to share the
local TLS setup, but the complete stack and OAuth flow have not been
end-to-end verified.

## State and restart

Native state lives under `$DEVENV_STATE/network`: `relay`, `tap` and `pds-rs`.
The Rust PDS stores blob bytes with each actor's SQLite repository. Its local
lexicon catalogue comes from `lexicons/`, avoiding remote schema discovery for
Motet records.

PLC deliberately has no `DATABASE_URL`: it keeps DIDs in memory. Stopping PLC
therefore loses identity state, while the other services retain their files.
Existing accounts and Worlds cannot resolve against the new directory. To
discard the native PDS, relay and Tap state after stopping **all** processes,
run the following and then start `devenv up` again:

```sh
: "${DEVENV_STATE:?Enter the dev shell first}"
rm -rf -- "$DEVENV_STATE/network"
```

Do not remove this directory while processes are running. PostgreSQL and
SeaweedFS state are outside it and are not affected; old projections, session
data and World data can remain even though their PLC identities no longer
resolve. Recreate accounts and Worlds for the new directory as needed.
Re-running `network-smoke` without restarting PLC needs no reset. The AppView's
OAuth and session keys under `$DEVENV_STATE/appview` are separate from this
reset, as are TLS certificates and the tenor master key.

The compose atproto services, AppView, tenor and proxy share PLC's network
namespace; PostgreSQL and S3 stay on the compose bridge. After stopping
compose, remove its three network volumes before starting a new PLC instance:

```sh
podman-compose -f deploy/compose.yaml down
podman volume rm motet_relay motet_tap motet_pds-rs
```

Compose keeps the tenor's master key in `motet_tenor-key` and the AppView's
OAuth keys in `motet_appview-oauth-key`. Keep these volumes with the data they
protect; removing the tenor key loses access to existing Worlds' keys. Do not
remove PostgreSQL or S3 volumes unless their data should also be discarded.

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

The Rust PDS uses the Dockerfile's `clap,smtp,metrics,hickory-dns` features.
Its advertised service hostname is `pds.motet.test`, without a port, and its
service DID is `did:web:pds.motet.test`. The reverse proxy supplies HTTPS on
port 443; the PDS trusts one proxy hop. The AppView, not the tenor, is the
OAuth client of the user's PDS. Local OAuth and the compose deployment still
need end-to-end verification; configuring these endpoints alone does not
establish that either works.

Build just the network packages without the workspace gates:

```sh
nix build --no-pure-eval .#plc .#indigo .#pds-rs
```

`nix flake check` does not build these packages. The service-backed CI gate
builds them to start the local network. `stack` builds and loads `plc-image`,
`relay-image`, `tap-image`, `pds-rs-image`, `caddy-image` and `dnsmasq-image`,
as well as the AppView and tenor images.

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
