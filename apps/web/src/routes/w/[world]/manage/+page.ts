import { error } from '@sveltejs/kit';
import { worldManage } from '#lib/demo/screens/world-manage.js';
import type { PageLoad } from './$types';

// World management isn't served by the AppView yet, so only the demo World has this screen.
export const load: PageLoad = ({ params }) => {
	if (params.world !== 'gilded-reach') {
		error(404, 'Not found');
	}

	return { screen: worldManage };
};
