import { error } from '@sveltejs/kit';
import { characters } from '#lib/demo/screens/characters.js';
import type { PageLoad } from './$types';

// Characters aren't served by the AppView yet, so only the demo World has this screen.
export const load: PageLoad = ({ params }) => {
	if (params.world !== 'gilded-reach') {
		error(404, 'Not found');
	}

	return { screen: characters };
};
