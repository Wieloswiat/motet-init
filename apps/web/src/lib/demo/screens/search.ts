// Demo content from the design export, as the screen first renders it.
// It stands in for AppView data until the screen loads real records.
import { resolve } from '$app/paths';
import { marks, portraits } from '../art.js';

export const search = {
	pageRef: {
		world: 'motet'
	},
	worldRef: {
		world: 'vellum'
	},
	inboxCount: 3,
	query: 'lantern',
	panelOpen: true,
	hasActive: true,
	activeCount: 1,
	noActive: false,
	tabs: [
		{
			label: 'All',
			icon: 'search',
			count: '',
			current: 'page' as const
		},
		{
			label: 'Worlds',
			icon: 'public',
			count: '1',
			current: false
		},
		{
			label: 'Posts',
			icon: 'forum',
			count: '14',
			current: false
		},
		{
			label: 'Scenes',
			icon: 'auto_stories',
			count: '3',
			current: false
		},
		{
			label: 'Lore',
			icon: 'menu_book',
			count: '2',
			current: false
		},
		{
			label: 'Characters',
			icon: 'group',
			count: '1',
			current: false
		},
		{
			label: 'People',
			icon: 'person',
			count: '1',
			current: false
		}
	],
	isAll: true,
	show: {
		worlds: true,
		posts: true,
		scenes: true,
		lore: true,
		characters: true,
		people: true
	},
	posts: [
		{
			name: 'Lady Maren of Ost',
			handle: 'Iga @iga.bsky.social',
			portrait: portraits['260-long'],
			pre: '',
			match: 'Lanterns',
			post: ' hung from every beam of the great hall, a hundred small suns swaying in the draught from the high windows.',
			world: 'The Gilded Reach',
			mark: marks.vellum,
			scene: 'The Feast of Lanterns',
			lane: 'In character',
			laneIcon: 'theater_comedy',
			when: 'Mar 3',
			href: resolve('/w/[world]/play/[scene]', {
				world: 'gilded-reach',
				scene: 'feast-of-lanterns'
			}),
			isCharacter: true,
			isAccount: false
		},
		{
			name: 'Ilse Varga',
			handle: 'Lena @lena.bsky.social',
			portrait: portraits['205-long'],
			pre: 'She took the storm ',
			match: 'lantern',
			post: ' down from its hook and walked out onto the ice without waiting for an answer.',
			world: 'The Salt Road',
			mark: marks.saltroad,
			scene: 'Thaw at the Ferry',
			lane: 'In character',
			laneIcon: 'theater_comedy',
			when: 'Feb 20',
			href: null,
			isCharacter: true,
			isAccount: false
		},
		{
			name: 'Aurelia',
			handle: '@aurelia.bsky.social',
			portrait: '',
			pre: 'Reminder: the ',
			match: 'Lantern',
			post: ' Rite scene opens on the 1st. Tell me by Friday if your character is releasing a lantern for anyone.',
			world: 'The Gilded Reach',
			mark: marks.vellum,
			scene: 'Table talk',
			lane: 'Out of character',
			laneIcon: 'chat',
			when: 'Mar 9',
			href: null,
			isCharacter: false,
			isAccount: true
		}
	],
	scenes: [
		{
			pre: 'The Feast of ',
			match: 'Lanterns',
			post: '',
			world: 'The Gilded Reach',
			mark: marks.vellum,
			where: 'Harrowgate Keep, the great hall',
			href: resolve('/w/[world]/play/[scene]', {
				world: 'gilded-reach',
				scene: 'feast-of-lanterns'
			}),
			archived: false
		},
		{
			pre: 'The Drowned ',
			match: 'Lantern',
			post: '',
			world: 'The Gilded Reach',
			mark: marks.vellum,
			where: 'Lantern Quay',
			href: null,
			archived: true
		},
		{
			pre: '',
			match: 'Lanterns',
			post: ' on the Ice',
			world: 'The Salt Road',
			mark: marks.saltroad,
			where: 'Velka Crossing',
			href: null,
			archived: false
		}
	],
	summary: '22 results for “lantern”',
	marks: {
		vellum: marks.vellum
	},
	portraits: {
		wren: portraits['15-hood']
	},
	scopes: [
		{
			value: 'everywhere',
			label: 'Everywhere',
			on: true
		},
		{
			value: 'mine',
			label: 'Your Worlds',
			on: false
		},
		{
			value: 'one',
			label: 'One World',
			on: false
		}
	],
	oneWorld: false,
	by: '',
	lane: 'both',
	icPressed: true,
	oocPressed: true,
	laneText: 'In and out of character',
	langCodes: [],
	genreTags: [
		{
			label: 'Fantasy',
			remove: 'Remove Fantasy'
		}
	],
	options: [
		{
			label: 'Exact phrase',
			on: false
		},
		{
			label: 'Include archived scenes',
			on: true
		},
		{
			label: 'Search inside spoilers',
			on: false
		}
	]
};

export type SearchData = typeof search;
