# 0003. A Rust workspace and a pnpm workspace in one repository

Status: Accepted

## Context

The backend is Rust (the backend plan's "Language" section); the web app is
TypeScript. Lexicons feed both, and changes often cross the line.

## Decision

- One repository. `crates/*` is a Cargo workspace; `apps/*` and `packages/*`
  are a pnpm workspace. `lexicons/` is shared and generates into both.
- Crates are split by responsibility and by what must stay pure:
  `motet-rules` and `motet-dice` have no I/O, so their properties can be
  tested exhaustively; `motet-log`, `motet-net` and `motet-xrpc` each own one
  boundary; the binaries (`motet-appview`, `motet-tenor`) compose them.
- Shared dependency versions and lints live in the workspace `Cargo.toml`;
  crates say `workspace = true`. Clippy's pedantic group is on and warnings
  are errors.
- Formatting and linting for the web packages are configured once at the
  root (Prettier, ESLint flat config, `tsconfig.base.json`).

## Consequences

A new crate starts by inheriting lints and versions. Anything that needs the
network or a database is kept out of `rules` and `dice`.
