import {
	createDecipheriv,
	createECDH,
	createPrivateKey,
	pbkdf2Sync,
	randomBytes
} from 'node:crypto';
import { readFile } from 'node:fs/promises';
import { expect, type Page, type Route } from '@playwright/test';
import {
	at_motet_pilot_auth_getsession as getSession,
	at_motet_pilot_beginworld as beginWorld,
	at_motet_pilot_createpost as createPost,
	at_motet_pilot_finishworld as finishWorld,
	at_motet_pilot_getscene as getScene,
	at_motet_pilot_getworld as getWorld,
	at_motet_pilot_openscene as openScene
} from '@motet/lexicons';

export const app = 'https://motet.test';
export const pds = 'https://pds.motet.test';
export const projectionTimeout = 120_000;
export const interactionTimeout = 60_000;

export type ProvisioningResource =
	{ method: 'getWorld'; actor: string } | { method: 'getScene'; actor: string; scene: string };

export type BrowserErrors = {
	errors: string[];
	worldProvisioning: (actor: string) => void;
	sceneProvisioning: (actor: string, scene: string) => void;
	provisioningComplete: () => void;
};

function provisioningSource(source: URL, resource: ProvisioningResource): boolean {
	if (source.origin !== app || source.searchParams.get('actor') !== resource.actor) return false;

	if (resource.method === 'getWorld') {
		return source.pathname === `/xrpc/${getWorld.GetWorldNsid}` && source.searchParams.size === 1;
	}

	return (
		source.pathname === `/xrpc/${getScene.GetSceneNsid}` &&
		source.searchParams.size === 3 &&
		source.searchParams.get('scene') === resource.scene &&
		source.searchParams.get('limit') === '100'
	);
}

export function expectedSetupResourceError(
	sourceUrl: string,
	status: number,
	provisioning: ProvisioningResource | null
): boolean {
	if (status !== 404) return false;

	let source: URL;

	try {
		source = new URL(sourceUrl);
	} catch {
		return false;
	}

	if (source.href === `${pds}/favicon.ico`) return true;

	return provisioning !== null && provisioningSource(source, provisioning);
}

// Chromium logs handled HTTP failures as console errors. World and scene creation poll for
// archive-derived projections (ADR 0020); the local OAuth PDS favicon is outside Motet.
export function collectBrowserErrors(page: Page): BrowserErrors {
	const errors: string[] = [];
	let provisioning: ProvisioningResource | null = null;

	page.on('response', (response) => {
		if (
			response.status() === 200 &&
			provisioning !== null &&
			provisioningSource(new URL(response.url()), provisioning)
		) {
			provisioning = null;
		}
	});
	page.on('console', (message) => {
		if (message.type() !== 'error') return;

		const source = message.location().url;

		const resourceStatus =
			/^Failed to load resource: the server responded with a status of (\d{3}) \(.*\)$/.exec(
				message.text()
			);

		if (
			resourceStatus &&
			expectedSetupResourceError(source, Number(resourceStatus[1]), provisioning)
		)
			return;

		errors.push(`${source}: ${message.text()}`);
	});
	page.on('pageerror', (error) => errors.push(error.message));

	return {
		errors,
		worldProvisioning: (actor) => {
			provisioning = { method: 'getWorld', actor };
		},
		sceneProvisioning: (actor, scene) => {
			provisioning = { method: 'getScene', actor, scene };
		},
		provisioningComplete: () => {
			provisioning = null;
		}
	};
}

// Each first label is 17 characters: the pinned PDS accepts at most 18.
export function uniqueLabel(prefix: string): string {
	return `${prefix}${randomBytes(8).toString('hex')}`;
}

export type Account = {
	handle: string;
	password: string;
	did: string;
};

export type World = {
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

export async function currentSession(page: Page): Promise<getSession.GetSessionOutput> {
	const response = await page.request.get(`${app}/xrpc/${getSession.GetSessionNsid}`);
	expect(response.status(), 'the AppView must expose the authenticated session').toBe(200);

	return response.json();
}

export async function signUp(page: Page): Promise<Account> {
	const label = uniqueLabel('u');
	const handle = `${label}.pds.motet.test`;
	const password = `Motet-password-${randomBytes(12).toString('hex')}`;
	await page.goto('/sign-in');

	const start = page.waitForResponse(
		(response) =>
			response.request().method() === 'POST' && response.url() === `${app}/oauth/sign-up`,
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

	expect(started.status(), 'the AppView must issue the PDS authorization URL').toBe(200);
	expect(await started.request().headerValue('origin')).toBe(app);
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

export async function signOut(page: Page): Promise<void> {
	await page.getByRole('button', { name: 'Sign out' }).click();
	await expect(page.getByRole('link', { name: 'Sign in', exact: true }).first()).toBeVisible();
	const anonymous = await page.request.get(`${app}/xrpc/${getSession.GetSessionNsid}`);
	expect(anonymous.status(), 'sign-out must revoke the AppView session').toBe(401);
}

export async function signIn(page: Page, account: Account): Promise<void> {
	await page.getByLabel('Your handle').fill(account.handle);

	const start = page.waitForResponse(
		(response) =>
			response.request().method() === 'POST' && response.url() === `${app}/oauth/sign-in`,
		{ timeout: interactionTimeout }
	);

	const [started] = await Promise.all([
		start,
		page.getByRole('button', { name: 'Continue', exact: true }).click()
	]);

	expect(started.status(), 'the AppView must issue the PDS authorization URL').toBe(200);
	expect(await started.request().headerValue('origin')).toBe(app);
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

export async function createWorld(
	page: Page,
	lostResponse?: 'retry' | 'reload',
	browserErrors?: BrowserErrors
): Promise<World> {
	const label = uniqueLabel('w');
	const handle = `${label}.motet.test`;
	const name = `World ${label}`;
	const passphrase = `Recovery #${randomBytes(12).toString('hex')} / owner`;
	const wrongPassphrase = `Wrong #${randomBytes(12).toString('hex')} / owner`;
	await page.goto('/new-world');
	await page.getByLabel('World name').fill(name);
	await page.getByLabel('World handle').fill(handle);
	await page.getByLabel('Backup passphrase').fill(passphrase);

	let reservationAttempts = 0;
	let firstInput: beginWorld.BeginWorldInput | undefined;
	let firstReservation: beginWorld.BeginWorldOutput | undefined;

	if (lostResponse) {
		await page.route(`**/xrpc/${beginWorld.BeginWorldNsid}`, async (route) => {
			const input: beginWorld.BeginWorldInput = route.request().postDataJSON();
			const response = await route.fetch();
			expect(
				response.status(),
				'the real reservation must succeed before its response is lost'
			).toBe(200);
			const result: beginWorld.BeginWorldOutput = await response.json();
			reservationAttempts += 1;

			if (reservationAttempts === 1) {
				firstInput = input;
				firstReservation = result;
				await route.abort();
			} else {
				if (lostResponse === 'retry') {
					expect(input, 'retry must retain the original reservation inputs').toEqual(firstInput);
					expect(result.reservation, 'retry must return the original reservation').toBe(
						firstReservation?.reservation
					);
				} else {
					expect(input.handle).toBe(firstInput?.handle);
					expect(input.op).not.toBe(firstInput?.op);
					expect(input.rotationKey).not.toBe(firstInput?.rotationKey);
					expect(result.reservation).not.toBe(firstReservation?.reservation);
				}

				await route.fulfill({ response });
			}
		});
		await page.getByRole('button', { name: 'Create World' }).click();
		await expect(page.getByRole('alert')).toContainText('Retry');

		if (lostResponse === 'reload') {
			page.once('dialog', (dialog) => dialog.accept());
			await page.reload();
			await page.getByLabel('World name').fill(name);
			await page.getByLabel('World handle').fill(handle);
			await page.getByLabel('Backup passphrase').fill(passphrase);
		}
	}

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
		page
			.getByRole('button', { name: lostResponse === 'retry' ? 'Retry creation' : 'Create World' })
			.click()
	]);

	if (lostResponse) expect(reservationAttempts).toBe(2);

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
	browserErrors?.worldProvisioning(handle);
	await page.getByRole('button', { name: 'Verify backup' }).click();
	await expect(page.getByRole('heading', { name: 'World ready' })).toBeVisible({
		timeout: projectionTimeout
	});
	const world = { did: created.did, handle, name, tenor: reserved.tenor, secrets };
	await waitForWorld(page, world);
	browserErrors?.provisioningComplete();
	await page.getByRole('link', { name: 'Go to World' }).click();
	await expect(page.getByRole('heading', { name })).toBeVisible({ timeout: interactionTimeout });

	return world;
}

export async function sceneProjection(
	page: Page,
	world: World,
	scene: string
): Promise<getScene.GetSceneOutput> {
	const params = new URLSearchParams({ actor: world.did, scene });
	const response = await page.request.get(`${app}/xrpc/${getScene.GetSceneNsid}?${params}`);
	expect(response.status(), 'the projected scene must be readable').toBe(200);

	return response.json();
}

export async function openPublicScene(
	page: Page,
	world: World,
	owner: Account,
	browserErrors?: BrowserErrors
): Promise<string> {
	await page.getByRole('link', { name: 'Open scene', exact: true }).click();
	await expect(
		page.getByRole('region', { name: 'Cast' }).getByText(owner.did, { exact: true })
	).toBeVisible();
	// Arm the exact scene before releasing its creation response: hydration can poll immediately.
	const creationUrl = `${app}/xrpc/${openScene.OpenSceneNsid}`;

	const provisionScene = async (route: Route): Promise<void> => {
		const response = await route.fetch();

		if (response.status() === 200) {
			const opened: openScene.OpenSceneOutput = await response.json();
			browserErrors?.sceneProvisioning(world.did, opened.scene);
		}

		await route.fulfill({ response });
	};

	if (browserErrors) await page.route(creationUrl, provisionScene);

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

	if (browserErrors) await page.unroute(creationUrl, provisionScene);

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
				browserErrors?.provisioningComplete();
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

export async function post(page: Page, text: string): Promise<createPost.CreatePostOutput> {
	await page.getByRole('textbox', { name: 'Write a post' }).fill(text);

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

export async function admitFirstPost(
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
