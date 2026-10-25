// Demo content from the design export, as the screen first renders it.
// It stands in for AppView data until the screen loads real records.
import { portraits } from '../art.js';

export const members = {
	worldRef: {
		world: 'vellum'
	},
	no: false,
	inboxCount: 3,
	query: '',
	filters: [
		{
			label: 'Everyone',
			icon: 'groups',
			count: '9',
			pressed: true
		},
		{
			label: 'GMs',
			icon: 'shield_person',
			count: '2',
			pressed: false
		},
		{
			label: 'Players',
			icon: 'person',
			count: '7',
			pressed: false
		},
		{
			label: 'Away',
			icon: 'bedtime',
			count: '2',
			pressed: false
		}
	],
	rows: [
		{
			display: 'Aurelia',
			handle: 'aurelia.bsky.social',
			initials: 'AB',
			hue: '--avatar-hue: 262',
			role: 'GM',
			chars: [
				{
					name: 'Steward Hollis',
					portrait: portraits['20-short']
				}
			],
			last: '2 hours ago',
			vtn: 'm-mb-aurelia-bsky-social',
			roleIcon: 'shield_person',
			you: false,
			away: false,
			awayText: ''
		},
		{
			display: 'Bram',
			handle: 'bram.writes',
			initials: 'BW',
			hue: '--avatar-hue: 96',
			role: 'GM',
			chars: [
				{
					name: 'Duke Aldric Vael',
					portrait: portraits['45-hat']
				}
			],
			last: 'Mar 9',
			vtn: 'm-mb-bram-writes',
			roleIcon: 'shield_person',
			you: false,
			away: true,
			awayText: 'Away until Mar 27'
		},
		{
			display: 'Iga',
			handle: 'iga.bsky.social',
			initials: 'IB',
			hue: '--avatar-hue: 141',
			role: 'Player',
			chars: [
				{
					name: 'Lady Maren of Ost',
					portrait: portraits['260-long']
				},
				{
					name: 'Anwen Ashdown',
					portrait: portraits['175-long']
				}
			],
			last: '2 hours ago',
			vtn: 'm-mb-iga-bsky-social',
			roleIcon: 'person',
			you: true,
			away: false,
			awayText: ''
		},
		{
			display: 'Nina',
			handle: 'nina.motet.at',
			initials: 'NM',
			hue: '--avatar-hue: 280',
			role: 'Player',
			chars: [
				{
					name: 'Isolde Corven',
					portrait: portraits['330-long']
				}
			],
			last: '6 hours ago',
			vtn: 'm-mb-nina-motet-at',
			roleIcon: 'person',
			you: false,
			away: false,
			awayText: ''
		},
		{
			display: 'Tom',
			handle: 'tom.writes',
			initials: 'TW',
			hue: '--avatar-hue: 45',
			role: 'Player',
			chars: [
				{
					name: 'Brother Cael',
					portrait: portraits['45-hood']
				}
			],
			last: 'Yesterday',
			vtn: 'm-mb-tom-writes',
			roleIcon: 'person',
			you: false,
			away: false,
			awayText: ''
		},
		{
			display: 'Kuba',
			handle: 'kuba.dev',
			initials: 'KD',
			hue: '--avatar-hue: 35',
			role: 'Player',
			chars: [
				{
					name: 'Sir Edric Corven',
					portrait: portraits['210-short']
				}
			],
			last: 'Yesterday',
			vtn: 'm-mb-kuba-dev',
			roleIcon: 'person',
			you: false,
			away: false,
			awayText: ''
		},
		{
			display: 'Ola',
			handle: 'ola.codes',
			initials: 'OC',
			hue: '--avatar-hue: 190',
			role: 'Player',
			chars: [
				{
					name: 'Wren of the Quay',
					portrait: portraits['15-hood']
				}
			],
			last: 'Yesterday',
			vtn: 'm-mb-ola-codes',
			roleIcon: 'person',
			you: false,
			away: false,
			awayText: ''
		},
		{
			display: 'Marek',
			handle: 'marek.pds.example',
			initials: 'MP',
			hue: '--avatar-hue: 0',
			role: 'Player',
			chars: [
				{
					name: 'Ser Jaume Tallow',
					portrait: portraits['120-hat']
				}
			],
			last: 'Mar 5',
			vtn: 'm-mb-marek-pds-example',
			roleIcon: 'person',
			you: false,
			away: true,
			awayText: 'Away until Mar 18'
		},
		{
			display: 'Ewa',
			handle: 'ewa.bsky.social',
			initials: 'EB',
			hue: '--avatar-hue: 330',
			role: 'Player',
			chars: [
				{
					name: 'Lord Pell Corven',
					portrait: portraits['15-hat']
				},
				{
					name: 'Mother Ysolde',
					portrait: portraits['300-hood']
				}
			],
			last: 'Mar 7',
			vtn: 'm-mb-ewa-bsky-social',
			roleIcon: 'person',
			you: false,
			away: false,
			awayText: ''
		}
	]
};

export type MembersData = typeof members;
