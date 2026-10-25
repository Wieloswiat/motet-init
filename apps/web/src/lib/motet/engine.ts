let loading: Promise<Window['Motet']> | undefined;

/**
 * Loads Motet's engine and the demo Worlds in the browser, once.
 *
 * Both are classic scripts from the design export: they assign
 * `window.Motet` and `window.MotetDemo` instead of exporting. The demo reads
 * the engine while it loads, so the imports run in order.
 */
export function loadEngine(): Promise<Window['Motet']> {
	loading ??= (async () => {
		await import('@motet/design/engine');
		await import('@motet/design/demo');

		return window.Motet;
	})();

	return loading;
}
