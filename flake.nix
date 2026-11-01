{
  description = "Motet: play-by-forum on atproto";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-parts.url = "github:hercules-ci/flake-parts";
    devenv = {
      url = "github:cachix/devenv";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    crane.url = "github:ipetkov/crane";
    # Required by devenv's flake module, which evaluates its container
    # outputs during `nix flake check`.
    nix2container = {
      url = "github:nlewo/nix2container";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    mk-shell-bin.url = "github:rrbutani/nix-mk-shell-bin";
  };

  nixConfig = {
    extra-substituters = [ "https://devenv.cachix.org" ];
    extra-trusted-public-keys = [ "devenv.cachix.org-1:w1cLUi8dv3hnoSPGAuibQv+f9TZLr6cv/Hm9XgU50cw=" ];
  };

  outputs =
    inputs@{ flake-parts, ... }:
    flake-parts.lib.mkFlake { inherit inputs; } {
      imports = [ inputs.devenv.flakeModule ];
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
          # One toolchain, read from rust-toolchain.toml, for the shell, the
          # build and CI.
          rustToolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
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
            };

          # `nix flake check`: everything that needs no running services.
          checks = rust.checks // network.checks;

          devenv.shells.default = import ./nix/devenv.nix {
            inherit rustToolchain;
            networkPackages = network.packages;
          };

          formatter = pkgs.nixfmt;
        };
    };
}
