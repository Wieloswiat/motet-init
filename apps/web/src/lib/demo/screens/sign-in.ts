// Demo content from the design export, as the screen first renders it.
// It stands in for AppView data until the screen loads real records.

export const signIn = {
	pageRef: {
		world: 'motet'
	},
	yes: true,
	no: false,
	uiLangs: ['en', 'pl'],
	uiLang: 'en',
	handle: 'iga.bsky.social',
	hasHost: true,
	noHost: false,
	hostText: 'You will confirm at bsky.social',
	createOpen: false,
	createClosed: true,
	hosts: [
		{
			label: 'Bluesky',
			hint: 'bsky.social, also signs you in to Bluesky',
			icon: 'cloud',
			on: true
		},
		{
			label: 'Motet',
			hint: 'motet.at, for play only',
			icon: 'forum',
			on: false
		},
		{
			label: 'Another host',
			hint: 'Your own or a community’s',
			icon: 'dns',
			on: false
		}
	],
	otherHost: false,
	createLabel: 'Continue at Bluesky'
};

export type SignInData = typeof signIn;
