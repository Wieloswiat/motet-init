import { userEvent } from 'vitest/browser';
import { cleanup, render } from 'vitest-browser-svelte';
import { describe, expect, it } from 'vitest';
import '../../app.css';
import HomePage from '../../routes/+page.svelte';
import ThemeSwitch from './ThemeSwitch.svelte';

// These reactive updates have no network or animation to settle. Installed
// Browser Mode otherwise stretches failed assertions to the full 15s test
// deadline, making mutation tools report an outer timeout instead of failure.
const localUpdate = { timeout: 1000 };

describe('ThemeSwitch', () => {
	// Exercise the real caller: losing the bound update can leave native
	// radios checked while the page keeps painting the previous theme.
	it('repaints the real page when the reader changes the selected theme', async () => {
		const screen = await render(HomePage);
		const main = screen.getByRole('main');
		const group = screen.getByRole('radiogroup', { name: 'Theme' });

		await group.getByText('dark', { exact: true }).click();
		await expect.element(main, localUpdate).toHaveStyle({
			color: 'rgb(209, 223, 230)',
			backgroundColor: 'rgb(6, 12, 16)'
		});
		await group.getByText('contrast-light', { exact: true }).click();
		await expect.element(main, localUpdate).toHaveStyle({
			color: 'rgb(13, 22, 28)',
			backgroundColor: 'rgb(225, 235, 242)'
		});
	});

	// Losing the label/input association or the controlled value would make
	// a pointer selection disagree with the choice announced to the reader.
	it('lets a reader replace the selected theme by clicking its visible label', async () => {
		const screen = await render(ThemeSwitch, { value: 'contrast-dark' });
		const group = screen.getByRole('radiogroup', { name: 'Theme' });

		for (const name of ['dark', 'contrast-light', 'light', 'contrast-dark']) {
			await group.getByText(name, { exact: true }).click();
			await expect
				.element(group.getByRole('radio', { name, exact: true }), localUpdate)
				.toBeChecked();
			await expect
				.element(group.getByRole('radio', { checked: true }), localUpdate)
				.toHaveAccessibleName(name);
		}
	});

	// One Tab must enter the selected choice, and arrows must move both
	// selection and focus. Separate radio names or a stale controlled value
	// would break this even if every theme label remained visible.
	it('supports one-tab entry and arrow-key selection without losing focus', async () => {
		const screen = await render(ThemeSwitch);
		const group = screen.getByRole('radiogroup', { name: 'Theme' });
		const light = group.getByRole('radio', { name: 'light', exact: true });
		const dark = group.getByRole('radio', { name: 'dark', exact: true });

		await userEvent.tab();
		await expect.element(light, localUpdate).toHaveFocus();
		await expect.element(light, localUpdate).toBeChecked();
		await userEvent.keyboard('{ArrowRight}');
		await expect.element(dark, localUpdate).toHaveFocus();
		await expect.element(dark, localUpdate).toBeChecked();
		await expect.element(light, localUpdate).not.toBeChecked();
		await userEvent.keyboard('{ArrowLeft}');
		await expect.element(light, localUpdate).toHaveFocus();
		await expect.element(light, localUpdate).toBeChecked();
		await expect.element(dark, localUpdate).not.toBeChecked();
	});

	// Use real component mount targets as theme boundaries, not a fake World
	// screen. Root-only Panda aliases would make both scopes use one palette.
	it('resolves nested theme colours locally without restyling the reader scope', async () => {
		const readerScope = document.createElement('section');
		const readerTarget = document.createElement('div');
		const nestedScope = document.createElement('section');

		readerScope.dataset.theme = 'dark';
		nestedScope.dataset.theme = 'light';
		readerScope.append(readerTarget, nestedScope);
		document.body.append(readerScope);

		try {
			const reader = await render(ThemeSwitch, { target: readerTarget });
			const nested = await render(ThemeSwitch, { target: nestedScope });

			const readerLabel = reader.locator
				.getByRole('radio', { name: 'light', exact: true })
				.element()
				.closest('label');

			const nestedLabel = nested.locator
				.getByRole('radio', { name: 'light', exact: true })
				.element()
				.closest('label');

			await expect.element(readerLabel, localUpdate).toHaveStyle({
				color: 'rgb(2, 17, 26)',
				backgroundColor: 'rgb(123, 186, 220)'
			});
			await expect.element(nestedLabel, localUpdate).toHaveStyle({
				color: 'rgb(245, 251, 255)',
				backgroundColor: 'rgb(43, 107, 138)'
			});
			nestedScope.dataset.theme = 'contrast-light';
			await expect.element(nestedLabel, localUpdate).toHaveStyle({
				color: 'rgb(245, 251, 255)',
				backgroundColor: 'rgb(26, 92, 123)'
			});
			await expect.element(readerLabel, localUpdate).toHaveStyle({
				color: 'rgb(2, 17, 26)',
				backgroundColor: 'rgb(123, 186, 220)'
			});
		} finally {
			await cleanup();
			readerScope.remove();
		}
	});
});
