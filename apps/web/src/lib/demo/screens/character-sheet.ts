// Demo content from the design export, as the screen first renders it.
// It stands in for AppView data until the screen loads real records.
import { resolve } from '$app/paths';
import { portraits } from '../art.js';

export const characterSheet = {
	worldRef: {
		world: 'vellum'
	},
	no: false,
	inboxCount: 3,
	portrait: portraits['260-long'],
	tabs: [
		{
			label: 'Biography',
			icon: 'history_edu',
			current: 'page' as const
		},
		{
			label: 'Relationships',
			icon: 'diversity_3',
			current: false
		},
		{
			label: 'Scenes',
			icon: 'forum',
			current: false
		}
	],
	is: {
		bio: true,
		rel: false,
		scenes: false
	},
	secretOpen: false,
	secretAction: 'Show',
	relations: [
		{
			name: 'Brother Cael',
			kind: 'Old friend',
			note: 'Taught her to read the Archive’s cipher. Owes her a favour he has not yet been asked for.',
			portrait: portraits['45-hood']
		},
		{
			name: 'Steward Hollis',
			kind: 'Distrusts',
			note: 'Smiles at her too often. Answers questions she has not asked.',
			portrait: portraits['20-short']
		},
		{
			name: 'Isolde Corven',
			kind: 'Rival',
			note: 'They want the same seat at the high table. They may also want the same thing from it.',
			portrait: portraits['330-long']
		},
		{
			name: 'Duke Aldric Vael',
			kind: 'Liege’s lord',
			note: 'Her father swore to him. She has spoken to him twice in three years.',
			portrait: portraits['45-hat']
		}
	],
	scenes: [
		{
			title: 'The Feast of Lanterns',
			where: 'Harrowgate Keep, the great hall',
			posts: '12 posts',
			time: '2h',
			href: resolve('/w/[world]/play/[scene]', {
				world: 'gilded-reach',
				scene: 'feast-of-lanterns'
			})
		},
		{
			title: 'The Steward’s Ledger',
			where: 'Harrowgate Keep, the steward’s rooms',
			posts: '7 posts',
			time: 'Mar 8',
			href: null
		},
		{
			title: 'Letters from Ost',
			where: 'Correspondence',
			posts: '9 posts',
			time: 'Mar 2',
			href: null
		},
		{
			title: 'Arrival',
			where: 'Harrowgate Keep, the gate',
			posts: '10 posts',
			time: 'Sep 14',
			href: null
		}
	]
};

export type CharacterSheetData = typeof characterSheet;
