{ pkgs }:
let
  inherit (pkgs) lib;
  version = "0.68.0";
  # Kani's compiler links to this exact nightly's rustc_driver ABI. Keep it
  # private to the verifier; production builds still use rust-toolchain.toml.
  toolchain = pkgs.rust-bin.nightly."2026-08-21".minimal;
  releases = {
    x86_64-linux = {
      target = "x86_64-unknown-linux-gnu";
      hash = "sha256-MuK0hNc+3gu/ZKLPCHnEJZQiSX2K7E7mfUSN6a54Q9M=";
    };
    aarch64-linux = {
      target = "aarch64-unknown-linux-gnu";
      hash = "sha256-AELSaIvxVQJw3vSnnJaq7qE4YxgYy0q1SAEg8tUalCw=";
    };
    aarch64-darwin = {
      target = "aarch64-apple-darwin";
      hash = "sha256-pdOaXV50glOlU6pi8pXGw5coeSeijl4yaR1/8u2gw5g=";
    };
  };
  release = releases.${pkgs.stdenv.hostPlatform.system};
in
pkgs.stdenv.mkDerivation {
  pname = "kani";
  inherit version;

  # The release also pins CBMC 6.11.0 and its bundled SAT solvers.
  src = pkgs.fetchurl {
    url = "https://github.com/model-checking/kani/releases/download/kani-${version}/kani-${version}-${release.target}.tar.gz";
    inherit (release) hash;
  };

  nativeBuildInputs = [
    pkgs.makeWrapper
  ]
  ++ lib.optionals pkgs.stdenv.isLinux [ pkgs.autoPatchelfHook ];
  buildInputs = lib.optionals pkgs.stdenv.isLinux [ pkgs.stdenv.cc.cc.lib ];
  dontBuild = true;
  # Rust metadata in the bundled libraries is input to the verifier.
  dontStrip = true;

  installPhase = ''
    runHook preInstall
    mkdir -p "$out"
    cp -r . "$out/"
    ln -sfn ${toolchain} "$out/toolchain"
    runHook postInstall
  '';

  preFixup = lib.optionalString pkgs.stdenv.isLinux ''
    addAutoPatchelfSearchPath ${toolchain}/lib
  '';

  postFixup = ''
    # Resolve CBMC and goto tools from this bundle, never an ambient version.
    # The C frontend also needs a real host compiler and its Nix wrappers.
    for command in cargo-kani kani; do
      makeWrapper "$out/bin/kani-driver" "$out/bin/$command" \
        --argv0 "$command" \
        --prefix PATH : "$out/bin:${lib.makeBinPath [ pkgs.stdenv.cc ]}"
    done
  '';

  meta = {
    description = "Kani Rust verifier with its pinned compiler and CBMC solver";
    homepage = "https://model-checking.github.io/kani/";
    license = [
      lib.licenses.asl20
      lib.licenses.mit
    ];
    platforms = builtins.attrNames releases;
    mainProgram = "kani";
  };
}
