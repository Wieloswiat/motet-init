<script lang="ts">
	import { goto, invalidateAll } from '$app/navigation';
	import { resolve } from '$app/paths';
	import '../app.css';
	import favicon from '#lib/assets/favicon.svg';
	import type { LayoutProps } from './$types';

	let { children, data }: LayoutProps = $props();
	let signingOut = $state(false);
	let signOutError = $state('');

	async function signOut() {
		signingOut = true;
		signOutError = '';

		try {
			const response = await fetch('/oauth/sign-out', { method: 'POST' });

			// Even a remote revocation failure clears the local session cookie.
			await invalidateAll();

			if (response.ok) {
				await goto(resolve('/sign-in'));
			} else {
				signOutError = 'Could not revoke the session with your account host.';
			}
		} catch {
			signOutError = 'Could not sign out. Please try again.';
		} finally {
			signingOut = false;
		}
	}
</script>

<svelte:head>
	<link rel="icon" href={favicon} />
</svelte:head>

<nav
	class="account-nav"
	aria-label="Account"
	style="display: flex; align-items: center; justify-content: flex-end; gap: 12px; padding: 8px 16px"
>
	{#if data.session}
		<span>Signed in as {data.session.handle}</span>
		<button
			class="m-button m-button--variant_text"
			type="button"
			onclick={signOut}
			disabled={signingOut}
		>
			Sign out
		</button>
	{:else}
		<a class="m-button m-button--variant_text" href={resolve('/sign-in')}>Sign in</a>
	{/if}
</nav>
{#if signOutError}
	<p role="alert">{signOutError}</p>
{/if}
{@render children()}

<style>
	/* Demo screens keep their fixed design-export chrome above the real account controls. */
	:global(body:has(.m-chrome__root)) .account-nav {
		margin-block-start: var(--chrome-offset, 72px);
	}
</style>
