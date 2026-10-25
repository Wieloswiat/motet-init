// WCAG 2 contrast, and the themes as tokens.css defines them.
//
// tokens.css comes from the design handoff (the Theme Creator), which
// generates it from tokens.json. Until tokens.json lives in this repository,
// tokens.css is the source of truth and is edited only by regenerating it.

export type Theme = { name: string; colours: Record<string, string> };

/** Relative luminance of an sRGB colour written #rrggbb (alpha ignored). */
function luminance(hex: string): number {
	const channel = (i: number) => {
		const byte = parseInt(hex.slice(1 + 2 * i, 3 + 2 * i), 16);
		const c = byte / 255;

		// The sRGB breakpoint 0.04045 lies strictly between bytes 10 and 11.
		// Compare the integral channel: no 8-bit value exists at the breakpoint.
		return byte <= 10 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
	};

	return 0.2126 * channel(0) + 0.7152 * channel(1) + 0.0722 * channel(2);
}

/** The WCAG 2 contrast ratio between two colours, from 1 to 21. */
export function contrast(a: string, b: string): number {
	// SAFETY: The array has two elements, and sort() keeps the length.
	const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x) as [number, number];

	return (hi + 0.05) / (lo + 0.05);
}

/** Every theme block in a tokens.css, with its opaque colour tokens. */
export function themes(css: string): Theme[] {
	const blocks = css.matchAll(/^(?::root, )?\[data-theme="([a-z-]+)"\]\s*\{([^}]*)\}/gm);

	return [...blocks].map((block) => {
		// SAFETY: Both capture groups participate in every block match.
		const name = block[1]!;
		const body = block[2]!;

		return {
			name,
			colours: Object.fromEntries(
				// A word boundary already excludes hexadecimal (and other word)
				// suffixes, so an alpha colour cannot become an opaque one.
				[...body.matchAll(/--([a-z0-9-]+):\s*(#[0-9a-f]{6})\b/g)].map(([, k, v]) => [k, v])
			)
		};
	});
}
