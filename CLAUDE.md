# Working in this repository

Read `README.md` for the layout and commands, `docs/architecture/overview.md`
for how the parts fit, `docs/development.md` for how code is written and
reviewed, and `docs/adr/` before changing a decision recorded there
(supersede it with a new ADR rather than editing it).

- Commands are devenv scripts (`nix/devenv.nix`); there is no justfile. Add a
  new command there with a description.
- Before pushing: `fmt`, `lint`, `tests`, and `tests-integration` when the
  change touches PostgreSQL, S3 or the web app. `devenv test` is the full CI
  gate.
- Code follows `docs/development.md`: Google's Rust style guide with the
  exceptions listed there, lint exceptions as
  `#[expect(lint, reason = "…")]` on the smallest scope, no allocation or
  copy without a reason, and comments that say why in short, active
  sentences. Check a change against its review checklist.
- Tests state properties of the system (`docs/testing.md`). Each new test
  names the property it protects and a plausible bug it catches. Don't add
  tests that restate the implementation, source text, defaults, wiring or a
  mock's output; update the acceptance table when one of its rows gains a
  test. Fix a flaky test's cause; don't retry or skip it.
- Motet's only formal proofs are the `proofs` job's Kani harnesses, scoped as
  in `docs/testing.md`. Don't call anything else proved unless a proof job ran
  and passed.
- After editing `lexicons/`, run `lexgen` and commit the generated code.
- Projection schema changes bump `PROJECTION_VERSION`; service-state changes
  are new migrations in `crates/appview/migrations`.
