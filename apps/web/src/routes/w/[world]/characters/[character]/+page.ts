import { error } from '@sveltejs/kit';
import { characterSheet } from '#lib/demo/screens/character-sheet.js';
import type { PageLoad } from './$types';

// Characters aren't served by the AppView yet, so only the demo character has this screen.
export const load: PageLoad = ({ params }) => {
	if (params.world !== 'gilded-reach' || params.character !== 'lady-maren') {
		error(404, 'Not found');
	}

	return { screen: characterSheet };
};
