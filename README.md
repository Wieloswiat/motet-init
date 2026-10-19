# Motet

Play-by-forum roleplaying on atproto. People's posts and characters live in
their own repositories; each World is a group account hosted by a **tenor**,
which admits posts into the World's scenes, rolls dice and keeps the World's
private Spaces. Motet's **AppView** indexes all of it and serves the web app.

The design is in [docs/architecture/overview.md](docs/architecture/overview.md);
the decisions behind it are in [docs/adr](docs/adr/README.md).

## Getting started

Everything comes from the nix flake: toolchains, PostgreSQL with pg_search,
S3 storage, and the commands below. With [nix](https://nixos.org) and
[direnv](https://direnv.net):

```sh
direnv allow          # or: nix develop --no-pure-eval
devenv up             # PostgreSQL and S3, in another terminal
tests                 # the default suites
```

`devenv info` lists every command. The ones used day to day:

| Command             | What it does                                                            |
| ------------------- | ----------------------------------------------------------------------- |
| `fmt`               | Format Rust, Nix, TOML and the web packages                             |
| `lint`              | Clippy (warnings are errors), typos, workflow lint, ESLint, type checks |
| `tests`             | Pure, property and in-process tests; vitest. No services needed         |
| `tests-integration` | Tests against PostgreSQL and S3, and Playwright (needs `devenv up`)     |
| `lexgen`            | Regenerate Rust and TypeScript types after editing `lexicons/`          |
| `devenv test`       | The whole CI gate, services included                                    |
| `stack`             | The deployment-shaped stack in containers (`deploy/compose.yaml`)       |
| `mutants`, `soak`   | Mutation testing; property tests with many more cases                   |

The web app: `pnpm --filter @motet/web dev`.

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
packages/design/   Design tokens, Panda CSS preset, theme contrast checks
packages/lexicons/ TypeScript types generated from lexicons/
lexicons/          Motet's lexicons (at.motet.*), the source of truth for records
nix/               Rust build and checks (crane), the dev environment (devenv)
deploy/            The compose stack
docs/              Architecture, testing, decisions
```

## Testing

Tests state properties of the system, not of its functions: replays give the
same state, every consumer agrees on what was admitted, nothing unauthorised
reaches a handler. See [docs/testing.md](docs/testing.md).

## Licence

Not chosen yet ([ADR 0001](docs/adr/0001-licence.md)).
