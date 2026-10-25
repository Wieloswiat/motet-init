// Motet's Panda CSS preset.
//
// Colours are CSS variables from tokens.css, not values: a World's theme
// replaces those variables at runtime on the subtree it governs, so one
// stylesheet serves every World. Panda gives the components typed,
// statically extracted styles over them.

import { definePreset } from '@pandacss/dev';

const roles = [
	'canvas',
	'surface',
	'surface-quiet',
	'surface-raised',
	'surface-overlay',
	'ink',
	'ink-muted',
	'line',
	'line-strong',
	'accent',
	'on-accent',
	'accent-ink',
	'accent-soft',
	'on-accent-soft',
	'ornament',
	'ornament-soft',
	'focus',
	'danger',
	'on-danger',
	'danger-ink',
	'danger-soft',
	'caution',
	'on-caution',
	'caution-ink',
	'caution-soft',
	'positive',
	'on-positive',
	'positive-ink',
	'positive-soft',
	'info',
	'on-info',
	'info-ink',
	'info-soft',
	'selection',
	'scrim'
] as const;

// SAFETY: fromEntries makes one entry for each name. The callers let TypeScript
// infer K from the names, so every key of K has an entry.
const fromVariables = <K extends string>(names: readonly K[], prefix = '') =>
	Object.fromEntries(names.map((n) => [n, { value: `var(--${prefix}${n})` }])) as Record<
		K,
		{ value: string }
	>;

export const motetPreset = definePreset({
	name: '@motet/design',
	theme: {
		extend: {
			semanticTokens: {
				colors: fromVariables(roles),
				shadows: fromVariables(['1', '2', '3', '4'] as const, 'elevation-'),
				spacing: fromVariables(
					['1', '2', '3', '4', '5', '6', '8', '10', '12', '16'] as const,
					'space-'
				),
				radii: fromVariables(['xs', 'sm', 'md', 'lg', 'xl', 'full', 'control'] as const, 'radius-'),
				fonts: fromVariables(
					['ui', 'reading', 'display', 'mono', 'dice', 'icons'] as const,
					'font-'
				)
			}
		}
	}
});
