# Motet

Play-by-forum roleplaying on atproto. People's posts and characters live in
their own repositories; each World is a group account hosted by a **tenor**,
which admits posts into the World's scenes, rolls dice and keeps the World's
private Spaces. Motet's **AppView** indexes all of it and serves the web app.

The design is in [docs/architecture/overview.md](docs/architecture/overview.md);
the decisions behind it are in [docs/adr](docs/adr/README.md).

## Getting started

Everything comes from the nix flake: toolchains, PostgreSQL with pg_search,
S3 storage, and a [local atproto network](docs/local-network.md). `tls-setup`
uses sudo to change local DNS, CA trust and port permissions; read the
[host setup](docs/local-network.md#host-setup) before running it, and record
the previous port setting. With [nix](https://nixos.org) and
[direnv](https://direnv.net):

```sh
direnv allow          # or: nix develop --no-pure-eval
tls-setup             # local DNS, port 443 and CA trust; uses sudo, rerun after reboot
devenv up             # in a terminal: database, storage, atproto services and proxy
```

In another dev shell, start the web upstream and open
[`https://motet.test`](https://motet.test):

```sh
pnpm --filter @motet/web dev --host 127.0.0.1 --port 4174 --strictPort
```

Run `tests` for the default suites. Run `tests-integration` with `devenv up`
running, or stop the stack and run `devenv test` to start its services for the
full CI gate. Stop the separate web server first: Playwright starts its own
preview on port 4174. Both commands require TLS setup as above. The
[local network guide](docs/local-network.md) describes host changes, CA and
browser trust, DNS, service origins and how to reset state.

`devenv info` lists every command. The ones used day to day:

| Command                          | What it does                                                                                    |
| -------------------------------- | ----------------------------------------------------------------------------------------------- |
| `fmt`                            | Format Rust, Nix, TOML and the web packages                                                     |
| `lint`                           | Clippy and rustdoc (warnings are errors), cargo-deny, typos, workflow and web lint, type checks |
| `tls-setup`                      | Set up local HTTPS, DNS and CA trust before starting the network                                |
| `tls-teardown`                   | Remove the local wildcard DNS route and dummy interface                                         |
| `tests`                          | Pure, property and in-process tests; vitest. No services needed                                 |
| `tests-integration`              | Service-backed tests and Playwright against the complete local stack (needs `devenv up`)        |
| `network-smoke`                  | Two local PDS profiles reach the AppView through the relay and Tap (needs `devenv up`)          |
| `lexgen`                         | Regenerate Rust and TypeScript types after editing `lexicons/`                                  |
| `devenv test`                    | The whole CI gate, services included                                                            |
| `stack`                          | The deployment-shaped stack in containers (`deploy/compose.yaml`)                               |
| `mutants`, `mutants-web`, `soak` | Rust and TypeScript/Svelte mutation testing; larger Rust property runs                          |

The reverse proxy shares `https://motet.test` between the web app and AppView.
The AppView is the OAuth client of the user's PDS; signup goes through the
PDS. This describes the intended local flow, not a claim that it has passed an
end-to-end check. The Theme Creator runs separately with `theme-creator`.

## Layout

```
crates/            Rust: the AppView, the tenor and the libraries they share
  rules/           The World's rules as a pure state machine (admission, places, roles)
  dice/            Dice notation: parser, roller, limits
  log/             Fenced, group-committed log segments in object storage
  xrpc/            XRPC serving: the fail-closed method registry and service auth
  net/             The one hardened outbound HTTP client; DID resolution
  spaces/          The seam to atproto Spaces: audiences and serving decisions
  render/          Typst rendering in a sandboxed, resource-limited worker
  lexicons/        Rust types generated from lexicons/
  lexgen/          The lexicon code generator (Rust and TypeScript)
  tenor/           The group host binary
  appview/         The AppView binary: indexer, archive, projections, outbox, API
  testkit/         Test support: delivery orders, services from the environment
  acceptance/      Acceptance tests that span several parts
apps/web/          SvelteKit web app
apps/theme-creator/ The Theme Creator, as exported: edits a World's theme record
packages/design/   The design system: tokens, Panda preset, World engine, docs, contrast checks
packages/lexicons/ TypeScript types generated from lexicons/
lexicons/          Motet's lexicons (at.motet.*), the source of truth for records
tools/             Vendored lint rules: anti-slop for Oxlint
nix/               Rust build and checks (crane), the dev environment (devenv)
deploy/            The compose stack
docs/              Architecture, development standards, testing, decisions
```

## Standards

[docs/development.md](docs/development.md) is how code is written and
reviewed: Google's Rust style guide with Motet's exceptions, the lint policy,
comments that say why, and the review checklist.

## Testing

Tests state properties of the system, not of its functions: replays give the
same state, every consumer agrees on what was admitted, nothing unauthorised
reaches a handler. A test must name the bug it would catch. See
[docs/testing.md](docs/testing.md).

## Licence

Not chosen yet ([ADR 0001](docs/adr/0001-licence.md)).
