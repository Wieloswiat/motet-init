import { query } from '#lib/api.js';

/** Query the configured public AppView origin, forwarding the browser session cookie. */
export async function serverQuery<Output>(
	request: Request,
	method: string,
	params: Record<string, string | number | undefined> = {}
): Promise<Output> {
	const configured = process.env.MOTET_APPVIEW_PUBLIC_ORIGIN;

	if (!configured) throw new Error('MOTET_APPVIEW_PUBLIC_ORIGIN is required by the web server');
	const origin = new URL(configured);

	if (
		origin.protocol !== 'https:' ||
		origin.username ||
		origin.password ||
		origin.pathname !== '/' ||
		origin.search ||
		origin.hash
	) {
		throw new Error('MOTET_APPVIEW_PUBLIC_ORIGIN must be an HTTPS origin');
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
			cache: 'no-store'
		});
	};

	return query<Output>(appviewFetch, method, params);
}
