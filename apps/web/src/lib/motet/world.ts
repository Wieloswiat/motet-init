import type { Attachment } from 'svelte/attachments';
import { loadEngine } from './engine.js';

/** The World whose theme a subtree takes: a demo World's key, or `motet` for Motet's own look. */
export interface WorldRef {
	world: string;
}

/**
 * Applies a World's theme to the element it is attached to.
 *
 * The engine compiles the record in the browser and writes custom properties
 * on the element, so the server-rendered page shows Motet's look until it
 * hydrates. ADR 0012 records why this is acceptable for the demo screens.
 */
export function world(ref: WorldRef | null): Attachment<HTMLElement> {
	return (element) => {
		if (ref === null) {
			return;
		}

		let detached = false;

		void loadEngine().then((motet) => {
			const record =
				ref.world === 'motet' ? motet.defaultRecord : window.MotetDemo.byKey(ref.world)?.record;

			if (!detached && record !== undefined) {
				motet.mountWorld(element, record, {});
			}
		});

		return () => {
			detached = true;
		};
	};
}
