# Architecture decision records

One file per decision that shapes the code: what was decided, why, and what
would make us revisit it. A decision is changed by a new record that
supersedes the old one, not by editing history. Status is one of Proposed,
Accepted, Provisional (working choice, expected to be revisited soon) or
Superseded.

| ADR                                            | Decision                                                                 | Status                                                 |
| ---------------------------------------------- | ------------------------------------------------------------------------ | ------------------------------------------------------ |
| [0001](0001-licence.md)                        | Motet's licence                                                          | Proposed (open)                                        |
| [0002](0002-nix-and-devenv.md)                 | One nix flake for the toolchain, services, checks and CI                 | Accepted                                               |
| [0003](0003-workspace-layout.md)               | A Rust workspace and a pnpm workspace in one repository                  | Accepted                                               |
| [0004](0004-atproto-crates.md)                 | Build on atproto-crates                                                  | Accepted; crates.io half superseded by 0013            |
| [0005](0005-testing.md)                        | Properties, integration and acceptance tests; no tautological unit tests | Accepted; retry result superseded by 0011              |
| [0006](0006-storage.md)                        | Fenced logs in object storage; SeaweedFS for development                 | Accepted; SlateDB added by 0014                        |
| [0007](0007-projections.md)                    | Rebuildable projections, runtime-checked SQL                             | Accepted                                               |
| [0008](0008-rendering.md)                      | Typst in a sandboxed worker; static SVG until typst.ts                   | Accepted                                               |
| [0009](0009-web-stack.md)                      | SvelteKit, Panda CSS and Ark UI                                          | Provisional; design package amended by 0012            |
| [0010](0010-lexicon-codegen.md)                | Our own lexicon code generator                                           | Accepted                                               |
| [0011](0011-flaky-tests-fail.md)               | A test that passes only on its retry fails CI                            | Accepted                                               |
| [0012](0012-design-system-and-demo-screens.md) | The exported design system, and demo screens in the web app              | Provisional; demo routes and data superseded by 0020   |
| [0013](0013-tenor-hosts-worlds.md)             | The tenor hosts Worlds itself, on atproto-crates' protocol crates        | Accepted; World sessions superseded by 0018            |
| [0014](0014-tenor-state-in-slatedb.md)         | Tenor state in SlateDB, with the shard log as its only WAL               | Accepted                                               |
| [0015](0015-opensocial-group.md)               | Adopt opensocial.group at a pinned revision                              | Provisional; group OAuth assumption superseded by 0018 |
| [0016](0016-local-atproto-network.md)          | A local atproto network in the dev shell, with patched indigo            | Accepted; PDS/patch by 0017; HTTP endpoints by 0019    |
| [0017](0017-rust-pds-local-network.md)         | Ship the local network with the Rust PDS; defer reference PDS coverage   | Accepted; HTTP endpoints superseded by 0019            |
| [0018](0018-world-service-auth.md)             | Member service tokens authorize World actions                            | Accepted; replay protection added by 0021              |
| [0019](0019-local-tls-for-oauth.md)            | Local HTTPS origins for PDS OAuth                                        | Accepted                                               |
| [0020](0020-appview-backed-web-screens.md)     | AppView-backed World and scene screens                                   | Accepted; demo routes and data supersede 0012          |
| [0021](0021-single-use-member-tokens.md)       | Consume member service tokens once                                       | Accepted                                               |
