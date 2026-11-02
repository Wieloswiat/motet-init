<script lang="ts">
	import { resolve } from '$app/paths';
	import LanguagePicker from '#lib/components/motet/LanguagePicker.svelte';
	import { world } from '#lib/motet/world.js';

	let { returnTo }: { returnTo: string } = $props();
	let identifier = $state('');

	function beginSignIn(event: SubmitEvent) {
		event.preventDefault();
		const params = new URLSearchParams({ identifier, returnTo });

		// OAuth leaves this origin; a navigation keeps form-action 'self' intact.
		window.location.assign(`/oauth/sign-in?${params}`);
	}

	const pageRef = { world: 'motet' };
</script>

<svelte:head>
	<title>Sign in</title>
</svelte:head>

<div
	{@attach world(pageRef)}
	class="m-page__root m-page__root--ground_canvas"
	style="box-sizing: border-box; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 24px; padding: 48px max(16px, env(safe-area-inset-right, 0px)) 48px max(16px, env(safe-area-inset-left, 0px))"
>
	<span style="font: 600 22px/1 var(--font-ui); letter-spacing: -0.02em">motet</span>
	<main class="m-page__card">
		<h1 style="margin: 0; font: 600 26px/32px var(--font-ui)">Sign in</h1>
		<form
			action="/oauth/sign-in"
			method="GET"
			onsubmit={beginSignIn}
			style="display: flex; flex-direction: column; gap: 16px"
		>
			<input type="hidden" name="returnTo" value={returnTo} />
			<div class="m-field__root">
				<label class="m-field__label" for="handle">Your handle</label>
				<div class="m-input__root" style="--control-h: 48px">
					<span class="m-input__addon" data-side="start" aria-hidden="true"
						><span class="m-icon" aria-hidden="true">alternate_email</span></span
					>
					<input
						class="m-input__control"
						id="handle"
						bind:value={identifier}
						name="identifier"
						type="text"
						placeholder="you.bsky.social"
						autocomplete="username"
						autocapitalize="none"
						spellcheck="false"
						aria-describedby="handle-help"
						required
						style="font-size: 16px"
					/>
				</div>
				<div class="m-field__help" id="handle-help">
					Your Bluesky handle works, or one from any other account host.
				</div>
			</div>
			<button
				class="m-button m-button--variant_filled"
				type="submit"
				style="justify-content: center; min-height: 48px"
				><span>Continue</span><span class="m-icon" aria-hidden="true">arrow_forward</span></button
			>
		</form>

		<div
			style="display: flex; align-items: center; gap: 12px; color: var(--ink-muted); font-size: 13px"
		>
			<span style="flex: 1; height: 1px; background-color: var(--line)"></span>New here?<span
				style="flex: 1; height: 1px; background-color: var(--line)"
			></span>
		</div>

		<a
			class="m-button m-button--variant_outline"
			href={`/oauth/sign-up?returnTo=${encodeURIComponent(returnTo)}`}
			rel="external"
			style="justify-content: center; min-height: 44px"
			><span class="m-icon" aria-hidden="true">person_add</span><span>Create an account</span></a
		>
	</main>
	<nav
		aria-label="Without an account"
		style="display: flex; gap: 8px; flex-wrap: wrap; justify-content: center"
	>
		<a class="m-button m-button--variant_text m-button--size_sm" href={resolve('/board')}
			><span class="m-icon" aria-hidden="true">auto_stories</span><span>Read public scenes</span></a
		>
		<a class="m-button m-button--variant_text m-button--size_sm" href={resolve('/search')}
			><span class="m-icon" aria-hidden="true">explore</span><span>Browse Worlds</span></a
		>
	</nav>
	<div style="width: 200px">
		<LanguagePicker
			multiple={false}
			label="Interface language"
			hideLabel
			value={['en']}
			leading="language"
			pid="signin-lang"
		/>
	</div>
</div>
