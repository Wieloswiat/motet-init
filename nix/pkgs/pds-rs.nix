{
  pkgs,
  crane,
  rustToolchain,
}:
let
  inherit (pkgs) lib;
  craneLib = (crane.mkLib pkgs).overrideToolchain (_: rustToolchain);
  rev = "09a868cdde938c789326f1db16ffc30bbf38c15e";

  src = pkgs.fetchgit {
    url = "https://tangled.org/ngerakines.me/atproto-crates";
    inherit rev;
    hash = "sha256-+eM4O4K9LD9CBUGhM44+VcRnPWJ9tV4wlArLETNQ0eg=";
    # The only submodule contains DASL test vectors, not server sources.
    fetchSubmodules = false;
  };

  common = {
    pname = "pds-rs";
    version = "0.15.0-rc.6";
    # Keep the lexicons, migrations, CSS and fonts embedded by the PDS build.
    inherit src;
    strictDeps = true;
    nativeBuildInputs = [ pkgs.pkg-config ];
    buildInputs = lib.optionals pkgs.stdenv.hostPlatform.isDarwin [ pkgs.libiconv ];
    # Match Dockerfile.pds. SQLite stores accounts, repositories and blob bytes.
    cargoExtraArgs = "--locked -p atproto-pds --bin pds --features clap,smtp,metrics,hickory-dns";
    BUILD_REV = rev;
    doCheck = false;
  };

  cargoArtifacts = craneLib.buildDepsOnly common;
in
craneLib.buildPackage (
  common
  // {
    inherit cargoArtifacts;
    meta = {
      description = "AT Protocol Personal Data Server with Spaces support";
      homepage = "https://tangled.org/ngerakines.me/atproto-crates";
      license = lib.licenses.mit;
      mainProgram = "pds";
    };
  }
)
