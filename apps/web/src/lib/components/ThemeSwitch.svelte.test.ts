import { page } from 'vitest/browser';
import { render } from 'vitest-browser-svelte';
import { describe, expect, it } from 'vitest';
import ThemeSwitch from './ThemeSwitch.svelte';

// The component is Ark's segment group over our tokens. What's worth checking
// is the contract with the page: every theme tokens.css defines is offered,
// and choosing one reports it.
describe('ThemeSwitch', () => {
	it('offers each theme and reports the choice', async () => {
		let chosen = '';
		render(ThemeSwitch, {
			props: {
				get value() {
					return 'light' as const;
				},
				set value(v) {
					chosen = v;
				}
			}
		});
		for (const name of ['light', 'dark', 'contrast-light', 'contrast-dark']) {
			await expect.element(page.getByRole('radio', { name, exact: true })).toBeInTheDocument();
		}
		await page.getByText('contrast-dark', { exact: true }).click();
		expect(chosen).toBe('contrast-dark');
	});
});
