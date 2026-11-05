import { at_motet_pilot_auth_getsession as getSession } from '@motet/lexicons';
import { XrpcError } from '#lib/api.js';
import { AppViewConfigError, serverQuery } from '#lib/server/appview.js';
import type { LayoutServerLoad } from './$types';

// The session decides which account controls every page shows, so a lookup
// that never answers would hold the whole page open. The AppView answers from
// its own index well inside this bound.
const SESSION_LOOKUP_TIMEOUT_MS = 2000;

export type SessionStatus = {
	session: getSession.GetSessionOutput | null;
	/**
	 * Set when the AppView did not answer, so the page cannot say whether the
	 * reader is signed in. Its absence means the lookup succeeded.
	 */
	sessionUnavailable?: true;
};

/**
 * Read the reader's session, and degrade to signed-out when the AppView fails.
 *
 * A refusal (401) proves the reader is signed out. Every other failure — a
 * network fault, the timeout above, a 5xx — leaves the status unknown, so the
 * page renders signed out with a notice. An error never passes for a session.
 *
 * Exported for the layout's test: SvelteKit accepts `_`-prefixed exports from a
 * route module and still treats only `load` as the layout's own surface.
 */
export async function _sessionStatus(request: Request): Promise<SessionStatus> {
	try {
		const session = await serverQuery<getSession.GetSessionOutput>(
			request,
			getSession.GetSessionNsid,
			{},
			{ timeoutMs: SESSION_LOOKUP_TIMEOUT_MS }
		);

		return { session };
	} catch (error) {
		// A misconfigured origin is a deployment fault. Degrading would leave
		// every page silently signed out with no sign of the cause.
		if (error instanceof AppViewConfigError) throw error;

		if (error instanceof XrpcError && error.status === 401) return { session: null };

		return { session: null, sessionUnavailable: true };
	}
}

export const load: LayoutServerLoad = async ({ request, setHeaders }) => {
	setHeaders({ 'cache-control': 'private, no-store' });

	return _sessionStatus(request);
};
