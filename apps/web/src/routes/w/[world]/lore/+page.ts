import { error } from '@sveltejs/kit';
import { lore } from '#lib/demo/screens/lore.js';
import type { PageLoad } from './$types';

// Lore isn't served by the AppView yet, so only the demo World has this screen.
export const load: PageLoad = ({ params }) => {
	if (params.world !== 'gilded-reach') {
		error(404, 'Not found');
	}

	return { screen: lore };
};
