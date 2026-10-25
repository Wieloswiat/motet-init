// Oxlint runs the anti-slop rules and its own default correctness rules.
// anti-slop is vendored in tools/oxlint/anti-slop; its UPSTREAM.md records the
// source. The rules read syntax and scope, not TypeScript types. In .svelte
// files Oxlint reads only the <script> blocks, so ESLint (eslint.config.js)
// still runs after Oxlint and checks the Svelte markup.
import { defineConfig } from 'oxlint';

export default defineConfig({
	// Oxlint already skips what .gitignore lists: build output, node_modules and
	// the dev shell state. The paths below are committed but are not our source.
	ignorePatterns: [
		// lexgen writes these files, and lexgen-check compares them byte for byte.
		'packages/lexicons/src/generated/**',
		// Vendored. It stays identical to upstream, so that an update can merge.
		'tools/oxlint/anti-slop/**',
		// Exported from the Motet design system (packages/design/UPSTREAM.md). A
		// new export replaces these files, so they stay as the export wrote them.
		'packages/design/src/panda/**',
		'packages/design/src/engine/**',
		'packages/design/src/demo/worlds.js',
		// The Theme Creator's own script, as exported (apps/theme-creator/UPSTREAM.md).
		'apps/theme-creator/src/main.js',
		'apps/theme-creator/src/frame.js',
		// Agent tools install their own files here. The list is anti-slop's.
		'.agent/**',
		'.agents/**',
		'.claude/**',
		'.codex/**',
		'.continue/**',
		'.cursor/**',
		'.gemini/**',
		'.opencode/**',
		'.pi/**',
		'.roo/**',
		'.windsurf/**'
	],
	jsPlugins: [{ name: 'anti-slop', specifier: './tools/oxlint/anti-slop/index.ts' }],
	// The category Oxlint enables by default, set to error: a finding is a failure.
	categories: { correctness: 'error' },
	rules: {
		// Every generic anti-slop rule. The Effect rules are not registered,
		// because Motet does not depend on Effect.
		'anti-slop/no-array-filter-map': 'error',
		// This rule and oxc/no-accumulating-spread find different copies of a
		// reducer's accumulator, so anti-slop enables both.
		'anti-slop/no-reduce-accumulator-copy': 'error',
		'oxc/no-accumulating-spread': 'error',
		'anti-slop/no-chained-type-assertions': 'error',
		'anti-slop/no-conditional-empty-object-spread': 'error',
		'anti-slop/no-known-value-widening': 'error',
		'anti-slop/no-module-mocking': 'error',
		'anti-slop/no-object-parameters': 'error',
		'anti-slop/no-reflect-apply': 'error',
		'anti-slop/no-reflect-get': 'error',
		'anti-slop/no-runtime-typeof': 'error',
		'anti-slop/no-shape-in-symbol-names': 'error',
		'anti-slop/no-unknown-parameters': 'error',
		'anti-slop/no-unknown-returns': 'error',
		'anti-slop/no-unknown-type-aliases': 'error',
		'anti-slop/no-unsafe-dictionary-type': 'error',
		'anti-slop/no-widen-then-assert': 'error',
		'anti-slop/require-readable-spacing': 'error',
		'anti-slop/require-safety-comment-for-type-assertion': 'error'
	}
});
