import { expect, test, type Page } from '@playwright/test';
import {
	at_motet_pilot_auth_getsession as getSession,
	at_motet_pilot_post as postRecord
} from '@motet/lexicons';
import {
	app,
	pds,
	collectBrowserErrors,
	expectedSetupResourceError,
	type BrowserErrors,
	projectionTimeout,
	interactionTimeout,
	uniqueLabel,
	signUp,
	signOut,
	signIn,
	createWorld,
	openPublicScene,
	admitFirstPost,
	post,
	sceneProjection,
	type Account,
	type World
} from './journey-support.js';

test.setTimeout(420_000);

// A broad URL-only allowance would hide outages, other Worlds, or reads after indexing succeeded.
test('setup diagnostics allow only its pending World 404 and the external PDS favicon, never outages', () => {
	const actor = 'created-world.motet.test';
	const source = `${app}/xrpc/at.motet.pilot.getWorld?actor=${actor}`;
	expect(expectedSetupResourceError(source, 404, actor)).toBe(true);
	expect(expectedSetupResourceError(source, 500, actor)).toBe(false);
	expect(expectedSetupResourceError(source, 502, actor)).toBe(false);
	expect(expectedSetupResourceError(source, 404, 'another-world.motet.test')).toBe(false);
	expect(expectedSetupResourceError(source, 404, null)).toBe(false);
	expect(expectedSetupResourceError(`${app}/favicon.ico`, 404, actor)).toBe(false);
	expect(expectedSetupResourceError(`${pds}/favicon.ico`, 404, null)).toBe(true);
	expect(expectedSetupResourceError(`${pds}/favicon.ico`, 500, null)).toBe(false);
});

// Reusing the allowance after the first success would hide a regression in a previously visible World.
test('setup console allowance expires at the first indexed response and still reports a same-World outage', async ({
	page
}) => {
	const actor = 'diagnostic-world.motet.test';
	const source = `${app}/xrpc/at.motet.pilot.getWorld?actor=${actor}`;
	const observed = collectBrowserErrors(page);
	let status = 404;

	await page.route(source, (route) =>
		route.fulfill({ status, contentType: 'application/json', body: '{}' })
	);
	await page.goto('/');
	observed.worldProvisioning(actor);

	async function readWorld(): Promise<void> {
		await page.evaluate(async (url) => {
			await fetch(url);
		}, source);
		// A renderer round-trip drains browser console events produced by the completed fetch.
		await page.evaluate(() => undefined);
	}

	await readWorld();
	expect(observed.errors).toEqual([]);
	status = 500;
	await readWorld();
	expect(observed.errors).toHaveLength(1);
	expect(observed.errors[0]).toContain(source);
	status = 200;
	await readWorld();
	status = 404;
	await readWorld();
	expect(observed.errors).toHaveLength(2);
	expect(observed.errors[1]).toContain(source);
});

type ExpectedPost = {
	uri: string;
	cid: string;
	body: string;
	seq: number;
};

type PublicScene = {
	owner: Account;
	world: World;
	scene: string;
	title: string;
	path: string;
	sceneUri: string;
	first: ExpectedPost;
};

async function revokeSession(page: Page): Promise<void> {
	const cookies = await page.context().cookies(app);
	expect(cookies.length, 'the owner must have a session before signing out').toBeGreaterThan(0);
	const cookie = cookies.map((entry) => `${entry.name}=${entry.value}`).join('; ');
	await signOut(page);

	const replay = await page.request.get(`${app}/xrpc/${getSession.GetSessionNsid}`, {
		headers: { cookie }
	});

	expect(
		replay.status(),
		'the previous session must be revoked, not merely removed from the browser'
	).toBe(401);
}

async function publicSceneFixture(page: Page, browserErrors: BrowserErrors): Promise<PublicScene> {
	const owner = await signUp(page);
	const world = await createWorld(page, undefined, browserErrors);
	const scene = await openPublicScene(page, world, owner);
	const admitted = await admitFirstPost(page, world, scene, owner);
	const title = await page.getByRole('heading', { level: 1 }).innerText();

	const body = await page
		.getByRole('region', { name: 'Admitted posts' })
		.getByRole('article')
		.locator('pre')
		.innerText();

	const path = new URL(page.url()).pathname;
	const projection = await sceneProjection(page, world, scene);
	await revokeSession(page);

	return {
		owner,
		world,
		scene,
		title,
		path,
		sceneUri: projection.scene.uri,
		first: { uri: admitted.uri, cid: admitted.cid, body, seq: 0 }
	};
}

async function expectScene(
	page: Page,
	fixture: PublicScene,
	expected: ExpectedPost[]
): Promise<void> {
	await expect(page).toHaveURL(`${app}${fixture.path}`);
	await expect(page).toHaveTitle(`${fixture.title} · Motet`);
	await expect(page.getByRole('heading', { name: fixture.title, exact: true })).toBeVisible();
	await expect(
		page.getByRole('region', { name: 'Cast' }).getByText(fixture.owner.did, { exact: true })
	).toBeVisible();
	const posts = page.getByRole('region', { name: 'Admitted posts' }).getByRole('article');
	await expect(posts).toHaveCount(expected.length);

	for (const [index, entry] of expected.entries()) {
		const article = posts.nth(index);
		await expect(article).toHaveAccessibleName(`Post by ${fixture.owner.did}`);
		await expect(article.getByText(fixture.owner.did, { exact: true })).toBeVisible();
		await expect(article.locator('pre')).toHaveText(entry.body);
		await expect(article.getByText(`Place ${entry.seq + 1}`, { exact: true })).toBeVisible();
		await expect(article.locator('img')).toHaveCount(0);
	}

	const projection = await sceneProjection(page, fixture.world, fixture.scene);
	expect(projection.world.did).toBe(fixture.world.did);
	expect(projection.scene.uri).toBe(fixture.sceneUri);
	expect(projection.posts, 'reloads and OAuth must not duplicate admitted posts').toHaveLength(
		expected.length
	);

	for (const [index, entry] of expected.entries()) {
		expect(entry.uri.startsWith(`at://${fixture.owner.did}/${postRecord.PostNsid}/`)).toBe(true);
		expect(projection.posts[index]).toMatchObject({
			uri: entry.uri,
			cid: entry.cid,
			author: fixture.owner.did,
			place: { seq: entry.seq },
			value: { body: entry.body }
		});
	}

	await expect(page.getByRole('alert')).toHaveCount(0);
}

async function expectAnonymousScene(page: Page): Promise<void> {
	const session = await page.request.get(`${app}/xrpc/${getSession.GetSessionNsid}`);
	expect(session.status(), 'the independent reader must not inherit the owner session').toBe(401);
	const compose = page.getByRole('region', { name: 'Write a post' });
	await expect(compose.getByText('Sign in to write a post.', { exact: true })).toBeVisible();
	await expect(compose.getByRole('link', { name: 'Sign in', exact: true })).toBeVisible();
	await expect(page.getByRole('textbox', { name: 'Write a post' })).toHaveCount(0);
	await expect(page.getByLabel('Posting as')).toHaveCount(0);
	await expect(page.getByRole('button', { name: 'Post', exact: true })).toHaveCount(0);
}

// A cookie-dependent public load, omitted post body, or stale hydration could
// hide an admitted post from an independent reader or expose posting controls.
test('anonymous readers navigate a public World and reload its exact admitted scene content without posting controls', async ({
	page,
	browser
}) => {
	const ownerErrors = collectBrowserErrors(page);
	const fixture = await publicSceneFixture(page, ownerErrors);
	const readerContext = await browser.newContext({ baseURL: app });

	try {
		const reader = await readerContext.newPage();
		const readerErrors = collectBrowserErrors(reader);
		const response = await reader.goto(`/w/${encodeURIComponent(fixture.world.did)}`);
		expect(response?.status(), 'the World must be public without the owner cookie').toBe(200);
		await expect(
			reader.getByRole('heading', { name: fixture.world.name, exact: true })
		).toBeVisible();
		await expect(reader.getByText(`@${fixture.world.handle}`, { exact: true })).toBeVisible();
		await expect(reader.getByText(fixture.world.did, { exact: true })).toBeVisible();
		await expect(reader.getByRole('link', { name: 'Open scene', exact: true })).toHaveCount(0);
		await reader.getByRole('link', { name: 'All scenes', exact: true }).click();
		await expect(reader.getByRole('heading', { name: 'Scenes', exact: true })).toBeVisible();
		await expect(reader.getByRole('link', { name: 'Open scene', exact: true })).toHaveCount(0);
		await reader.getByRole('link', { name: fixture.title, exact: true }).click();
		await expectScene(reader, fixture, [fixture.first]);
		await expectAnonymousScene(reader);
		await reader.reload();
		await expectScene(reader, fixture, [fixture.first]);
		await expectAnonymousScene(reader);
		expect(readerErrors.errors, 'public navigation and reload must have no browser errors').toEqual(
			[]
		);
		expect(ownerErrors.errors, 'real owner setup must have no browser errors').toEqual([]);
	} finally {
		await readerContext.close();
	}
});

// Losing the scene's returnTo during real PDS OAuth can strand the writer on
// the home page; a stale session can publish as a different DID after sign-in.
test('in-scene sign-in returns the anonymous reader to the exact scene and admits the owner next post', async ({
	page,
	browser
}) => {
	const ownerErrors = collectBrowserErrors(page);
	const fixture = await publicSceneFixture(page, ownerErrors);
	const readerContext = await browser.newContext({ baseURL: app });

	try {
		const reader = await readerContext.newPage();
		const readerErrors = collectBrowserErrors(reader);
		await reader.goto(fixture.path);
		await expectScene(reader, fixture, [fixture.first]);
		await expectAnonymousScene(reader);

		const signInLink = reader
			.getByRole('region', { name: 'Write a post' })
			.getByRole('link', { name: 'Sign in', exact: true });

		await expect(signInLink).toHaveAttribute(
			'href',
			`/sign-in?returnTo=${encodeURIComponent(fixture.path)}`
		);
		await signInLink.click();
		await expect(reader).toHaveURL(`${app}/sign-in?returnTo=${encodeURIComponent(fixture.path)}`);
		await signIn(reader, fixture.owner);
		await expect(reader).toHaveURL(`${app}${fixture.path}`, { timeout: interactionTimeout });
		await expectScene(reader, fixture, [fixture.first]);
		await expect(reader.getByRole('textbox', { name: 'Write a post' })).toBeEnabled();
		const body = `Owner after scene OAuth ${uniqueLabel('p')}`;
		const published = await post(reader, body);
		expect(['admitted', 'pending']).toContain(published.status);
		expect(published.uri).not.toBe(fixture.first.uri);
		await expect(reader.getByRole('status')).toContainText(
			'Admitted. Your post is now in the scene.',
			{ timeout: projectionTimeout }
		);
		const expected = [fixture.first, { uri: published.uri, cid: published.cid, body, seq: 1 }];
		await expectScene(reader, fixture, expected);
		await reader.reload();
		await expectScene(reader, fixture, expected);
		await revokeSession(reader);
		await reader.goto(fixture.path);
		await expectScene(reader, fixture, expected);
		await expectAnonymousScene(reader);
		expect(
			readerErrors.errors,
			'scene OAuth, posting and sign-out must have no browser errors'
		).toEqual([]);
		expect(ownerErrors.errors, 'real owner setup must have no browser errors').toEqual([]);
	} finally {
		await readerContext.close();
	}
});
