# anti-slop, vendored

Oxlint rules from [dmmulroy/anti-slop](https://github.com/dmmulroy/anti-slop),
under the MIT licence in `LICENSE`. Upstream publishes no npm package: the
rules are meant to be copied and then owned. This record identifies the
upstream bytes, so that a later update can do a three-way merge.

## Source

- Repository: <https://github.com/dmmulroy/anti-slop>
- Commit: `13909c9e07590f5a85d93c62712259fe14a883f7` (2026-10-13, "Merge pull
  request #45 from haydenbleasel/feat/group-singleline-bindings")
- Copied tree: `skills/install-anti-slop/assets/anti-slop` at that commit (git
  tree `54b50f11bed7f52ca1948f8802f4df591644762f`). Upstream generates this
  tree from its `src/` and leaves out the `*.test.ts` files.
- Method: the upstream installer, run on 2026-10-25 from the repository root,
  with the default destination:
  `node <checkout>/skills/install-anti-slop/scripts/install.mjs`

## Local changes

The copied files are unchanged. Two files are added:

- `LICENSE`: upstream's root `LICENSE` at the same commit, byte for byte. The
  installer does not copy it, and MIT requires the notice with every copy.
- `UPSTREAM.md`: this record.

`vendor/eslint-stylistic/` is a vendored copy inside the vendored copy. Its
`LICENSE` and `UPSTREAM.md` must stay with it.

## How Motet uses it

- `oxlint.config.ts` registers `index.ts` as the `anti-slop` plugin. It sets
  every generic rule, and Oxlint's `oxc/no-accumulating-spread`, to `error`.
  `pnpm lint` runs Oxlint and then ESLint.
- `effect/` is copied but not registered, because Motet does not depend on
  Effect. Register it only if a package manifest adds `effect`.
- Oxlint, ESLint, Prettier and typos ignore this directory, so that it stays
  identical to upstream. `pnpm lint` uses the rules but does not test them.
  Upstream's RuleTester suites test them: `src/**/*.test.ts` at the commit
  above.
- `oxlint` and `@oxlint/plugins` are pinned to one exact version in the root
  `package.json` (1.89.0 at install). Upstream tested this commit with 1.78.0.
  The JS plugin API is alpha and does not follow semver, so upgrade the two
  packages together and then run `pnpm lint`. Renovate's `group:monorepos`
  preset puts both packages in one pull request.
- The rules are TypeScript. Oxlint loads them with Node.js type stripping,
  which needs Node.js 22.18 or later.
- The rules read syntax and lexical scope, not TypeScript types. In `.svelte`
  files, Oxlint lints only the `<script>` blocks. No anti-slop rule examines
  the markup.

## Check that the copy is pristine

```sh
git clone https://github.com/dmmulroy/anti-slop /tmp/anti-slop
git -C /tmp/anti-slop checkout 13909c9e07590f5a85d93c62712259fe14a883f7
diff -r /tmp/anti-slop/skills/install-anti-slop/assets/anti-slop tools/oxlint/anti-slop
```

The only differences must be `LICENSE` and `UPSTREAM.md`, each reported as
"Only in tools/oxlint/anti-slop".

## Update

Follow upstream's `skills/install-anti-slop/references/update.md`. Put the
incoming revision in a temporary directory, not here. Merge with the commit
above as the base. Then record here the new commit, the changes you adopted
and every local change.
