// Demo content from the design export, as the screen first renders it.
// It stands in for AppView data until the screen loads real records.
import { resolve } from '$app/paths';

export const lore = {
	worldRef: {
		world: 'vellum'
	},
	no: false,
	inboxCount: 3,
	tree: [
		{
			label: 'Places',
			icon: 'map',
			count: '4',
			open: true,
			expanded: true,
			pages: [
				{
					label: 'Harrowgate Keep',
					stub: false,
					selected: true
				},
				{
					label: 'Lantern Quay',
					stub: true,
					selected: false
				},
				{
					label: 'The Northern March',
					stub: true,
					selected: false
				},
				{
					label: 'Ost',
					stub: true,
					selected: false
				}
			]
		},
		{
			label: 'Houses',
			icon: 'flag',
			count: '3',
			open: false,
			expanded: false,
			pages: [
				{
					label: 'House Vael',
					stub: true,
					selected: false
				},
				{
					label: 'House Corven',
					stub: true,
					selected: false
				},
				{
					label: 'House Ost',
					stub: true,
					selected: false
				}
			]
		},
		{
			label: 'Faith',
			icon: 'church',
			count: '1',
			open: true,
			expanded: true,
			pages: [
				{
					label: 'The Lantern Rite',
					stub: false,
					selected: false
				}
			]
		},
		{
			label: 'History',
			icon: 'history_edu',
			count: '1',
			open: false,
			expanded: false,
			pages: [
				{
					label: 'The Ash Crown War',
					stub: true,
					selected: false
				}
			]
		},
		{
			label: 'Rules',
			icon: 'gavel',
			count: '1',
			open: true,
			expanded: true,
			pages: [
				{
					label: 'Checks and traits',
					stub: false,
					selected: false
				}
			]
		}
	],
	page: {
		title: 'Harrowgate Keep',
		group: 'Places',
		edited: 'Edited by Aurelia, Mar 6',
		vis: 'World members',
		visIcon: 'group',
		info: [
			{
				k: 'Kind',
				v: 'Fortress, ducal seat'
			},
			{
				k: 'Held by',
				v: 'House Vael'
			},
			{
				k: 'Lord',
				v: 'Duke Aldric Vael'
			},
			{
				k: 'Stands over',
				v: 'Lantern Quay, on the Ferrow'
			},
			{
				k: 'Garrison',
				v: 'Sixty, under Ser Jaume Tallow'
			}
		],
		toc: [
			{
				label: 'The great hall',
				href: null
			},
			{
				label: 'The Archive',
				href: null
			},
			{
				label: 'Who lives there',
				href: null
			}
		],
		mentions: [
			{
				title: 'The Feast of Lanterns',
				href: resolve('/w/[world]/play/[scene]', {
					world: 'gilded-reach',
					scene: 'feast-of-lanterns'
				})
			},
			{
				title: 'Whispers in the Archive',
				href: null
			},
			{
				title: 'The Steward’s Ledger',
				href: null
			}
		]
	},
	is: {
		keep: true,
		rite: false,
		checks: false,
		stub: false
	},
	hasInfo: true,
	hasToc: true,
	hasMentions: true,
	spoilerOpen: false,
	spoilerAction: 'Show'
};

export type LoreData = typeof lore;
