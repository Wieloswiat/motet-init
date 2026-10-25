// Demo content from the design export, as the screen first renders it.
// It stands in for AppView data until the screen loads real records.
import { resolve } from '$app/paths';
import { marks } from '../art.js';

export const inbox = {
	pageRef: {
		world: 'motet'
	},
	needsCount: 3,
	heading: 'Inbox',
	filters: [
		{
			label: 'All',
			icon: 'inbox',
			count: 5,
			hasCount: true,
			current: true
		},
		{
			label: 'Needs you',
			icon: 'front_hand',
			count: 3,
			hasCount: true,
			current: false
		},
		{
			label: 'Playing',
			icon: 'theater_comedy',
			count: 3,
			hasCount: true,
			current: false
		},
		{
			label: 'Watching',
			icon: 'visibility',
			count: 1,
			hasCount: true,
			current: false
		},
		{
			label: 'Updates',
			icon: 'campaign',
			count: 1,
			hasCount: true,
			current: false
		}
	],
	groups: [
		{
			title: 'Needs you',
			items: [
				{
					id: 'feast',
					group: 'needs',
					tags: ['needs', 'playing'],
					kind: 'turn',
					kindIcon: 'front_hand',
					kindLabel: 'Your turn',
					world: 'vellum',
					worldName: 'The Gilded Reach',
					title: 'The Feast of Lanterns',
					excerpt:
						'Brother Cael: “The Corven banner,” he said quietly, before she could ask. “It came down this morning.”',
					time: '2h',
					unread: true,
					role: 'Player',
					level: 'all',
					href: resolve('/w/[world]/play/[scene]', {
						world: 'gilded-reach',
						scene: 'feast-of-lanterns'
					}),
					vtn: 'm-i-feast',
					mark: marks.vellum,
					rootClass:
						'm-inbox-item__root m-inbox-item__root--kind_turn m-inbox-item__root--unread_true',
					kindClass: 'm-inbox-item__kind m-inbox-item__kind--kind_turn',
					titleClass: 'm-inbox-item__title m-inbox-item__title--unread_true',
					roleIcon: 'person',
					roleTip: 'You play in this scene',
					levelIcon: 'notifications_active',
					levelTip: 'Notifications: All posts',
					menuOpen: false,
					menuTitle: 'Notify me about this scene',
					levels: [
						{
							label: 'All posts',
							hint: 'Every new post',
							icon: 'notifications_active',
							on: true,
							checked: true
						},
						{
							label: 'Your turn and mentions',
							hint: 'When you are asked to post or named',
							icon: 'notifications',
							on: false,
							checked: false
						},
						{
							label: 'Off',
							hint: 'Still shown in the World',
							icon: 'notifications_off',
							on: false,
							checked: false
						}
					],
					readIcon: 'mark_email_read',
					readLabel: 'Mark as read'
				},
				{
					id: 'silent',
					group: 'needs',
					tags: ['needs', 'playing'],
					kind: 'mention',
					kindIcon: 'alternate_email',
					kindLabel: 'Mention',
					world: 'kepler',
					worldName: 'Kepler Relay',
					title: 'Silent Hours',
					excerpt: 'Horvat: “Lind, get up here with the spectrum log. And bring the paper copy.”',
					time: '5h',
					unread: true,
					role: 'Player',
					level: 'mentions',
					href: null,
					vtn: 'm-i-silent',
					mark: marks.kepler,
					rootClass:
						'm-inbox-item__root m-inbox-item__root--kind_mention m-inbox-item__root--unread_true',
					kindClass: 'm-inbox-item__kind m-inbox-item__kind--kind_mention',
					titleClass: 'm-inbox-item__title m-inbox-item__title--unread_true',
					roleIcon: 'person',
					roleTip: 'You play in this scene',
					levelIcon: 'notifications',
					levelTip: 'Notifications: Your turn and mentions',
					menuOpen: false,
					menuTitle: 'Notify me about this scene',
					levels: [
						{
							label: 'All posts',
							hint: 'Every new post',
							icon: 'notifications_active',
							on: false,
							checked: false
						},
						{
							label: 'Your turn and mentions',
							hint: 'When you are asked to post or named',
							icon: 'notifications',
							on: true,
							checked: true
						},
						{
							label: 'Off',
							hint: 'Still shown in the World',
							icon: 'notifications_off',
							on: false,
							checked: false
						}
					],
					readIcon: 'mark_email_read',
					readLabel: 'Mark as read'
				},
				{
					id: 'saltapp',
					group: 'needs',
					tags: ['needs', 'updates'],
					kind: 'decision',
					kindIcon: 'gavel',
					kindLabel: 'Changes requested',
					world: 'saltroad',
					worldName: 'The Salt Road',
					title: 'Your application: Marit Sand',
					excerpt:
						'Marta K.: Lovely sheet. Could her backstory say how she got across the river in winter?',
					time: '1d',
					unread: true,
					role: 'Applicant',
					level: 'all',
					href: null,
					vtn: 'm-i-saltapp',
					mark: marks.saltroad,
					rootClass:
						'm-inbox-item__root m-inbox-item__root--kind_decision m-inbox-item__root--unread_true',
					kindClass: 'm-inbox-item__kind m-inbox-item__kind--kind_decision',
					titleClass: 'm-inbox-item__title m-inbox-item__title--unread_true',
					roleIcon: 'how_to_reg',
					roleTip: 'Your application',
					levelIcon: 'notifications_active',
					levelTip: 'Notifications: All posts',
					menuOpen: false,
					menuTitle: 'Notify me about this thread',
					levels: [
						{
							label: 'All posts',
							hint: 'Every new post',
							icon: 'notifications_active',
							on: true,
							checked: true
						},
						{
							label: 'Your turn and mentions',
							hint: 'When you are asked to post or named',
							icon: 'notifications',
							on: false,
							checked: false
						},
						{
							label: 'Off',
							hint: 'Still shown in the World',
							icon: 'notifications_off',
							on: false,
							checked: false
						}
					],
					readIcon: 'mark_email_read',
					readLabel: 'Mark as read'
				}
			]
		},
		{
			title: 'New activity',
			items: [
				{
					id: 'archive',
					group: 'activity',
					tags: ['watching'],
					kind: 'post',
					kindIcon: 'forum',
					kindLabel: '2 new posts',
					world: 'vellum',
					worldName: 'The Gilded Reach',
					title: 'Whispers in the Archive',
					excerpt:
						'Isolde Corven: She left the ledger open on the reading desk, exactly where Cael would trip over it.',
					time: '6h',
					unread: true,
					role: 'Watching',
					level: 'all',
					href: null,
					vtn: 'm-i-archive',
					mark: marks.vellum,
					rootClass:
						'm-inbox-item__root m-inbox-item__root--kind_post m-inbox-item__root--unread_true',
					kindClass: 'm-inbox-item__kind m-inbox-item__kind--kind_post',
					titleClass: 'm-inbox-item__title m-inbox-item__title--unread_true',
					roleIcon: 'visibility',
					roleTip: 'You follow this scene',
					levelIcon: 'notifications_active',
					levelTip: 'Notifications: All posts',
					menuOpen: false,
					menuTitle: 'Notify me about this scene',
					levels: [
						{
							label: 'All posts',
							hint: 'Every new post',
							icon: 'notifications_active',
							on: true,
							checked: true
						},
						{
							label: 'Your turn and mentions',
							hint: 'When you are asked to post or named',
							icon: 'notifications',
							on: false,
							checked: false
						},
						{
							label: 'Off',
							hint: 'Still shown in the World',
							icon: 'notifications_off',
							on: false,
							checked: false
						}
					],
					readIcon: 'mark_email_read',
					readLabel: 'Mark as read'
				},
				{
					id: 'tabletalk',
					group: 'activity',
					tags: ['playing'],
					kind: 'post',
					kindIcon: 'chat',
					kindLabel: 'Out of character',
					world: 'vellum',
					worldName: 'The Gilded Reach',
					title: 'Table talk',
					excerpt: 'Bram: Heads up, I’m away the week of the 20th. Aurelia has the NPCs.',
					time: '1d',
					unread: true,
					role: 'Member',
					level: 'mentions',
					href: null,
					vtn: 'm-i-tabletalk',
					mark: marks.vellum,
					rootClass:
						'm-inbox-item__root m-inbox-item__root--kind_post m-inbox-item__root--unread_true',
					kindClass: 'm-inbox-item__kind m-inbox-item__kind--kind_post',
					titleClass: 'm-inbox-item__title m-inbox-item__title--unread_true',
					roleIcon: 'group',
					roleTip: 'You are a member of this World',
					levelIcon: 'notifications',
					levelTip: 'Notifications: Your turn and mentions',
					menuOpen: false,
					menuTitle: 'Notify me about this thread',
					levels: [
						{
							label: 'All posts',
							hint: 'Every new post',
							icon: 'notifications_active',
							on: false,
							checked: false
						},
						{
							label: 'Your turn and mentions',
							hint: 'When you are asked to post or named',
							icon: 'notifications',
							on: true,
							checked: true
						},
						{
							label: 'Off',
							hint: 'Still shown in the World',
							icon: 'notifications_off',
							on: false,
							checked: false
						}
					],
					readIcon: 'mark_email_read',
					readLabel: 'Mark as read'
				},
				{
					id: 'turn14',
					group: 'activity',
					tags: ['watching'],
					kind: 'post',
					kindIcon: 'event',
					kindLabel: 'Turn results',
					world: 'ledger',
					worldName: 'Cold Front',
					title: 'Turn 14: The Baltic Assembly',
					excerpt: 'Desk Officer: Norway’s proposal passes 4 to 3. Poland abstains.',
					time: '1d',
					unread: false,
					role: 'Watching',
					level: 'all',
					href: null,
					vtn: 'm-i-turn14',
					mark: marks.ledger,
					rootClass:
						'm-inbox-item__root m-inbox-item__root--kind_post m-inbox-item__root--unread_false',
					kindClass: 'm-inbox-item__kind m-inbox-item__kind--kind_post',
					titleClass: 'm-inbox-item__title m-inbox-item__title--unread_false',
					roleIcon: 'visibility',
					roleTip: 'You follow this scene',
					levelIcon: 'notifications_active',
					levelTip: 'Notifications: All posts',
					menuOpen: false,
					menuTitle: 'Notify me about this scene',
					levels: [
						{
							label: 'All posts',
							hint: 'Every new post',
							icon: 'notifications_active',
							on: true,
							checked: true
						},
						{
							label: 'Your turn and mentions',
							hint: 'When you are asked to post or named',
							icon: 'notifications',
							on: false,
							checked: false
						},
						{
							label: 'Off',
							hint: 'Still shown in the World',
							icon: 'notifications_off',
							on: false,
							checked: false
						}
					],
					readIcon: 'mark_chat_unread',
					readLabel: 'Mark as unread'
				}
			]
		},
		{
			title: 'Updates',
			items: [
				{
					id: 'chapter',
					group: 'updates',
					tags: ['updates'],
					kind: 'post',
					kindIcon: 'campaign',
					kindLabel: 'Announcement',
					world: 'vellum',
					worldName: 'The Gilded Reach',
					title: 'Chapter two opens on April 1',
					excerpt: 'Aurelia: New scenes at the Lantern Quay. Current scenes keep running.',
					time: 'Mar 9',
					unread: false,
					role: 'Member',
					level: 'all',
					href: null,
					vtn: 'm-i-chapter',
					mark: marks.vellum,
					rootClass:
						'm-inbox-item__root m-inbox-item__root--kind_post m-inbox-item__root--unread_false',
					kindClass: 'm-inbox-item__kind m-inbox-item__kind--kind_post',
					titleClass: 'm-inbox-item__title m-inbox-item__title--unread_false',
					roleIcon: 'group',
					roleTip: 'You are a member of this World',
					levelIcon: 'notifications_active',
					levelTip: 'Notifications: All posts',
					menuOpen: false,
					menuTitle: 'Notify me about this thread',
					levels: [
						{
							label: 'All posts',
							hint: 'Every new post',
							icon: 'notifications_active',
							on: true,
							checked: true
						},
						{
							label: 'Your turn and mentions',
							hint: 'When you are asked to post or named',
							icon: 'notifications',
							on: false,
							checked: false
						},
						{
							label: 'Off',
							hint: 'Still shown in the World',
							icon: 'notifications_off',
							on: false,
							checked: false
						}
					],
					readIcon: 'mark_chat_unread',
					readLabel: 'Mark as unread'
				},
				{
					id: 'cycle',
					group: 'updates',
					tags: ['updates', 'playing'],
					kind: 'decision',
					kindIcon: 'check_circle',
					kindLabel: 'Sheet approved',
					world: 'kepler',
					worldName: 'Kepler Relay',
					title: 'Ensign Petra Lind',
					excerpt: 'ctrl: Approved for cycle 3. Welcome aboard.',
					time: 'Feb 27',
					unread: false,
					role: 'Player',
					level: 'mentions',
					href: null,
					vtn: 'm-i-cycle',
					mark: marks.kepler,
					rootClass:
						'm-inbox-item__root m-inbox-item__root--kind_decision m-inbox-item__root--unread_false',
					kindClass: 'm-inbox-item__kind m-inbox-item__kind--kind_decision',
					titleClass: 'm-inbox-item__title m-inbox-item__title--unread_false',
					roleIcon: 'person',
					roleTip: 'You play in this scene',
					levelIcon: 'notifications',
					levelTip: 'Notifications: Your turn and mentions',
					menuOpen: false,
					menuTitle: 'Notify me about this scene',
					levels: [
						{
							label: 'All posts',
							hint: 'Every new post',
							icon: 'notifications_active',
							on: false,
							checked: false
						},
						{
							label: 'Your turn and mentions',
							hint: 'When you are asked to post or named',
							icon: 'notifications',
							on: true,
							checked: true
						},
						{
							label: 'Off',
							hint: 'Still shown in the World',
							icon: 'notifications_off',
							on: false,
							checked: false
						}
					],
					readIcon: 'mark_chat_unread',
					readLabel: 'Mark as unread'
				}
			]
		}
	],
	empty: false,
	noneUnread: false
};

export type InboxData = typeof inbox;
