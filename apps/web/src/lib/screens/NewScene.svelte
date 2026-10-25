<script lang="ts">
	import { resolve } from '$app/paths';
	import Chrome from '#lib/components/motet/Chrome.svelte';
	import WorldHeader from '#lib/components/motet/WorldHeader.svelte';
	import { world } from '#lib/motet/world.js';
	import type { NewSceneData } from '#lib/demo/screens/new-scene.js';

	let { data }: { data: NewSceneData } = $props();
</script>

<svelte:head>
	<title>New scene</title>
</svelte:head>

<div {@attach world(data.worldRef)} class="m-world__root">
	<Chrome world="vellum" account="iga" inbox={data.inboxCount} />
	<WorldHeader current="Play" size="slim" role="player" />
	<main class="m-world__page" style="padding-top: 24px">
		<form
			style="max-width: 780px; margin-inline: auto; display: flex; flex-direction: column; gap: 24px"
		>
			<a
				class="m-button m-button--variant_text m-button--size_sm"
				href={resolve('/w/[world]/play', { world: 'gilded-reach' })}
				style="align-self: flex-start; margin-inline-start: -8px"
				><span class="m-icon" aria-hidden="true">arrow_back</span><span>Play</span></a
			>
			<h1 class="h-step">New scene</h1>

			<div class="m-field__root">
				<label class="m-field__label" for="title">Title</label>
				<div class="m-input__root">
					<input
						class="m-input__control"
						id="title"
						type="text"
						value={data.title}
						style="font-family: var(--font-display); font-size: 18px"
					/>
				</div>
			</div>

			<div
				style="display: grid; grid-template-columns: repeat(auto-fit, minmax(min(240px, 100%), 1fr)); gap: 16px"
			>
				<div class="m-select__root">
					<span class="m-field__label" id="loc-l">Location</span><button
						class="m-select__trigger"
						type="button"
						aria-haspopup="listbox"
						aria-labelledby="loc-l loc-v"
						><span
							class="m-select__value"
							id="loc-v"
							style="display: flex; gap: 8px; align-items: center"
							><span class="m-icon m-icon--sm" aria-hidden="true">castle</span>Harrowgate Keep, the
							Archive</span
						><span class="m-icon m-select__indicator" aria-hidden="true">expand_more</span></button
					>
				</div>
				<div class="m-select__root">
					<span class="m-field__label" id="arc-l">Arc</span><button
						class="m-select__trigger"
						type="button"
						aria-haspopup="listbox"
						aria-labelledby="arc-l arc-v"
						><span
							class="m-select__value"
							id="arc-v"
							style="display: flex; gap: 8px; align-items: center"
							><span class="m-icon m-icon--sm" aria-hidden="true">auto_stories</span>A Crown of Ash</span
						><span class="m-icon m-select__indicator" aria-hidden="true">expand_more</span></button
					>
				</div>
			</div>

			<div class="m-field__root" style="position: relative">
				<label class="m-field__label" for="cast-in">Cast</label>
				<div class="m-tags__root" style="min-height: 48px">
					{#each data.cast as c (c.name)}
						<span class="m-tags__tag" style="gap: 6px; padding-inline-start: 4px"
							><span class="m-avatar m-avatar--kind_character m-avatar--size_xs" aria-hidden="true"
								><img src={c.portrait} alt="" /></span
							><span>{c.name}</span>{#if c.removable}<button
									class="m-tags__remove"
									type="button"
									aria-label={c.removeLabel}
									><span class="m-icon" aria-hidden="true">close</span></button
								>{/if}</span
						>
					{/each}
					<input
						class="m-tags__control"
						id="cast-in"
						type="text"
						placeholder="Add a character"
						value={data.query}
						autocomplete="off"
					/>
				</div>
				{#if data.listOpen}
					<div
						class="m-menu__root"
						role="listbox"
						aria-label="Characters"
						style="position: absolute; top: calc(100% + 4px); left: 0; z-index: 4; width: min(360px, 100%)"
					>
						{#each data.suggestions as s (s.name)}
							<button class="m-menu__item opt" type="button" role="option" aria-selected="false"
								><span
									class="m-avatar m-avatar--kind_character m-avatar--size_sm"
									aria-hidden="true"><img src={s.portrait} alt="" /></span
								><span style="display: grid; flex: 1"
									><span>{s.name}</span><span
										style="font-size: 12.5px; color: var(--menu-ink-muted, var(--ink-muted))"
										>{s.who}</span
									></span
								></button
							>
						{/each}
						{#if data.noSuggestions}<div
								style="padding: 10px; color: var(--menu-ink-muted, var(--ink-muted)); font-size: 13px"
							>
								Everyone is in the cast.
							</div>{/if}
					</div>
				{/if}
				<div class="m-field__help">
					<span>Each player is asked to join. A GM character can be added later.</span>
				</div>
			</div>

			<fieldset class="m-fieldset__root" style="margin: 0">
				<legend class="m-fieldset__legend">Posting</legend>
				<div
					class="m-segmented__root"
					role="radiogroup"
					aria-label="Posting"
					style="margin-bottom: 12px"
				>
					{#each data.orders as o (o.label)}<button
							class="m-segmented__item"
							type="button"
							role="radio"
							aria-checked={o.checked}
							><span class="m-icon" aria-hidden="true">{o.icon}</span>{o.label}</button
						>{/each}
				</div>
				{#if data.inOrder}
					<ol
						style="margin: 0; padding: 0; list-style: none; display: grid; gap: 6px; max-width: 420px"
					>
						{#each data.cast as c (c.name)}
							<li
								style="display: flex; align-items: center; gap: 10px; padding: 6px 8px; border: 1px solid var(--line); border-radius: var(--radius-md); background-color: var(--surface-raised)"
							>
								<span
									class="m-icon"
									aria-hidden="true"
									style="color: var(--ink-muted); cursor: grab">drag_indicator</span
								>
								<span style="width: 2ch; font: 600 13px/1 var(--font-ui); color: var(--ink-muted)"
									>{c.n}</span
								>
								<span class="m-avatar m-avatar--kind_character m-avatar--size_xs" aria-hidden="true"
									><img src={c.portrait} alt="" /></span
								>
								<span style="flex: 1; font-size: 14px">{c.name}</span>
								<button
									class="m-icon-button m-icon-button--variant_standard m-icon-button--size_sm"
									type="button"
									aria-label={c.upLabel}
									disabled={c.first}
									><span class="m-icon" aria-hidden="true">arrow_upward</span></button
								>
							</li>
						{/each}
					</ol>
				{/if}
				{#if data.free}<p style="margin: 0; font-size: 14px; color: var(--ink-muted)">
						Anyone in the cast posts whenever they like. Nobody is asked for a turn.
					</p>{/if}
				<div
					style="display: flex; align-items: center; gap: 12px; margin-top: 14px; flex-wrap: wrap"
				>
					<span style="font-size: 14px">Ask for a post within</span>
					<div class="m-segmented__root" role="radiogroup" aria-label="Ask for a post within">
						{#each data.paces as o (o.label)}<button
								class="m-segmented__item"
								type="button"
								role="radio"
								aria-checked={o.checked}>{o.label}</button
							>{/each}
					</div>
				</div>
			</fieldset>

			<fieldset class="m-fieldset__root" style="margin: 0">
				<legend class="m-fieldset__legend">Who can read it</legend>
				<div class="m-radio__root">
					{#each data.visibility as v (v.label)}
						<label class="m-radio__item"
							><input class="m-radio__control" type="radio" name="vis" checked={v.on} /><span
								class="m-radio__text"
								><span class="m-radio__label">{v.label}</span><span class="m-radio__description"
									>{v.hint}</span
								></span
							></label
						>
					{/each}
				</div>
			</fieldset>

			<section class="m-composer__root m-marked" aria-label="Opening post">
				<div class="m-composer__head">
					<button
						class="m-composer__speaker"
						type="button"
						aria-haspopup="listbox"
						aria-label="Speaker: Lady Maren of Ost. Change speaker"
						><span class="m-avatar m-avatar--kind_character m-avatar--size_sm" aria-hidden="true"
							><img src={data.portraits.maren} alt="" /></span
						><span class="m-composer__speakerName">Lady Maren of Ost</span><span
							class="m-icon"
							aria-hidden="true">expand_more</span
						></button
					>
					<span class="m-composer__destination"
						><span class="m-icon" aria-hidden="true">forum</span><strong>Opening post</strong><span
							class="m-chip m-chip--kind_fact"><span>In character</span></span
						></span
					>
				</div>
				<textarea
					class="m-composer__editor"
					aria-label="Opening post as Lady Maren of Ost"
					placeholder="Set the scene as Lady Maren of Ost…"
					style="min-height: 160px"
					>The Archive smelled of tallow and wet stone. Maren had come for the household rolls of House Corven, and found Brother Cael already sitting with them, as if he had been waiting.</textarea
				>
				<div class="m-composer__foot">
					<div class="m-composer__tools" role="toolbar" aria-label="Formatting">
						<button
							class="m-icon-button m-icon-button--variant_standard m-icon-button--size_sm"
							type="button"
							aria-label="Bold"><span class="m-icon" aria-hidden="true">format_bold</span></button
						>
						<button
							class="m-icon-button m-icon-button--variant_standard m-icon-button--size_sm"
							type="button"
							aria-label="Italic"
							><span class="m-icon" aria-hidden="true">format_italic</span></button
						>
						<button
							class="m-icon-button m-icon-button--variant_standard m-icon-button--size_sm"
							type="button"
							aria-label="Roll dice"><span class="m-icon" aria-hidden="true">casino</span></button
						>
					</div>
					<span class="m-composer__submit"
						><span
							class="m-composer__status"
							role="status"
							aria-label="Draft saved on this device"
							title="Draft saved on this device"
							><span class="m-icon" aria-hidden="true">cloud_done</span></span
						></span
					>
				</div>
			</section>

			<div
				style="display: flex; gap: 8px; align-items: center; flex-wrap: wrap; padding-top: 8px; border-top: 1px solid var(--line)"
			>
				<span style="flex: 1; min-width: 200px; font-size: 13px; color: var(--ink-muted)"
					>{data.submitHint}</span
				>
				<a
					class="m-button m-button--variant_text"
					href={resolve('/w/[world]/play', { world: 'gilded-reach' })}><span>Cancel</span></a
				>
				<a
					class="m-button m-button--variant_filled"
					href={resolve('/w/[world]/play/[scene]', {
						world: 'gilded-reach',
						scene: 'feast-of-lanterns'
					})}><span class="m-icon" aria-hidden="true">send</span><span>Start scene</span></a
				>
			</div>
		</form>
	</main>
</div>

<style>
	.h-step {
		margin: 0;
		font-family: var(--font-display);
		font-weight: var(--display-weight);
		font-size: calc(30px * var(--display-adjust, 1));
		line-height: 1.15;
		text-transform: var(--display-case);
		letter-spacing: var(--display-tracking);
	}
	@layer reset {
		.opt {
			background: none;
			border: 0;
			font: inherit;
			color: inherit;
			cursor: pointer;
			text-align: start;
		}
	}
</style>
