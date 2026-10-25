<script lang="ts">
	import { resolve } from '$app/paths';
	import LanguagePicker from '#lib/components/motet/LanguagePicker.svelte';
	import { world } from '#lib/motet/world.js';
	import type { SignInData } from '#lib/demo/screens/sign-in.js';

	let { data }: { data: SignInData } = $props();
</script>

<svelte:head>
	<title>Sign in</title>
</svelte:head>

<div
	{@attach world(data.pageRef)}
	class="m-page__root m-page__root--ground_canvas"
	style="box-sizing: border-box; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 24px; padding: 48px max(16px, env(safe-area-inset-right, 0px)) 48px max(16px, env(safe-area-inset-left, 0px))"
>
	<span style="font: 600 22px/1 var(--font-ui); letter-spacing: -0.02em">motet</span>
	<main class="m-page__card">
		<h1 style="margin: 0; font: 600 26px/32px var(--font-ui)">Sign in</h1>
		<form style="display: flex; flex-direction: column; gap: 16px">
			<div class="m-field__root">
				<label class="m-field__label" for="handle">Your handle</label>
				<div class="m-input__root" style="--control-h: 48px">
					<span class="m-input__addon" data-side="start" aria-hidden="true"
						><span class="m-icon" aria-hidden="true">alternate_email</span></span
					>
					<input
						class="m-input__control"
						id="handle"
						type="text"
						value={data.handle}
						placeholder="you.bsky.social"
						autocomplete="username"
						autocapitalize="none"
						spellcheck="false"
						aria-describedby="handle-help"
						style="font-size: 16px"
					/>
				</div>
				<div class="m-field__help" id="handle-help">
					{#if data.hasHost}<span class="m-icon" aria-hidden="true">dns</span><span
							>{data.hostText}</span
						>{/if}
					{#if data.noHost}<span
							>Your Bluesky handle works, or one from any other account host.</span
						>{/if}
				</div>
			</div>
			<a
				class="m-button m-button--variant_filled"
				href={resolve('/')}
				style="justify-content: center; min-height: 48px"
				><span>Continue</span><span class="m-icon" aria-hidden="true">arrow_forward</span></a
			>
		</form>

		<div
			style="display: flex; align-items: center; gap: 12px; color: var(--ink-muted); font-size: 13px"
		>
			<span style="flex: 1; height: 1px; background-color: var(--line)"></span>New here?<span
				style="flex: 1; height: 1px; background-color: var(--line)"
			></span>
		</div>

		{#if data.createClosed}
			<button
				class="m-button m-button--variant_outline"
				type="button"
				style="justify-content: center; min-height: 44px"
				><span class="m-icon" aria-hidden="true">person_add</span><span>Create an account</span
				></button
			>
		{/if}
		{#if data.createOpen}
			<fieldset class="m-fieldset__root" style="margin: 0">
				<legend class="m-fieldset__legend">Where your account lives</legend>
				<div class="m-fieldset__help" style="margin-bottom: 8px">
					Your posts and characters stay with your account if you move hosts.
				</div>
				<div class="m-radio__root m-radio__root--variant_cards">
					{#each data.hosts as h (h.label)}
						<label class="m-radio__item m-radio__item--variant_cards"
							><input
								class="m-radio__control m-radio__control--variant_cards"
								type="radio"
								name="host"
								checked={h.on}
							/><span class="m-radio__text m-radio__text--variant_cards"
								><span class="m-radio__label m-radio__label--variant_cards">{h.label}</span><span
									class="m-radio__description">{h.hint}</span
								></span
							><span class="m-icon m-radio__icon m-radio__icon--variant_cards" aria-hidden="true"
								>{h.icon}</span
							></label
						>
					{/each}
				</div>
				{#if data.otherHost}
					<div class="m-field__root" style="margin-top: 12px">
						<label class="m-field__label" for="hosturl">Host address</label>
						<div class="m-input__root">
							<input
								class="m-input__control"
								id="hosturl"
								type="text"
								placeholder="pds.example.org"
							/>
						</div>
					</div>
				{/if}
				<a
					class="m-button m-button--variant_tonal"
					href={resolve('/')}
					style="justify-content: center; min-height: 44px; margin-top: 16px"
					><span>{data.createLabel}</span><span class="m-icon" aria-hidden="true"
						>arrow_forward</span
					></a
				>
			</fieldset>
		{/if}
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
			value={[data.uiLang]}
			leading="language"
			pid="signin-lang"
		/>
	</div>
</div>
