// Demo content from the design export, as the screen first renders it.
// It stands in for AppView data until the screen loads real records.
import { portraits } from '../art.js';

export const worldSettings = {
	worldRef: {
		world: 'vellum'
	},
	no: false,
	inboxCount: 3,
	portrait: portraits['260-long'],
	away: false,
	awayAttr: false,
	yes: true,
	myChars: [
		{
			value: 'maren',
			label: 'Lady Maren of Ost',
			short: 'Lady Maren',
			portrait: portraits['260-long']
		},
		{
			value: 'anwen',
			label: 'Anwen Ashdown',
			short: 'Anwen',
			portrait: portraits['175-long']
		}
	],
	moveChars: ['maren'],
	extraOpts: [
		{
			value: 'scenes',
			label: 'New scenes',
			short: 'new scenes',
			icon: 'forum'
		},
		{
			value: 'ann',
			label: 'GM announcements',
			short: 'GM announcements',
			icon: 'campaign'
		},
		{
			value: 'lore',
			label: 'Lore changes',
			short: 'lore changes',
			icon: 'menu_book'
		},
		{
			value: 'members',
			label: 'New members',
			short: 'new members',
			icon: 'group_add'
		}
	],
	extraVals: ['scenes', 'ann'],
	myCards: [
		{
			label: 'Lady Maren of Ost',
			stats: '4 scenes, 38 posts',
			portrait: portraits['260-long']
		},
		{
			label: 'Anwen Ashdown',
			stats: '2 scenes, 11 posts',
			portrait: portraits['175-long']
		}
	],
	levels: [
		{
			label: 'All posts',
			checked: true
		},
		{
			label: 'Turns and mentions',
			checked: false
		},
		{
			label: 'Off',
			checked: false
		}
	],
	extras: [
		{
			label: 'New scenes',
			on: true
		},
		{
			label: 'GM announcements',
			on: true
		},
		{
			label: 'Lore changes',
			on: false
		}
	],
	overrides: [
		{
			id: 'whispers',
			scene: 'Whispers in the Archive',
			level: 'Muted',
			icon: 'notifications_off'
		},
		{
			id: 'tabletalk',
			scene: 'Table talk',
			level: 'Your turn and mentions',
			icon: 'notifications'
		}
	],
	noOverrides: false,
	modes: [
		{
			label: 'World',
			icon: 'routine',
			checked: true
		},
		{
			label: 'System',
			icon: 'contrast',
			checked: false
		},
		{
			label: 'Light',
			icon: 'light_mode',
			checked: false
		},
		{
			label: 'Dark',
			icon: 'dark_mode',
			checked: false
		}
	],
	plainAttr: false,
	decoAttr: true
};

export type WorldSettingsData = typeof worldSettings;
