# The development environment: toolchains, local services and git hooks.
#
#   nix develop --no-pure-eval   (or `direnv allow`)
#   devenv up                    PostgreSQL with pg_search, SeaweedFS (S3)
#   just                         list tasks
#
# CI enters this same shell, so a task that works here works there.
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
    cargo-insta
    sqlx-cli
    just
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
    pnpm.enable = true;
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
    # Playwright uses nixpkgs' browsers rather than downloading its own.
    PLAYWRIGHT_BROWSERS_PATH = "${pkgs.playwright-driver.browsers}";
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

  enterShell = ''
    echo "Motet dev shell. 'devenv up' starts PostgreSQL and SeaweedFS (S3); 'just' lists tasks."
  '';
}
