// Demo content from the design export, as the screen first renders it.
// It stands in for AppView data until the screen loads real records.
import { resolve } from '$app/paths';
import { marks, portraits } from '../art.js';

export const homeBoard = {
	pageRef: {
		world: 'motet'
	},
	inboxCount: 1,
	yes: true,
	langCodes: ['en', 'pl'],
	calls: [
		{
			key: 'vellum',
			world: 'The Gilded Reach',
			by: 'Aurelia',
			when: '2 days ago',
			title: 'Two seats at the Feast',
			body: 'House Corven needs an heir who can lie through a seven-course dinner, and the river ports need a smuggler who cannot be bought twice. Both arrive at Harrowgate on the night the lanterns are lit.',
			seats: '2 seats open',
			closes: 'Closes Apr 30',
			pace: 'Weekly',
			system: 'Freeform, d20 checks',
			lang: 'English',
			apply: resolve('/w/[world]/apply', { world: 'gilded-reach' }),
			about: resolve('/w/[world]/visitor', { world: 'gilded-reach' }),
			ref: {
				world: 'vellum'
			},
			mark: marks.vellum
		},
		{
			key: 'ashfall',
			world: 'Ashfall',
			by: 'Rdza',
			when: '5 days ago',
			title: 'Szukamy zwiadowców',
			body: 'Przełęcz odmarzła wcześniej niż zwykle. Osada wysyła drużynę na północ i potrzebuje ludzi, którzy wrócą z mapą, a nie z opowieściami.',
			seats: '3 seats open',
			closes: 'Closes May 12',
			pace: 'Twice a week',
			system: 'd20',
			lang: 'Polski',
			apply: null,
			about: null,
			ref: {
				world: 'ashfall'
			},
			mark: marks.ashfall
		},
		{
			key: 'oskad',
			world: 'Oskad',
			by: 'Telegrafista',
			when: '1 week ago',
			title: 'Nowy zastępca szeryfa',
			body: 'Poprzedni zastępca wyjechał w kanion i wrócił tylko jego koń. Rada miasta płaci podwójnie, ale nie odpowiada na pytania.',
			seats: '1 seat open',
			closes: 'Open until filled',
			pace: 'Weekly',
			system: 'Freeform',
			lang: 'Polski',
			apply: null,
			about: null,
			ref: {
				world: 'oskad'
			},
			mark: marks.oskad
		}
	],
	reads: [
		{
			ref: {
				world: 'kepler'
			},
			mark: marks.kepler,
			world: 'Kepler Relay',
			title: 'Silent Hours',
			name: 'Oyelaran',
			epithet: 'Comms officer, night shift',
			portrait: portraits['70-short'],
			text: '0312. The carrier came back on the second band, eleven seconds after it dropped. Same burst length, same gap. Oyelaran logged it by hand, the way the manual says to when the automatics disagree with you.\n“Ring B to engineering. Is anyone else hearing this?”'
		},
		{
			ref: {
				world: 'neon'
			},
			mark: marks.neon,
			world: 'Neon Meridian',
			title: 'Deep Line 7',
			name: 'Kestrel',
			epithet: 'Netrunner, unlicensed',
			portrait: portraits['190-visor'],
			text: 'The maglev doors sighed open on a platform that was not on any public map. Kestrel kept her hands in her jacket and counted cameras: four, one of them dead, one of them pretending to be.\n“You said the drop would be clean,” she subvocalised.'
		},
		{
			ref: {
				world: 'saltroad'
			},
			mark: marks.saltroad,
			world: 'The Salt Road',
			title: 'Thaw at the Ferry',
			name: 'Ilse Varga',
			epithet: 'Ferry-keeper’s daughter',
			portrait: portraits['205-long'],
			text: 'The ice had begun to talk in the night. Ilse heard it from the loft, the long groaning creak that meant the river was thinking about moving again, and by morning there was a black seam of water down the middle of the crossing.\n“Nobody goes over today,” she said, before the merchant could open his mouth. “Not you, not your mules, not your salt.”'
		}
	],
	marks: {
		hollow: marks.hollow,
		neon: marks.neon
	}
};

export type HomeBoardData = typeof homeBoard;
