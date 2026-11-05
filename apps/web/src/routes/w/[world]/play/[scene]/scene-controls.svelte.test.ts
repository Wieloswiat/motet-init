import { render } from 'vitest-browser-svelte';
import { expect, it } from 'vitest';
import ScenePage from './+page.svelte';
import type { PageProps } from './$types';

const localUpdate = { timeout: 1000 };
const world = { did: 'did:plc:world', displayName: 'A public World' };
const member = { did: 'did:plc:opener', handle: 'opener.example.test' };

function props(controls: { setCast: boolean; close: boolean } | null, state = 'open'): PageProps {
	return {
		params: { world: world.did, scene: 'scene1' },
		form: null,
		data: {
			session: member,
			world,
			sceneKey: 'scene1',
			returnTo: '/w/did:plc:world/play/scene1',
			sceneControls: controls,
			sceneControlsUnavailable: controls === null,
			scenePage: {
				world,
				posts: [],
				scene: {
					uri: 'at://did:plc:world/at.motet.pilot.scene/scene1',
					cid: 'bafy-scene',
					value: {
						$type: 'at.motet.pilot.scene',
						title: 'At the crossroads',
						state,
						cast: [member.did],
						openedBy: member.did
					}
				}
			}
		}
	};
}

// Inferring authority from the opener/session, treating unavailable permissions
// as a grant, or sharing one grant between actions would leak write controls.
it.each([
	{ name: 'permission lookup unavailable', controls: null },
	{ name: 'neither grant', controls: { setCast: false, close: false } },
	{ name: 'cast-only grant', controls: { setCast: true, close: false } },
	{ name: 'close-only grant', controls: { setCast: false, close: true } },
	{ name: 'both grants', controls: { setCast: true, close: true } }
])(
	'shows only independently authorized scene actions for $name, never opener authority',
	async ({ controls }) => {
		const screen = await render(ScenePage, props(controls));
		await expect
			.element(screen.getByRole('heading', { name: 'At the crossroads' }), localUpdate)
			.toBeVisible();
		const edit = screen.getByRole('button', { name: 'Edit cast', exact: true });
		const close = screen.getByRole('button', { name: 'Close scene', exact: true });

		if (controls?.close) await expect.element(close, localUpdate).toBeVisible();
		else await expect.element(close, localUpdate).not.toBeInTheDocument();

		if (controls?.setCast) {
			await expect.element(edit, localUpdate).toBeVisible();
			await edit.click();
			const input = screen.getByRole('textbox', { name: 'Cast handles or DIDs' });
			await expect.element(input, localUpdate).toHaveValue(member.did);
			await input.fill('another.example.test');
			await screen.getByRole('button', { name: 'Cancel cast edit' }).click();
			await expect.element(input, localUpdate).not.toBeInTheDocument();
			await expect.element(edit, localUpdate).toBeVisible();
		} else await expect.element(edit, localUpdate).not.toBeInTheDocument();
	}
);

// An accidental session/opener fallback could expose mutations or the
// authenticated composer while an anonymous reader views the public scene.
it('anonymous readers can read a scene without seeing mutation or posting inputs', async () => {
	const anonymous = props(null);
	anonymous.data.session = null;
	const screen = await render(ScenePage, anonymous);
	await expect
		.element(screen.getByRole('heading', { name: 'At the crossroads' }), localUpdate)
		.toBeVisible();
	await expect
		.element(screen.getByText('Sign in to write a post.', { exact: true }), localUpdate)
		.toBeVisible();

	for (const name of ['Post', 'Edit cast', 'Close scene']) {
		await expect
			.element(screen.getByRole('button', { name, exact: true }), localUpdate)
			.not.toBeInTheDocument();
	}

	await expect
		.element(screen.getByRole('textbox', { name: 'Write a post' }), localUpdate)
		.not.toBeInTheDocument();
});

// Checking only sign-in or grants, rather than scene state, would expose a
// composer and scene mutations after closure despite the tenor refusing them.
it('closed scenes explain posting refusal and hide all scene writes even for the authorized opener', async () => {
	const screen = await render(ScenePage, props({ setCast: true, close: true }, 'closed'));
	await expect
		.element(
			screen
				.getByRole('status')
				.filter({ hasText: 'This scene is closed. New posts are not accepted.' }),
			localUpdate
		)
		.toBeVisible();
	await expect
		.element(screen.getByRole('textbox', { name: 'Write a post' }), localUpdate)
		.not.toBeInTheDocument();

	for (const name of ['Post', 'Edit cast', 'Close scene', 'Save cast']) {
		await expect
			.element(screen.getByRole('button', { name, exact: true }), localUpdate)
			.not.toBeInTheDocument();
	}
});
