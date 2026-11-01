# Native processes and OCI images share the same pinned service builds.
{
  pkgs,
  crane,
  rustToolchain,
}:
let
  inherit (pkgs) lib;
  plc = pkgs.callPackage ./pkgs/plc.nix { };
  indigo = pkgs.callPackage ./pkgs/indigo.nix { };
  pds-rs = import ./pkgs/pds-rs.nix { inherit pkgs crane rustToolchain; };
  image =
    name: executable: extraEnv:
    pkgs.dockerTools.streamLayeredImage {
      inherit name;
      tag = "dev";
      contents = [
        pkgs.cacert
        pkgs.curl
      ];
      fakeRootCommands = ''
        mkdir -p ./data ./tmp
        chmod 1777 ./tmp
        chown 65534:65534 ./data
      '';
      config = {
        Entrypoint = [ executable ];
        WorkingDir = "/data";
        User = "65534:65534";
        Env = [ "SSL_CERT_FILE=${pkgs.cacert}/etc/ssl/certs/ca-bundle.crt" ] ++ extraEnv;
      };
    };
in
{
  packages = {
    inherit
      plc
      indigo
      pds-rs
      ;
  }
  // lib.optionalAttrs pkgs.stdenv.hostPlatform.isLinux {
    plc-image = image "motet-plc" "${plc}/bin/plc-server" [ ];
    relay-image = image "motet-relay" "${indigo}/bin/relay" [ ];
    tap-image = image "motet-tap" "${indigo}/bin/tap" [ ];
    pds-rs-image = image "motet-pds-rs" "${pds-rs}/bin/pds" [ "PDS_LEXICON_DIR=${../lexicons}" ];
  };
  checks = {
    network-plc = plc;
    network-indigo = indigo;
    network-pds-rs = pds-rs;
  };
}
