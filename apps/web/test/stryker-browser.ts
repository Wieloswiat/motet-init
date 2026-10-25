import { inject } from 'vitest';

declare module 'vitest' {
	export interface ProvidedContext {
		strykerActiveMutant: string;
	}
}

// The command runner sets a Node environment variable, not browser state.
// Setup files run before test imports, so module-scope Svelte mutants are
// active before the component is evaluated. Normal tests leave no global.
const activeMutant = inject('strykerActiveMutant');

if (activeMutant !== '') {
	Object.assign(globalThis, { __stryker__: { activeMutant } });
}
