import { error } from '@sveltejs/kit';
import { scene } from '#lib/demo/screens/scene.js';
import type { PageLoad } from './$types';

// Only the demo World exists until the AppView serves Worlds.
export const load: PageLoad = ({ params }) => {
	if (params.world !== 'gilded-reach' || params.scene !== 'feast-of-lanterns') {
		error(404, 'Not found');
	}

	return { screen: scene };
};
