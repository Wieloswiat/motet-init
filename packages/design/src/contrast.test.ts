import { readFileSync } from 'node:fs';
import fc from 'fast-check';
import { describe, expect, it } from 'vitest';
import { contrast, themes } from './contrast.js';

const css = readFileSync(new URL('./tokens.css', import.meta.url), 'utf8');

const byte = fc.integer({ min: 0, max: 255 });
const rgb = fc.tuple(byte, byte, byte);

// Generate bytes, not arbitrary CSS: contrast accepts opaque #rrggbb colours.
function hex(channels: readonly number[]): string {
	return `#${channels.map((channel) => channel.toString(16).padStart(2, '0')).join('')}`;
}

const colour = rgb.map(hex);

const propertySettings: Pick<fc.Parameters, 'numRuns' | 'seed' | 'path'> = { numRuns: 500 };

if (process.env.FC_SEED !== undefined) {
	propertySettings.seed = Number(process.env.FC_SEED);
}

if (process.env.FC_PATH !== undefined) {
	propertySettings.path = process.env.FC_PATH;
}

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

// The theme checks are only as good as `contrast`, so it must give WCAG's
// ratios. WebAIM publishes these pairs to two decimals, truncated rather than
// rounded by Dec2: https://webaim.org/resources/contrastchecker/contrast.js?2
const reference: [string, string, number][] = [
	['#000000', '#ffffff', 21],
	['#0a0a0a', '#000000', 1.06],
	['#0b0b0b', '#000000', 1.06],
	['#767676', '#ffffff', 4.54],
	['#0000ff', '#ffffff', 8.59],
	['#ff0000', '#000000', 5.25]
];

describe('contrast', () => {
	it.each(reference)('%s and %s match the published %s:1 precision', (a, b, published) => {
		const ratio = contrast(a, b);

		expect(ratio).toBeGreaterThanOrEqual(published);
		expect(ratio).toBeLessThan(published + 0.01);
	});

	it.each([
		['#ff0000', 5.252],
		['#00ff00', 15.304],
		['#0000ff', 2.444]
	] as const)('%s has its WCAG primary luminance against black', (primary, ratio) => {
		// The sRGB primary luminances are fixed WCAG reference values, not
		// another implementation of the transfer curve under test.
		expect(contrast(primary, '#000000')).toBeCloseTo(ratio, 10);
	});

	it('uses the linear WCAG value at the last 8-bit step below the sRGB breakpoint', () => {
		// WCAG's linear endpoint gives the exact contrast 17473/16473 for byte
		// 10 against black. Two-decimal checker displays conceal a wrong branch.
		// https://www.w3.org/WAI/WCAG22/Understanding/contrast-minimum.html#dfn-relative-luminance
		expect(contrast('#0a0a0a', '#000000')).toBeCloseTo(17473 / 16473, 10);
	});

	it('does not change when foreground and background are exchanged', () => {
		fc.assert(
			fc.property(colour, colour, (foreground, background) => {
				expect(contrast(foreground, background)).toBe(contrast(background, foreground));
			}),
			propertySettings
		);
	});

	it('gives identical colours the minimum contrast of 1:1', () => {
		fc.assert(
			fc.property(colour, (value) => {
				expect(contrast(value, value)).toBe(1);
			}),
			propertySettings
		);
	});

	it('keeps relative luminance between black and white for every sRGB colour', () => {
		fc.assert(
			fc.property(colour, (value) => {
				// Against black, a luminance in [0, 1] must produce contrast in
				// [1, 21]. This checks the private luminance through its public use.
				expect(contrast(value, '#000000')).toBeGreaterThanOrEqual(1);
				expect(contrast(value, '#000000')).toBeLessThanOrEqual(21);
				expect(contrast(value, '#ffffff')).toBeGreaterThanOrEqual(1);
				expect(contrast(value, '#ffffff')).toBeLessThanOrEqual(21);
			}),
			propertySettings
		);
	});

	it('keeps every foreground/background pair within the WCAG contrast range', () => {
		fc.assert(
			fc.property(colour, colour, (foreground, background) => {
				const ratio = contrast(foreground, background);

				expect(ratio).toBeGreaterThanOrEqual(1);
				expect(ratio).toBeLessThanOrEqual(21);
			}),
			propertySettings
		);
	});

	it.each([
		['red', 0],
		['green', 1],
		['blue', 2]
	] as const)(
		'increasing %s alone increases luminance rather than reversing contrast',
		(_channelName, channelIndex) => {
			fc.assert(
				fc.property(rgb, fc.integer({ min: 0, max: 254 }), (channels, lowerValue) => {
					const lower = [...channels];
					const higher = [...channels];

					lower[channelIndex] = lowerValue;
					higher[channelIndex] = lowerValue + 1;

					expect(contrast(hex(higher), '#000000')).toBeGreaterThan(contrast(hex(lower), '#000000'));
					expect(contrast(hex(higher), '#ffffff')).toBeLessThan(contrast(hex(lower), '#ffffff'));
				}),
				propertySettings
			);
		}
	);
});

describe('theme extraction for accessibility checks', () => {
	const letter = fc.constantFrom(...'abcdefghijklmnopqrstuvwxyz');

	const themeName = fc
		.array(fc.oneof(letter, fc.constant('-')), { minLength: 1, maxLength: 20 })
		.map((characters) => characters.join(''));

	const roleName = fc
		.tuple(letter, fc.array(fc.oneof(letter, fc.constantFrom(...'0123456789-')), { maxLength: 20 }))
		.map(([first, rest]) => first + rest.join(''));

	const handoff = fc.uniqueArray(
		fc.record({
			name: themeName,
			colours: fc.dictionary(roleName, colour, { minKeys: 1, maxKeys: 8 })
		}),
		{ minLength: 2, maxLength: 5, selector: (theme) => theme.name }
	);

	it('keeps every theme and opaque role without leaking neighbouring selectors or alpha colours', () => {
		fc.assert(
			fc.property(
				handoff,
				fc.boolean(),
				fc.constantFrom('', ' ', '\t', '\n'),
				(expected, rootAlias, gap) => {
					const blocks = expected.map(({ name, colours }, index) => {
						const selector = `${rootAlias && index === 0 ? ':root, ' : ''}[data-theme="${name}"]`;

						const declarations = Object.entries(colours)
							.map(([role, value]) => `  --${role}:${gap}${value};`)
							.join('\n');

						return `${selector}${gap}{\n${declarations}\n  --scrim: #01234567;\n  --overlay: rgb(0 0 0 / 0.5);\n}`;
					});

					const stylesheet = [
						':root { --outside: #fedcba; }',
						...blocks,
						'.world [data-theme="unrelated"] { --outside: #abcdef; }'
					].join('\n');

					expect(themes(stylesheet)).toEqual(expected);
				}
			),
			propertySettings
		);
	});

	it('retains an empty theme so missing accessibility roles cannot disappear silently', () => {
		expect(themes('[data-theme="dark"]{}')).toEqual([{ name: 'dark', colours: {} }]);
	});

	it('keeps the last opaque declaration, matching the CSS cascade', () => {
		expect(
			themes('[data-theme="light"] { --ink: #ffffff; --ink: #000000; --surface: #fefefe; }')
		).toEqual([{ name: 'light', colours: { ink: '#000000', surface: '#fefefe' } }]);
	});

	it('does not treat shortened, alpha, or suffixed hexadecimal values as opaque colours', () => {
		expect(
			themes(`[data-theme="light"] {
  --short: #abc;
  --alpha: #11223344;
  --hex-suffix: #abcdefa;
  --word-suffix: #abcdefg;
  --alias: var(--ink);
  --opaque: #123456;
}`)
		).toEqual([{ name: 'light', colours: { opaque: '#123456' } }]);
	});

	it('does not turn an incomplete or unrelated selector into a named theme', () => {
		expect(
			themes(`:root { --ink: #000000; }
[data-theme=""] { --ink: #000000; }
[data-theme="light"]:focus { --ink: #000000; }
[data-theme="unfinished" { --ink: #000000; }
[data-theme="unclosed"] { --ink: #000000;`)
		).toEqual([]);
	});
});

describe('tokens.css', () => {
	describe.each([
		['light', 4.5],
		['dark', 4.5],
		['contrast-light', 7],
		['contrast-dark', 7]
	] as const)('%s', (name, minimum) => {
		it.each(text)(`%s on %s reaches ${minimum}:1`, (fg, bg) => {
			const theme = themes(css).find((candidate) => candidate.name === name);

			expect(theme, `${name} theme`).toBeDefined();

			const colours = theme!.colours;

			expect(colours[fg], fg).toBeDefined();
			expect(colours[bg], bg).toBeDefined();
			expect(contrast(colours[fg]!, colours[bg]!)).toBeGreaterThanOrEqual(minimum);
		});

		it.each(graphics)('%s on %s reaches 3:1', (fg, bg) => {
			const theme = themes(css).find((candidate) => candidate.name === name);

			expect(theme, `${name} theme`).toBeDefined();

			const colours = theme!.colours;

			expect(colours[fg], fg).toBeDefined();
			expect(colours[bg], bg).toBeDefined();
			expect(contrast(colours[fg]!, colours[bg]!)).toBeGreaterThanOrEqual(3);
		});
	});
});
