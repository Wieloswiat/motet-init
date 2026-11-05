import { error } from '@sveltejs/kit';
import {
	at_motet_pilot_getscene,
	at_motet_pilot_getworld,
	at_motet_pilot_getscenecontrols
} from '@motet/lexicons';
import { XrpcError } from '#lib/api.js';
import { serverQuery } from '#lib/server/appview.js';
import type { PageServerLoad } from './$types';

export const load: PageServerLoad = async ({ request, params, url, parent }) => {
	let world: at_motet_pilot_getworld.GetWorldOutput;

	try {
		world = await serverQuery<at_motet_pilot_getworld.GetWorldOutput>(
			request,
			at_motet_pilot_getworld.GetWorldNsid,
			{ actor: params.world }
		);
	} catch (cause) {
		if (cause instanceof XrpcError && cause.status === 404) error(404, 'World not found');
		error(502, 'Could not load this World from the AppView');
	}

	let sceneControls: at_motet_pilot_getscenecontrols.GetSceneControlsOutput | null = null;
	let sceneControlsUnavailable = false;
	const { session } = await parent();

	if (session) {
		try {
			sceneControls = await serverQuery<at_motet_pilot_getscenecontrols.GetSceneControlsOutput>(
				request,
				at_motet_pilot_getscenecontrols.GetSceneControlsNsid,
				{ world: world.did }
			);
		} catch {
			// Permission lookup failure must not hide the public scene or imply authority.
			sceneControlsUnavailable = true;
		}
	}

	try {
		const scenePage = await serverQuery<at_motet_pilot_getscene.GetSceneOutput>(
			request,
			at_motet_pilot_getscene.GetSceneNsid,
			{ actor: world.did, scene: params.scene, limit: 100 }
		);

		return {
			world,
			scenePage,
			sceneControls,
			sceneControlsUnavailable,
			sceneKey: params.scene,
			returnTo: url.pathname + url.search
		};
	} catch (cause) {
		if (cause instanceof XrpcError && cause.status === 404) {
			// A just-opened scene can reach the tenor before its public projection.
			return {
				world,
				scenePage: null,
				sceneControls,
				sceneControlsUnavailable,
				sceneKey: params.scene,
				returnTo: url.pathname + url.search
			};
		}

		error(502, 'Could not load this scene from the AppView');
	}
};
