# 0025. Native devenv as the development shell entry point

Status: Accepted

Supersedes ADR 0002's single-flake entry point and flake-parts shell clause.
The hermetic checks, deployment packages and images remain flake outputs.

## Context

The flake-parts shell ships a compatibility wrapper, not the native devenv
CLI. `devenv shell` and `devenv info` do not work normally without root
`devenv.nix` and `devenv.yaml`. Developers and CI need the same commands.

## Decision

- Root `devenv.yaml` declares nixpkgs, rust-overlay and crane inputs.
  Root `devenv.nix` builds the toolchain from `rust-toolchain.toml`, Kani
  and network packages. It imports `nix/devenv.nix`, the only definition
  of scripts, hooks, services, processes and the service-backed gate.
- Use the native devenv 2.4.0 CLI. Enter with `devenv shell`, run a script
  with `devenv shell -- <script>`, start services with `devenv up`, and
  run the full gate with `devenv test`. Direnv uses `devenv direnvrc` and
  `use devenv`, not nix-direnv's `use flake`.
- Remove the flake-parts devenv module and its compatibility shell.
  Keep crane checks, network packages, Kani and images in the flake.
  There is no `devenv-up` flake package; use native `devenv test` in CI.
- `flake.lock` is canonical for nixpkgs, rust-overlay and crane.
  Declare matching revision URLs in `devenv.yaml` and commit `devenv.lock`.
  Shell evaluation rejects revision or source-hash divergence, and a
  hermetic flake check compares the two locks. This deliberate duplication
  keeps native devenv input handling without fetching the working tree as
  a local flake input or copying build output into the Nix store.
- CI installs the native CLI using the official Nix/Cachix setup, pinned
  to the 2.4.0 commit recorded for its modules in `devenv.lock`. Gate, proofs and scheduled jobs use native devenv.
  Hermetic checks and images keep `nix flake check` and `nix build`.

## Consequences

Contributors install Nix and devenv; direnv is optional. `nix develop` is
no longer the shell entry point. Updating shared dependencies means updating
`flake.lock`, matching `devenv.yaml` revisions, and running `devenv update`.
A missed update fails visibly instead of silently using different tools.
Devenv's module inputs are separately pinned in `devenv.lock`; deployment
builds do not need them. Native shell startup must preserve script precedence
(the repository's `fmt`, not GNU fmt) and the local service environment.

Local TLS setup still changes host DNS, CA trust and port permissions.
Changing the entry point does not remove those requirements or establish
that the service-backed gate passes on an unconfigured host.

## Rejected alternatives

- Keep the flake-parts shell: its compatibility wrapper does not provide
  the normal native CLI entry points requested by the user.
- Consume the local flake's packages from devenv: this couples shell entry
  to flake evaluation and adds a local-source locking boundary. Shared Nix
  functions already provide the same packages without that boundary.
- Maintain separate unguarded locks: a partial update could silently give
  CI builds and developers different toolchains or package versions.

## Revisit when

Revisit this decision if native devenv can consume one shared lock directly,
or if maintaining the guarded locks becomes more costly than the separation
between interactive development and hermetic deployment builds.

References: [inputs](https://devenv.sh/inputs/),
[YAML schema](https://devenv.sh/reference/yaml-options/),
[composition](https://devenv.sh/composing-using-imports/),
[direnv](https://devenv.sh/integrations/direnv/), and
[GitHub Actions](https://devenv.sh/integrations/github-actions/).
