import { at_motet_pilot_auth_getsession as getSession } from '@motet/lexicons';
import { XrpcError } from '#lib/api.js';
import { serverQuery } from '#lib/server/appview.js';
import type { LayoutServerLoad } from './$types';

export const load: LayoutServerLoad = async ({ request, setHeaders }) => {
	setHeaders({ 'cache-control': 'private, no-store' });

	try {
		const session = await serverQuery<getSession.GetSessionOutput>(
			request,
			getSession.GetSessionNsid
		);

		return { session };
	} catch (error) {
		if (error instanceof XrpcError && error.status === 401) return { session: null };
		throw error;
	}
};
