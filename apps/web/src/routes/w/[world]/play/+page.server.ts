import { error } from '@sveltejs/kit';
import { at_motet_pilot_getworld, at_motet_pilot_listscenes } from '@motet/lexicons';
import { XrpcError } from '#lib/api.js';
import { serverQuery } from '#lib/server/appview.js';
import type { PageServerLoad } from './$types';

export const load: PageServerLoad = async ({ request, params, url }) => {
	try {
		const [world, listing] = await Promise.all([
			serverQuery<at_motet_pilot_getworld.GetWorldOutput>(
				request,
				at_motet_pilot_getworld.GetWorldNsid,
				{ actor: params.world }
			),
			serverQuery<at_motet_pilot_listscenes.ListScenesOutput>(
				request,
				at_motet_pilot_listscenes.ListScenesNsid,
				{ actor: params.world, limit: 100, cursor: url.searchParams.get('cursor') ?? undefined }
			)
		]);

		return { world, listing, returnTo: url.pathname + url.search };
	} catch (cause) {
		if (cause instanceof XrpcError && cause.status === 404) error(404, 'World not found');
		error(502, 'Could not load scenes from the AppView');
	}
};
