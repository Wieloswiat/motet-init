<script lang="ts">
	import { resolve } from '$app/paths';
	import Chrome from '#lib/components/motet/Chrome.svelte';
	import WorldHeader from '#lib/components/motet/WorldHeader.svelte';
	import { world } from '#lib/motet/world.js';
	import type { CharactersData } from '#lib/demo/screens/characters.js';

	let { data }: { data: CharactersData } = $props();
</script>

<svelte:head>
	<title>Characters</title>
</svelte:head>

<div {@attach world(data.worldRef)} class="m-world__root">
	<Chrome world="vellum" account="iga" inbox={data.inboxCount} />
	<WorldHeader current="Characters" size="slim" role="player" />
	<main
		class="m-world__page"
		style="padding-top: 24px; display: flex; flex-direction: column; gap: 28px"
	>
		<div style="display: flex; flex-wrap: wrap; gap: 12px; align-items: center">
			<div class="m-field__root" style="flex: 1 1 240px; max-width: 320px">
				<label class="m-field__label m-visually-hidden" for="find">Find a character</label>
				<div class="m-input__root">
					<span class="m-input__addon" data-side="start" aria-hidden="true"
						><span class="m-icon" aria-hidden="true">search</span></span
					><input
						class="m-input__control"
						id="find"
						type="search"
						placeholder="Find a character"
						value={data.query}
					/>
				</div>
			</div>
			<div
				class="m-scroll-row"
				role="group"
				aria-label="Show"
				style="flex: 1 1 300px; min-width: 0"
			>
				{#each data.filters as f (f.label)}
					<button class="m-chip m-chip--kind_filter" type="button" aria-pressed={f.pressed}
						><span class="m-icon" aria-hidden="true">{f.icon}</span><span>{f.label}</span></button
					>
				{/each}
			</div>
			<div
				class="m-segmented__root"
				role="radiogroup"
				aria-label="View"
				style="margin-inline-start: auto"
			>
				{#each data.views as v (v.label)}<button
						class="m-segmented__item"
						type="button"
						role="radio"
						aria-checked={v.checked}
						aria-label={v.label}
						title={v.label}><span class="m-icon" aria-hidden="true">{v.icon}</span></button
					>{/each}
			</div>
		</div>

		{#each data.groups as g (g.name)}
			<section aria-labelledby={g.hid} style="display: flex; flex-direction: column; gap: 14px">
				<div class="m-vt" style="display: flex; gap: 10px; align-items: baseline; --vt: {g.vtn}">
					<span class="m-icon" aria-hidden="true" style="color: var(--ornament); align-self: center"
						>flag</span
					>
					<h2 id={g.hid} class="h-group">{g.name}</h2>
					<span style="font-size: 13px; color: var(--ink-muted)">{g.count}</span>
				</div>
				{#if data.cards}
					<div
						style="display: grid; grid-template-columns: repeat(auto-fill, minmax(min(340px, 100%), 1fr)); gap: 16px"
					>
						{#each g.items as c (c.name)}
							<article
								class="m-character__root m-ornamented m-marked m-vt m-vt-box"
								style="--vt: {c.vtn}"
							>
								<div class="m-character__portrait m-vt" style="--vt: {c.vtp}">
									<img src={c.portrait} alt="" />
								</div>
								<div class="m-character__body">
									<h3 class="m-character__name">
										<span class="m-vt" style="--vt: {c.vtt}">{c.name}</span>
									</h3>
									<span class="m-character__epithet">{c.epithet}</span>
									<dl class="m-character__facts">
										<dt>Age</dt>
										<dd>{c.age}</dd>
										<dt>Trait</dt>
										<dd>{c.trait}</dd>
										<dt>Scenes</dt>
										<dd>{c.scenes}</dd>
									</dl>
									<div class="m-character__owner">
										<span
											class="m-avatar m-avatar--kind_account m-avatar--size_xs"
											aria-hidden="true"
											style={c.hue}><span aria-hidden="true">{c.initials}</span></span
										><span>{c.owner}</span>
									</div>
								</div>
								<div class="m-character__foot">
									<span class={c.flagClass}
										><span class="m-icon m-icon--sm" aria-hidden="true">{c.flagIcon}</span
										>{c.flagLabel}</span
									>
									<span style="margin-inline-start: auto; display: flex; gap: 4px"
										><a
											class="m-button m-button--variant_text m-button--size_sm"
											href={resolve('/w/[world]/characters/[character]', {
												world: 'gilded-reach',
												character: 'lady-maren'
											})}
											><span class="m-icon" aria-hidden="true">description</span><span
												>Full card</span
											></a
										></span
									>
								</div>
							</article>
						{/each}
					</div>
				{/if}
				{#if data.list}
					<div style="display: flex; flex-direction: column; border-top: 1px solid var(--line)">
						{#each g.items as c (c.name)}
							<a
								class="lrow m-vt m-vt-box"
								href={resolve('/w/[world]/characters/[character]', {
									world: 'gilded-reach',
									character: 'lady-maren'
								})}
								style="--vt: {c.vtn}; display: grid; grid-template-columns: 40px minmax(0, 2fr) minmax(0, 1.4fr) minmax(0, 1fr) auto; gap: 14px; align-items: center; padding: 10px 12px; border-bottom: 1px solid var(--line)"
							>
								<span
									class="m-avatar m-avatar--kind_character m-avatar--size_md m-vt"
									aria-hidden="true"
									style="--vt: {c.vtp}"><img src={c.portrait} alt="" /></span
								>
								<span style="display: grid; min-width: 0"
									><span
										class="m-post__name m-vt"
										style="font-size: calc(16px * var(--name-adjust, 1)); width: fit-content; --vt: {c.vtt}"
										>{c.name}</span
									><span
										style="font-size: 13px; color: var(--ink-muted); white-space: nowrap; overflow: hidden; text-overflow: ellipsis"
										>{c.epithet}</span
									></span
								>
								<span
									class="lsub"
									style="font-size: 13px; color: var(--ink-muted); min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap"
									>{c.owner}</span
								>
								<span class="lsub" style="font-size: 13px; color: var(--ink-muted)"
									>{c.scenes} scenes</span
								>
								<span class="lsub {c.flagClass}" style="justify-self: start"
									><span class="m-icon m-icon--sm" aria-hidden="true">{c.flagIcon}</span
									>{c.flagLabel}</span
								>
							</a>
						{/each}
					</div>
				{/if}
			</section>
		{/each}
		{#if data.empty}<p style="margin: 0; color: var(--ink-muted)">No characters match.</p>{/if}
	</main>
</div>

<style>
	.h-group {
		margin: 0;
		font-family: var(--font-display);
		font-weight: var(--display-weight);
		font-size: calc(22px * var(--display-adjust, 1));
		line-height: 1.15;
		text-transform: var(--display-case);
		letter-spacing: var(--display-tracking);
	}
	.lrow {
		text-decoration: none;
		color: var(--ink);
	}
	.lrow:hover {
		background-color: var(--surface-quiet);
	}
	@media (max-width: 720px) {
		.lrow {
			grid-template-columns: 40px minmax(0, 1fr) !important;
		}
		.lrow > .lsub {
			grid-column: 2;
		}
	}
</style>
