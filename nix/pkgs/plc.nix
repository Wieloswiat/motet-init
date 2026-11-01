{
  lib,
  stdenvNoCC,
  fetchFromGitHub,
  fetchPnpmDeps,
  nodejs_24,
  pnpm_11,
  pnpmConfigHook,
  makeWrapper,
}:
let
  pnpm = pnpm_11;
in
stdenvNoCC.mkDerivation (finalAttrs: {
  pname = "motet-plc";
  version = "0-unstable-2026-10-21";
  src = fetchFromGitHub {
    owner = "did-method-plc";
    repo = "did-method-plc";
    # Upstream HEAD on 2026-10-26, including its Node 24 server update.
    rev = "ed532fadf23c524c0dfddc2e7c2b7da85b432ef5";
    hash = "sha256-Aj6ImiT0Qv+hwuQB4ukyMN862ZMjDB0ElCcTfottf74=";
  };

  strictDeps = true;
  nativeBuildInputs = [
    nodejs_24
    pnpm
    pnpmConfigHook
    makeWrapper
  ];
  # The compiler lives at the workspace root; the service adds Datadog, not PLC.
  pnpmWorkspaces = [
    "did-method-plc"
    "@did-plc/server..."
  ];
  env = {
    # Nix supplies pnpm; neither packageManager nor devEngines may download it.
    pnpm_config_pm_on_fail = "ignore";
    # Keep dependency links inside the installed workspace, not a global store.
    pnpm_config_enable_global_virtual_store = "false";
  };
  pnpmDeps = fetchPnpmDeps {
    inherit (finalAttrs)
      pname
      version
      src
      pnpmWorkspaces
      env
      ;
    inherit pnpm;
    fetcherVersion = 4;
    hash = "sha256-Ts63x1AgKZ+mKz82HT14paxzT2Q4Muu2OJ0NMBzYTsg=";
  };

  buildPhase = ''
    runHook preBuild
    pnpm --filter '@did-plc/server...' run build
    runHook postBuild
  '';
  installPhase = ''
    runHook preInstall
    # Unlike deploy --legacy, this keeps the upstream lockfile frozen.
    pnpm --filter did-method-plc --filter '@did-plc/server...' install \
      --offline --prod --frozen-lockfile --ignore-scripts \
      --config.confirm-modules-purge=false
    mkdir -p "$out/lib/plc/packages" "$out/bin"
    cp -r node_modules package.json "$out/lib/plc/"
    for package in lib server; do
      mkdir -p "$out/lib/plc/packages/$package"
      cp -r "packages/$package/dist" "packages/$package/node_modules" \
        "packages/$package/package.json" "$out/lib/plc/packages/$package/"
    done
    # The dev shell exports Motet's PostgreSQL URL. PLC must remain in memory.
    # A fixed working directory also prevents dotenv loading the caller's .env.
    makeWrapper ${lib.getExe nodejs_24} "$out/bin/plc-server" \
      --chdir "$out/lib/plc" \
      --unset DATABASE_URL \
      --set-default PORT 2582 \
      --set-default NODE_ENV production \
      --add-flags "$out/lib/plc/packages/server/dist/bin.js"
    runHook postInstall
  '';
  meta = {
    description = "Pinned DID PLC directory for the local atproto network";
    homepage = "https://github.com/did-method-plc/did-method-plc";
    license = lib.licenses.mit;
    mainProgram = "plc-server";
    platforms = lib.platforms.unix;
  };
})
