// Demo content from the design export, as the screen first renders it.
// It stands in for AppView data until the screen loads real records.

export const startWorld = {
	pageRef: {
		world: 'motet'
	},
	previewRef: {
		world: 'hollow'
	},
	heroRef: {
		world: 'hollow'
	},
	no: false,
	yes: true,
	inboxCount: 3,
	building: true,
	done: false,
	steps: [
		{
			n: '1',
			title: 'The World',
			state: 'current',
			current: 'step' as const,
			done: false,
			notDone: true
		},
		{
			n: '2',
			title: 'The table',
			state: 'upcoming',
			current: false,
			done: false,
			notDone: true
		},
		{
			n: '3',
			title: 'The look',
			state: 'upcoming',
			current: false,
			done: false,
			notDone: true
		},
		{
			n: '4',
			title: 'People and moderation',
			state: 'upcoming',
			current: false,
			done: false,
			notDone: true
		},
		{
			n: '5',
			title: 'Review',
			state: 'upcoming',
			current: false,
			done: false,
			notDone: true
		}
	],
	is: {
		world: true,
		table: false,
		look: false,
		people: false,
		review: false
	},
	name: 'The Drowned Bells',
	handleStem: 'drowned-bells',
	handle: 'drowned-bells.motet.at',
	desc: 'At the lowest tides the drowned town of Sallow rings its bells again. Whoever hears them dreams of the people who rang them, and wakes knowing one of their secrets.',
	descCount: '164 / 300',
	lang: 'en',
	also: ['pl'],
	genres: [
		{
			label: 'Fantasy',
			on: false
		},
		{
			label: 'Horror',
			on: true
		},
		{
			label: 'Mystery',
			on: true
		},
		{
			label: 'Sci-fi',
			on: false
		},
		{
			label: 'Post-apocalyptic',
			on: false
		},
		{
			label: 'Historical',
			on: false
		},
		{
			label: 'Modern',
			on: false
		},
		{
			label: 'Weird West',
			on: false
		},
		{
			label: 'Strategy',
			on: false
		}
	],
	systems: [
		{
			label: 'Freeform',
			on: false
		},
		{
			label: 'Dice checks',
			on: true
		},
		{
			label: 'A game system',
			on: false
		}
	],
	namedSystem: false,
	paces: [
		{
			label: 'Twice a week',
			on: false
		},
		{
			label: 'Weekly',
			on: true
		},
		{
			label: 'Every two weeks',
			on: false
		}
	],
	content: [
		{
			label: 'In the story',
			icon: 'visibility',
			items: ['Death and grief', 'Drowning', 'Body horror'],
			chipClass: 'm-chip m-chip--kind_fact'
		},
		{
			label: 'Off screen only',
			icon: 'visibility_off',
			items: ['Torture'],
			chipClass: 'm-chip m-chip--kind_fact'
		},
		{
			label: 'Never',
			icon: 'block',
			items: ['Sexual violence', 'Harm to children'],
			chipClass: 'm-chip m-chip--kind_fact m-chip--tone_danger'
		}
	],
	themes: [
		{
			label: 'Motet',
			hint: 'Quiet and round, no ornament',
			on: false,
			ref: {
				world: 'saltroad'
			}
		},
		{
			label: 'Court',
			hint: 'Gilt frames, classical serif',
			on: false,
			ref: {
				world: 'vellum'
			}
		},
		{
			label: 'Gothic',
			hint: 'Near black, slow fades',
			on: true,
			ref: {
				world: 'hollow'
			}
		},
		{
			label: 'Wasteland',
			hint: 'Stencil type, rust and grain',
			on: false,
			ref: {
				world: 'ashfall'
			}
		},
		{
			label: 'Neon',
			hint: 'Clipped panels, technical type',
			on: false,
			ref: {
				world: 'neon'
			}
		},
		{
			label: 'Relay',
			hint: 'Amber terminal light',
			on: false,
			ref: {
				world: 'kepler'
			}
		},
		{
			label: 'Operations',
			hint: 'Dense, plain, tabular',
			on: false,
			ref: {
				world: 'ledger'
			}
		}
	],
	gms: [
		{
			handle: 'bram.writes',
			removeLabel: 'Remove @bram.writes'
		}
	],
	joins: [
		{
			label: 'Applications',
			hint: 'People apply with a character; GMs approve',
			icon: 'how_to_reg',
			on: true
		},
		{
			label: 'Open',
			hint: 'Anyone can join and start playing',
			icon: 'person_add',
			on: false
		},
		{
			label: 'Invitation only',
			hint: 'GMs invite players by handle',
			icon: 'lock',
			on: false
		}
	],
	reads: [
		{
			label: 'Anyone',
			on: false
		},
		{
			label: 'World members',
			on: true
		}
	],
	takes: [
		{
			label: 'Rules and content lines',
			on: true,
			locked: true
		},
		{
			label: 'Harassment, hate or trolling',
			on: true,
			locked: false
		},
		{
			label: 'Unwanted sexual content',
			on: true,
			locked: false
		},
		{
			label: 'Threats or danger',
			on: true,
			locked: false
		},
		{
			label: 'Spam or a scam',
			on: false,
			locked: false
		},
		{
			label: 'Something else',
			on: true,
			locked: false
		}
	],
	pbfAttr: true,
	summary: [
		{
			k: 'Name',
			v: 'The Drowned Bells',
			editLabel: 'Change name'
		},
		{
			k: 'Account',
			v: '@drowned-bells.motet.at',
			editLabel: 'Change account'
		},
		{
			k: 'Languages',
			v: 'English, Polski',
			editLabel: 'Change languages'
		},
		{
			k: 'Genre',
			v: 'Horror, Mystery',
			editLabel: 'Change genre'
		},
		{
			k: 'Rules and pace',
			v: 'Dice checks, weekly',
			editLabel: 'Change rules and pace'
		},
		{
			k: 'Starting theme',
			v: 'Gothic',
			editLabel: 'Change starting theme'
		},
		{
			k: 'GMs',
			v: 'You, @bram.writes',
			editLabel: 'Change gms'
		},
		{
			k: 'Joining',
			v: 'Applications',
			editLabel: 'Change joining'
		},
		{
			k: 'Moderation',
			v: '5 kinds of report, follows PBF Safety Collective',
			editLabel: 'Change moderation'
		}
	],
	hasBack: false,
	noBack: true,
	notLast: true,
	isLast: false,
	blocked: false,
	nextLabel: 'The table',
	nameShown: 'The Drowned Bells',
	initials: 'DB',
	descShown:
		'At the lowest tides the drowned town of Sallow rings its bells again. Whoever hears them dreams of the people who rang them, and wakes knowing one of their secrets.',
	gmLine: 'Iga, Bram',
	joinIcon: 'how_to_reg',
	joinState: 'Recruiting',
	joinAction: 'Apply to join',
	langName: 'English',
	gmAsked: '@bram.writes has been asked to co-run it'
};

export type StartWorldData = typeof startWorld;
