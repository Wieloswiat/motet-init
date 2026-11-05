import { error } from '@sveltejs/kit';
import { worldSettings } from '#lib/demo/screens/world-settings.js';
import type { PageLoad } from './$types';

// World settings aren't served by the AppView yet, so only the demo World has this screen.
export const load: PageLoad = ({ params }) => {
	if (params.world !== 'gilded-reach') {
		error(404, 'Not found');
	}

	return { screen: worldSettings };
};
