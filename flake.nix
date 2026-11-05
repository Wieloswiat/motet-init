{
  description = "Motet: play-by-forum on atproto";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-parts.url = "github:hercules-ci/flake-parts";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    crane.url = "github:ipetkov/crane";
  };

  nixConfig = {
    extra-substituters = [ "https://devenv.cachix.org" ];
    extra-trusted-public-keys = [ "devenv.cachix.org-1:w1cLUi8dv3hnoSPGAuibQv+f9TZLr6cv/Hm9XgU50cw=" ];
  };

  outputs =
    inputs@{ flake-parts, ... }:
    flake-parts.lib.mkFlake { inherit inputs; } {
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "aarch64-darwin"
      ];

      perSystem =
        { system, ... }:
        let
          pkgs = import inputs.nixpkgs {
            inherit system;
            overlays = [ (import inputs.rust-overlay) ];
          };
          # The production toolchain stays stable; Kani owns a separate,
          # ABI-matched nightly compiler in its private Nix closure.
          rustToolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
          kani = import ./nix/pkgs/kani.nix { inherit pkgs; };
          rust = import ./nix/rust.nix {
            inherit pkgs rustToolchain;
            crane = inputs.crane;
            src = ./.;
          };
          network = import ./nix/network.nix {
            inherit pkgs rustToolchain;
            crane = inputs.crane;
          };
        in
        {
          _module.args.pkgs = pkgs;

          packages =
            rust.packages
            // network.packages
            // {
              default = rust.packages.motet-appview;
              inherit kani;
            };

          # `nix flake check`: everything that needs no running services. The
          # local network's packages aren't checks: the gate builds them to
          # start them, and `nix build` builds them on demand.
          checks = rust.checks // {
            # A partial lock update must not give CI and developers different tools.
            shell-pins =
              let
                shellLock = builtins.fromJSON (builtins.readFile ./devenv.lock);
                samePin =
                  name:
                  let
                    shellPin = shellLock.nodes.${shellLock.nodes.root.inputs.${name}}.locked;
                  in
                  inputs.${name}.rev == shellPin.rev && inputs.${name}.narHash == shellPin.narHash;
              in
              assert
                builtins.all samePin [
                  "nixpkgs"
                  "rust-overlay"
                  "crane"
                ]
                || throw "devenv.lock differs from flake.lock; align devenv.yaml and run devenv update";
              pkgs.runCommand "motet-shell-pins" { } "touch $out";
          };

          formatter = pkgs.nixfmt;
        };
    };
}
