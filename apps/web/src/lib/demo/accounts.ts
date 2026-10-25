// Demo accounts and their Worlds, from the design export's chrome. They stand
// in for the signed-in account until sign-in exists.
import type { ResolvedPathname } from '$app/types';
import { resolve } from '$app/paths';
import { marks } from './art.js';

export interface DemoAccount {
	display: string;
	handle: string;
	initials: string;
	/** Hue of the account avatar's ground. */
	hue: number;
	worlds: {
		key: keyof typeof marks;
		name: string;
		/** Who the account is in that World: a character, or a role. */
		as: string;
		/** Absent for demo Worlds that have no screens. */
		href: ResolvedPathname | null;
		turn: boolean;
	}[];
}

export const accounts = {
	iga: {
		display: 'Iga',
		handle: 'iga.bsky.social',
		initials: 'IB',
		hue: 141,
		worlds: [
			{
				key: 'vellum',
				name: 'The Gilded Reach',
				as: 'Lady Maren of Ost',
				href: resolve('/w/[world]/play', { world: 'gilded-reach' }),
				turn: true
			},
			{
				key: 'kepler',
				name: 'Kepler Relay',
				as: 'Ensign Petra Lind',
				href: null,
				turn: false
			},
			{
				key: 'saltroad',
				name: 'The Salt Road',
				as: 'Application sent',
				href: null,
				turn: false
			},
			{ key: 'ledger', name: 'Cold Front', as: 'Watching', href: null, turn: false }
		]
	},
	nadia: {
		display: 'Nadia',
		handle: 'nadia.pds.example',
		initials: 'NP',
		hue: 32,
		worlds: [
			{ key: 'hollow', name: 'Hollowmere', as: 'Ida Wilk', href: null, turn: false },
			{ key: 'neon', name: 'Neon Meridian', as: 'Watching', href: null, turn: false }
		]
	},
	aurelia: {
		display: 'Aurelia',
		handle: 'aurelia.bsky.social',
		initials: 'AB',
		hue: 262,
		worlds: [
			{
				key: 'vellum',
				name: 'The Gilded Reach',
				as: 'Game master',
				href: resolve('/w/[world]/manage', { world: 'gilded-reach' }),
				turn: false
			},
			{
				key: 'saltroad',
				name: 'The Salt Road',
				as: 'Mother Agathe',
				href: null,
				turn: false
			}
		]
	}
} satisfies Record<string, DemoAccount>;

export type AccountKey = keyof typeof accounts;
