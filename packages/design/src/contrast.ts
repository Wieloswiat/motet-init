// WCAG 2 contrast, and the themes as tokens.css defines them.
//
// tokens.css comes from the design handoff (the Theme Creator), which
// generates it from tokens.json. Until tokens.json lives in this repository,
// tokens.css is the source of truth and is edited only by regenerating it.

export type Theme = { name: string; colours: Record<string, string> };

/** Relative luminance of an sRGB colour written #rrggbb (alpha ignored). */
function luminance(hex: string): number {
	const channel = (i: number) => {
		const c = parseInt(hex.slice(1 + 2 * i, 3 + 2 * i), 16) / 255;
		return c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
	};
	return 0.2126 * channel(0) + 0.7152 * channel(1) + 0.0722 * channel(2);
}

/** The WCAG 2 contrast ratio between two colours, from 1 to 21. */
export function contrast(a: string, b: string): number {
	const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x) as [number, number];
	return (hi + 0.05) / (lo + 0.05);
}

/** Every theme block in a tokens.css, with its opaque colour tokens. */
export function themes(css: string): Theme[] {
	const blocks = css.matchAll(/^(?::root, )?\[data-theme="([a-z-]+)"\]\s*\{([^}]*)\}/gm);
	return [...blocks].map(([, name = '', body = '']) => ({
		name,
		colours: Object.fromEntries(
			[...body.matchAll(/--([a-z0-9-]+):\s*(#[0-9a-f]{6})\b(?![0-9a-f])/g)].map(([, k, v]) => [
				k,
				v
			])
		)
	}));
}
