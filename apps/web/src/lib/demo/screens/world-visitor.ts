// Demo content from the design export, as the screen first renders it.
// It stands in for AppView data until the screen loads real records.

export const worldVisitor = {
	worldRef: {
		world: 'vellum'
	},
	no: false,
	inboxCount: 1,
	rules: [
		{
			icon: 'schedule',
			title: 'Post once a week in each of your scenes',
			body: 'If a week gets away from you, say so in Table talk. The GMs will move your character along so the scene does not wait.',
			open: true
		},
		{
			icon: 'theater_comedy',
			title: 'Write only your own character',
			body: 'Other characters react in their players’ posts, never in yours. You may describe what your character notices, not what anyone else decides.',
			open: false
		},
		{
			icon: 'casino',
			title: 'Contested actions take a d20 check',
			body: 'Roll d20 plus your character’s trait in the composer when a GM or another player asks. The GM narrates what the result means.',
			open: false
		},
		{
			icon: 'chat',
			title: 'Out-of-character talk stays out of the story',
			body: 'Use the out-of-character lane in a scene, or Table talk for anything bigger than the scene.',
			open: false
		},
		{
			icon: 'block',
			title: 'Lines are lines',
			body: 'Nothing on the Never list appears, on screen or off. Anyone can fade a scene to black, no reason given.',
			open: false
		}
	],
	roles: [
		{
			kind: 'Courtier',
			icon: 'flag',
			title: 'Heir of House Corven',
			body: 'Your house’s banner came down this morning. Find out who ordered it before the Feast ends.',
			status: '1 application so far'
		},
		{
			kind: 'Commoner',
			icon: 'anchor',
			title: 'A smuggler of the Lantern Quay',
			body: 'Half the Keep’s wine comes through your boats. Customs closes at dusk; you do not.',
			status: 'No applications yet'
		},
		{
			kind: 'Open',
			icon: 'person_search',
			title: 'Bring your own',
			body: 'A retainer, a priest of the Rite, a scholar of the Archive. Tell the GMs who needs your character.',
			status: 'Always open'
		}
	]
};

export type WorldVisitorData = typeof worldVisitor;
