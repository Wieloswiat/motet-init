import { error } from '@sveltejs/kit';
import { at_motet_pilot_getworld, at_motet_pilot_listscenes } from '@motet/lexicons';
import { XrpcError } from '#lib/api.js';
import { serverQuery } from '#lib/server/appview.js';
import { decodeScene, type SceneView } from './play/records.js';
import type { PageServerLoad } from './$types';

export const load: PageServerLoad = async ({ params, request }) => {
	let world: at_motet_pilot_getworld.GetWorldOutput;

	try {
		world = await serverQuery<at_motet_pilot_getworld.GetWorldOutput>(
			request,
			at_motet_pilot_getworld.GetWorldNsid,
			{ actor: params.world }
		);
	} catch (cause) {
		if (cause instanceof XrpcError && cause.status === 404) error(404, 'World not found');

		const status =
			cause instanceof XrpcError && cause.status >= 400 && cause.status <= 599 ? cause.status : 503;

		error(status, 'Could not load this World from the AppView. Please try again.');
	}

	const scenes: SceneView[] = [];
	let hasMoreScenes = false;
	let invalidScenes = false;
	let scenesUnavailable = false;

	try {
		const result = await serverQuery<at_motet_pilot_listscenes.ListScenesOutput>(
			request,
			at_motet_pilot_listscenes.ListScenesNsid,
			{ actor: world.did }
		);

		hasMoreScenes = result.cursor !== undefined;

		for (const record of result.scenes) {
			const scene = decodeScene(record, world.did);

			if (scene === null) invalidScenes = true;
			else scenes.push(scene);
		}
	} catch {
		scenesUnavailable = true;
	}

	return { world, scenes, hasMoreScenes, invalidScenes, scenesUnavailable };
};
