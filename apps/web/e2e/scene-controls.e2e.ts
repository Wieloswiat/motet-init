import { expect, test, type Page } from '@playwright/test';
import {
	at_motet_pilot_setscenecast as setCast,
	at_motet_pilot_closescene as closeScene
} from '@motet/lexicons';
import {
	app,
	interactionTimeout,
	projectionTimeout,
	signUp,
	createWorld,
	openPublicScene,
	sceneProjection
} from './journey-support.js';

test.setTimeout(420_000);

// Losing a successful response must not mint a new operation or refresh its
// compare-and-swap CID. That would conflict with the write already accepted.
async function loseFirstResponse(page: Page, nsid: string): Promise<() => number> {
	let firstBody: string | null = null;
	let attempts = 0;
	await page.route(`**/xrpc/${nsid}`, async (route) => {
		const body = route.request().postData();
		attempts += 1;

		if (attempts === 1) firstBody = body;
		else
			expect(body, 'retry must preserve the whole request, including op and swapRecord').toBe(
				firstBody
			);
		const response = await route.fetch();
		expect(response.status(), 'the real mutation must succeed before its response is lost').toBe(
			200
		);

		if (attempts === 1) await route.abort();
		else await route.fulfill({ response });
	});

	return () => attempts;
}

// A leaked role heuristic would expose controls to outsiders. A new op on retry
// would duplicate/conflict; optimistic rendering could hide an unapplied cast.
test('only authorized readers control scenes, and lost cast/close responses retry without duplicating changes or allowing closed-scene posting', async ({
	page,
	browser
}) => {
	const owner = await signUp(page);
	const world = await createWorld(page);
	const scene = await openPublicScene(page, world, owner);
	const path = new URL(page.url()).pathname;
	const outsiderContext = await browser.newContext({ baseURL: app });

	try {
		const outsider = await outsiderContext.newPage();
		await outsider.goto(path);
		await expect(outsider.getByRole('button', { name: 'Edit cast', exact: true })).toHaveCount(0);
		await expect(outsider.getByRole('button', { name: 'Close scene', exact: true })).toHaveCount(0);
		const account = await signUp(outsider);
		await outsider.goto(path);
		await expect(outsider.getByRole('button', { name: 'Edit cast', exact: true })).toHaveCount(0);
		await expect(outsider.getByRole('button', { name: 'Close scene', exact: true })).toHaveCount(0);

		const castAttempts = await loseFirstResponse(page, setCast.SetSceneCastNsid);
		await page.getByRole('button', { name: 'Edit cast', exact: true }).click();
		await page.getByLabel('Cast handles or DIDs').fill(`${owner.did}\n${account.handle}`);
		await page.getByRole('button', { name: 'Save cast', exact: true }).click();
		await expect(page.getByRole('alert')).toContainText('Retry', { timeout: interactionTimeout });
		await expect(page.getByLabel('Cast handles or DIDs')).toBeDisabled();
		await page.getByRole('button', { name: 'Retry scene change', exact: true }).click();
		await expect(page.getByRole('status').filter({ hasText: 'Cast updated.' })).toBeVisible({
			timeout: projectionTimeout
		});
		expect(castAttempts()).toBe(2);
		await expect(
			page
				.getByRole('region', { name: 'Cast', exact: true })
				.getByText(account.did, { exact: true })
		).toBeVisible();
		expect((await sceneProjection(page, world, scene)).scene.value).toMatchObject({
			cast: expect.arrayContaining([owner.did, account.did])
		});

		await page.unroute(`**/xrpc/${setCast.SetSceneCastNsid}`);
		await page.getByRole('button', { name: 'Edit cast', exact: true }).click();
		await page.getByLabel('Cast handles or DIDs').fill(owner.did);
		await page.getByRole('button', { name: 'Save cast', exact: true }).click();
		await expect(page.getByRole('status').filter({ hasText: 'Cast updated.' })).toBeVisible({
			timeout: projectionTimeout
		});
		await expect(
			page
				.getByRole('region', { name: 'Cast', exact: true })
				.getByText(account.did, { exact: true })
		).toHaveCount(0);
		expect((await sceneProjection(page, world, scene)).scene.value).toMatchObject({
			cast: [owner.did]
		});

		const closeAttempts = await loseFirstResponse(page, closeScene.CloseSceneNsid);
		await page.getByRole('button', { name: 'Close scene', exact: true }).click();
		await expect(page.getByRole('alert')).toContainText('Retry', { timeout: interactionTimeout });
		await page.getByRole('button', { name: 'Retry scene change', exact: true }).click();
		await expect(page.getByRole('status').filter({ hasText: 'Scene closed.' })).toBeVisible({
			timeout: projectionTimeout
		});
		expect(closeAttempts()).toBe(2);
		expect((await sceneProjection(page, world, scene)).scene.value).toMatchObject({
			state: 'closed'
		});
		await page.reload();
		await expect(
			page.getByText('This scene is closed. New posts are not accepted.', { exact: true })
		).toBeVisible();
		await expect(page.getByRole('textbox', { name: 'Write a post' })).toHaveCount(0);
		await expect(page.getByRole('button', { name: 'Post', exact: true })).toHaveCount(0);
		await outsider.reload();
		await expect(
			outsider.getByText('This scene is closed. New posts are not accepted.', { exact: true })
		).toBeVisible();
	} finally {
		await outsiderContext.close();
	}
});
