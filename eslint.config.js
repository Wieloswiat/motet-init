// `pnpm lint` runs Oxlint (oxlint.config.ts) and then ESLint. ESLint stays
// because Oxlint cannot use the Svelte parser: only ESLint checks Svelte markup.
import prettier from 'eslint-config-prettier';
import js from '@eslint/js';
import svelte from 'eslint-plugin-svelte';
import { defineConfig, globalIgnores } from 'eslint/config';
import globals from 'globals';
import ts from 'typescript-eslint';

export default defineConfig(
	globalIgnores([
		'target/',
		'**/node_modules/',
		'**/.svelte-kit/',
		'**/build/',
		'**/dist/',
		'**/styled-system/',
		'**/reports/mutation/',
		'**/.stryker-tmp/',
		'**/playwright-report/',
		'**/test-results/',
		'packages/lexicons/src/generated/',
		// Vendored anti-slop rules (see oxlint.config.ts). They stay identical to
		// upstream, so that an update can merge.
		'tools/oxlint/anti-slop/',
		// Exported from the Motet design system (packages/design/UPSTREAM.md).
		'packages/design/src/panda/',
		'packages/design/src/engine/',
		'packages/design/src/demo/worlds.js',
		// The Theme Creator's own script, as exported (apps/theme-creator/UPSTREAM.md).
		'apps/theme-creator/src/main.js',
		'apps/theme-creator/src/frame.js',
		'.devenv/',
		'.direnv/'
	]),
	js.configs.recommended,
	ts.configs.recommended,
	svelte.configs.recommended,
	prettier,
	svelte.configs.prettier,
	{
		languageOptions: { globals: { ...globals.browser, ...globals.node } },
		rules: {
			// TypeScript checks undefined names better than ESLint can.
			'no-undef': 'off'
		}
	},
	{
		files: ['**/*.svelte', '**/*.svelte.ts', '**/*.svelte.js'],
		languageOptions: {
			parserOptions: {
				projectService: true,
				extraFileExtensions: ['.svelte'],
				parser: ts.parser
			}
		}
	}
);
