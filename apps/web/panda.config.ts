import { defineConfig } from '@pandacss/dev';
import base from '@pandacss/preset-base';
import motet, { motetCssVarRoot } from '@motet/design/preset';

export default defineConfig({
	preflight: true,
	// Panda's own token preset stays out: tokens.css is the single source of
	// values, and Motet's preset only names its variables. The base preset
	// brings Panda's conditions and utilities.
	presets: [base, motet],
	// Aliases must resolve where a reader or World overrides the raw tokens,
	// not only at :root where var() would freeze them to the root palette.
	cssVarRoot: `${motetCssVarRoot}, [data-theme]`,
	// The raw tokens own the short names (--surface); Panda's aliases get a prefix.
	prefix: { cssVar: 'm', className: '' },
	// Screens ported from the design export write recipe classes in markup,
	// which Panda can't extract. Emit every recipe until they call the recipes.
	staticCss: { recipes: '*' },
	include: ['./src/**/*.{ts,svelte}'],
	exclude: [],
	outdir: 'styled-system',
	strictTokens: true,
	strictPropertyValues: true
});
