/* Types for the design export's demo content (worlds.js). It assigns window.MotetDemo. */
import type { WorldRecord } from '../engine/motet.js';

export interface DemoWorld {
	key: string;
	name: string;
	record: WorldRecord;
	/** The World's mark, as an SVG data URL. */
	mark: string;
	gms: string[];
	state: string;
	language: string;
}

export interface MotetDemo {
	worlds: DemoWorld[];
	byKey(key: string): DemoWorld | null;
	/** A placeholder portrait, as an SVG data URL. */
	portrait(hue: number, variant: string, light: boolean): string;
}

declare global {
	interface Window {
		MotetDemo: MotetDemo;
	}
}
