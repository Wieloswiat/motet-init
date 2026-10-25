import { expect, test } from '@playwright/test';

// The demo screens' paths, from the design page's list. They render demo data
// only, so these tests check loading and theming, not any World workflow.
const screens = [
	'/',
	'/board',
	'/inbox',
	'/search',
	'/settings',
	'/sign-in',
	'/profile/aurelia.bsky.social',
	'/profile/iga.bsky.social',
	'/new-world',
	'/w/gilded-reach/visitor',
	'/w/gilded-reach/apply',
	'/w/gilded-reach',
	'/w/gilded-reach/play',
	'/w/gilded-reach/play/feast-of-lanterns',
	'/w/gilded-reach/play/new',
	'/w/gilded-reach/characters',
	'/w/gilded-reach/characters/lady-maren',
	'/w/gilded-reach/lore',
	'/w/gilded-reach/members',
	'/w/gilded-reach/settings',
	'/w/gilded-reach/manage'
];

for (const path of screens) {
	// The engine and demo are classic scripts loaded through Vite. A CSP
	// that blocks them, a missing asset or a hydration error shows up here.
	test(`${path} loads under the production content security policy without errors`, async ({
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

		const response = await page.goto(path, { waitUntil: 'networkidle' });

		expect(response?.status()).toBe(200);
		await expect(page.getByRole('heading').first()).toBeVisible();
		expect(errors).toEqual([]);
	});
}

test('a World page paints its own art once the engine mounts the World', async ({ page }) => {
	await page.goto('/w/gilded-reach/play/feast-of-lanterns');

	// The art is painted on the root's ::before. Without the engine, or with
	// Motet's own record in its place, the page has no art at all.
	const root = page.locator('.m-world__root').first();

	await expect
		.poll(() => root.evaluate((element) => getComputedStyle(element, '::before').backgroundImage))
		.toMatch(/^url\("data:image\/svg\+xml/);
});

test('an unknown World is a 404, not an empty screen', async ({ page }) => {
	const response = await page.goto('/w/no-such-world');

	expect(response?.status()).toBe(404);
});
