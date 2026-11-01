{
  lib,
  buildGoModule,
  fetchFromGitHub,
}:
buildGoModule {
  pname = "indigo";
  version = "0-unstable-2026-03-11";

  # tap-v0.1.10 pins both services to the same source and dependency graph.
  src = fetchFromGitHub {
    owner = "bluesky-social";
    repo = "indigo";
    rev = "4f47add43060c27e8a37d9d76482ecddf001fcd8";
    hash = "sha256-nDOLIRWTyj/R0h+70+bGi85RVe2OKLNbnSaKyyqc93Q=";
  };

  vendorHash = "sha256-s1S+b+QbptqJ2mxqkvsn7M5VWfLrlwpWgRjg6lq2WVE=";
  patches = [ ../patches/indigo-private-hosts.patch ];
  subPackages = [
    "cmd/relay"
    "cmd/tap"
  ];

  # Both services use the CGO SQLite driver; integration checks own service tests.
  env.CGO_ENABLED = "1";
  doCheck = false;

  meta = {
    description = "AT Protocol relay and Tap repository synchronization services";
    homepage = "https://github.com/bluesky-social/indigo";
    license = with lib.licenses; [
      mit
      asl20
    ];
    mainProgram = "relay";
  };
}
