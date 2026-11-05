import { query } from '#lib/api.js';

/**
 * The web server cannot address the AppView: the origin is missing or is not
 * an HTTPS origin. This is a deployment fault, not an AppView outage, so
 * callers must not degrade to a signed-out page and hide it.
 */
export class AppViewConfigError extends Error {
	constructor(message: string) {
		super(message);
		this.name = 'AppViewConfigError';
	}
}

/**
 * Query the configured public AppView origin, forwarding the browser session cookie.
 *
 * `timeoutMs` bounds the whole exchange, response body included. An expired
 * timeout reaches the caller as a rejected fetch, the same as a network fault.
 */
export async function serverQuery<Output>(
	request: Request,
	method: string,
	params: Record<string, string | number | undefined> = {},
	{ timeoutMs }: { timeoutMs?: number } = {}
): Promise<Output> {
	const configured = process.env.MOTET_APPVIEW_PUBLIC_ORIGIN;

	if (!configured) {
		throw new AppViewConfigError('MOTET_APPVIEW_PUBLIC_ORIGIN is required by the web server');
	}

	let origin: URL;

	try {
		origin = new URL(configured);
	} catch {
		throw new AppViewConfigError('MOTET_APPVIEW_PUBLIC_ORIGIN must be an HTTPS origin');
	}

	if (
		origin.protocol !== 'https:' ||
		origin.username ||
		origin.password ||
		origin.pathname !== '/' ||
		origin.search ||
		origin.hash
	) {
		throw new AppViewConfigError('MOTET_APPVIEW_PUBLIC_ORIGIN must be an HTTPS origin');
	}

	// Do not derive this destination from the request Host header: it carries
	// the browser's session cookie and must only go to the configured AppView.
	const appviewFetch: typeof fetch = (input, init) => {
		const headers = new Headers(init?.headers);
		const cookie = request.headers.get('cookie');

		if (cookie) headers.set('cookie', cookie);

		return globalThis.fetch(new URL(String(input), origin), {
			...init,
			headers,
			cache: 'no-store',
			signal: timeoutMs === undefined ? undefined : AbortSignal.timeout(timeoutMs)
		});
	};

	return query<Output>(appviewFetch, method, params);
}
