import { error } from '@sveltejs/kit';
import { profiles } from '#lib/demo/screens/profile.js';
import type { PageLoad } from './$types';

export const load: PageLoad = ({ params }) => {
	const screen = Object.values(profiles).find((profile) => profile.person.handle === params.handle);

	if (screen === undefined) {
		error(404, 'Not found');
	}

	return { screen };
};
