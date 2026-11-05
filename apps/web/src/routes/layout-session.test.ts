import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { AppViewConfigError } from '#lib/server/appview.js';
import { _sessionStatus } from './+layout.server.js';

const origin = 'https://appview.motet.test';
const session = { did: 'did:plc:reader', handle: 'reader.motet.test' };

/** Answer the session lookup as the AppView would, with a real Response. */
function answer(status: number, body: string): void {
	vi.stubGlobal('fetch', () =>
		Promise.resolve(new Response(body, { status, headers: { 'content-type': 'application/json' } }))
	);
}

function pageRequest(): Request {
	return new Request('https://motet.test/w/did:plc:world', {
		headers: { cookie: 'motet_session=browser-cookie' }
	});
}

beforeEach(() => {
	vi.stubEnv('MOTET_APPVIEW_PUBLIC_ORIGIN', origin);
});

afterEach(() => {
	vi.unstubAllGlobals();
	vi.unstubAllEnvs();
});

describe('the layout session lookup', () => {
	// A reader of any page must keep their page when the AppView is unwell.
	// Rethrowing the failure would have turned every public and design page
	// into a 500 whenever the AppView was down.
	it('reports an unknown sign-in status when the AppView answers 500', async () => {
		answer(500, JSON.stringify({ error: 'InternalServerError', message: 'index unavailable' }));

		await expect(_sessionStatus(pageRequest())).resolves.toEqual({
			session: null,
			sessionUnavailable: true
		});
	});

	// A refused connection is not a refusal of the session. Treating it as
	// signed-out-without-notice would tell a signed-in reader they were out.
	it('reports an unknown sign-in status when the AppView refuses the connection', async () => {
		vi.stubGlobal('fetch', () => Promise.reject(new TypeError('fetch failed')));

		await expect(_sessionStatus(pageRequest())).resolves.toEqual({
			session: null,
			sessionUnavailable: true
		});
	});

	// The lookup decides the account controls of every page, so an AppView that
	// accepts the connection and never answers must not hold the page open.
	it('stops waiting when the AppView accepts the request and never answers', async () => {
		vi.stubGlobal(
			'fetch',
			(_input: RequestInfo | URL, init?: RequestInit) =>
				new Promise<Response>((_resolve, reject) => {
					init?.signal?.addEventListener('abort', () => reject(init.signal?.reason));
				})
		);

		await expect(_sessionStatus(pageRequest())).resolves.toEqual({
			session: null,
			sessionUnavailable: true
		});
	}, 10_000);

	// A refusal is evidence: the AppView checked the cookie and rejected it, so
	// the page is signed out and needs no notice.
	it('reports a signed-out reader without the notice when the AppView answers 401', async () => {
		answer(401, JSON.stringify({ error: 'AuthenticationRequired', message: 'no session' }));

		await expect(_sessionStatus(pageRequest())).resolves.toEqual({ session: null });
	});

	// The degradation must not swallow a working session: a signed-in reader
	// keeps their account controls when the AppView answers.
	it('returns the session when the AppView answers', async () => {
		answer(200, JSON.stringify(session));

		await expect(_sessionStatus(pageRequest())).resolves.toEqual({ session });
	});

	// A missing or untrusted origin is a deployment fault. Degrading it would
	// leave every page signed out with no sign of the misconfiguration.
	it('fails loudly when the AppView origin is not configured', async () => {
		vi.stubEnv('MOTET_APPVIEW_PUBLIC_ORIGIN', '');

		await expect(_sessionStatus(pageRequest())).rejects.toThrow(AppViewConfigError);
	});
});
