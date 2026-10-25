// Demo content from the design export, as the screen first renders it.
// It stands in for AppView data until the screen loads real records.
import { portraits } from '../art.js';

export const worldManage = {
	worldRef: {
		world: 'vellum'
	},
	no: false,
	inboxCount: 4,
	sections: [
		{
			label: 'Needs you',
			icon: 'dashboard',
			count: '4',
			hasCount: true,
			current: true
		},
		{
			label: 'Moderation',
			icon: 'report',
			count: '2',
			hasCount: true,
			current: false
		},
		{
			label: 'Applications',
			icon: 'how_to_reg',
			count: '2',
			hasCount: true,
			current: false
		},
		{
			label: 'Recruiting',
			icon: 'person_add',
			count: '0',
			hasCount: false,
			current: false
		},
		{
			label: 'Members and roles',
			icon: 'groups',
			count: '0',
			hasCount: false,
			current: false
		},
		{
			label: 'About and rules',
			icon: 'gavel',
			count: '0',
			hasCount: false,
			current: false
		},
		{
			label: 'Theme',
			icon: 'palette',
			count: '0',
			hasCount: false,
			current: false
		},
		{
			label: 'Archive or hand over',
			icon: 'archive',
			count: '0',
			hasCount: false,
			current: false
		}
	],
	pane: 'content',
	is: {
		overview: true,
		moderation: false,
		applications: false,
		recruiting: false,
		members: false,
		about: false,
		theme: false,
		danger: false
	},
	tasks: [
		{
			icon: 'how_to_reg',
			kind: 'Applications',
			title: '2 waiting',
			detail: 'Tamsin Corven, Rook. Recruiting closes April 30.',
			actions: [
				{
					label: 'Review',
					cls: 'm-button m-button--variant_tonal m-button--size_sm'
				}
			]
		},
		{
			icon: 'hourglass_top',
			kind: 'Waiting on a GM',
			title: 'The Steward’s Ledger',
			detail: 'Lady Maren posted 6 days ago. Steward Hollis is next.',
			actions: [
				{
					label: 'Open scene',
					cls: 'm-button m-button--variant_text m-button--size_sm'
				}
			]
		},
		{
			icon: 'hourglass_top',
			kind: 'Waiting on a GM',
			title: 'Weirwood Pass',
			detail: 'Ser Jaume’s player is away. Move him along or pause the scene.',
			actions: [
				{
					label: 'Open scene',
					cls: 'm-button m-button--variant_text m-button--size_sm'
				}
			]
		},
		{
			icon: 'report',
			kind: 'Moderation',
			title: '2 open reports',
			detail: 'The oldest is from Mar 9, about a post in Table talk.',
			actions: [
				{
					label: 'Open queue',
					cls: 'm-button m-button--variant_tonal m-button--size_sm'
				}
			]
		}
	],
	repFilters: [
		{
			label: 'Open',
			count: '2',
			checked: true
		},
		{
			label: 'Escalated',
			count: '1',
			checked: false
		},
		{
			label: 'Closed',
			count: '2',
			checked: false
		}
	],
	reports: [
		{
			vtn: 'm-r-r1',
			icon: 'gavel',
			reason: 'Breaks the World’s rules or content lines',
			where: 'A post in Table talk',
			when: '2 days ago',
			current: true
		},
		{
			vtn: 'm-r-r2',
			icon: 'report',
			reason: 'Harassment, hate or trolling',
			where: 'A post in Weirwood Pass',
			when: 'Yesterday',
			current: false
		}
	],
	noReports: false,
	hasRep: true,
	rep: {
		id: 'r1',
		status: 'open',
		icon: 'gavel',
		reason: 'Breaks the World’s rules or content lines',
		where: 'A post in Table talk',
		when: '2 days ago',
		author: 'Ewa',
		handle: 'ewa.bsky.social',
		hue: '--avatar-hue: 330',
		initials: 'EB',
		scene: 'Table talk, out of character',
		version: 'Mar 9, 14:02',
		text: 'Pell already knows the banner was Tamsin’s doing, so when the Corvens sit down at the feast he’s going to call it out in front of the whole hall.',
		reporter: 'Kuba',
		reporterHandle: 'kuba.dev',
		rhue: '--avatar-hue: 35',
		note: 'Spoils the Corven plot for people who aren’t in it.',
		history: 'No earlier reports about @ewa.bsky.social.',
		timeline: [
			{
				icon: 'flag',
				text: 'Kuba reported it',
				when: 'Mar 9'
			}
		],
		rinitial: 'K',
		flagClass: 'm-flag m-flag--kind_caution',
		flagIcon: 'hourglass_top',
		statusLabel: 'Open',
		decided: false,
		actionable: true,
		decisionClass: '',
		decisionIcon: '',
		decisionTitle: '',
		decisionText: ''
	},
	escOpen: false,
	escExpanded: false,
	escTargets: [
		{
			label: 'Motet moderation',
			hint: 'Motet’s rules, in every World',
			icon: 'shield',
			on: true
		},
		{
			label: 'PBF Safety Collective',
			hint: 'This World’s service for harassment',
			icon: 'policy',
			on: false
		},
		{
			label: 'Another service',
			hint: 'Any service, by its handle',
			icon: 'add_moderator',
			on: false
		}
	],
	escOther: false,
	escNoteOn: true,
	escWhoOn: false,
	escSendLabel: 'Send to Motet moderation',
	escReason: 'Breaks the rules or the law',
	escNote: '',
	escCount: '0 / 2000',
	worldLang: 'en',
	alsoLangs: ['pl'],
	apps: [
		{
			name: 'Tamsin Corven',
			meta: '2 days ago',
			portrait: portraits['340-long'],
			current: true
		},
		{
			name: 'Rook',
			meta: '5 days ago',
			portrait: portraits['180-hat'],
			current: false
		}
	],
	app: {
		id: 'tamsin',
		name: 'Tamsin Corven',
		epithet: 'Heir of House Corven, for now',
		role: 'Heir of House Corven',
		house: 'Corven',
		trait: 'Silver tongue',
		pace: 'About once a week',
		owner: 'Nadia, @nadia.pds.example',
		initials: 'NP',
		hue: '--avatar-hue: 32',
		p: [340, 'long'],
		meta: '2 days ago',
		bg: 'Tamsin was raised to inherit nothing. Her brother was the heir until the river took him two springs ago, and the court has never let her forget that she is the second choice.',
		secret:
			'She tore the banner down herself, before dawn, and does not yet know why her father let her.',
		portrait: portraits['340-long']
	},
	decided: false,
	decisionClass: 'm-alert__root m-alert__root--variant_inline m-alert__root--tone_info',
	decisionIcon: 'edit_note',
	decisionTitle: 'Changes asked for',
	decisionText: 'The application goes back to Nadia with your note.',
	asking: false,
	askLabel: 'Ask for changes',
	recruitingOn: true,
	recruitingAttr: true,
	seats: '2',
	fields: [
		{
			vtn: 'm-f-name',
			label: 'Name',
			help: '',
			kind: 'Short text, built in',
			fixed: true,
			movable: false,
			private: false,
			public: true,
			required: true,
			reqLabel: 'Name is required',
			open: false,
			openAttr: false,
			chev: 'edit',
			editLabel: 'Edit Name',
			editId: 'fe-name',
			labelId: 'fl-name',
			helpId: 'fh-name',
			typeId: 'ft-name',
			visId: 'fv-name',
			limitId: 'fw-name',
			canType: false,
			types: [
				{
					label: 'Short text',
					icon: 'short_text',
					checked: true
				},
				{
					label: 'Long text',
					icon: 'notes',
					checked: false
				},
				{
					label: 'Choice',
					icon: 'radio_button_checked',
					checked: false
				},
				{
					label: 'Number',
					icon: 'numbers',
					checked: false
				},
				{
					label: 'Image',
					icon: 'image',
					checked: false
				}
			],
			isChoice: false,
			isLong: false,
			opts: [],
			multi: false,
			limit: '600',
			vis: [
				{
					label: 'On the card',
					icon: 'badge',
					checked: true
				},
				{
					label: 'GMs only',
					icon: 'lock',
					checked: false
				}
			]
		},
		{
			vtn: 'm-f-epithet',
			label: 'Epithet',
			help: 'A few words the table sees under the name',
			kind: 'Short text, built in',
			fixed: true,
			movable: false,
			private: false,
			public: true,
			required: true,
			reqLabel: 'Epithet is required',
			open: false,
			openAttr: false,
			chev: 'edit',
			editLabel: 'Edit Epithet',
			editId: 'fe-epithet',
			labelId: 'fl-epithet',
			helpId: 'fh-epithet',
			typeId: 'ft-epithet',
			visId: 'fv-epithet',
			limitId: 'fw-epithet',
			canType: false,
			types: [
				{
					label: 'Short text',
					icon: 'short_text',
					checked: true
				},
				{
					label: 'Long text',
					icon: 'notes',
					checked: false
				},
				{
					label: 'Choice',
					icon: 'radio_button_checked',
					checked: false
				},
				{
					label: 'Number',
					icon: 'numbers',
					checked: false
				},
				{
					label: 'Image',
					icon: 'image',
					checked: false
				}
			],
			isChoice: false,
			isLong: false,
			opts: [],
			multi: false,
			limit: '600',
			vis: [
				{
					label: 'On the card',
					icon: 'badge',
					checked: true
				},
				{
					label: 'GMs only',
					icon: 'lock',
					checked: false
				}
			]
		},
		{
			vtn: 'm-f-portrait',
			label: 'Portrait',
			help: '',
			kind: 'Image',
			fixed: false,
			movable: true,
			private: false,
			public: true,
			required: false,
			reqLabel: 'Portrait is required',
			open: false,
			openAttr: false,
			chev: 'edit',
			editLabel: 'Edit Portrait',
			editId: 'fe-portrait',
			labelId: 'fl-portrait',
			helpId: 'fh-portrait',
			typeId: 'ft-portrait',
			visId: 'fv-portrait',
			limitId: 'fw-portrait',
			canType: true,
			types: [
				{
					label: 'Short text',
					icon: 'short_text',
					checked: false
				},
				{
					label: 'Long text',
					icon: 'notes',
					checked: false
				},
				{
					label: 'Choice',
					icon: 'radio_button_checked',
					checked: false
				},
				{
					label: 'Number',
					icon: 'numbers',
					checked: false
				},
				{
					label: 'Image',
					icon: 'image',
					checked: true
				}
			],
			isChoice: false,
			isLong: false,
			opts: [],
			multi: false,
			limit: '600',
			vis: [
				{
					label: 'On the card',
					icon: 'badge',
					checked: true
				},
				{
					label: 'GMs only',
					icon: 'lock',
					checked: false
				}
			]
		},
		{
			vtn: 'm-f-house',
			label: 'House',
			help: '',
			kind: 'Choice: Ost, Corven, Vael, The Archive, None',
			fixed: false,
			movable: true,
			private: false,
			public: true,
			required: true,
			reqLabel: 'House is required',
			open: false,
			openAttr: false,
			chev: 'edit',
			editLabel: 'Edit House',
			editId: 'fe-house',
			labelId: 'fl-house',
			helpId: 'fh-house',
			typeId: 'ft-house',
			visId: 'fv-house',
			limitId: 'fw-house',
			canType: true,
			types: [
				{
					label: 'Short text',
					icon: 'short_text',
					checked: false
				},
				{
					label: 'Long text',
					icon: 'notes',
					checked: false
				},
				{
					label: 'Choice',
					icon: 'radio_button_checked',
					checked: true
				},
				{
					label: 'Number',
					icon: 'numbers',
					checked: false
				},
				{
					label: 'Image',
					icon: 'image',
					checked: false
				}
			],
			isChoice: true,
			isLong: false,
			opts: [
				{
					value: 'Ost',
					aria: 'Option 1',
					removeLabel: 'Remove Ost'
				},
				{
					value: 'Corven',
					aria: 'Option 2',
					removeLabel: 'Remove Corven'
				},
				{
					value: 'Vael',
					aria: 'Option 3',
					removeLabel: 'Remove Vael'
				},
				{
					value: 'The Archive',
					aria: 'Option 4',
					removeLabel: 'Remove The Archive'
				},
				{
					value: 'None',
					aria: 'Option 5',
					removeLabel: 'Remove None'
				}
			],
			multi: false,
			limit: '600',
			vis: [
				{
					label: 'On the card',
					icon: 'badge',
					checked: true
				},
				{
					label: 'GMs only',
					icon: 'lock',
					checked: false
				}
			]
		},
		{
			vtn: 'm-f-trait',
			label: 'Trait',
			help: 'Adds to d20 checks',
			kind: 'Choice: Silver tongue, Steel, Letters, Shadows, Faith',
			fixed: false,
			movable: true,
			private: false,
			public: true,
			required: true,
			reqLabel: 'Trait is required',
			open: false,
			openAttr: false,
			chev: 'edit',
			editLabel: 'Edit Trait',
			editId: 'fe-trait',
			labelId: 'fl-trait',
			helpId: 'fh-trait',
			typeId: 'ft-trait',
			visId: 'fv-trait',
			limitId: 'fw-trait',
			canType: true,
			types: [
				{
					label: 'Short text',
					icon: 'short_text',
					checked: false
				},
				{
					label: 'Long text',
					icon: 'notes',
					checked: false
				},
				{
					label: 'Choice',
					icon: 'radio_button_checked',
					checked: true
				},
				{
					label: 'Number',
					icon: 'numbers',
					checked: false
				},
				{
					label: 'Image',
					icon: 'image',
					checked: false
				}
			],
			isChoice: true,
			isLong: false,
			opts: [
				{
					value: 'Silver tongue',
					aria: 'Option 1',
					removeLabel: 'Remove Silver tongue'
				},
				{
					value: 'Steel',
					aria: 'Option 2',
					removeLabel: 'Remove Steel'
				},
				{
					value: 'Letters',
					aria: 'Option 3',
					removeLabel: 'Remove Letters'
				},
				{
					value: 'Shadows',
					aria: 'Option 4',
					removeLabel: 'Remove Shadows'
				},
				{
					value: 'Faith',
					aria: 'Option 5',
					removeLabel: 'Remove Faith'
				}
			],
			multi: false,
			limit: '600',
			vis: [
				{
					label: 'On the card',
					icon: 'badge',
					checked: true
				},
				{
					label: 'GMs only',
					icon: 'lock',
					checked: false
				}
			]
		},
		{
			vtn: 'm-f-secret',
			label: 'A secret your character keeps',
			help: '',
			kind: 'Long text, up to 200 words',
			fixed: false,
			movable: true,
			private: true,
			public: false,
			required: false,
			reqLabel: 'A secret your character keeps is required',
			open: false,
			openAttr: false,
			chev: 'edit',
			editLabel: 'Edit A secret your character keeps',
			editId: 'fe-secret',
			labelId: 'fl-secret',
			helpId: 'fh-secret',
			typeId: 'ft-secret',
			visId: 'fv-secret',
			limitId: 'fw-secret',
			canType: true,
			types: [
				{
					label: 'Short text',
					icon: 'short_text',
					checked: false
				},
				{
					label: 'Long text',
					icon: 'notes',
					checked: true
				},
				{
					label: 'Choice',
					icon: 'radio_button_checked',
					checked: false
				},
				{
					label: 'Number',
					icon: 'numbers',
					checked: false
				},
				{
					label: 'Image',
					icon: 'image',
					checked: false
				}
			],
			isChoice: false,
			isLong: true,
			opts: [],
			multi: false,
			limit: '200',
			vis: [
				{
					label: 'On the card',
					icon: 'badge',
					checked: false
				},
				{
					label: 'GMs only',
					icon: 'lock',
					checked: true
				}
			]
		},
		{
			vtn: 'm-f-bg',
			label: 'Background',
			help: '',
			kind: 'Long text, up to 600 words',
			fixed: false,
			movable: true,
			private: false,
			public: true,
			required: true,
			reqLabel: 'Background is required',
			open: false,
			openAttr: false,
			chev: 'edit',
			editLabel: 'Edit Background',
			editId: 'fe-bg',
			labelId: 'fl-bg',
			helpId: 'fh-bg',
			typeId: 'ft-bg',
			visId: 'fv-bg',
			limitId: 'fw-bg',
			canType: true,
			types: [
				{
					label: 'Short text',
					icon: 'short_text',
					checked: false
				},
				{
					label: 'Long text',
					icon: 'notes',
					checked: true
				},
				{
					label: 'Choice',
					icon: 'radio_button_checked',
					checked: false
				},
				{
					label: 'Number',
					icon: 'numbers',
					checked: false
				},
				{
					label: 'Image',
					icon: 'image',
					checked: false
				}
			],
			isChoice: false,
			isLong: true,
			opts: [],
			multi: false,
			limit: '600',
			vis: [
				{
					label: 'On the card',
					icon: 'badge',
					checked: true
				},
				{
					label: 'GMs only',
					icon: 'lock',
					checked: false
				}
			]
		}
	],
	dirty: false,
	published: false,
	addOpen: false,
	fieldTypes: [
		{
			label: 'Short text',
			icon: 'short_text'
		},
		{
			label: 'Long text',
			icon: 'notes'
		},
		{
			label: 'Choice',
			icon: 'radio_button_checked'
		},
		{
			label: 'Number',
			icon: 'numbers'
		},
		{
			label: 'Image',
			icon: 'image'
		}
	],
	members: [
		{
			id: 'aurelia',
			display: 'Aurelia',
			sub: 'Owner, you',
			initials: 'AB',
			hue: '--avatar-hue: 262',
			role: 'gm',
			owner: true,
			roleLabel: 'Role of Aurelia',
			removeLabel: 'Remove Aurelia',
			roles: [
				{
					label: 'GM',
					checked: true
				},
				{
					label: 'Player',
					checked: false
				},
				{
					label: 'Reader',
					checked: false
				}
			]
		},
		{
			id: 'bram',
			display: 'Bram',
			sub: '@bram.writes, away until Mar 27',
			initials: 'BW',
			hue: '--avatar-hue: 96',
			role: 'gm',
			owner: false,
			roleLabel: 'Role of Bram',
			removeLabel: 'Remove Bram',
			roles: [
				{
					label: 'GM',
					checked: true
				},
				{
					label: 'Player',
					checked: false
				},
				{
					label: 'Reader',
					checked: false
				}
			]
		},
		{
			id: 'iga',
			display: 'Iga',
			sub: 'Lady Maren of Ost',
			initials: 'IB',
			hue: '--avatar-hue: 141',
			role: 'player',
			owner: false,
			roleLabel: 'Role of Iga',
			removeLabel: 'Remove Iga',
			roles: [
				{
					label: 'GM',
					checked: false
				},
				{
					label: 'Player',
					checked: true
				},
				{
					label: 'Reader',
					checked: false
				}
			]
		},
		{
			id: 'nina',
			display: 'Nina',
			sub: 'Isolde Corven',
			initials: 'NM',
			hue: '--avatar-hue: 280',
			role: 'player',
			owner: false,
			roleLabel: 'Role of Nina',
			removeLabel: 'Remove Nina',
			roles: [
				{
					label: 'GM',
					checked: false
				},
				{
					label: 'Player',
					checked: true
				},
				{
					label: 'Reader',
					checked: false
				}
			]
		},
		{
			id: 'tom',
			display: 'Tom',
			sub: 'Brother Cael',
			initials: 'TW',
			hue: '--avatar-hue: 45',
			role: 'player',
			owner: false,
			roleLabel: 'Role of Tom',
			removeLabel: 'Remove Tom',
			roles: [
				{
					label: 'GM',
					checked: false
				},
				{
					label: 'Player',
					checked: true
				},
				{
					label: 'Reader',
					checked: false
				}
			]
		},
		{
			id: 'ewa',
			display: 'Ewa',
			sub: 'Lord Pell Corven, Mother Ysolde',
			initials: 'EB',
			hue: '--avatar-hue: 330',
			role: 'player',
			owner: false,
			roleLabel: 'Role of Ewa',
			removeLabel: 'Remove Ewa',
			roles: [
				{
					label: 'GM',
					checked: false
				},
				{
					label: 'Player',
					checked: true
				},
				{
					label: 'Reader',
					checked: false
				}
			]
		}
	]
};

export type WorldManageData = typeof worldManage;
