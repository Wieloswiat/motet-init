import { expect, test, type BrowserContext, type Page } from '@playwright/test';
import {
	at_motet_pilot_createpost as createPost,
	at_motet_pilot_getpoststatus as getPostStatus,
	at_motet_pilot_openscene as openScene
} from '@motet/lexicons';
import {
	app,
	projectionTimeout,
	interactionTimeout,
	uniqueLabel,
	signUp,
	signOut,
	signIn,
	createWorld,
	sceneProjection,
	openPublicScene,
	post,
	admitFirstPost,
	type Account,
	type World
} from './journey-support.js';

const tenor = 'https://tenor.motet.test';
const plc = 'https://plc.motet.test';

type ObservedRequest = {
	method: string;
	url: string;
	body: Buffer;
};

function requestsFrom(context: BrowserContext): ObservedRequest[] {
	const observed: ObservedRequest[] = [];

	context.on('request', (request) => {
		observed.push({
			method: request.method(),
			url: request.url(),
			body: request.postDataBuffer() ?? Buffer.alloc(0)
		});
	});

	return observed;
}

async function signOutAndIn(page: Page, account: Account): Promise<void> {
	await signOut(page);
	await page.goto('/sign-in');
	await signIn(page, account);
}

function assertNoSecrets(observed: ObservedRequest[], secrets: Buffer[]): void {
	expect(observed.length, 'the browser request recorder must be active').toBeGreaterThan(0);

	for (const request of observed) {
		const url = new URL(request.url);
		const where = `${request.method} ${url.origin}${url.pathname}`;

		for (const secret of secrets) {
			expect(
				request.body.includes(secret),
				`a browser request body leaked an owner secret: ${where}`
			).toBe(false);
			expect(
				Buffer.from(request.url).includes(secret),
				`a browser URL leaked an owner secret: ${where}`
			).toBe(false);
		}
	}
}

async function describeTenor(page: Page): Promise<string> {
	const response = await page.request.get(`${tenor}/xrpc/com.atproto.server.describeServer`);
	expect(response.status(), 'the configured tenor must describe its service').toBe(200);
	const description: { did: string; availableUserDomains: string[] } = await response.json();
	expect(description.did).toBe('did:web:tenor.motet.test');
	expect(description.availableUserDomains).toContain('.motet.test');

	return description.did;
}

async function assertTenorHost(page: Page, world: World, tenorDid: string): Promise<void> {
	expect(world.tenor).toBe(tenorDid);
	const documentResponse = await page.request.get(`${plc}/${world.did}`);
	expect(documentResponse.status(), 'the created World DID must resolve in PLC').toBe(200);

	const document: { id: string; service: { id: string; type: string; serviceEndpoint: string }[] } =
		await documentResponse.json();

	expect(document.id).toBe(world.did);

	const service = document.service.find(
		(entry) => entry.id === `${world.did}#motet_tenor` || entry.id === '#motet_tenor'
	);

	expect(service, 'the World DID must name the actual tenor as its host').toMatchObject({
		type: 'MotetTenor',
		serviceEndpoint: tenor
	});

	const repo = await page.request.get(
		`${tenor}/xrpc/com.atproto.repo.describeRepo?${new URLSearchParams({ repo: world.did })}`
	);

	expect(repo.status(), 'the named tenor must actually host the World repository').toBe(200);

	const descriptionOfRepo: { did: string; handle: string; handleIsCorrect: boolean } =
		await repo.json();

	expect(descriptionOfRepo).toMatchObject({
		did: world.did,
		handle: world.handle,
		handleIsCorrect: true
	});
}

test.setTimeout(420_000);

// A broken PAR continuation or cookie binding could make signup appear to
// succeed while the new account is not the authenticated identity.
test('PDS prompt=create signs in the new account and sign-out requires a real sign-in', async ({
	page
}) => {
	const account = await signUp(page);
	await signOutAndIn(page, account);
});

// A plaintext export or a request containing the actual scalar would leak the
// key even if its field were renamed to something other than privateKey.
test('World backup requires the right passphrase and sends no actual owner secret', async ({
	page,
	context
}) => {
	const observed = requestsFrom(context);
	await signUp(page);
	const world = await createWorld(page);
	assertNoSecrets(observed, world.secrets);
});

// Generating a new operation or owner key after a lost reservation response
// can strand the already reserved handle and prevent the World from being created.
test('retrying a lost World reservation completes the same handle', async ({ page }) => {
	await signUp(page);
	await createWorld(page, 'retry');
});

// A reload discards the unsaved owner key; a new attempt must replace the
// unsigned reservation or the owner cannot finish creating the same World.
test('a fresh attempt after reload can create the previously reserved handle', async ({ page }) => {
	await signUp(page);
	await createWorld(page, 'reload');
});

// Dropping the opener from the published cast would prevent the person who
// opened the scene from posting, even though the form initially listed them.
test('a newly opened public scene retains its opener in the projected cast', async ({ page }) => {
	const owner = await signUp(page);
	const world = await createWorld(page);
	await openPublicScene(page, world, owner);
});

// If a response is lost after the tenor commits, retrying must not create a
// second scene or change the scene that the opener reaches.
test('retrying a lost open-scene response reaches the original scene', async ({ page }) => {
	await signUp(page);
	await createWorld(page);
	await page.getByRole('link', { name: 'Open scene', exact: true }).click();
	const title = `Scene ${uniqueLabel('s')}`;
	await page.getByLabel('Scene title').fill(title);
	let firstScene = '';
	let firstOperation = '';

	await page.route(`**/xrpc/${openScene.OpenSceneNsid}`, async (route) => {
		const input: openScene.OpenSceneInput = route.request().postDataJSON();
		const response = await route.fetch();
		expect(response.status()).toBe(200);
		const result: openScene.OpenSceneOutput = await response.json();

		if (!firstScene) {
			firstScene = result.scene;
			firstOperation = input.op;
			await route.abort();
		} else {
			expect(input.op).toBe(firstOperation);
			expect(result.scene).toBe(firstScene);
			await route.fulfill({ response });
		}
	});
	await page.getByRole('button', { name: 'Open scene', exact: true }).click();
	await expect(page.getByRole('alert')).toContainText('Retry with the same details');
	await page.getByRole('button', { name: 'Open scene', exact: true }).click();
	await page.waitForURL((url) => url.pathname.endsWith(`/play/${encodeURIComponent(firstScene)}`), {
		timeout: interactionTimeout
	});
	await expect(page.getByRole('heading', { name: title })).toBeVisible({
		timeout: projectionTimeout
	});
});

// A new operation on retry would publish and admit the same draft twice;
// reusing it after success would instead prevent the next draft from posting.
test('a lost post response admits one post and the next draft gets a new operation', async ({
	page
}) => {
	const owner = await signUp(page);
	const world = await createWorld(page);
	const scene = await openPublicScene(page, world, owner);
	const text = `Retry post ${uniqueLabel('p')}`;
	const nextText = `Next post ${uniqueLabel('p')}`;
	let attempts = 0;
	let firstOperation = '';
	let firstUri = '';
	let firstCid = '';

	await page.route(`**/xrpc/${createPost.CreatePostNsid}`, async (route) => {
		const input: createPost.CreatePostInput = route.request().postDataJSON();
		const response = await route.fetch();
		expect(response.status()).toBe(200);
		const result: createPost.CreatePostOutput = await response.json();
		attempts += 1;

		if (attempts === 1) {
			firstOperation = input.op;
			firstUri = result.uri;
			firstCid = result.cid;
			await route.abort();
		} else {
			if (attempts === 2) {
				expect(input.op).toBe(firstOperation);
				expect(result.uri).toBe(firstUri);
				expect(result.cid).toBe(firstCid);
			} else {
				expect(input.op).not.toBe(firstOperation);
			}

			await route.fulfill({ response });
		}
	});

	await page.getByRole('textbox', { name: 'Write a post' }).fill(text);
	await page.getByRole('button', { name: 'Post', exact: true }).click();
	await expect(page.getByRole('status')).toContainText('Could not post:');
	await expect(page.getByRole('textbox', { name: 'Write a post' })).toHaveValue(text);
	await page.getByRole('button', { name: 'Post', exact: true }).click();
	await expect(page.getByRole('status')).toContainText('Admitted. Your post is now in the scene.', {
		timeout: projectionTimeout
	});
	const projection = await sceneProjection(page, world, scene);
	expect(projection.posts).toHaveLength(1);
	expect(projection.posts[0]).toMatchObject({ uri: firstUri, cid: firstCid, author: owner.did });
	const posts = page.getByRole('region', { name: 'Admitted posts' });
	await expect(posts.getByRole('article')).toHaveCount(1);
	await expect(posts.getByRole('article').first()).toContainText(text);

	await expect(page.getByRole('button', { name: 'Post', exact: true })).toBeEnabled();
	const next = await post(page, nextText);
	expect(next.uri).not.toBe(firstUri);
	await expect(page.getByRole('status')).toContainText('Admitted. Your post is now in the scene.', {
		timeout: projectionTimeout
	});
	await expect(posts.getByRole('article')).toHaveCount(2);
	expect(attempts).toBe(3);
});

// Reusing a failed draft's operation for changed content would make the
// AppView reject the new draft, and an unknown-status message hides the cause.
test('failed admission reports its reason and edited drafts get new operations', async ({
	page
}) => {
	const owner = await signUp(page);
	const world = await createWorld(page);
	await openPublicScene(page, world, owner);
	const text = `Draft ${uniqueLabel('p')}`;
	const edited = `Edited draft ${uniqueLabel('p')}`;
	const operations: string[] = [];

	await page.route(`**/xrpc/${createPost.CreatePostNsid}`, async (route) => {
		const input: createPost.CreatePostInput = route.request().postDataJSON();
		operations.push(input.op);
		const response = await route.fetch();
		expect(response.status()).toBe(200);
		const result: createPost.CreatePostOutput = await response.json();

		if (operations.length < 3) {
			await route.fulfill({
				response,
				json: { ...result, status: 'failed', reason: 'Admission unavailable' }
			});
		} else {
			await route.fulfill({ response });
		}
	});

	await page.getByRole('textbox', { name: 'Write a post' }).fill(text);
	await page.getByRole('button', { name: 'Post', exact: true }).click();
	await expect(page.getByRole('status')).toHaveText('Admission failed: Admission unavailable');
	await page.getByLabel('Posting as').selectOption('out-of-character');
	await page.getByRole('button', { name: 'Post', exact: true }).click();
	await expect(page.getByRole('status')).toHaveText('Admission failed: Admission unavailable');
	await page.getByRole('textbox', { name: 'Write a post' }).fill(edited);
	await page.getByRole('button', { name: 'Post', exact: true }).click();
	await expect(page.getByRole('status')).toContainText('Admitted. Your post is now in the scene.', {
		timeout: projectionTimeout
	});
	expect(new Set(operations).size).toBe(3);
	await expect(page.getByRole('region', { name: 'Admitted posts' })).toContainText(edited);
});

// A missing cast or membership check could admit an outsider, or the AppView
// could display a refused version alongside the owner's first post.
test('the first admitted post is place 1 and a nonmember is refused without appearing', async ({
	page,
	browser
}) => {
	const owner = await signUp(page);
	const world = await createWorld(page);
	const scene = await openPublicScene(page, world, owner);
	const admitted = await admitFirstPost(page, world, scene, owner);
	const outsiderContext = await browser.newContext({ baseURL: app });

	try {
		const outsider = await outsiderContext.newPage();
		const stranger = await signUp(outsider);
		expect(stranger.did).not.toBe(owner.did);
		await outsider.goto(`/w/${encodeURIComponent(world.did)}/play/${encodeURIComponent(scene)}`);
		const immediateText = `Immediately refused post ${uniqueLabel('p')}`;
		const immediate = await post(outsider, immediateText);
		expect(immediate.status).toBe('refused');
		expect(immediate.reason).toBe('Forbidden');
		await expect(outsider.getByText('Refused: Forbidden', { exact: true })).toBeVisible({
			timeout: interactionTimeout
		});

		// The outbox may reach a refusal after the browser receives pending.
		await outsider.route(`**/xrpc/${createPost.CreatePostNsid}`, async (route) => {
			const response = await route.fetch();
			const outcome: createPost.CreatePostOutput = await response.json();
			expect(outcome.status).toBe('refused');
			expect(outcome.reason).toBe('Forbidden');
			await route.fulfill({ response, json: { ...outcome, status: 'pending' } });
		});

		const outsiderText = `Refused outsider post ${uniqueLabel('p')}`;

		const statusResponse = outsider.waitForResponse(
			(response) =>
				response.request().method() === 'GET' &&
				new URL(response.url()).pathname === `/xrpc/${getPostStatus.GetPostStatusNsid}`,
			{ timeout: interactionTimeout }
		);

		const refused = await post(outsider, outsiderText);
		expect(refused.status).toBe('pending');

		const status = await statusResponse;
		expect(status.status()).toBe(200);
		expect(await status.json()).toMatchObject({
			uri: refused.uri,
			cid: refused.cid,
			status: 'refused',
			reason: 'Forbidden'
		});
		await expect(outsider.getByText('Refused: Forbidden', { exact: true })).toBeVisible({
			timeout: interactionTimeout
		});
		const outsiderPosts = outsider.getByRole('region', { name: 'Admitted posts' });
		await expect(outsiderPosts.getByText(immediateText, { exact: true })).toHaveCount(0);
		await expect(outsiderPosts.getByText(outsiderText, { exact: true })).toHaveCount(0);
		await outsider.reload();
		await expect(outsiderPosts.getByText(immediateText, { exact: true })).toHaveCount(0);
		await expect(outsiderPosts.getByText(outsiderText, { exact: true })).toHaveCount(0);
		const afterRefusal = await sceneProjection(page, world, scene);
		expect(afterRefusal.posts).toHaveLength(1);
		expect(afterRefusal.posts[0]).toMatchObject({
			uri: admitted.uri,
			cid: admitted.cid,
			author: owner.did
		});
	} finally {
		await outsiderContext.close();
	}
});

// Wrong service discovery or a lost OAuth, creation, projection or admission
// link could make individual screens work while the complete journey fails.
test('complete HTTPS journey signs up, hosts a World on the tenor, opens a scene and admits a post', async ({
	page,
	context
}) => {
	const observed = requestsFrom(context);
	const tenorDid = await describeTenor(page);
	const owner = await signUp(page);
	await signOutAndIn(page, owner);
	const world = await createWorld(page);
	await assertTenorHost(page, world, tenorDid);
	const scene = await openPublicScene(page, world, owner);
	await admitFirstPost(page, world, scene, owner);
	assertNoSecrets(observed, world.secrets);
});
