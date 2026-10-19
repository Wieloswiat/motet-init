import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { contrast, themes } from './contrast.js';

const css = readFileSync(new URL('./tokens.css', import.meta.url), 'utf8');

// Text on the surfaces it is set on, and labels on the fills they sit on.
const text: [string, string][] = [
	['ink', 'surface'],
	['ink', 'canvas'],
	['ink-muted', 'surface'],
	['ink-muted', 'surface-quiet'],
	['accent-ink', 'surface'],
	['on-accent', 'accent'],
	['on-accent-soft', 'accent-soft'],
	['danger-ink', 'surface'],
	['on-danger', 'danger'],
	['caution-ink', 'surface'],
	['on-caution', 'caution'],
	['positive-ink', 'surface'],
	['on-positive', 'positive'],
	['info-ink', 'surface'],
	['on-info', 'info'],
	['chrome-ink', 'chrome-bg-solid']
];
// Borders and focus rings that carry meaning: WCAG's 3:1 for UI components.
const graphics: [string, string][] = [
	['line-strong', 'surface'],
	['focus', 'surface']
];

describe('tokens.css', () => {
	const all = themes(css);

	it('defines the four themes', () => {
		expect(all.map((t) => t.name).sort()).toEqual([
			'contrast-dark',
			'contrast-light',
			'dark',
			'light'
		]);
	});

	describe.each(all)('$name', ({ name, colours }) => {
		const minimum = name.startsWith('contrast') ? 7 : 4.5;

		it.each(text)(`%s on %s reaches ${minimum}:1`, (fg, bg) => {
			expect(colours[fg], fg).toBeDefined();
			expect(colours[bg], bg).toBeDefined();
			expect(contrast(colours[fg]!, colours[bg]!)).toBeGreaterThanOrEqual(minimum);
		});

		it.each(graphics)('%s on %s reaches 3:1', (fg, bg) => {
			expect(contrast(colours[fg]!, colours[bg]!)).toBeGreaterThanOrEqual(3);
		});
	});
});
