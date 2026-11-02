import { error } from '@sveltejs/kit';
import { worldApply } from '#lib/demo/screens/world-apply.js';
import type { PageLoad } from './$types';

// Applications aren't served by the AppView yet, so only the demo World has this screen.
export const load: PageLoad = ({ params }) => {
	if (params.world !== 'gilded-reach') {
		error(404, 'Not found');
	}

	return { screen: worldApply };
};
