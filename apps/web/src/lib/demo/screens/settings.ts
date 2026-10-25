// Demo content from the design export, as the screen first renders it.
// It stands in for AppView data until the screen loads real records.
import { marks, portraits } from '../art.js';

export const settings = {
	pageRef: {
		world: 'motet'
	},
	previewRef: {
		world: 'vellum'
	},
	inboxCount: 3,
	pane: 'nav',
	sections: [
		{
			label: 'Account',
			icon: 'account_circle',
			current: false
		},
		{
			label: 'Profile',
			icon: 'person',
			current: false
		},
		{
			label: 'Language and time',
			icon: 'language',
			current: false
		},
		{
			label: 'Reading and appearance',
			icon: 'format_size',
			current: true
		},
		{
			label: 'Notifications',
			icon: 'notifications',
			current: false
		},
		{
			label: 'Writing',
			icon: 'edit_note',
			current: false
		},
		{
			label: 'Privacy',
			icon: 'lock',
			current: false
		},
		{
			label: 'Moderation and safety',
			icon: 'shield',
			current: false
		},
		{
			label: 'Your data',
			icon: 'folder_open',
			current: false
		}
	],
	title: 'Reading and appearance',
	is: {
		account: false,
		profile: false,
		language: false,
		reading: true,
		notifications: false,
		writing: false,
		privacy: false,
		moderation: false,
		data: false
	},
	no: false,
	uiLangs: ['en', 'pl'],
	uiLang: 'en',
	readLangs: ['en', 'pl'],
	dateFormats: [
		{
			label: '31.03.2026',
			icon: null,
			checked: true
		},
		{
			label: 'Mar 31, 2026',
			icon: null,
			checked: false
		},
		{
			label: '2026-03-31',
			icon: null,
			checked: false
		}
	],
	services: [
		{
			name: 'Motet moderation',
			handle: 'moderation.motet.at',
			icon: 'shield',
			hue: '--avatar-hue: 205',
			labels: 'Spam, impersonation, illegal content',
			locked: true,
			optional: false,
			on: true,
			switchLabel: null
		},
		{
			name: 'PBF Safety Collective',
			handle: 'safety.pbf.example',
			icon: 'policy',
			hue: '--avatar-hue: 150',
			labels: 'Harassment, hate, graphic content',
			locked: false,
			optional: true,
			on: true,
			switchLabel: 'Use PBF Safety Collective'
		}
	],
	svcDraft: '',
	labelRows: [
		{
			label: 'Graphic violence',
			source: 'PBF Safety Collective',
			locked: false,
			open: true,
			options: [
				{
					label: 'Show',
					checked: false
				},
				{
					label: 'Warn',
					checked: true
				},
				{
					label: 'Hide',
					checked: false
				}
			]
		},
		{
			label: 'Gore',
			source: 'PBF Safety Collective',
			locked: false,
			open: true,
			options: [
				{
					label: 'Show',
					checked: false
				},
				{
					label: 'Warn',
					checked: false
				},
				{
					label: 'Hide',
					checked: true
				}
			]
		},
		{
			label: 'Hate symbols',
			source: 'PBF Safety Collective',
			locked: false,
			open: true,
			options: [
				{
					label: 'Show',
					checked: false
				},
				{
					label: 'Warn',
					checked: false
				},
				{
					label: 'Hide',
					checked: true
				}
			]
		},
		{
			label: 'Spam',
			source: 'Motet moderation',
			locked: true,
			open: false,
			options: [
				{
					label: 'Show',
					checked: false
				},
				{
					label: 'Warn',
					checked: false
				},
				{
					label: 'Hide',
					checked: true
				}
			]
		}
	],
	myReports: [
		{
			what: 'A post by Horvat in Silent Hours',
			to: 'PBF Safety Collective',
			toIcon: 'policy',
			when: '2 days ago',
			status: 'Open',
			flagClass: 'm-flag m-flag--kind_draft',
			flagIcon: 'hourglass_top'
		},
		{
			what: 'A post in Table talk',
			to: 'The Gilded Reach GMs',
			toIcon: 'shield_person',
			when: 'Mar 9',
			status: 'Post hidden',
			flagClass: 'm-flag m-flag--kind_done',
			flagIcon: 'check_circle'
		},
		{
			what: '@spamlord.pds.example',
			to: 'Motet moderation',
			toIcon: 'shield',
			when: 'Feb 2',
			status: 'Account suspended on Motet',
			flagClass: 'm-flag m-flag--kind_done',
			flagIcon: 'check_circle'
		}
	],
	sw: {
		listWorlds: true,
		motion: false,
		transparency: false,
		decoration: true,
		plain: false,
		contrast: false,
		gm: true,
		email: true,
		push: true,
		quiet: true,
		askDraft: true,
		words: true,
		cw: false,
		showWatch: false,
		safety: true
	},
	flip: {},
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
	faces: [
		{
			label: 'World’s choice',
			icon: null,
			checked: true
		},
		{
			label: 'Serif',
			icon: null,
			checked: false
		},
		{
			label: 'Sans',
			icon: null,
			checked: false
		},
		{
			label: 'System',
			icon: null,
			checked: false
		}
	],
	densities: [
		{
			label: 'Comfortable',
			icon: null,
			checked: false
		},
		{
			label: 'Standard',
			icon: null,
			checked: true
		},
		{
			label: 'Compact',
			icon: null,
			checked: false
		}
	],
	scale: 100,
	scaleText: '100%',
	scaleFill: '--fill: 23.076923076923077%',
	readingSwitches: [
		{
			label: 'World decoration',
			hint: 'Ornaments, art and textures',
			on: true
		},
		{
			label: 'Use Motet appearance in every World',
			hint: 'Worlds keep their names and marks',
			on: false
		},
		{
			label: 'High contrast',
			hint: 'Text at 7:1 in every World',
			on: false
		},
		{
			label: 'Reduce motion',
			hint: 'Changes happen in place: no sliding, morphing or parallax',
			on: false
		},
		{
			label: 'Reduce transparency',
			hint: 'Solid chrome and menus',
			on: false
		}
	],
	portraits: {
		cael: portraits['45-hood']
	},
	roleRows: [
		{
			label: 'Scenes you play in',
			hint: 'Your turn and mentions always reach you',
			icon: 'theater_comedy',
			options: [
				{
					label: 'All posts',
					checked: true
				},
				{
					label: 'Turns only',
					checked: false
				}
			]
		},
		{
			label: 'Scenes and Worlds you watch',
			hint: 'New posts while you are away',
			icon: 'visibility',
			options: [
				{
					label: 'All posts',
					checked: false
				},
				{
					label: 'Daily digest',
					checked: true
				},
				{
					label: 'Off',
					checked: false
				}
			]
		},
		{
			label: 'GM announcements',
			hint: 'In Worlds you belong to',
			icon: 'campaign',
			options: [
				{
					label: 'On',
					checked: true
				},
				{
					label: 'Off',
					checked: false
				}
			]
		}
	],
	emailOn: true,
	quietOn: true,
	digests: [
		{
			label: 'Right away',
			icon: null,
			checked: false
		},
		{
			label: 'Daily',
			icon: null,
			checked: true
		},
		{
			label: 'Weekly',
			icon: null,
			checked: false
		}
	],
	marks: {
		vellum: marks.vellum,
		kepler: marks.kepler
	},
	draftLocal: false,
	draftSync: true,
	editors: [
		{
			label: 'Formatted',
			icon: null,
			checked: true
		},
		{
			label: 'Markup',
			icon: null,
			checked: false
		}
	]
};

export type SettingsData = typeof settings;
