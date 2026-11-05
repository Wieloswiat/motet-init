# The development environment: toolchains, local services and git hooks.
#
#   nix develop --no-pure-eval   (or `direnv allow`)
#   devenv up                    PostgreSQL, SeaweedFS, atproto network, AppView and tenor
#   devenv test                  the whole CI gate, services included
#   devenv info                  lists the scripts below with descriptions
#
# Commands live here as devenv scripts, so they're on PATH in the shell and
# CI runs exactly the same ones (`nix develop --no-pure-eval -c <script>`).
{ rustToolchain, networkPackages }:
{
  pkgs,
  lib,
  config,
  ...
}:
let
  pgPort = 54329;
  s3Port = 9010;
  s3Key = "motet";
  s3Secret = "motet-dev-secret";
  networkState = "${config.devenv.state}/network";
  networkPassword = "motet-network-dev";
  # Leave readiness to process-compose; ready.timeout makes wrappers supervise dependencies.
  httpReady = port: path: {
    http.get = { inherit port path; };
    period = 1;
    failure_threshold = 180;
  };
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
  packages =
    with pkgs;
    [
      rustToolchain
      cargo-nextest
      cargo-deny
      cargo-mutants
      cargo-llvm-cov
      sqlx-cli
      typst
      seaweedfs
      taplo
      caddy
      dnsmasq
      openssl
      typos
      actionlint
      nixfmt
      podman-compose
    ]
    ++ lib.optionals pkgs.stdenv.isLinux [
      iproute2
      nss.tools
      util-linux
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
  # nixpkgs marks it insecure.) Reserve enough volume slots for separate
  # archive, test and tenor buckets; each new collection can allocate seven.
  # The log's fencing tests also check this store's conditional writes.
  processes.seaweedfs = {
    exec = ''
      mkdir -p "$DEVENV_STATE/seaweedfs"
      exec ${pkgs.seaweedfs}/bin/weed server \
        -dir="$DEVENV_STATE/seaweedfs" -ip=127.0.0.1 -ip.bind=127.0.0.1 \
        -master.port=9014 -master.telemetry=false -volume.port=9012 -volume.max=32 -filer.port=9013 \
        -s3 -s3.port=${toString s3Port} -s3.config=${s3Config}
    '';
    # S3 rejects anonymous requests, but any HTTP response means it is listening.
    ready = {
      exec = "${lib.getExe pkgs.curl} --silent --output /dev/null http://127.0.0.1:${toString s3Port}/";
      period = 1;
      failure_threshold = 180;
    };
  };
  processes.seaweedfs-buckets = {
    after = [ "devenv:processes:seaweedfs" ];
    exec = ''
      set -euo pipefail
      marker="$DEVENV_RUNTIME/motet-buckets-ready"
      rm -f "$marker"
      until printf 's3.bucket.list\n' \
        | ${pkgs.seaweedfs}/bin/weed shell -master=127.0.0.1:9014 >/dev/null 2>&1; do
        sleep 1
      done
      for bucket in motet-dev motet-test motet-tenor; do
        if ! output=$(printf 's3.bucket.create -name %s\n' "$bucket" \
          | ${pkgs.seaweedfs}/bin/weed shell -master=127.0.0.1:9014 2>&1); then
          # Creating an existing bucket fails; other errors must still stop startup.
          printf 's3.bucket.list\n' \
            | ${pkgs.seaweedfs}/bin/weed shell -master=127.0.0.1:9014 \
            | grep -Eq "^[[:space:]]+$bucket[[:space:]]" \
            || { echo "$output" >&2; exit 1; }
        fi
      done
      touch "$marker"
      exec sleep infinity
    '';
    ready = {
      exec = ''test -f "$DEVENV_RUNTIME/motet-buckets-ready"'';
      period = 1;
      failure_threshold = 180;
    };
  };

  processes.motet-dns = {
    exec = ''
      exec ${lib.getExe pkgs.dnsmasq} --keep-in-foreground --conf-file=/dev/null \
        --no-resolv --no-hosts --bind-interfaces --listen-address=127.0.0.1 --port=53535 \
        --local=/motet.test/ --address=/motet.test/127.0.0.1
    '';
    ready = {
      exec = "${pkgs.bash}/bin/bash -c 'exec 3<>/dev/tcp/127.0.0.1/53535' ";
      period = 1;
      failure_threshold = 180;
    };
  };

  processes.caddy = {
    after = [ "devenv:processes:motet-dns" ];
    exec = "exec ${lib.getExe pkgs.caddy} run --config ${./Caddyfile} --adapter caddyfile";
    env = {
      XDG_CONFIG_HOME = "${config.devenv.state}/caddy/config";
      XDG_DATA_HOME = "${config.devenv.state}/caddy/data";
    };
    # Backends may still be starting; readiness only requires the TLS listener.
    ready = {
      exec = "${pkgs.bash}/bin/bash -c 'exec 3<>/dev/tcp/127.0.0.1/443' ";
      period = 1;
      failure_threshold = 180;
    };
  };

  processes.plc = {
    # The shell's PostgreSQL URL belongs to Motet, not this ephemeral directory.
    exec = ''
      set -euo pipefail
      unset DATABASE_URL
      exec ${networkPackages.plc}/bin/plc-server
    '';
    env.PORT = "2582";
    ready = httpReady 2582 "/_health";
  };
  processes.relay = {
    after = [
      "devenv:processes:caddy"
      "devenv:processes:plc"
    ];
    exec = ''
      set -euo pipefail
      mkdir -p "${networkState}/relay"
      exec ${networkPackages.indigo}/bin/relay serve
    '';
    env = {
      RELAY_ALLOW_PRIVATE_HOSTS = "true";
      RELAY_PLC_HOST = "https://plc.motet.test";
      RELAY_ADMIN_PASSWORD = networkPassword;
      RELAY_API_BIND = "127.0.0.1:2470";
      RELAY_METRICS_LISTEN = "127.0.0.1:2471";
      DATABASE_URL = "sqlite://${networkState}/relay/relay.sqlite";
      RELAY_PERSIST_DIR = "${networkState}/relay/persist";
    };
    ready = httpReady 2470 "/_health";
  };
  processes.tap = {
    after = [ "devenv:processes:relay" ];
    exec = ''
      set -euo pipefail
      mkdir -p "${networkState}/tap"
      exec ${networkPackages.indigo}/bin/tap run
    '';
    env = {
      TAP_BIND = "127.0.0.1:2480";
      TAP_DATABASE_URL = "sqlite://${networkState}/tap/tap.sqlite";
      TAP_RELAY_URL = "https://relay.motet.test";
      TAP_PLC_URL = "https://plc.motet.test";
      # The isolated network contains both pilot and World repos.
      TAP_FULL_NETWORK = "true";
      TAP_COLLECTION_FILTERS = "at.motet.pilot.*,group.opensocial.*";
      TAP_DISABLE_ACKS = "false";
      TAP_ADMIN_PASSWORD = networkPassword;
    };
    ready = {
      exec = "${lib.getExe pkgs.curl} --fail --silent --user admin:${networkPassword} http://localhost:2480/health";
      period = 1;
      failure_threshold = 180;
    };
  };
  processes.pds-rs = {
    after = [
      "devenv:processes:caddy"
      "devenv:processes:plc"
    ];
    exec = ''
      set -euo pipefail
      mkdir -p "$PDS_DATA_DIRECTORY"
      exec ${networkPackages.pds-rs}/bin/pds
    '';
    env = {
      PDS_BIND = "127.0.0.1";
      PDS_PORT = "4800";
      PDS_HOSTNAME = "pds.motet.test";
      PDS_SERVICE_DID = "did:web:pds.motet.test";
      PDS_TRUSTED_PROXY_HOPS = "1";
      PDS_DID_PLC_URL = "https://plc.motet.test";
      PDS_CRAWLERS = "https://relay.motet.test";
      PDS_INVITE_REQUIRED = "false";
      PDS_SIGNUP_EMAIL_VERIFICATION = "false";
      PDS_ALLOW_DEV_DEFAULTS = "true";
      PDS_SERVICE_HANDLE_DOMAINS = ".pds.motet.test";
      PDS_DATA_DIRECTORY = "${networkState}/pds-rs";
      PDS_LEXICON_DIR = "${../lexicons}";
    };
    ready = httpReady 4800 "/xrpc/_health";
  };

  # Cargo shares the shell's incremental cache with tests, so edits do not
  # require rebuilding the whole Nix package before a process can start.
  processes.tenor = {
    after = [
      "devenv:processes:seaweedfs-buckets"
      "devenv:processes:relay"
    ];
    exec = ''
      set -euo pipefail
      umask 077
      mkdir -p "$(dirname "$MOTET_TENOR_MASTER_KEY_FILE")"
      if [ ! -f "$MOTET_TENOR_MASTER_KEY_FILE" ]; then
        temp=$(mktemp "$MOTET_TENOR_MASTER_KEY_FILE.XXXXXX")
        trap 'rm -f "$temp"' EXIT
        dd if=/dev/urandom of="$temp" bs=32 count=1 status=none
        ln "$temp" "$MOTET_TENOR_MASTER_KEY_FILE" || test -f "$MOTET_TENOR_MASTER_KEY_FILE"
        rm -f "$temp"
        trap - EXIT
      fi
      chmod 600 "$MOTET_TENOR_MASTER_KEY_FILE"
      cargo build --jobs "''${CARGO_BUILD_JOBS:-2}" -p motet-tenor --bin motet-tenor
      exec "''${CARGO_TARGET_DIR:-target}/debug/motet-tenor" serve
    '';
    # A cold Cargo build can take longer than a service-only startup.
    ready = (httpReady 2584 "/xrpc/com.atproto.server.describeServer") // {
      failure_threshold = 1800;
    };
  };
  # Register local hosts through the authenticated administrator endpoint.
  processes.relay-hosts = {
    after = [
      "devenv:processes:caddy"
      "devenv:processes:relay"
      "devenv:processes:pds-rs"
      "devenv:processes:tenor"
    ];
    exec = ''
      set -euo pipefail
      marker="$DEVENV_RUNTIME/motet-relay-hosts-ready"
      rm -f "$marker"
      for host in "$MOTET_PDS_RS_URL" "$MOTET_TENOR_PUBLIC_URL"; do
        ${lib.getExe pkgs.curl} --fail-with-body --silent --show-error \
          --user "admin:$MOTET_RELAY_ADMIN_PASSWORD" \
          --header 'Content-Type: application/json' \
          --data "{\"hostname\":\"$host\"}" \
          "$MOTET_RELAY_URL/admin/pds/requestCrawl"
      done
      touch "$marker"
      exec sleep infinity
    '';
    ready = {
      exec = ''test -f "$DEVENV_RUNTIME/motet-relay-hosts-ready"'';
      period = 1;
      failure_threshold = 180;
    };
  };
  processes.appview = {
    after = [
      "devenv:processes:postgres"
      "devenv:processes:seaweedfs-buckets"
      "devenv:processes:relay-hosts"
      "devenv:processes:tap"
    ];
    exec = ''
      set -euo pipefail
      umask 077
      for key in "$MOTET_APPVIEW_OAUTH_SIGNING_KEY_FILE" "$MOTET_APPVIEW_SESSION_KEY_FILE"; do
        mkdir -p "$(dirname "$key")"
        if [ ! -f "$key" ]; then
          temp=$(mktemp "$key.XXXXXX")
          trap 'rm -f "$temp"' EXIT
          dd if=/dev/urandom of="$temp" bs=32 count=1 status=none
          ln "$temp" "$key" || test -f "$key"
          rm -f "$temp"
          trap - EXIT
        fi
        chmod 600 "$key"
      done
      cargo build --jobs "''${CARGO_BUILD_JOBS:-2}" -p motet-appview --bin motet-appview
      bin="''${CARGO_TARGET_DIR:-target}/debug/motet-appview"
      "$bin" migrate
      exec "$bin" serve
    '';
    ready = (httpReady 2583 "/_health") // {
      failure_threshold = 1800;
    };
  };

  env = {
    DATABASE_URL = "postgres://motet:motet@127.0.0.1:${toString pgPort}/motet";
    MOTET_TEST_DATABASE_URL = "postgres://motet:motet@127.0.0.1:${toString pgPort}/postgres";
    MOTET_TEST_S3_ENDPOINT = "http://127.0.0.1:${toString s3Port}";
    MOTET_TEST_S3_BUCKET = "motet-test";
    MOTET_TEST_S3_ACCESS_KEY = s3Key;
    MOTET_TEST_S3_SECRET_KEY = s3Secret;
    MOTET_PLC_DIRECTORY = "https://plc.motet.test";
    MOTET_RELAY_URL = "https://relay.motet.test";
    MOTET_TAP_URL = "https://tap.motet.test";
    MOTET_PDS_RS_URL = "https://pds.motet.test";
    MOTET_APPVIEW_URL = "https://motet.test";
    MOTET_APPVIEW_PUBLIC_ORIGIN = "https://motet.test";
    MOTET_APPVIEW_SERVICE_DID = "did:web:motet.test#motet_appview";
    MOTET_APPVIEW_DEFAULT_PDS = "https://pds.motet.test";
    MOTET_APPVIEW_TENOR_URL = "https://tenor.motet.test";
    MOTET_APPVIEW_TENOR_DID = "did:web:tenor.motet.test";
    MOTET_APPVIEW_OAUTH_SIGNING_KEY_FILE = "${config.devenv.state}/appview/oauth-signing-key";
    MOTET_APPVIEW_SESSION_KEY_FILE = "${config.devenv.state}/appview/session-key";
    MOTET_APPVIEW_TAP_URL = "https://tap.motet.test";
    MOTET_APPVIEW_TAP_ADMIN_PASSWORD = networkPassword;
    MOTET_APPVIEW_S3_ENDPOINT = "http://127.0.0.1:${toString s3Port}";
    MOTET_APPVIEW_S3_BUCKET = "motet-dev";
    MOTET_APPVIEW_S3_ACCESS_KEY = s3Key;
    MOTET_APPVIEW_S3_SECRET_KEY = s3Secret;
    MOTET_TENOR_PUBLIC_URL = "https://tenor.motet.test";
    MOTET_TENOR_HANDLE_DOMAINS = ".motet.test";
    MOTET_TENOR_CRAWLERS = "https://relay.motet.test";
    MOTET_TENOR_S3_ENDPOINT = "http://127.0.0.1:${toString s3Port}";
    MOTET_TENOR_S3_BUCKET = "motet-tenor";
    MOTET_TENOR_S3_ACCESS_KEY = s3Key;
    MOTET_TENOR_S3_SECRET_KEY = s3Secret;
    MOTET_TENOR_MASTER_KEY_FILE = "${config.devenv.state}/tenor/master-key";
    MOTET_ALLOW_PRIVATE_NETWORK = "true";
    MOTET_RELAY_ADMIN_PASSWORD = networkPassword;
    MOTET_TAP_ADMIN_PASSWORD = networkPassword;
    MOTET_TLS_CERT = "${config.devenv.state}/tls/server.crt";
    MOTET_TLS_KEY = "${config.devenv.state}/tls/server.key";
    SSL_CERT_FILE = "${config.devenv.state}/tls/ca-bundle.crt";
    CURL_CA_BUNDLE = "${config.devenv.state}/tls/ca-bundle.crt";
    REQUESTS_CA_BUNDLE = "${config.devenv.state}/tls/ca-bundle.crt";
    NODE_EXTRA_CA_CERTS = "${config.devenv.state}/tls/rootCA.crt";
    NO_PROXY = "localhost,127.0.0.1,::1,motet.test,.motet.test";
    no_proxy = "localhost,127.0.0.1,::1,motet.test,.motet.test";
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
        nixfmt flake.nix nix/*.nix nix/pkgs/*.nix
        taplo fmt
        pnpm format
      '';
    };
    fmt-check = {
      description = "Fail if anything is unformatted";
      exec = ''
        set -euo pipefail
        cargo fmt --all --check
        nixfmt --check flake.nix nix/*.nix nix/pkgs/*.nix
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
      description = "Integration tests and Playwright against the complete local stack (needs devenv up)";
      exec = ''
        set -euo pipefail
        # A cold start builds the applications as well as starting the services.
        for port in ${toString pgPort} ${toString s3Port} 53535 443 2582 2470 2480 4800 2584 2583; do
          timeout 1800 bash -c "until (echo > /dev/tcp/127.0.0.1/$port) 2>/dev/null; do sleep 1; done" \
            || { echo "port $port never opened; is devenv up running?" >&2; exit 1; }
        done
        cargo nextest run --workspace --profile integration --run-ignored only "$@"
        pnpm test:e2e
      '';
    };

    # Design.
    theme-creator = {
      description = "Serve the Theme Creator, an editor for World themes (exported; saves only in the browser)";
      exec = ''pnpm --filter @motet/theme-creator dev "$@"'';
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
    tls-setup = {
      description = "Set up local HTTPS, trusted certificates and loopback hostnames (Linux; needs sudo)";
      exec = ''
        set -euo pipefail
        export MOTET_SYSTEM_CA_BUNDLE="${pkgs.cacert}/etc/ssl/certs/ca-bundle.crt"
        exec ${pkgs.bash}/bin/bash ${./tls-setup.sh} "$@"
      '';
    };
    tls-teardown = {
      description = "Remove the local Motet wildcard DNS route (Linux; needs sudo)";
      exec = ''
        set -euo pipefail
        ip_cmd=$(command -v ip)
        if "$ip_cmd" link show dev motetdns >/dev/null 2>&1; then
          if ! "$ip_cmd" -details link show dev motetdns | grep -qw dummy; then
            echo 'The motetdns interface is not a dummy link; refusing to remove it.' >&2
            exit 1
          fi
          if command -v resolvectl >/dev/null 2>&1 && resolvectl status >/dev/null 2>&1; then
            sudo "$(command -v resolvectl)" revert motetdns
          fi
          sudo "$ip_cmd" link delete motetdns type dummy
        fi
      '';
    };
    network-smoke = {
      description = "Verify two local PDS profiles reach the AppView through relay and Tap (needs devenv up)";
      exec = ''
        set -euo pipefail
        exec ${pkgs.python3}/bin/python ${./network-smoke.py} "$@"
      '';
    };
    stack = {
      description = "Deployment-shaped stack with the local atproto network in podman";
      exec = ''
        set -euo pipefail
        for service in appview tenor plc relay tap pds-rs caddy dnsmasq; do
          nix build ".#$service-image" -o "result-$service"
          "./result-$service" | podman load
        done
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
        tests-integration
        network-smoke
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
    # stdenv also supplies GNU fmt; prefer the repository's formatter command.
    export PATH="${config.scripts.fmt.scriptPackage}/bin:$PATH"
    echo "Motet dev shell. 'devenv up' starts the local network, AppView and tenor; 'devenv info' lists scripts."
  '';
}
