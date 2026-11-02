import {
	createDecipheriv,
	createECDH,
	createPrivateKey,
	pbkdf2Sync,
	randomBytes
} from 'node:crypto';
import { readFile } from 'node:fs/promises';
import { expect, test, type BrowserContext, type Page } from '@playwright/test';
import {
	at_motet_pilot_auth_getsession as getSession,
	at_motet_pilot_beginworld as beginWorld,
	at_motet_pilot_createpost as createPost,
	at_motet_pilot_finishworld as finishWorld,
	at_motet_pilot_getscene as getScene,
	at_motet_pilot_getworld as getWorld,
	at_motet_pilot_openscene as openScene
} from '@motet/lexicons';

const app = 'https://motet.test';
const pds = 'https://pds.motet.test';
const tenor = 'https://tenor.motet.test';
const plc = 'https://plc.motet.test';
const projectionTimeout = 120_000;
const interactionTimeout = 60_000;

// Each first label is 17 characters: the pinned PDS accepts at most 18.
function uniqueLabel(prefix: string): string {
	return `${prefix}${randomBytes(8).toString('hex')}`;
}

type Account = {
	handle: string;
	password: string;
	did: string;
};

type ObservedRequest = {
	method: string;
	url: string;
	body: Buffer;
};

type World = {
	did: string;
	handle: string;
	name: string;
	tenor: string;
	secrets: Buffer[];
};

type OwnerBackup = {
	format: string;
	version: number;
	worldDid: string;
	didKey: string;
	kdf: { name: string; iterations: number; salt: string };
	cipher: { name: string; iv: string; ciphertext: string };
};

type PrivateJwk = {
	kty: string;
	crv: string;
	x: string;
	y: string;
	d: string;
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

async function currentSession(page: Page): Promise<getSession.GetSessionOutput> {
	const response = await page.request.get(`${app}/xrpc/${getSession.GetSessionNsid}`);
	expect(response.status(), 'the AppView must expose the authenticated session').toBe(200);

	return response.json();
}

async function signUp(page: Page): Promise<Account> {
	const label = uniqueLabel('u');
	const handle = `${label}.pds.motet.test`;
	const password = `Motet-password-${randomBytes(12).toString('hex')}`;
	await page.goto('/sign-in');

	const start = page.waitForRequest(
		(request) => request.method() === 'POST' && request.url() === `${app}/oauth/sign-up`,
		{ timeout: interactionTimeout }
	);

	// A normal PDS authorization request must lead to its signup page:
	// the PDS only selects that page for the PAR's prompt=create.
	const authorization = page.waitForRequest(
		(request) => request.method() === 'GET' && request.url().startsWith(`${pds}/oauth/authorize?`),
		{ timeout: interactionTimeout }
	);

	const [started, request] = await Promise.all([
		start,
		authorization,
		page.getByRole('button', { name: 'Create an account' }).click()
	]);

	expect(await started.headerValue('origin')).toBe(app);
	const authorizationUrl = new URL(request.url());
	expect(authorizationUrl.searchParams.get('client_id')).toBe(`${app}/oauth/client-metadata.json`);
	expect(authorizationUrl.searchParams.get('request_uri')).toBeTruthy();
	await expect(page).toHaveURL(/^https:\/\/pds\.motet\.test\/account\/signup\?/, {
		timeout: interactionTimeout
	});
	await expect(page.getByRole('heading', { name: 'Create an account' })).toBeVisible();
	await page.getByLabel('Handle', { exact: true }).fill(label);
	await page.getByLabel('Email address').fill(`${label}@example.invalid`);
	await page.getByLabel('Password', { exact: true }).fill(password);
	// A one-time PDS portal login forces the later sign-in to use the password,
	// rather than passing because the PDS silently retained its portal session.
	await page.getByRole('checkbox', { name: "Don't stay signed in on this browser" }).check();
	await page.getByRole('button', { name: 'Create account', exact: true }).click();
	await page.getByRole('button', { name: /^Allow as / }).click({ timeout: interactionTimeout });
	await expect(page.getByText(`Signed in as ${handle}`, { exact: true })).toBeVisible({
		timeout: interactionTimeout
	});

	const session = await currentSession(page);
	expect(session.handle).toBe(handle);

	const resolution = await page.request.get(
		`${pds}/xrpc/com.atproto.identity.resolveHandle?${new URLSearchParams({ handle })}`
	);

	expect(resolution.status(), 'the PDS must resolve the new handle').toBe(200);
	const resolved: { did: string } = await resolution.json();
	expect(session.did, 'the OAuth session must be the PDS account, not another identity').toBe(
		resolved.did
	);
	expect(session.did).toMatch(/^did:plc:/);

	return { handle, password, did: session.did };
}

async function signOutAndIn(page: Page, account: Account): Promise<void> {
	await page.getByRole('button', { name: 'Sign out' }).click();
	await expect(page.getByRole('link', { name: 'Sign in', exact: true }).first()).toBeVisible();
	const anonymous = await page.request.get(`${app}/xrpc/${getSession.GetSessionNsid}`);
	expect(anonymous.status(), 'sign-out must revoke the AppView session').toBe(401);
	await page.goto('/sign-in');
	await page.getByLabel('Your handle').fill(account.handle);

	const start = page.waitForRequest(
		(request) => request.method() === 'POST' && request.url() === `${app}/oauth/sign-in`,
		{ timeout: interactionTimeout }
	);

	const [started] = await Promise.all([
		start,
		page.getByRole('button', { name: 'Continue', exact: true }).click()
	]);

	expect(await started.headerValue('origin')).toBe(app);
	await expect(page).toHaveURL(/^https:\/\/pds\.motet\.test\/oauth\/authorize\?/, {
		timeout: interactionTimeout
	});
	await page.getByLabel('Password', { exact: true }).fill(account.password);
	await page.getByRole('button', { name: 'Continue', exact: true }).click();
	await page.getByRole('button', { name: /^Allow as / }).click({ timeout: interactionTimeout });
	await expect(page.getByText(`Signed in as ${account.handle}`, { exact: true })).toBeVisible({
		timeout: interactionTimeout
	});
	const session = await currentSession(page);
	expect(session.did, 'sign-in must restore the same account').toBe(account.did);
	expect(session.handle).toBe(account.handle);
}

function base58(bytes: Buffer): string {
	const alphabet = '123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz';
	let value = BigInt(`0x${bytes.toString('hex')}`);
	let result = '';

	while (value > 0n) {
		result = alphabet[Number(value % 58n)] + result;
		value /= 58n;
	}

	for (const byte of bytes) {
		if (byte !== 0) break;
		result = `1${result}`;
	}

	return result;
}

// Decrypt with Node's crypto, independently of the browser's backup importer.
// Recomputing the public did:key binds the recovered scalar to the key the
// browser actually sent in its reservation, rather than merely checking names.
function backupSecrets(
	file: string,
	passphrase: string,
	wrongPassphrase: string,
	expectedDid: string,
	rotationKey: string
): Buffer[] {
	const backup: OwnerBackup = JSON.parse(file);
	expect(backup.format).toBe('at.motet.owner-key.backup');
	expect(backup.version).toBe(1);
	expect(backup.worldDid).toBe(expectedDid);
	expect(backup.kdf.name).toBe('PBKDF2-SHA-256');
	expect(backup.kdf.iterations).toBeGreaterThanOrEqual(600_000);
	expect(backup.cipher.name).toBe('AES-256-GCM');
	const encrypted = Buffer.from(backup.cipher.ciphertext, 'base64url');

	const key = pbkdf2Sync(
		passphrase,
		Buffer.from(backup.kdf.salt, 'base64url'),
		backup.kdf.iterations,
		32,
		'sha256'
	);

	const decipher = createDecipheriv('aes-256-gcm', key, Buffer.from(backup.cipher.iv, 'base64url'));
	decipher.setAAD(Buffer.from(JSON.stringify([backup.format, 1, backup.worldDid, backup.didKey])));
	decipher.setAuthTag(encrypted.subarray(-16));
	const plaintext = Buffer.concat([decipher.update(encrypted.subarray(0, -16)), decipher.final()]);
	const jwk: PrivateJwk = JSON.parse(plaintext.toString('utf8'));
	expect(jwk.kty).toBe('EC');
	expect(jwk.crv).toBe('P-256');
	const scalar = Buffer.from(jwk.d, 'base64url');
	expect(scalar.length, 'the downloaded backup must contain an actual P-256 private scalar').toBe(
		32
	);
	const curve = createECDH('prime256v1');
	curve.setPrivateKey(scalar);

	const publicKey = Buffer.concat([
		Buffer.from([0x80, 0x24]),
		curve.getPublicKey(undefined, 'compressed')
	]);

	expect(`did:key:z${base58(publicKey)}`).toBe(rotationKey);
	expect(backup.didKey).toBe(rotationKey);

	const privateKey = createPrivateKey({ key: jwk, format: 'jwk' });
	const der = privateKey.export({ type: 'pkcs8', format: 'der' });
	const pem = privateKey.export({ type: 'pkcs8', format: 'pem' });
	const patterns: Buffer[] = [scalar, plaintext, der, Buffer.from(pem)];

	for (const bytes of [scalar, der, Buffer.from(passphrase), Buffer.from(wrongPassphrase)]) {
		patterns.push(
			bytes,
			Buffer.from(bytes.toString('hex')),
			Buffer.from(bytes.toString('base64')),
			Buffer.from(bytes.toString('base64url'))
		);
	}

	for (const value of [passphrase, wrongPassphrase]) {
		patterns.push(
			Buffer.from(encodeURIComponent(value)),
			Buffer.from(JSON.stringify(value).slice(1, -1))
		);
	}

	patterns.push(Buffer.from(JSON.stringify([...scalar])));

	for (const secret of patterns) {
		expect(
			Buffer.from(file).includes(secret),
			'the downloaded backup must not contain plaintext secrets'
		).toBe(false);
	}

	return patterns;
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

async function waitForWorld(page: Page, world: World): Promise<void> {
	const url = `${app}/xrpc/${getWorld.GetWorldNsid}?${new URLSearchParams({ actor: world.handle })}`;
	await expect
		.poll(
			async () => {
				const response = await page.request.get(url);

				if (response.status() === 404) return 'not projected';
				expect(response.status(), 'getWorld must return 200 or 404 while indexing').toBe(200);
				const projected: getWorld.GetWorldOutput = await response.json();

				return `${projected.did}|${projected.handle}|${projected.displayName}`;
			},
			{ message: `World ${world.handle} must reach the AppView`, timeout: projectionTimeout }
		)
		.toBe(`${world.did}|${world.handle}|${world.name}`);
}

async function createWorld(page: Page): Promise<World> {
	const label = uniqueLabel('w');
	const handle = `${label}.motet.test`;
	const name = `World ${label}`;
	const passphrase = `Recovery #${randomBytes(12).toString('hex')} / owner`;
	const wrongPassphrase = `Wrong #${randomBytes(12).toString('hex')} / owner`;
	await page.goto('/new-world');
	await page.getByLabel('World name').fill(name);
	await page.getByLabel('World handle').fill(handle);
	await page.getByLabel('Backup passphrase').fill(passphrase);

	const reservationResponse = page.waitForResponse(
		(response) =>
			response.request().method() === 'POST' &&
			response.url() === `${app}/xrpc/${beginWorld.BeginWorldNsid}`,
		{ timeout: interactionTimeout }
	);

	const creationResponse = page.waitForResponse(
		(response) =>
			response.request().method() === 'POST' &&
			response.url() === `${app}/xrpc/${finishWorld.FinishWorldNsid}`,
		{ timeout: interactionTimeout }
	);

	const [reservation, creation] = await Promise.all([
		reservationResponse.then((response) => {
			expect(response.status(), 'the real tenor reservation must succeed').toBe(200);

			return response;
		}),
		creationResponse,
		page.getByRole('button', { name: 'Create World' }).click()
	]);

	const reservationInput: beginWorld.BeginWorldInput = reservation.request().postDataJSON();
	const reserved: beginWorld.BeginWorldOutput = await reservation.json();
	expect(reservationInput.handle).toBe(handle);
	expect(reservationInput.name).toBe(name);
	expect(reservationInput.visibility).toBe('public');
	expect(creation.status(), 'the signed World creation must succeed').toBe(200);
	const created: finishWorld.FinishWorldOutput = await creation.json();
	expect(created.did).toMatch(/^did:plc:/);
	expect(created.handle).toBe(handle);
	await expect(page.getByRole('heading', { name: 'Back up the owner key' })).toBeVisible({
		timeout: interactionTimeout
	});
	const downloadEvent = page.waitForEvent('download', { timeout: interactionTimeout });
	await page.getByRole('button', { name: 'Download encrypted backup' }).click();
	const download = await downloadEvent;
	expect(download.suggestedFilename()).toMatch(/owner-key\.json$/);
	const path = await download.path();

	if (path === null) throw new Error('the encrypted owner backup was not saved');
	const file = await readFile(path, 'utf8');

	const secrets = backupSecrets(
		file,
		passphrase,
		wrongPassphrase,
		created.did,
		reservationInput.rotationKey
	);

	await page.getByLabel('Backup file').setInputFiles(path);
	await page.getByLabel('Backup passphrase').fill(wrongPassphrase);
	await page.getByRole('button', { name: 'Verify backup' }).click();
	await expect(page.getByRole('alert')).toHaveText(
		'Could not verify backup. Check the file and passphrase.',
		{ timeout: interactionTimeout }
	);
	await expect(page.getByRole('heading', { name: 'World ready' })).toHaveCount(0);
	await page.getByLabel('Backup passphrase').fill(passphrase);
	await page.getByRole('button', { name: 'Verify backup' }).click();
	await expect(page.getByRole('heading', { name: 'World ready' })).toBeVisible({
		timeout: projectionTimeout
	});
	const world = { did: created.did, handle, name, tenor: reserved.tenor, secrets };
	await waitForWorld(page, world);
	await page.getByRole('link', { name: 'Go to World' }).click();
	await expect(page.getByRole('heading', { name })).toBeVisible({ timeout: interactionTimeout });

	return world;
}

async function sceneProjection(
	page: Page,
	world: World,
	scene: string
): Promise<getScene.GetSceneOutput> {
	const params = new URLSearchParams({ actor: world.did, scene });
	const response = await page.request.get(`${app}/xrpc/${getScene.GetSceneNsid}?${params}`);
	expect(response.status(), 'the projected scene must be readable').toBe(200);

	return response.json();
}

async function openPublicScene(page: Page, world: World, owner: Account): Promise<string> {
	await page.getByRole('link', { name: 'Open scene', exact: true }).click();
	await expect(
		page.getByRole('region', { name: 'Cast' }).getByText(owner.did, { exact: true })
	).toBeVisible();
	const title = `Scene ${uniqueLabel('s')}`;
	await page.getByLabel('Scene title').fill(title);

	const responseEvent = page.waitForResponse(
		(response) =>
			response.request().method() === 'POST' &&
			response.url() === `${app}/xrpc/${openScene.OpenSceneNsid}`,
		{ timeout: interactionTimeout }
	);

	await page.getByRole('button', { name: 'Open scene', exact: true }).click();
	const response = await responseEvent;
	expect(response.status(), 'opening the scene as the owner must succeed').toBe(200);
	const opened: openScene.OpenSceneOutput = await response.json();
	await page.waitForURL(
		(url) => url.pathname.endsWith(`/play/${encodeURIComponent(opened.scene)}`),
		{
			timeout: interactionTimeout
		}
	);
	const params = new URLSearchParams({ actor: world.did, scene: opened.scene });
	await expect
		.poll(
			async () => {
				const projected = await page.request.get(`${app}/xrpc/${getScene.GetSceneNsid}?${params}`);

				if (projected.status() === 404) return '';
				expect(projected.status(), 'getScene must return 200 or 404 while indexing').toBe(200);
				const body: getScene.GetSceneOutput = await projected.json();

				return body.scene.uri;
			},
			{ message: `Scene ${opened.scene} must reach the AppView`, timeout: projectionTimeout }
		)
		.toBe(opened.uri);
	await page.reload();
	await expect(page.getByRole('heading', { name: title })).toBeVisible({
		timeout: interactionTimeout
	});
	await expect(
		page.getByRole('region', { name: 'Cast' }).getByText(owner.did, { exact: true })
	).toBeVisible();

	return opened.scene;
}

async function post(page: Page, text: string): Promise<createPost.CreatePostOutput> {
	await page.getByLabel('Write a post').fill(text);

	const responseEvent = page.waitForResponse(
		(response) =>
			response.request().method() === 'POST' &&
			response.url() === `${app}/xrpc/${createPost.CreatePostNsid}`,
		{ timeout: interactionTimeout }
	);

	await page.getByRole('button', { name: 'Post', exact: true }).click();
	const response = await responseEvent;
	expect(response.status(), 'publishing the post must return an admission outcome').toBe(200);

	return response.json();
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

// Dropping the opener from the published cast would prevent the person who
// opened the scene from posting, even though the form initially listed them.
test('a newly opened public scene retains its opener in the projected cast', async ({ page }) => {
	const owner = await signUp(page);
	const world = await createWorld(page);
	await openPublicScene(page, world, owner);
});

async function admitFirstPost(
	page: Page,
	world: World,
	scene: string,
	owner: Account
): Promise<createPost.CreatePostOutput> {
	const ownerText = `Owner post ${uniqueLabel('p')} <img src=x onerror=alert(1)>`;
	const admitted = await post(page, ownerText);
	expect(['admitted', 'pending']).toContain(admitted.status);
	await expect
		.poll(
			async () => {
				const projection = await sceneProjection(page, world, scene);

				return projection.posts.some(
					(entry) => entry.uri === admitted.uri && entry.cid === admitted.cid
				);
			},
			{
				message: 'the exact owner post version must be admitted and projected',
				timeout: projectionTimeout
			}
		)
		.toBe(true);
	const projection = await sceneProjection(page, world, scene);
	expect(projection.posts).toHaveLength(1);
	expect(projection.posts[0]).toMatchObject({
		uri: admitted.uri,
		cid: admitted.cid,
		author: owner.did
	});
	await page.reload();
	const posts = page.getByRole('region', { name: 'Admitted posts' });
	await expect(posts.getByRole('article')).toHaveCount(1);
	await expect(posts.getByRole('article').first()).toContainText(ownerText);
	await expect(posts.getByRole('article').first().locator('img')).toHaveCount(0);
	await expect(posts.getByRole('article').first()).toContainText('Place 1');

	return admitted;
}

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
		const outsiderText = `Refused outsider post ${uniqueLabel('p')}`;
		const refused = await post(outsider, outsiderText);
		expect(refused.status, 'a signed-in person outside this World and cast must be refused').toBe(
			'refused'
		);
		expect(refused.reason, 'the tenor must explicitly deny the nonmember').toBe('Forbidden');
		await expect(outsider.getByText('Refused: Forbidden', { exact: true })).toBeVisible({
			timeout: interactionTimeout
		});
		const outsiderPosts = outsider.getByRole('region', { name: 'Admitted posts' });
		await expect(outsiderPosts.getByText(outsiderText, { exact: true })).toHaveCount(0);
		await outsider.reload();
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
