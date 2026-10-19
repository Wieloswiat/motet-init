# Working in this repository

Read `README.md` for the layout and commands, `docs/architecture/overview.md`
for how the parts fit, and `docs/adr/` before changing a decision recorded
there (supersede it with a new ADR rather than editing it).

- Commands are devenv scripts (`nix/devenv.nix`); there is no justfile. Add a
  new command there with a description.
- Before pushing: `fmt`, `lint`, `tests`, and `tests-integration` when the
  change touches PostgreSQL, S3 or the web app. `devenv test` is the full CI
  gate.
- Tests state properties of the system (`docs/testing.md`). Don't add tests
  that restate the implementation; update the acceptance table when one of
  its rows gains a test.
- After editing `lexicons/`, run `lexgen` and commit the generated code.
- Projection schema changes bump `PROJECTION_VERSION`; service-state changes
  are new migrations in `crates/appview/migrations`.
