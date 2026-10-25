import { error } from '@sveltejs/kit';
import { characterSheet } from '#lib/demo/screens/character-sheet.js';
import type { PageLoad } from './$types';

// Only the demo World exists until the AppView serves Worlds.
export const load: PageLoad = ({ params }) => {
	if (params.world !== 'gilded-reach' || params.character !== 'lady-maren') {
		error(404, 'Not found');
	}

	return { screen: characterSheet };
};
