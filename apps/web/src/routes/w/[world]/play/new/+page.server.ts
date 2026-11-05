import { error } from '@sveltejs/kit';
import { at_motet_pilot_getworld } from '@motet/lexicons';
import { XrpcError } from '#lib/api.js';
import { serverQuery } from '#lib/server/appview.js';
import type { PageServerLoad } from './$types';

export const load: PageServerLoad = async ({ request, params, url }) => {
	try {
		const world = await serverQuery<at_motet_pilot_getworld.GetWorldOutput>(
			request,
			at_motet_pilot_getworld.GetWorldNsid,
			{ actor: params.world }
		);

		return { world, returnTo: url.pathname + url.search };
	} catch (cause) {
		if (cause instanceof XrpcError && cause.status === 404) error(404, 'World not found');
		error(502, 'Could not load this World from the AppView');
	}
};
