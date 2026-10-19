# The Rust workspace, built with crane: one dependency build shared by every
# check, so `nix flake check` and CI reuse the same artifacts.
{
  pkgs,
  crane,
  rustToolchain,
  src,
}:
let
  inherit (pkgs) lib;
  craneLib = (crane.mkLib pkgs).overrideToolchain (_: rustToolchain);

  # Cargo sources, plus what builds and tests read at compile or run time.
  # Anything not listed here can't affect a build, so editing docs or the web
  # app never invalidates the Rust cache.
  source = lib.fileset.toSource {
    root = src;
    fileset = lib.fileset.unions [
      (craneLib.fileset.commonCargoSources src)
      (src + "/.config/nextest.toml")
      (src + "/deny.toml")
      (src + "/lexicons")
      (lib.fileset.fileFilter (f: f.hasExt "snap") src)
      (lib.fileset.maybeMissing (src + "/crates/appview/migrations"))
      (lib.fileset.fileFilter (f: f.hasExt "txt") (src + "/crates"))
    ];
  };

  common = {
    src = source;
    pname = "motet";
    version = "0.0.0";
    strictDeps = true;
    nativeBuildInputs = [ pkgs.git ];
    buildInputs = lib.optionals pkgs.stdenv.hostPlatform.isDarwin [ pkgs.libiconv ];
  };

  cargoArtifacts = craneLib.buildDepsOnly common;

  binary =
    {
      crate,
      main ? crate,
    }:
    craneLib.buildPackage (
      common
      // {
        inherit cargoArtifacts;
        pname = crate;
        cargoExtraArgs = "--locked -p ${crate}";
        doCheck = false; # tests run once, in checks.nextest
        meta.mainProgram = main;
      }
    );

  binaries = {
    motet-appview = binary { crate = "motet-appview"; };
    motet-tenor = binary { crate = "motet-tenor"; };
    motet-render-worker = binary {
      crate = "motet-render";
      main = "motet-render-worker";
    };
    motet-lexgen = binary { crate = "motet-lexgen"; };
  };

  # OCI images for deploy/compose.yaml, built without Docker. Linux only.
  image =
    name: bin:
    pkgs.dockerTools.streamLayeredImage {
      inherit name;
      tag = "dev";
      contents = [ pkgs.cacert ];
      config = {
        Entrypoint = [ (lib.getExe bin) ];
        User = "65534:65534";
        Env = [ "SSL_CERT_FILE=${pkgs.cacert}/etc/ssl/certs/ca-bundle.crt" ];
      };
    };
in
{
  packages =
    binaries
    // lib.optionalAttrs pkgs.stdenv.hostPlatform.isLinux {
      appview-image = image "motet-appview" binaries.motet-appview;
      tenor-image = image "motet-tenor" binaries.motet-tenor;
    };

  checks = {
    fmt = craneLib.cargoFmt { src = source; };

    clippy = craneLib.cargoClippy (
      common
      // {
        inherit cargoArtifacts;
        cargoClippyExtraArgs = "--all-targets --all-features -- --deny warnings";
      }
    );

    # The default suite: pure and in-process tests, property tests included.
    # Integration tests need services and run outside the sandbox (the tests-integration script).
    nextest = craneLib.cargoNextest (
      common
      // {
        inherit cargoArtifacts;
        cargoNextestExtraArgs = "--profile ci";
        # Property tests: a fixed seed in the sandbox would hide nothing new
        # but make failures reproducible; proptest persists regressions in
        # each crate's proptest-regressions/ which are committed.
        PROPTEST_CASES = "256";
      }
    );

    # Advisories need the network, so they run in CI (the audit script), not here.
    deny = craneLib.cargoDeny {
      src = source;
      cargoDenyChecks = "bans licenses sources";
    };

    # Generated lexicon code must match the lexicons it was generated from.
    lexicons-fresh = pkgs.runCommand "lexicons-fresh" { } ''
      cd ${source}
      ${lib.getExe binaries.motet-lexgen} check --lexicons lexicons --rust crates/lexicons/src/generated
      touch $out
    '';
  };
}
