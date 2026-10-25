// Demo content from the design export, as the screen first renders it.
// It stands in for AppView data until the screen loads real records.
import { resolve } from '$app/paths';
import { marks, portraits } from '../art.js';

export const homeShelves = {
	pageRef: {
		world: 'motet'
	},
	inboxCount: 3,
	yours: [
		{
			key: 'vellum',
			name: 'The Gilded Reach',
			as: 'Lady Maren and Anwen',
			asIcon: 'person',
			href: resolve('/w/[world]', { world: 'gilded-reach' }),
			flag: 'turn',
			ref: {
				world: 'vellum'
			},
			mark: marks.vellum,
			hasFlag: true,
			flagClass: 'm-flag m-flag--kind_turn',
			flagIcon: 'front_hand',
			flagLabel: 'Your turn',
			hasNote: false,
			note: null,
			noteIcon: null
		},
		{
			key: 'kepler',
			name: 'Kepler Relay',
			as: 'Ensign Petra Lind',
			asIcon: 'person',
			href: null,
			flag: null,
			ref: {
				world: 'kepler'
			},
			mark: marks.kepler,
			hasFlag: false,
			flagClass: '',
			flagIcon: '',
			flagLabel: '',
			hasNote: true,
			note: '2 new posts',
			noteIcon: 'mark_chat_unread'
		},
		{
			key: 'saltroad',
			name: 'The Salt Road',
			as: 'Marit Sand',
			asIcon: 'person',
			href: null,
			flag: 'pending',
			ref: {
				world: 'saltroad'
			},
			mark: marks.saltroad,
			hasFlag: true,
			flagClass: 'm-flag m-flag--kind_caution',
			flagIcon: 'hourglass_top',
			flagLabel: 'Application sent',
			hasNote: false,
			note: null,
			noteIcon: null
		},
		{
			key: 'ledger',
			name: 'Cold Front',
			as: 'Watching',
			asIcon: 'visibility',
			href: null,
			flag: null,
			ref: {
				world: 'ledger'
			},
			mark: marks.ledger,
			hasFlag: false,
			flagClass: '',
			flagIcon: '',
			flagLabel: '',
			hasNote: true,
			note: 'Turn 14 of 40',
			noteIcon: 'event'
		}
	],
	marks: {
		vellum: marks.vellum,
		kepler: marks.kepler
	},
	portraits: {
		maren: portraits['260-long'],
		cael: portraits['45-hood'],
		petra: portraits['150-short'],
		oyelaran: portraits['70-short'],
		horvat: portraits['30-visor']
	},
	genres: [
		{
			label: 'Fantasy',
			on: false,
			pressed: false
		},
		{
			label: 'Sci-fi',
			on: false,
			pressed: false
		},
		{
			label: 'Horror',
			on: false,
			pressed: false
		},
		{
			label: 'Mystery',
			on: false,
			pressed: false
		},
		{
			label: 'Post-apocalyptic',
			on: false,
			pressed: false
		},
		{
			label: 'Weird West',
			on: false,
			pressed: false
		}
	],
	yes: true,
	langCodes: ['en', 'pl'],
	recruitingPressed: false,
	shelves: [
		{
			title: 'Recruiting now',
			icon: 'how_to_reg',
			items: [
				{
					key: 'ashfall',
					name: 'Ashfall',
					shelf: 'recruiting',
					genre: ['Post-apocalyptic'],
					tags: ['Post-apocalyptic', 'Survival'],
					lang: 'Polski',
					pace: 'Twice a week',
					state: 'Recruiting',
					stateIcon: 'how_to_reg',
					recruiting: true,
					gms: 'Rdza',
					pitch: 'Osada przetrwała trzecią zimę. Teraz ktoś musi pójść za góry.',
					ref: {
						world: 'ashfall'
					},
					mark: marks.ashfall,
					href: null,
					hasPace: true,
					hasPeople: false,
					noPeople: true,
					people: [],
					peopleText: ''
				},
				{
					key: 'oskad',
					name: 'Oskad',
					shelf: 'recruiting',
					genre: ['Weird West', 'Mystery'],
					tags: ['Weird West', 'Mystery'],
					lang: 'Polski',
					pace: 'Weekly',
					state: 'Recruiting',
					stateIcon: 'how_to_reg',
					recruiting: true,
					gms: 'Telegrafista',
					pitch: 'Miasteczko przy torach, telegraf i coś, co mieszka w kanionach.',
					ref: {
						world: 'oskad'
					},
					mark: marks.oskad,
					href: null,
					hasPace: true,
					hasPeople: false,
					noPeople: true,
					people: [],
					peopleText: ''
				},
				{
					key: 'elarid',
					name: 'Elarid',
					shelf: 'recruiting',
					genre: ['Fantasy'],
					tags: ['Fantasy', 'Adventure'],
					lang: 'Polski',
					pace: 'Weekly',
					state: 'Recruiting',
					stateIcon: 'how_to_reg',
					recruiting: true,
					gms: 'Kronikarz',
					pitch: 'Kontynent po Kataklizmie. Magia wraca, a z nią stare wojny.',
					ref: {
						world: 'elarid'
					},
					mark: marks.elarid,
					href: null,
					hasPace: true,
					hasPeople: false,
					noPeople: true,
					people: [],
					peopleText: ''
				}
			]
		},
		{
			title: 'Played by people you know',
			icon: 'group',
			items: [
				{
					key: 'neon',
					name: 'Neon Meridian',
					shelf: 'people',
					genre: ['Sci-fi'],
					tags: ['Cyberpunk', 'Heist'],
					lang: 'English',
					pace: 'Twice a week',
					state: 'Full, waitlist open',
					stateIcon: 'group',
					recruiting: false,
					gms: 'vex, Ada R.',
					pitch: 'A city that sells memories by the gram. Your crew owes the wrong people.',
					ref: {
						world: 'neon'
					},
					mark: marks.neon,
					href: null,
					hasPace: true,
					hasPeople: true,
					noPeople: false,
					people: [
						{
							initial: 'O',
							hue: '--avatar-hue: 190'
						},
						{
							initial: 'P',
							hue: '--avatar-hue: 330'
						}
					],
					peopleText: 'Ola and Paweł play here'
				},
				{
					key: 'hollow',
					name: 'Hollowmere',
					shelf: 'people',
					genre: ['Horror', 'Mystery'],
					tags: ['Horror', 'Mystery'],
					lang: 'Polski',
					pace: 'Weekly',
					state: 'Invitation only',
					stateIcon: 'lock',
					recruiting: false,
					gms: 'Wrona',
					pitch: 'Parafia, w której nikt nie umiera do końca.',
					ref: {
						world: 'hollow'
					},
					mark: marks.hollow,
					href: null,
					hasPace: true,
					hasPeople: true,
					noPeople: false,
					people: [
						{
							initial: 'N',
							hue: '--avatar-hue: 280'
						}
					],
					peopleText: 'Nina plays here'
				}
			]
		},
		{
			title: 'From the archives',
			icon: 'inventory_2',
			items: [
				{
					key: 'mglawica',
					name: 'Mgławica Mocy',
					shelf: 'archive',
					genre: ['Sci-fi'],
					tags: ['Space opera'],
					lang: 'Polski',
					pace: '',
					state: 'Archive',
					stateIcon: 'inventory_2',
					recruiting: false,
					gms: 'Mistrz Gry',
					pitch: 'Kosmiczna opera. Całe archiwum do przeczytania.',
					ref: {
						world: 'mglawica'
					},
					mark: marks.mglawica,
					href: null,
					hasPace: false,
					hasPeople: false,
					noPeople: true,
					people: [],
					peopleText: ''
				}
			]
		}
	],
	nothing: false
};

export type HomeShelvesData = typeof homeShelves;
