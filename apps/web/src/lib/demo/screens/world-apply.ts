// Demo content from the design export, as the screen first renders it.
// It stands in for AppView data until the screen loads real records.
import { portraits } from '../art.js';

export const worldApply = {
	worldRef: {
		world: 'vellum'
	},
	no: false,
	inboxCount: 1,
	steps: [
		{
			n: '1',
			title: 'Before you start',
			state: 'current',
			current: 'step' as const,
			done: false,
			notDone: true,
			locked: false
		},
		{
			n: '2',
			title: 'Character',
			state: 'upcoming',
			current: false,
			done: false,
			notDone: true,
			locked: true
		},
		{
			n: '3',
			title: 'Story',
			state: 'upcoming',
			current: false,
			done: false,
			notDone: true,
			locked: true
		},
		{
			n: '4',
			title: 'Review',
			state: 'upcoming',
			current: false,
			done: false,
			notDone: true,
			locked: true
		}
	],
	is: {
		rules: true,
		character: false,
		story: false,
		review: false
	},
	sent: false,
	notSent: true,
	agreed: false,
	paces: [
		{
			label: 'More than once a week',
			on: false
		},
		{
			label: 'About once a week',
			on: true
		},
		{
			label: 'Less often',
			on: false
		}
	],
	paceLabel: 'About once a week',
	roles: [
		{
			label: 'Heir of House Corven',
			hint: 'Courtier',
			icon: 'flag',
			on: true
		},
		{
			label: 'A smuggler of the Lantern Quay',
			hint: 'Commoner',
			icon: 'anchor',
			on: false
		},
		{
			label: 'Bring your own',
			hint: 'Tell the GMs who needs your character',
			icon: 'person_search',
			on: false
		}
	],
	roleLabel: 'Heir of House Corven',
	name: 'Tamsin Corven',
	nameMissing: false,
	nameInvalid: false,
	epithet: 'Heir of House Corven, for now',
	house: 'Corven',
	age: 24,
	traits: [
		{
			label: 'Silver tongue',
			icon: 'chat_bubble',
			pressed: true
		},
		{
			label: 'Steel',
			icon: 'swords',
			pressed: false
		},
		{
			label: 'Letters',
			icon: 'history_edu',
			pressed: false
		},
		{
			label: 'Shadows',
			icon: 'visibility_off',
			pressed: false
		},
		{
			label: 'Faith',
			icon: 'church',
			pressed: false
		}
	],
	traitLabel: 'Silver tongue',
	portrait: portraits['340-long'],
	hasPortrait: true,
	noPortrait: false,
	bg: 'Tamsin was raised to inherit nothing. Her brother was the heir until the river took him two springs ago, and the court has never let her forget that she is the second choice. She is good at dinners, better at letters, and very good at remembering who smiled at the wrong moment.',
	bgCount: '52 words',
	hasBack: false,
	notLast: true,
	isLast: false,
	nextLabel: 'Character',
	blocked: true
};

export type WorldApplyData = typeof worldApply;
