import { error } from '@sveltejs/kit';
import { worldVisitor } from '#lib/demo/screens/world-visitor.js';
import type { PageLoad } from './$types';

// What a visitor sees at the World's address. It moves there once sign-in
// tells members from visitors.
export const load: PageLoad = ({ params }) => {
	if (params.world !== 'gilded-reach') {
		error(404, 'Not found');
	}

	return { screen: worldVisitor };
};
