// Demo content from the design export, as the screen first renders it.
// It stands in for AppView data until the screen loads real records.
import { resolve } from '$app/paths';
import { marks, portraits } from '../art.js';

// Flags such as Muted appear after the reader mutes or blocks the account.
const noStates: { icon: string; label: string }[] = [];

export const profiles = {
	aurelia: {
		pageRef: {
			world: 'motet'
		},
		inboxCount: 3,
		own: false,
		other: true,
		person: {
			display: 'Aurelia',
			handle: 'aurelia.bsky.social',
			initials: 'AB',
			hue: 262,
			pronouns: 'she/her',
			langs: 'English, Polski',
			since: 'On Motet since September 2025',
			host: 'bsky.social',
			bio: 'I run slow court games where everyone is lying a little. Weekly posts, long ones welcome. Ask me about the Lantern Rite.',
			shared: [
				['vellum', 'The Gilded Reach', resolve('/w/[world]/play', { world: 'gilded-reach' })]
			],
			characters: [
				{
					name: 'Mother Agathe',
					kind: 'Character',
					p: [300, 'hood'],
					cards: [['saltroad', 'The Salt Road', 'Ferry chapel, 9 scenes', 'Approved', 'done']]
				},
				{
					name: 'Wen Liang',
					kind: 'Character',
					p: [150, 'short'],
					cards: [['kepler', 'Kepler Relay', 'Retired after cycle 2', 'Retired', 'draft']]
				},
				{
					name: 'Steward Hollis',
					kind: 'GM character',
					p: [20, 'short'],
					cards: [['vellum', 'The Gilded Reach', 'Voice of the Duke’s household', 'GM', 'gm']]
				}
			],
			worlds: [
				[
					'Runs',
					[
						[
							'vellum',
							'The Gilded Reach',
							'GM, owner',
							'shield_person',
							'Recruiting',
							'how_to_reg',
							'Since Sep 2025',
							resolve('/w/[world]/manage', { world: 'gilded-reach' })
						],
						[
							'ashfall',
							'The Ember Court',
							'GM',
							'shield_person',
							'Completed',
							'inventory_2',
							'2024 to 2025',
							'#ember'
						]
					]
				],
				[
					'Plays in',
					[
						[
							'saltroad',
							'The Salt Road',
							'Mother Agathe',
							'person',
							'Active',
							'forum',
							'Since Jan',
							'#saltroad'
						],
						[
							'kepler',
							'Kepler Relay',
							'Wen Liang, retired',
							'person',
							'Active',
							'forum',
							'Cycle 1 to 2',
							'#kepler'
						]
					]
				]
			],
			posts: [
				{
					speaker: 'Steward Hollis',
					lane: 'In character',
					p: [20, 'short'],
					text: '“My lady of Ost.” The steward bowed exactly as low as courtesy required, and not a hair lower. “His Grace asks whether the North will light a lantern tonight.”',
					world: 'vellum',
					worldName: 'The Gilded Reach',
					scene: 'The Feast of Lanterns',
					when: '2 hours ago',
					href: resolve('/w/[world]/play/[scene]', {
						world: 'gilded-reach',
						scene: 'feast-of-lanterns'
					})
				},
				{
					speaker: 'Aurelia',
					lane: 'Out of character',
					p: null,
					text: 'Round three: Maren, then Cael, then me. No rush before Sunday.',
					world: 'vellum',
					worldName: 'The Gilded Reach',
					scene: 'The Feast of Lanterns',
					when: '2 hours ago',
					href: resolve('/w/[world]/play/[scene]', {
						world: 'gilded-reach',
						scene: 'feast-of-lanterns'
					})
				},
				{
					speaker: 'Mother Agathe',
					lane: 'In character',
					p: [300, 'hood'],
					text: 'The ferry bell rang twice, which meant either a corpse or a bishop. Agathe put the kettle on for both.',
					world: 'saltroad',
					worldName: 'The Salt Road',
					scene: 'Thaw at the Ferry',
					when: 'Mar 7',
					href: null
				}
			],
			hueStyle: '--avatar-hue: 262',
			hasPronouns: true
		},
		hasShared: true,
		shared: [
			{
				mark: marks.vellum,
				name: 'The Gilded Reach',
				href: resolve('/w/[world]/play', { world: 'gilded-reach' })
			}
		],
		hasState: false,
		states: noStates,
		tabs: [
			{
				label: 'Characters',
				icon: 'theater_comedy',
				count: '3',
				current: 'page' as const
			},
			{
				label: 'Worlds',
				icon: 'public',
				count: '4',
				current: false
			},
			{
				label: 'Public posts',
				icon: 'forum',
				count: '',
				current: false
			}
		],
		is: {
			characters: true,
			worlds: false,
			posts: false
		},
		characters: [
			{
				name: 'Mother Agathe',
				kind: 'Character',
				hasKind: false,
				portrait: portraits['300-hood'],
				cards: [
					{
						mark: marks.saltroad,
						world: 'The Salt Road',
						note: 'Ferry chapel, 9 scenes',
						flag: 'Approved',
						flagClass: 'm-flag m-flag--kind_done',
						flagIcon: 'check_circle',
						href: null
					}
				]
			},
			{
				name: 'Wen Liang',
				kind: 'Character',
				hasKind: false,
				portrait: portraits['150-short'],
				cards: [
					{
						mark: marks.kepler,
						world: 'Kepler Relay',
						note: 'Retired after cycle 2',
						flag: 'Retired',
						flagClass: 'm-flag m-flag--kind_draft',
						flagIcon: 'archive',
						href: null
					}
				]
			},
			{
				name: 'Steward Hollis',
				kind: 'GM character',
				hasKind: true,
				portrait: portraits['20-short'],
				cards: [
					{
						mark: marks.vellum,
						world: 'The Gilded Reach',
						note: 'Voice of the Duke’s household',
						flag: 'GM',
						flagClass: 'm-flag m-flag--kind_draft',
						flagIcon: 'shield_person',
						href: resolve('/w/[world]/characters/[character]', {
							world: 'gilded-reach',
							character: 'lady-maren'
						})
					}
				]
			}
		],
		worldGroups: [
			{
				title: 'Runs',
				items: [
					{
						mark: marks.vellum,
						name: 'The Gilded Reach',
						role: 'GM, owner',
						roleIcon: 'shield_person',
						state: 'Recruiting',
						stateIcon: 'how_to_reg',
						since: 'Since Sep 2025',
						href: resolve('/w/[world]/manage', { world: 'gilded-reach' })
					},
					{
						mark: marks.ashfall,
						name: 'The Ember Court',
						role: 'GM',
						roleIcon: 'shield_person',
						state: 'Completed',
						stateIcon: 'inventory_2',
						since: '2024 to 2025',
						href: null
					}
				]
			},
			{
				title: 'Plays in',
				items: [
					{
						mark: marks.saltroad,
						name: 'The Salt Road',
						role: 'Mother Agathe',
						roleIcon: 'person',
						state: 'Active',
						stateIcon: 'forum',
						since: 'Since Jan',
						href: null
					},
					{
						mark: marks.kepler,
						name: 'Kepler Relay',
						role: 'Wen Liang, retired',
						roleIcon: 'person',
						state: 'Active',
						stateIcon: 'forum',
						since: 'Cycle 1 to 2',
						href: null
					}
				]
			}
		],
		posts: [
			{
				speaker: 'Steward Hollis',
				laneLabel: 'In character',
				text: '“My lady of Ost.” The steward bowed exactly as low as courtesy required, and not a hair lower. “His Grace asks whether the North will light a lantern tonight.”',
				mark: marks.vellum,
				world: 'The Gilded Reach',
				scene: 'The Feast of Lanterns',
				when: '2 hours ago',
				href: resolve('/w/[world]/play/[scene]', {
					world: 'gilded-reach',
					scene: 'feast-of-lanterns'
				}),
				hasPortrait: true,
				noPortrait: false,
				portrait: portraits['20-short'],
				initials: 'AB',
				avatarClass: 'm-avatar m-avatar--kind_character m-avatar--size_md',
				hue: '--avatar-hue: 262'
			},
			{
				speaker: 'Aurelia',
				laneLabel: 'Out of character',
				text: 'Round three: Maren, then Cael, then me. No rush before Sunday.',
				mark: marks.vellum,
				world: 'The Gilded Reach',
				scene: 'The Feast of Lanterns',
				when: '2 hours ago',
				href: resolve('/w/[world]/play/[scene]', {
					world: 'gilded-reach',
					scene: 'feast-of-lanterns'
				}),
				hasPortrait: false,
				noPortrait: true,
				portrait: '',
				initials: 'AB',
				avatarClass: 'm-avatar m-avatar--kind_account m-avatar--size_md',
				hue: '--avatar-hue: 262'
			},
			{
				speaker: 'Mother Agathe',
				laneLabel: 'In character',
				text: 'The ferry bell rang twice, which meant either a corpse or a bishop. Agathe put the kettle on for both.',
				mark: marks.saltroad,
				world: 'The Salt Road',
				scene: 'Thaw at the Ferry',
				when: 'Mar 7',
				href: null,
				hasPortrait: true,
				noPortrait: false,
				portrait: portraits['300-hood'],
				initials: 'AB',
				avatarClass: 'm-avatar m-avatar--kind_character m-avatar--size_md',
				hue: '--avatar-hue: 262'
			}
		],
		menuShown: false,
		menuOpen: false,
		muteLabel: 'Mute',
		blockLabel: 'Block',
		reporting: false,
		hasToast: false,
		toastText: '',
		toastIcon: 'check_circle'
	},
	iga: {
		pageRef: {
			world: 'motet'
		},
		inboxCount: 3,
		own: true,
		other: false,
		person: {
			display: 'Iga',
			handle: 'iga.bsky.social',
			initials: 'IB',
			hue: 141,
			pronouns: 'she/her',
			langs: 'Polski, English',
			since: 'On Motet since September 2025',
			host: 'bsky.social',
			bio: 'Court intrigue, slow burns, letters that arrive too late. Posting weekly, usually on Sundays.',
			shared: [],
			characters: [
				{
					name: 'Lady Maren of Ost',
					kind: 'Character',
					p: [260, 'long'],
					cards: [['vellum', 'The Gilded Reach', 'Envoy of the Northern March', 'Approved', 'done']]
				},
				{
					name: 'Anwen Ashdown',
					kind: 'Character',
					p: [175, 'long'],
					cards: [['vellum', 'The Gilded Reach', 'Lantern-priest of the Quay', 'Approved', 'done']]
				},
				{
					name: 'Ensign Petra Lind',
					kind: 'Character',
					p: [150, 'short'],
					cards: [['kepler', 'Kepler Relay', 'Comms deck, ring B', 'Approved', 'done']]
				},
				{
					name: 'Marit Sand',
					kind: 'Character',
					p: [200, 'short'],
					cards: [
						['saltroad', 'The Salt Road', 'Application, only you see this', 'In review', 'caution']
					]
				}
			],
			worlds: [
				[
					'Plays in',
					[
						[
							'vellum',
							'The Gilded Reach',
							'Lady Maren, Anwen',
							'person',
							'Recruiting',
							'how_to_reg',
							'Since Sep 2025',
							resolve('/w/[world]/play', { world: 'gilded-reach' })
						],
						[
							'kepler',
							'Kepler Relay',
							'Ensign Petra Lind',
							'person',
							'Active',
							'forum',
							'Since Feb',
							'#kepler'
						]
					]
				],
				[
					'Applied to, only you see this',
					[
						[
							'saltroad',
							'The Salt Road',
							'Marit Sand',
							'person',
							'Changes asked for',
							'edit_note',
							'Mar 4',
							'#saltroad'
						]
					]
				]
			],
			posts: [
				{
					speaker: 'Lady Maren of Ost',
					lane: 'In character',
					p: [260, 'long'],
					text: 'Lanterns hung from every beam of the great hall, a hundred small suns swaying in the draught from the high windows.',
					world: 'vellum',
					worldName: 'The Gilded Reach',
					scene: 'The Feast of Lanterns',
					when: 'Mar 3',
					href: resolve('/w/[world]/play/[scene]', {
						world: 'gilded-reach',
						scene: 'feast-of-lanterns'
					})
				}
			],
			hueStyle: '--avatar-hue: 141',
			hasPronouns: true
		},
		hasShared: false,
		shared: [],
		hasState: false,
		states: noStates,
		tabs: [
			{
				label: 'Characters',
				icon: 'theater_comedy',
				count: '4',
				current: 'page' as const
			},
			{
				label: 'Worlds',
				icon: 'public',
				count: '3',
				current: false
			},
			{
				label: 'Public posts',
				icon: 'forum',
				count: '',
				current: false
			}
		],
		is: {
			characters: true,
			worlds: false,
			posts: false
		},
		characters: [
			{
				name: 'Lady Maren of Ost',
				kind: 'Character',
				hasKind: false,
				portrait: portraits['260-long'],
				cards: [
					{
						mark: marks.vellum,
						world: 'The Gilded Reach',
						note: 'Envoy of the Northern March',
						flag: 'Approved',
						flagClass: 'm-flag m-flag--kind_done',
						flagIcon: 'check_circle',
						href: resolve('/w/[world]/characters/[character]', {
							world: 'gilded-reach',
							character: 'lady-maren'
						})
					}
				]
			},
			{
				name: 'Anwen Ashdown',
				kind: 'Character',
				hasKind: false,
				portrait: portraits['175-long'],
				cards: [
					{
						mark: marks.vellum,
						world: 'The Gilded Reach',
						note: 'Lantern-priest of the Quay',
						flag: 'Approved',
						flagClass: 'm-flag m-flag--kind_done',
						flagIcon: 'check_circle',
						href: resolve('/w/[world]/characters/[character]', {
							world: 'gilded-reach',
							character: 'lady-maren'
						})
					}
				]
			},
			{
				name: 'Ensign Petra Lind',
				kind: 'Character',
				hasKind: false,
				portrait: portraits['150-short'],
				cards: [
					{
						mark: marks.kepler,
						world: 'Kepler Relay',
						note: 'Comms deck, ring B',
						flag: 'Approved',
						flagClass: 'm-flag m-flag--kind_done',
						flagIcon: 'check_circle',
						href: null
					}
				]
			},
			{
				name: 'Marit Sand',
				kind: 'Character',
				hasKind: false,
				portrait: portraits['200-short'],
				cards: [
					{
						mark: marks.saltroad,
						world: 'The Salt Road',
						note: 'Application, only you see this',
						flag: 'In review',
						flagClass: 'm-flag m-flag--kind_caution',
						flagIcon: 'hourglass_top',
						href: null
					}
				]
			}
		],
		worldGroups: [
			{
				title: 'Plays in',
				items: [
					{
						mark: marks.vellum,
						name: 'The Gilded Reach',
						role: 'Lady Maren, Anwen',
						roleIcon: 'person',
						state: 'Recruiting',
						stateIcon: 'how_to_reg',
						since: 'Since Sep 2025',
						href: resolve('/w/[world]/play', { world: 'gilded-reach' })
					},
					{
						mark: marks.kepler,
						name: 'Kepler Relay',
						role: 'Ensign Petra Lind',
						roleIcon: 'person',
						state: 'Active',
						stateIcon: 'forum',
						since: 'Since Feb',
						href: null
					}
				]
			},
			{
				title: 'Applied to, only you see this',
				items: [
					{
						mark: marks.saltroad,
						name: 'The Salt Road',
						role: 'Marit Sand',
						roleIcon: 'person',
						state: 'Changes asked for',
						stateIcon: 'edit_note',
						since: 'Mar 4',
						href: null
					}
				]
			}
		],
		posts: [
			{
				speaker: 'Lady Maren of Ost',
				laneLabel: 'In character',
				text: 'Lanterns hung from every beam of the great hall, a hundred small suns swaying in the draught from the high windows.',
				mark: marks.vellum,
				world: 'The Gilded Reach',
				scene: 'The Feast of Lanterns',
				when: 'Mar 3',
				href: resolve('/w/[world]/play/[scene]', {
					world: 'gilded-reach',
					scene: 'feast-of-lanterns'
				}),
				hasPortrait: true,
				noPortrait: false,
				portrait: portraits['260-long'],
				initials: 'IB',
				avatarClass: 'm-avatar m-avatar--kind_character m-avatar--size_md',
				hue: '--avatar-hue: 141'
			}
		],
		menuShown: false,
		menuOpen: false,
		muteLabel: 'Mute',
		blockLabel: 'Block',
		reporting: false,
		hasToast: false,
		toastText: '',
		toastIcon: 'check_circle'
	}
};

export type ProfileData = (typeof profiles)[keyof typeof profiles];
