# The development environment: toolchains, local services and git hooks.
#
#   nix develop --no-pure-eval   (or `direnv allow`)
#   devenv up                    PostgreSQL with pg_search, SeaweedFS (S3)
#   devenv test                  the whole CI gate, services included
#   devenv info                  lists the scripts below with descriptions
#
# Commands live here as devenv scripts, so they're on PATH in the shell and
# CI runs exactly the same ones (`nix develop --no-pure-eval -c <script>`).
{ rustToolchain }:
{ pkgs, lib, ... }:
let
  pgPort = 54329;
  s3Port = 9010;
  s3Key = "motet";
  s3Secret = "motet-dev-secret";
  s3Config = pkgs.writeText "seaweedfs-s3.json" (
    builtins.toJSON {
      identities = [
        {
          name = "motet";
          credentials = [
            {
              accessKey = s3Key;
              secretKey = s3Secret;
            }
          ];
          actions = [
            "Admin"
            "Read"
            "Write"
            "List"
            "Tagging"
          ];
        }
      ];
    }
  );
in
{
  packages = with pkgs; [
    rustToolchain
    cargo-nextest
    cargo-deny
    cargo-mutants
    cargo-llvm-cov
    sqlx-cli
    typst
    seaweedfs
    taplo
    typos
    actionlint
    nixfmt
    podman-compose
  ];

  languages.javascript = {
    enable = true;
    package = pkgs.nodejs_24;
    pnpm = {
      enable = true;
      install.enable = true;
    };
  };

  services.postgres = {
    enable = true;
    package = pkgs.postgresql_18;
    extensions = ext: [ ext.pg_search ];
    listen_addresses = "127.0.0.1";
    port = pgPort;
    initialDatabases = [ { name = "motet"; } ];
    # Superuser: integration tests create a database each, and migrations
    # create extensions. Development only.
    initialScript = "CREATE ROLE motet SUPERUSER LOGIN PASSWORD 'motet';";
    settings.shared_preload_libraries = "pg_search";
  };

  # Object storage: SeaweedFS's S3 gateway. (MinIO was abandoned upstream and
  # nixpkgs marks it insecure.) The fencing tests in crates/log need
  # conditional writes, so they double as a check that this store has them.
  processes.seaweedfs.exec = ''
    mkdir -p "$DEVENV_STATE/seaweedfs"
    exec ${pkgs.seaweedfs}/bin/weed server \
      -dir="$DEVENV_STATE/seaweedfs" -ip=127.0.0.1 -ip.bind=127.0.0.1 \
      -master.port=9014 -volume.port=9012 -filer.port=9013 \
      -s3 -s3.port=${toString s3Port} -s3.config=${s3Config}
  '';
  processes.seaweedfs-buckets.exec = ''
    for bucket in motet-dev motet-test; do
      until echo "s3.bucket.create -name $bucket" \
        | ${pkgs.seaweedfs}/bin/weed shell -master=127.0.0.1:9014 >/dev/null 2>&1; do
        sleep 1
      done
    done
  '';

  env = {
    DATABASE_URL = "postgres://motet:motet@127.0.0.1:${toString pgPort}/motet";
    MOTET_TEST_DATABASE_URL = "postgres://motet:motet@127.0.0.1:${toString pgPort}/postgres";
    MOTET_TEST_S3_ENDPOINT = "http://127.0.0.1:${toString s3Port}";
    MOTET_TEST_S3_BUCKET = "motet-test";
    MOTET_TEST_S3_ACCESS_KEY = s3Key;
    MOTET_TEST_S3_SECRET_KEY = s3Secret;
  }
  // lib.optionalAttrs pkgs.stdenv.isLinux {
    # Vitest's browser mode and Playwright launch nixpkgs' Chromium (see
    # apps/web/vite.config.ts), so the npm package's browser build never has to
    # match nixpkgs'. Elsewhere, `pnpm exec playwright install chromium`.
    PLAYWRIGHT_CHROMIUM_EXECUTABLE = "${pkgs.chromium}/bin/chromium";
    PLAYWRIGHT_SKIP_BROWSER_DOWNLOAD = "1";
  };

  git-hooks.hooks = {
    rustfmt = {
      enable = true;
      packageOverrides.cargo = rustToolchain;
      packageOverrides.rustfmt = rustToolchain;
    };
    nixfmt.enable = true;
    taplo.enable = true;
    typos.enable = true;
    actionlint.enable = true;
  };

  # devenv runs each script with `bash <file>`, without errexit. A script that
  # does not set it returns the status of its last command only, so an earlier
  # failure is lost. For this reason, every script with more than one command
  # starts with `set -euo pipefail` and keeps each command on its own line:
  # errexit ignores a failure on the left side of `&&`.
  scripts = {
    # Everyday.
    fmt = {
      description = "Format Rust, Nix, TOML and the web packages";
      exec = ''
        set -euo pipefail
        cargo fmt --all
        nixfmt flake.nix nix/*.nix
        taplo fmt
        pnpm format
      '';
    };
    fmt-check = {
      description = "Fail if anything is unformatted";
      exec = ''
        set -euo pipefail
        cargo fmt --all --check
        nixfmt --check flake.nix nix/*.nix
        taplo fmt --check
        pnpm format:check
      '';
    };
    lint = {
      description = "Clippy and rustdoc (warnings are errors), dependency policy, typos, workflow lint, web lint and type checks";
      exec = ''
        set -euo pipefail
        cargo clippy --workspace --all-targets --all-features -- --deny warnings
        # Rustdoc's lints, such as broken intra-doc links, run only in
        # `cargo doc`. No crate is published, so people read the docs of
        # private items as much as public ones: check both.
        RUSTDOCFLAGS="--deny warnings" cargo doc --workspace --no-deps --all-features --document-private-items
        # Advisories need the network, so they run in `audit`, not here.
        cargo deny check bans licenses sources
        typos
        actionlint
        pnpm lint
        pnpm check
      '';
    };
    tests = {
      description = "Default suites: pure, property and in-process tests, and vitest (no services)";
      exec = ''
        set -euo pipefail
        cargo nextest run --workspace "$@"
        pnpm test
      '';
    };
    tests-integration = {
      description = "Integration tests against PostgreSQL and S3, and Playwright (needs devenv up)";
      exec = ''
        set -euo pipefail
        cargo nextest run --workspace --profile integration --run-ignored only "$@"
        pnpm test:e2e
      '';
    };

    # Code generation.
    lexgen = {
      description = "Regenerate Rust and TypeScript types from lexicons/";
      exec = ''
        cargo run -q -p motet-lexgen -- generate --lexicons lexicons \
          --rust crates/lexicons/src/generated --ts packages/lexicons/src/generated
      '';
    };
    lexgen-check = {
      description = "Fail if generated lexicon code is stale";
      exec = ''
        cargo run -q -p motet-lexgen -- check --lexicons lexicons \
          --rust crates/lexicons/src/generated --ts packages/lexicons/src/generated
      '';
    };

    # Running.
    stack = {
      description = "Deployment-shaped stack (AppView, tenor, Tap, Spaces PDS, PostgreSQL, S3) in podman";
      exec = ''
        set -euo pipefail
        nix build .#appview-image -o result-appview
        ./result-appview | podman load
        nix build .#tenor-image -o result-tenor
        ./result-tenor | podman load
        podman-compose -f deploy/compose.yaml up "$@"
      '';
    };

    ci = {
      description = "The CI gate: every check and suite (needs the services from devenv up)";
      exec = ''
        set -euo pipefail
        fmt-check
        lint
        lexgen-check
        tests --profile ci
        # A cold start includes initdb and SeaweedFS's master election.
        for port in ${toString pgPort} ${toString s3Port}; do
          timeout 180 bash -c "until (echo > /dev/tcp/127.0.0.1/$port) 2>/dev/null; do sleep 1; done" \
            || { echo "port $port never opened; is devenv up running?" >&2; exit 1; }
        done
        tests-integration
      '';
    };

    # Slower assurance.
    audit = {
      description = "Known advisories in Rust crates and production npm packages (needs network)";
      exec = ''
        set -euo pipefail
        cargo deny check advisories
        pnpm audit --prod
      '';
    };
    mutants = {
      description = "Mutation testing: which code can change without any test noticing?";
      exec = ''cargo mutants --workspace --test-tool nextest "$@"'';
    };
    mutants-web = {
      description = "Mutation testing for TypeScript and Svelte with Stryker and Vitest";
      exec = "pnpm test:mutation";
    };
    coverage = {
      description = "Line coverage of the default suites, as lcov.info";
      exec = "cargo llvm-cov nextest --workspace --lcov --output-path lcov.info";
    };
    soak = {
      description = "Property tests with many more cases (default 20000)";
      exec = ''PROPTEST_CASES="''${1:-20000}" cargo nextest run --workspace --release'';
    };
  };

  # `devenv test` starts the processes above, then runs this: the whole CI
  # gate in one command. CI starts the processes itself and runs `ci`, so it
  # can keep their logs.
  enterTest = "ci";

  enterShell = ''
    echo "Motet dev shell. 'devenv up' starts PostgreSQL and S3; 'devenv info' lists scripts."
  '';
}
