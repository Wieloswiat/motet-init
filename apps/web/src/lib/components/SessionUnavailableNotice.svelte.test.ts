import { render } from 'vitest-browser-svelte';
import { describe, expect, it } from 'vitest';
import '../../app.css';
import SessionUnavailableNotice from './SessionUnavailableNotice.svelte';

describe('SessionUnavailableNotice', () => {
	// A reader whose sign-in status could not be checked must be told so in
	// words a screen reader announces. Dropping the status role, or leaving the
	// meaning in the icon alone, leaves that reader with a Sign in link and no
	// explanation of why the account controls are missing.
	it('announces that the sign-in status is unavailable and how to retry', async () => {
		const screen = await render(SessionUnavailableNotice);
		const notice = screen.getByRole('status');

		await expect.element(notice).toHaveTextContent(/Sign-in status unavailable/);
		await expect.element(notice).toHaveTextContent(/shows you as signed out/);
		await expect.element(notice).toHaveTextContent(/Reload to try again/);
	});
});
