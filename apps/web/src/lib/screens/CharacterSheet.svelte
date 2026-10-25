<script lang="ts">
	import { resolve } from '$app/paths';
	import Chrome from '#lib/components/motet/Chrome.svelte';
	import WorldHeader from '#lib/components/motet/WorldHeader.svelte';
	import { world } from '#lib/motet/world.js';
	import type { CharacterSheetData } from '#lib/demo/screens/character-sheet.js';

	let { data }: { data: CharacterSheetData } = $props();
</script>

<svelte:head>
	<title>Character sheet</title>
</svelte:head>

<div {@attach world(data.worldRef)} class="m-world__root">
	<Chrome world="vellum" account="iga" inbox={data.inboxCount} />
	<WorldHeader current="Characters" size="slim" role="player" />
	<main class="m-world__page" style="padding-top: 24px">
		<a
			class="m-button m-button--variant_text m-button--size_sm"
			href={resolve('/w/[world]/characters', { world: 'gilded-reach' })}
			style="margin-inline-start: -8px; margin-bottom: 16px"
			><span class="m-icon" aria-hidden="true">arrow_back</span><span>Characters</span></a
		>
		<div class="m-world__layout">
			<article style="display: flex; flex-direction: column; gap: 24px; min-width: 0">
				<header style="display: flex; gap: 24px; flex-wrap: wrap; align-items: flex-end">
					<div
						class="m-ornamented"
						style="flex: 0 0 200px; aspect-ratio: 3 / 4; max-width: 100%; padding: 8px; background-color: var(--surface-raised); border: var(--border-width, 1px) solid var(--frame-line, var(--line)); border-radius: var(--frame-radius, var(--radius-lg)); corner-shape: var(--frame-shape, var(--corner-shape)); box-shadow: var(--elevation-2), var(--edge-light)"
					>
						<img
							src={data.portrait}
							alt="Portrait of Lady Maren of Ost"
							style="width: 100%; height: 100%; object-fit: cover; border-radius: var(--radius-md); display: block"
						/>
					</div>
					<div
						style="flex: 1 1 300px; min-width: 0; display: flex; flex-direction: column; gap: 10px"
					>
						<h1
							style="margin: 0; font-family: var(--font-name, var(--font-display)); font-weight: var(--display-weight); font-size: calc(40px * var(--display-adjust, 1)); line-height: 1.05; text-transform: var(--display-case); letter-spacing: var(--display-tracking); text-wrap: balance"
						>
							Lady Maren of Ost
						</h1>
						<p style="margin: 0; font-size: 16px; color: var(--ink-muted)">
							Envoy of the Northern March
						</p>
						<div style="display: flex; flex-wrap: wrap; gap: 8px; align-items: center">
							<span class="m-chip m-chip--kind_fact"
								><span class="m-icon" aria-hidden="true">flag</span><span>House Ost</span></span
							>
							<span class="m-flag m-flag--kind_done"
								><span class="m-icon m-icon--sm" aria-hidden="true">check_circle</span>Approved by
								Aurelia</span
							>
						</div>
						<div style="display: flex; flex-wrap: wrap; gap: 8px; margin-top: 6px">
							<button class="m-button m-button--variant_tonal" type="button"
								><span class="m-icon" aria-hidden="true">edit</span><span>Edit card</span></button
							>
							<button class="m-button m-button--variant_text" type="button"
								><span class="m-icon" aria-hidden="true">data_object</span><span>View record</span
								></button
							>
						</div>
					</div>
				</header>

				<nav class="m-tabs__root" aria-label="Character" style="margin-inline: -12px">
					{#each data.tabs as t (t.label)}<button
							class="m-tabs__tab tabbtn"
							type="button"
							aria-current={t.current}
							><span class="m-icon" aria-hidden="true">{t.icon}</span>{t.label}</button
						>{/each}
				</nav>

				{#if data.is.bio}
					<div class="m-prose m-prose--lead m-vt-pane">
						<p>
							Maren is the second of Lord Ost’s three children and the only one he trusts to speak
							for him. She has spent three winters at Harrowgate, which is two longer than any envoy
							of the North before her, and she has learned to be useful to everyone without
							belonging to anyone.
						</p>
						<h2 class="h-sub">Appearance</h2>
						<p>
							Tall, plainly dressed by court standards, always in the grey wool of the March. She
							wears her mother’s ring on a chain because it no longer fits her finger, and she will
							not have it resized.
						</p>
						<h2 class="h-sub">At the Feast</h2>
						<p>
							She has been sent to bargain for grain against a hard spring. She has also been sent,
							though her father did not say so, to find out who will hold Harrowgate when the Duke
							dies.
						</p>
					</div>
					<div class="m-collapsible__root m-collapsible__root--variant_spoiler">
						<button
							class="m-collapsible__trigger m-collapsible__trigger--variant_spoiler"
							type="button"
							aria-expanded={data.secretOpen}
							aria-controls="secret"
							><span class="m-icon" aria-hidden="true">lock</span><span
								class="m-collapsible__label m-collapsible__label--variant_spoiler">Secret</span
							><span class="m-collapsible__topic m-collapsible__topic--variant_spoiler"
								>You and the GMs</span
							><span class="m-collapsible__action m-collapsible__action--variant_spoiler"
								>{data.secretAction}</span
							></button
						>
						{#if data.secretOpen}<div
								class="m-collapsible__content m-collapsible__content--variant_spoiler"
								id="secret"
								role="region"
								style="font-family: var(--font-reading); font-size: 16px; line-height: 1.55"
							>
								Her brother, the heir of Ost, is not at home recovering from a fever. He has been in
								a Corven prison since midwinter, and Maren has told no one.
							</div>{/if}
					</div>
				{/if}

				{#if data.is.rel}
					<div class="m-vt-pane" style="display: flex; flex-direction: column">
						{#each data.relations as r (r.name)}
							<a
								class="rel"
								href="#rel"
								style="display: grid; grid-template-columns: 40px minmax(0, 1fr); gap: 2px 14px; padding: 12px; margin-inline: -12px; border-radius: var(--radius-md); align-items: center"
							>
								<span
									class="m-avatar m-avatar--kind_character m-avatar--size_md"
									aria-hidden="true"
									style="grid-row: span 2"><img src={r.portrait} alt="" /></span
								>
								<span style="display: flex; gap: 10px; align-items: baseline; flex-wrap: wrap"
									><span class="m-post__name" style="font-size: calc(16px * var(--name-adjust, 1))"
										>{r.name}</span
									><span class="m-chip m-chip--kind_fact" style="min-height: 22px"
										><span>{r.kind}</span></span
									></span
								>
								<span style="font-family: var(--font-reading); font-size: 15px; line-height: 1.5"
									>{r.note}</span
								>
							</a>
						{/each}
					</div>
				{/if}

				{#if data.is.scenes}
					<div class="m-vt-pane" style="display: flex; flex-direction: column">
						{#each data.scenes as s (s.title)}
							<a
								class="m-scene-row__root m-scene-row__root--unread_false"
								href={s.href}
								style="grid-template-columns: 3px minmax(0, 1fr) auto 6ch"
							>
								<span class="m-scene-row__marker" aria-hidden="true"></span>
								<span class="m-scene-row__main"
									><span class="m-scene-row__title">{s.title}</span><span class="m-scene-row__where"
										><span class="m-meta"
											><span class="m-icon m-icon--sm" aria-hidden="true">location_on</span><span
												>{s.where}</span
											></span
										></span
									></span
								>
								<span class="m-scene-row__state"
									><span class="m-meta" style="font-size: 13px; color: var(--ink-muted)"
										><span class="m-icon m-icon--sm" aria-hidden="true">forum</span><span
											>{s.posts}</span
										></span
									></span
								>
								<time class="m-scene-row__time">{s.time}</time>
							</a>
						{/each}
					</div>
				{/if}
			</article>

			<aside
				class="m-world__rail"
				aria-label="Facts"
				style="display: flex; flex-direction: column; gap: 20px"
			>
				<dl class="m-datalist__root m-datalist__root--variant_stacked" style="margin: 0">
					<div class="m-datalist__item m-datalist__item--variant_stacked">
						<dt class="m-datalist__label m-datalist__label--variant_stacked">From</dt>
						<dd class="m-datalist__value m-datalist__value--variant_stacked">
							Ost, the Northern March
						</dd>
					</div>
					<div class="m-datalist__item m-datalist__item--variant_stacked">
						<dt class="m-datalist__label m-datalist__label--variant_stacked">Age</dt>
						<dd class="m-datalist__value m-datalist__value--variant_stacked">27</dd>
					</div>
					<div class="m-datalist__item m-datalist__item--variant_stacked">
						<dt class="m-datalist__label m-datalist__label--variant_stacked">Trait</dt>
						<dd class="m-datalist__value m-datalist__value--variant_stacked">
							Letters, +3 to checks
						</dd>
					</div>
					<div class="m-datalist__item m-datalist__item--variant_stacked">
						<dt class="m-datalist__label m-datalist__label--variant_stacked">In play since</dt>
						<dd class="m-datalist__value m-datalist__value--variant_stacked">September 2025</dd>
					</div>
					<div class="m-datalist__item m-datalist__item--variant_stacked">
						<dt class="m-datalist__label m-datalist__label--variant_stacked">Posts</dt>
						<dd class="m-datalist__value m-datalist__value--variant_stacked">
							38, last 2 hours ago
						</dd>
					</div>
				</dl>
				<div style="display: flex; flex-direction: column; gap: 8px">
					<h3 style="margin: 0; font: 600 13px/18px var(--font-ui); color: var(--ink-muted)">
						Played by
					</h3>
					<div style="display: flex; gap: 10px; align-items: center">
						<span
							class="m-avatar m-avatar--kind_account m-avatar--size_sm"
							aria-hidden="true"
							style="--avatar-hue: 141"><span aria-hidden="true">IB</span></span
						><span style="display: grid; font-size: 14px"
							><span style="font-weight: 600">Iga</span><span
								style="font-size: 12.5px; color: var(--ink-muted)">@iga.bsky.social</span
							></span
						>
					</div>
				</div>
			</aside>
		</div>
	</main>
</div>

<style>
	.h-sub {
		margin: 1.2em 0 0.3em;
		font-family: var(--font-display);
		font-weight: var(--display-weight);
		font-size: calc(21px * var(--display-adjust, 1));
		line-height: 1.2;
		text-transform: var(--display-case);
		letter-spacing: var(--display-tracking);
	}
	@layer reset {
		.tabbtn {
			background: none;
			border: 0;
			cursor: pointer;
			font: inherit;
		}
	}
	.rel {
		text-decoration: none;
		color: var(--ink);
	}
	.rel:hover {
		background-color: var(--surface-quiet);
	}
</style>
