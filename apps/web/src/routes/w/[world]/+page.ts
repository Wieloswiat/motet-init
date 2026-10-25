import { error } from '@sveltejs/kit';
import { worldMember } from '#lib/demo/screens/world-member.js';
import type { PageLoad } from './$types';

// Only the demo World exists until the AppView serves Worlds.
export const load: PageLoad = ({ params }) => {
	if (params.world !== 'gilded-reach') {
		error(404, 'Not found');
	}

	return { screen: worldMember };
};
