import { expect, test, type Locator } from '@playwright/test';

// The oracle is WCAG 2 relative luminance over the colours Chromium
// actually paints, not the token parser or contrast function under test.
function luminance(colour: string): number {
	const channels = colour
		.slice(colour.indexOf('(') + 1, -1)
		.split(',')
		.map((channel) => {
			const linear = Number(channel) / 255;

			return linear <= 0.04045 ? linear / 12.92 : ((linear + 0.055) / 1.055) ** 2.4;
		});

	const [red = NaN, green = NaN, blue = NaN] = channels;

	return 0.2126 * red + 0.7152 * green + 0.0722 * blue;
}

async function renderedContrast(text: Locator, surface: Locator): Promise<number> {
	const foreground = await text.evaluate((element) => getComputedStyle(element).color);
	const background = await surface.evaluate((element) => getComputedStyle(element).backgroundColor);
	const a = luminance(foreground);
	const b = luminance(background);

	return (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05);
}

test('the design page hydrates and changes theme under the production content security policy', async ({
	page
}) => {
	const errors: string[] = [];

	page.on('console', (message) => {
		if (message.type() === 'error') {
			errors.push(message.text());
		}
	});
	page.on('pageerror', (error) => {
		errors.push(error.message);
	});

	const response = await page.goto('/design');
	const policy = response?.headers()['content-security-policy'];

	expect(policy).toContain("object-src 'none'");
	// Posts are untrusted. Script source lists must not allow inline or
	// evaluated code, even if the server-rendered heading would still show.
	expect(policy).not.toMatch(/(default|script)-src[^;]*'unsafe-(inline|eval)'/);
	await expect(page.getByRole('heading', { name: 'Motet', level: 1 })).toBeVisible();
	await page.getByRole('radiogroup', { name: 'Theme' }).getByText('dark', { exact: true }).click();
	// The painted result waits for real hydration; an SSR-only page or a
	// frozen root alias used to leave this surface light.
	await expect(page.getByRole('main')).toHaveCSS('background-color', 'rgb(6, 12, 16)');
	await expect(page.getByRole('radio', { name: 'dark', exact: true })).toBeChecked();
	expect(errors).toEqual([]);
});

const palettes = [
	{
		name: 'dark',
		foreground: 'rgb(209, 223, 230)',
		background: 'rgb(6, 12, 16)',
		minimum: 4.5
	},
	{
		name: 'contrast-light',
		foreground: 'rgb(13, 22, 28)',
		background: 'rgb(225, 235, 242)',
		minimum: 7
	},
	{
		name: 'contrast-dark',
		foreground: 'rgb(232, 246, 254)',
		background: 'rgb(6, 12, 16)',
		minimum: 7
	},
	{
		name: 'light',
		foreground: 'rgb(32, 43, 48)',
		background: 'rgb(225, 235, 242)',
		minimum: 4.5
	}
];

for (const { name, foreground, background, minimum } of palettes) {
	// An updated data-theme marker alone missed the root-alias regression.
	// The real page must paint the chosen palette and keep its text legible.
	test(`choosing ${name} repaints the page and preserves readable text`, async ({ page }) => {
		await page.goto('/design');

		const main = page.getByRole('main');
		const group = page.getByRole('radiogroup', { name: 'Theme' });

		// Light is tested as a return journey, not as a restatement of the
		// initial value. This also exercises hydration before changing back.
		await group.getByText('dark', { exact: true }).click();
		await expect(main).toHaveCSS('background-color', 'rgb(6, 12, 16)');

		if (name !== 'dark') {
			await group.getByText(name, { exact: true }).click();
		}

		const choice = group.getByRole('radio', { name, exact: true });

		const selectedLabel = group
			.locator('label')
			.filter({ has: page.getByRole('radio', { name, exact: true }) });

		await expect(choice).toBeChecked();
		await expect(group.getByRole('radio', { checked: true })).toHaveCount(1);
		await expect(main).toHaveCSS('background-color', background);
		await expect(main).toHaveCSS('color', foreground);
		expect(await renderedContrast(main, main)).toBeGreaterThanOrEqual(minimum);
		expect(
			await renderedContrast(page.getByRole('heading', { name: 'Motet', level: 1 }), main)
		).toBeGreaterThanOrEqual(minimum);
		expect(await renderedContrast(selectedLabel, selectedLabel)).toBeGreaterThanOrEqual(minimum);
	});
}
