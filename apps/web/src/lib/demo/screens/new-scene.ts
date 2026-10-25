// Demo content from the design export, as the screen first renders it.
// It stands in for AppView data until the screen loads real records.
import { portraits } from '../art.js';

export const newScene = {
	worldRef: {
		world: 'vellum'
	},
	no: false,
	inboxCount: 3,
	title: 'Household Rolls',
	cast: [
		{
			name: 'Lady Maren of Ost',
			portrait: portraits['260-long'],
			removable: false,
			removeLabel: 'Remove Lady Maren of Ost',
			n: '1',
			first: true,
			upLabel: 'Move Lady Maren of Ost up'
		},
		{
			name: 'Brother Cael',
			portrait: portraits['45-hood'],
			removable: true,
			removeLabel: 'Remove Brother Cael',
			n: '2',
			first: false,
			upLabel: 'Move Brother Cael up'
		}
	],
	query: '',
	listOpen: false,
	suggestions: [
		{
			name: 'Isolde Corven',
			who: '@nina.motet.at',
			portrait: portraits['330-long']
		},
		{
			name: 'Wren of the Quay',
			who: '@ola.codes',
			portrait: portraits['15-hood']
		},
		{
			name: 'Sir Edric Corven',
			who: '@kuba.dev',
			portrait: portraits['210-short']
		},
		{
			name: 'Ser Jaume Tallow',
			who: '@marek.pds.example',
			portrait: portraits['120-hat']
		}
	],
	noSuggestions: false,
	orders: [
		{
			label: 'In order',
			icon: 'format_list_numbered',
			checked: true
		},
		{
			label: 'Free',
			icon: 'shuffle',
			checked: false
		}
	],
	inOrder: true,
	free: false,
	paces: [
		{
			label: '3 days',
			checked: false
		},
		{
			label: 'A week',
			checked: true
		},
		{
			label: 'Two weeks',
			checked: false
		}
	],
	visibility: [
		{
			label: 'World members',
			hint: 'Everyone in The Gilded Reach',
			on: true
		},
		{
			label: 'Cast only',
			hint: 'The characters in this scene and the GMs',
			on: false
		},
		{
			label: 'Anyone',
			hint: 'Also shown to visitors and in search',
			on: false
		}
	],
	portraits: {
		maren: portraits['260-long']
	},
	submitHint: 'Aurelia and Bram see new scenes in their Inbox.'
};

export type NewSceneData = typeof newScene;
