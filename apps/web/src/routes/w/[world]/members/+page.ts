import { error } from '@sveltejs/kit';
import { members } from '#lib/demo/screens/members.js';
import type { PageLoad } from './$types';

// The roster lives in the members Space, which the AppView doesn't read yet, so only the demo World has this screen.
export const load: PageLoad = ({ params }) => {
	if (params.world !== 'gilded-reach') {
		error(404, 'Not found');
	}

	return { screen: members };
};
