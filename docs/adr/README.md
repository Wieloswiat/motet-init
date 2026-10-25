# Architecture decision records

One file per decision that shapes the code: what was decided, why, and what
would make us revisit it. A decision is changed by a new record that
supersedes the old one, not by editing history. Status is one of Proposed,
Accepted, Provisional (working choice, expected to be revisited soon) or
Superseded.

| ADR                                            | Decision                                                                 | Status                                      |
| ---------------------------------------------- | ------------------------------------------------------------------------ | ------------------------------------------- |
| [0001](0001-licence.md)                        | Motet's licence                                                          | Proposed (open)                             |
| [0002](0002-nix-and-devenv.md)                 | One nix flake for the toolchain, services, checks and CI                 | Accepted                                    |
| [0003](0003-workspace-layout.md)               | A Rust workspace and a pnpm workspace in one repository                  | Accepted                                    |
| [0004](0004-atproto-crates.md)                 | Build on atproto-crates                                                  | Accepted                                    |
| [0005](0005-testing.md)                        | Properties, integration and acceptance tests; no tautological unit tests | Accepted; retry result superseded by 0011   |
| [0006](0006-storage.md)                        | Fenced logs in object storage; SeaweedFS for development                 | Accepted                                    |
| [0007](0007-projections.md)                    | Rebuildable projections, runtime-checked SQL                             | Accepted                                    |
| [0008](0008-rendering.md)                      | Typst in a sandboxed worker; static SVG until typst.ts                   | Accepted                                    |
| [0009](0009-web-stack.md)                      | SvelteKit, Panda CSS and Ark UI                                          | Provisional; design package amended by 0012 |
| [0010](0010-lexicon-codegen.md)                | Our own lexicon code generator                                           | Accepted                                    |
| [0011](0011-flaky-tests-fail.md)               | A test that passes only on its retry fails CI                            | Accepted                                    |
| [0012](0012-design-system-and-demo-screens.md) | The exported design system, and demo screens in the web app              | Provisional                                 |
