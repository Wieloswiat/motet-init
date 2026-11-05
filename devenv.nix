{ pkgs, inputs, ... }:
let
  # flake.lock owns build pins; reject a shell using different source contents.
  lock = builtins.fromJSON (builtins.readFile ./flake.lock);
  samePin =
    name:
    let
      pinned = lock.nodes.${lock.nodes.root.inputs.${name}}.locked;
    in
    inputs.${name}.rev == pinned.rev && inputs.${name}.narHash == pinned.narHash;
  rustToolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
  kani = import ./nix/pkgs/kani.nix { inherit pkgs; };
  network = import ./nix/network.nix {
    inherit pkgs rustToolchain;
    crane = inputs.crane;
  };
in
assert
  builtins.all samePin [
    "nixpkgs"
    "rust-overlay"
    "crane"
  ]
  || throw "devenv inputs differ from flake.lock; align devenv.yaml revisions and run devenv update";
{
  imports = [
    (import ./nix/devenv.nix {
      inherit rustToolchain kani;
      networkPackages = network.packages;
    })
  ];
}
