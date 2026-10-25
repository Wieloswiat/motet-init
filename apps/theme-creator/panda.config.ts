import { defineConfig } from '@pandacss/dev';
import base from '@pandacss/preset-base';
import motet, { motetCssVarRoot } from '@motet/design/preset';

// The same setup as apps/web, so the editor looks like the app it themes.
export default defineConfig({
	preflight: true,
	presets: [base, motet],
	cssVarRoot: motetCssVarRoot,
	prefix: { cssVar: 'm', className: '' },
	// The editor builds its markup in plain script with recipe class names,
	// which Panda can't extract. Every recipe is emitted instead.
	staticCss: { recipes: '*' },
	include: [],
	outdir: 'styled-system'
});
