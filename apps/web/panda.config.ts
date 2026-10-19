import { defineConfig } from '@pandacss/dev';
import { motetPreset } from '@motet/design/preset';

export default defineConfig({
	preflight: true,
	// The base preset brings Panda's conditions and utilities. Panda's own
	// token preset stays out: tokens.css is the single source of values, and
	// ours only names its variables.
	presets: ['@pandacss/preset-base', motetPreset],
	include: ['./src/**/*.{ts,svelte}'],
	exclude: [],
	outdir: 'styled-system',
	strictTokens: true,
	strictPropertyValues: true
});
