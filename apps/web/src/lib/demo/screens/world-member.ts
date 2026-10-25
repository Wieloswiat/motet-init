// Demo content from the design export, as the screen first renders it.
// It stands in for AppView data until the screen loads real records.
import { resolve } from '$app/paths';
import { portraits } from '../art.js';

export const worldMember = {
	worldRef: {
		world: 'vellum'
	},
	no: false,
	inboxCount: 3,
	mine: [
		{
			title: 'The Feast of Lanterns',
			where: 'Harrowgate Keep, the great hall',
			as: 'Lady Maren',
			cast: [portraits['260-long'], portraits['45-hood'], portraits['20-short']],
			flag: 'turn',
			count: 3,
			time: '2h',
			href: resolve('/w/[world]/play/[scene]', {
				world: 'gilded-reach',
				scene: 'feast-of-lanterns'
			}),
			unread: true,
			rootClass: 'm-scene-row__root m-scene-row__root--unread_true',
			markerClass: 'm-scene-row__marker m-scene-row__marker--unread_true',
			titleClass: 'm-scene-row__title m-scene-row__title--unread_true',
			hasArc: false,
			whereIcon: 'location_on',
			hasFlag: true,
			flagClass: 'm-flag m-flag--kind_turn',
			flagIcon: 'front_hand',
			flagLabel: 'Your turn',
			hasCount: true,
			countTip: '3 unread',
			castTip: 'Cast: Lady Maren of Ost, Brother Cael, Steward Hollis'
		},
		{
			title: 'Contraband at Low Tide',
			where: 'Lantern Quay, the customs house',
			as: 'Anwen',
			cast: [portraits['175-long'], portraits['15-hood'], portraits['210-short']],
			flag: null,
			count: 1,
			time: '1d',
			href: null,
			unread: true,
			rootClass: 'm-scene-row__root m-scene-row__root--unread_true',
			markerClass: 'm-scene-row__marker m-scene-row__marker--unread_true',
			titleClass: 'm-scene-row__title m-scene-row__title--unread_true',
			hasArc: false,
			whereIcon: 'location_on',
			hasFlag: false,
			flagClass: '',
			flagIcon: '',
			flagLabel: '',
			hasCount: true,
			countTip: '1 unread',
			castTip: 'Cast: Anwen Ashdown, Wren of the Quay, Sir Edric Corven'
		},
		{
			title: 'The Steward’s Ledger',
			where: 'Harrowgate Keep, the steward’s rooms',
			as: 'Lady Maren',
			cast: [portraits['260-long'], portraits['20-short']],
			flag: 'gm',
			count: null,
			time: 'Mar 8',
			href: null,
			unread: false,
			rootClass: 'm-scene-row__root m-scene-row__root--unread_false',
			markerClass: 'm-scene-row__marker m-scene-row__marker--unread_false',
			titleClass: 'm-scene-row__title m-scene-row__title--unread_false',
			hasArc: false,
			whereIcon: 'location_on',
			hasFlag: true,
			flagClass: 'm-flag m-flag--kind_draft',
			flagIcon: 'hourglass_top',
			flagLabel: 'Waiting on GM',
			hasCount: false,
			countTip: '',
			castTip: 'Cast: Lady Maren of Ost, Steward Hollis'
		},
		{
			title: 'Letters from Ost',
			where: 'Correspondence',
			as: 'Lady Maren',
			cast: [portraits['260-long']],
			flag: 'draft',
			count: null,
			time: 'Mar 2',
			href: null,
			unread: false,
			rootClass: 'm-scene-row__root m-scene-row__root--unread_false',
			markerClass: 'm-scene-row__marker m-scene-row__marker--unread_false',
			titleClass: 'm-scene-row__title m-scene-row__title--unread_false',
			hasArc: false,
			whereIcon: 'location_on',
			hasFlag: true,
			flagClass: 'm-flag m-flag--kind_draft',
			flagIcon: 'draft',
			flagLabel: 'Draft',
			hasCount: false,
			countTip: '',
			castTip: 'Cast: Lady Maren of Ost'
		}
	],
	others: [
		{
			title: 'Whispers in the Archive',
			where: 'Harrowgate Keep',
			arc: 'A Crown of Ash',
			cast: [portraits['45-hood'], portraits['330-long']],
			count: 2,
			time: '6h',
			href: null,
			unread: true,
			rootClass: 'm-scene-row__root m-scene-row__root--unread_true',
			markerClass: 'm-scene-row__marker m-scene-row__marker--unread_true',
			titleClass: 'm-scene-row__title m-scene-row__title--unread_true',
			hasArc: true,
			whereIcon: 'location_on',
			hasFlag: false,
			flagClass: '',
			flagIcon: '',
			flagLabel: '',
			hasCount: true,
			countTip: '2 unread',
			castTip: 'Cast: Brother Cael, Isolde Corven',
			ooc: null,
			flag: null
		},
		{
			title: 'Table talk',
			where: 'Everyone',
			arc: null,
			cast: [],
			count: 4,
			time: '1d',
			href: null,
			unread: true,
			rootClass: 'm-scene-row__root m-scene-row__root--unread_true',
			markerClass: 'm-scene-row__marker m-scene-row__marker--unread_true',
			titleClass: 'm-scene-row__title m-scene-row__title--unread_true',
			hasArc: false,
			whereIcon: 'group',
			hasFlag: false,
			flagClass: '',
			flagIcon: '',
			flagLabel: '',
			hasCount: true,
			countTip: '4 unread',
			castTip: '',
			ooc: true,
			flag: null
		},
		{
			title: 'Weirwood Pass',
			where: 'The Northern March',
			arc: 'A Crown of Ash',
			cast: [portraits['120-hat'], portraits['15-hood']],
			count: null,
			time: 'Mar 5',
			href: null,
			unread: false,
			rootClass: 'm-scene-row__root m-scene-row__root--unread_false',
			markerClass: 'm-scene-row__marker m-scene-row__marker--unread_false',
			titleClass: 'm-scene-row__title m-scene-row__title--unread_false',
			hasArc: true,
			whereIcon: 'location_on',
			hasFlag: true,
			flagClass: 'm-flag m-flag--kind_done',
			flagIcon: 'person_search',
			flagLabel: 'Open to join',
			hasCount: false,
			countTip: '',
			castTip: 'Cast: Ser Jaume Tallow, Wren of the Quay',
			ooc: null,
			flag: 'open'
		}
	],
	chars: [
		{
			name: 'Lady Maren of Ost',
			where: 'The great hall',
			portrait: portraits['260-long'],
			href: resolve('/w/[world]/characters/[character]', {
				world: 'gilded-reach',
				character: 'lady-maren'
			})
		},
		{
			name: 'Anwen Ashdown',
			where: 'The customs house',
			portrait: portraits['175-long'],
			href: resolve('/w/[world]/characters', { world: 'gilded-reach' })
		}
	],
	pinned: [
		{
			icon: 'gavel',
			label: 'Table rules and content',
			href: resolve('/w/[world]/visitor', { world: 'gilded-reach' }),
			note: '',
			hasNote: false
		},
		{
			icon: 'castle',
			label: 'The houses of the Reach',
			href: resolve('/w/[world]/lore', { world: 'gilded-reach' }),
			note: 'Updated Mar 10',
			hasNote: true
		},
		{
			icon: 'map',
			label: 'Harrowgate and the river',
			href: resolve('/w/[world]/lore', { world: 'gilded-reach' }),
			note: '',
			hasNote: false
		},
		{
			icon: 'event',
			label: 'Absences and schedule',
			href: null,
			note: '',
			hasNote: false
		}
	]
};

export type WorldMemberData = typeof worldMember;
