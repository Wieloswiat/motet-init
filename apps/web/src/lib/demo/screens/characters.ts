// Demo content from the design export, as the screen first renders it.
// It stands in for AppView data until the screen loads real records.
import { portraits } from '../art.js';

export const characters = {
	worldRef: {
		world: 'vellum'
	},
	no: false,
	inboxCount: 3,
	query: '',
	filters: [
		{
			label: 'Everyone',
			icon: 'group',
			pressed: true
		},
		{
			label: 'Player characters',
			icon: 'theater_comedy',
			pressed: false
		},
		{
			label: 'GM characters',
			icon: 'shield_person',
			pressed: false
		},
		{
			label: 'Yours',
			icon: 'person',
			pressed: false
		},
		{
			label: 'Awaiting review',
			icon: 'hourglass_top',
			pressed: false
		}
	],
	views: [
		{
			label: 'Cards',
			icon: 'grid_view',
			checked: true
		},
		{
			label: 'List',
			icon: 'view_list',
			checked: false
		}
	],
	cards: true,
	list: false,
	groups: [
		{
			name: 'House Ost',
			hid: 'g-0',
			vtn: 'm-chg-0',
			items: [
				{
					name: 'Lady Maren of Ost',
					epithet: 'Envoy of the Northern March',
					house: 'House Ost',
					age: 27,
					trait: 'Letters',
					scenes: 4,
					p: [260, 'long'],
					owner: 'Played by you',
					initials: 'IB',
					hue: '--avatar-hue: 141',
					kind: 'pc',
					mine: true,
					vtn: 'm-ch-lady-maren-of-ost',
					vtp: 'm-chp-lady-maren-of-ost',
					vtt: 'm-cht-lady-maren-of-ost',
					portrait: portraits['260-long'],
					flagClass: 'm-flag m-flag--kind_done',
					flagIcon: 'check_circle',
					flagLabel: 'Approved'
				}
			],
			count: '1'
		},
		{
			name: 'House Corven',
			hid: 'g-1',
			vtn: 'm-chg-1',
			items: [
				{
					name: 'Isolde Corven',
					epithet: 'Second daughter of House Corven',
					house: 'House Corven',
					age: 22,
					trait: 'Shadows',
					scenes: 3,
					p: [330, 'long'],
					owner: 'Played by @nina.motet.at',
					initials: 'NM',
					hue: '--avatar-hue: 280',
					kind: 'pc',
					mine: null,
					vtn: 'm-ch-isolde-corven',
					vtp: 'm-chp-isolde-corven',
					vtt: 'm-cht-isolde-corven',
					portrait: portraits['330-long'],
					flagClass: 'm-flag m-flag--kind_done',
					flagIcon: 'check_circle',
					flagLabel: 'Approved'
				},
				{
					name: 'Sir Edric Corven',
					epithet: 'Sworn sword of House Corven',
					house: 'House Corven',
					age: 35,
					trait: 'Steel',
					scenes: 2,
					p: [210, 'short'],
					owner: 'Played by @kuba.dev',
					initials: 'KD',
					hue: '--avatar-hue: 35',
					kind: 'pc',
					mine: null,
					vtn: 'm-ch-sir-edric-corven',
					vtp: 'm-chp-sir-edric-corven',
					vtt: 'm-cht-sir-edric-corven',
					portrait: portraits['210-short'],
					flagClass: 'm-flag m-flag--kind_done',
					flagIcon: 'check_circle',
					flagLabel: 'Approved'
				}
			],
			count: '2'
		},
		{
			name: 'House Vael',
			hid: 'g-2',
			vtn: 'm-chg-2',
			items: [
				{
					name: 'Duke Aldric Vael',
					epithet: 'Duke of the Reach, ailing',
					house: 'House Vael',
					age: 68,
					trait: 'Faith',
					scenes: 1,
					p: [45, 'hat'],
					owner: 'GM character, Bram',
					initials: 'BW',
					hue: '--avatar-hue: 96',
					kind: 'npc',
					mine: null,
					vtn: 'm-ch-duke-aldric-vael',
					vtp: 'm-chp-duke-aldric-vael',
					vtt: 'm-cht-duke-aldric-vael',
					portrait: portraits['45-hat'],
					flagClass: 'm-flag m-flag--kind_draft',
					flagIcon: 'shield_person',
					flagLabel: 'GM character'
				},
				{
					name: 'Steward Hollis',
					epithet: 'Voice of the Duke’s household',
					house: 'House Vael',
					age: 51,
					trait: 'Silver tongue',
					scenes: 3,
					p: [20, 'short'],
					owner: 'GM character, Aurelia',
					initials: 'AB',
					hue: '--avatar-hue: 262',
					kind: 'npc',
					mine: null,
					vtn: 'm-ch-steward-hollis',
					vtp: 'm-chp-steward-hollis',
					vtt: 'm-cht-steward-hollis',
					portrait: portraits['20-short'],
					flagClass: 'm-flag m-flag--kind_draft',
					flagIcon: 'shield_person',
					flagLabel: 'GM character'
				},
				{
					name: 'Ser Jaume Tallow',
					epithet: 'Captain of the Duke’s watch',
					house: 'House Vael',
					age: 44,
					trait: 'Steel',
					scenes: 2,
					p: [120, 'hat'],
					owner: 'Played by @marek.pds.example',
					initials: 'MP',
					hue: '--avatar-hue: 0',
					kind: 'pc',
					mine: null,
					vtn: 'm-ch-ser-jaume-tallow',
					vtp: 'm-chp-ser-jaume-tallow',
					vtt: 'm-cht-ser-jaume-tallow',
					portrait: portraits['120-hat'],
					flagClass: 'm-flag m-flag--kind_done',
					flagIcon: 'check_circle',
					flagLabel: 'Approved'
				}
			],
			count: '3'
		},
		{
			name: 'The Archive',
			hid: 'g-3',
			vtn: 'm-chg-3',
			items: [
				{
					name: 'Brother Cael',
					epithet: 'Keeper of the Archive',
					house: 'The Archive',
					age: 41,
					trait: 'Letters',
					scenes: 3,
					p: [45, 'hood'],
					owner: 'Played by @tom.writes',
					initials: 'TW',
					hue: '--avatar-hue: 45',
					kind: 'pc',
					mine: null,
					vtn: 'm-ch-brother-cael',
					vtp: 'm-chp-brother-cael',
					vtt: 'm-cht-brother-cael',
					portrait: portraits['45-hood'],
					flagClass: 'm-flag m-flag--kind_done',
					flagIcon: 'check_circle',
					flagLabel: 'Approved'
				}
			],
			count: '1'
		},
		{
			name: 'Unsworn',
			hid: 'g-4',
			vtn: 'm-chg-4',
			items: [
				{
					name: 'Anwen Ashdown',
					epithet: 'Lantern-priest of the Quay',
					house: 'Unsworn',
					age: 31,
					trait: 'Faith',
					scenes: 2,
					p: [175, 'long'],
					owner: 'Played by you',
					initials: 'IB',
					hue: '--avatar-hue: 141',
					kind: 'pc',
					mine: true,
					vtn: 'm-ch-anwen-ashdown',
					vtp: 'm-chp-anwen-ashdown',
					vtt: 'm-cht-anwen-ashdown',
					portrait: portraits['175-long'],
					flagClass: 'm-flag m-flag--kind_done',
					flagIcon: 'check_circle',
					flagLabel: 'Approved'
				},
				{
					name: 'Wren of the Quay',
					epithet: 'Lantern-keeper’s apprentice',
					house: 'Unsworn',
					age: 19,
					trait: 'Shadows',
					scenes: 2,
					p: [15, 'hood'],
					owner: 'Played by @ola.codes',
					initials: 'OC',
					hue: '--avatar-hue: 190',
					kind: 'pc',
					mine: null,
					vtn: 'm-ch-wren-of-the-quay',
					vtp: 'm-chp-wren-of-the-quay',
					vtt: 'm-cht-wren-of-the-quay',
					portrait: portraits['15-hood'],
					flagClass: 'm-flag m-flag--kind_done',
					flagIcon: 'check_circle',
					flagLabel: 'Approved'
				}
			],
			count: '2'
		}
	],
	empty: false
};

export type CharactersData = typeof characters;
