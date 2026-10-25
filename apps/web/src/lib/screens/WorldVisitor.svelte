<script lang="ts">
	import { resolve } from '$app/paths';
	import Chrome from '#lib/components/motet/Chrome.svelte';
	import WorldHeader from '#lib/components/motet/WorldHeader.svelte';
	import { world } from '#lib/motet/world.js';
	import type { WorldVisitorData } from '#lib/demo/screens/world-visitor.js';

	let { data }: { data: WorldVisitorData } = $props();
</script>

<svelte:head>
	<title>World overview, visitor</title>
</svelte:head>

<div {@attach world(data.worldRef)} class="m-world__root">
	<Chrome world="vellum" account="nadia" inbox={data.inboxCount} />
	<WorldHeader current="Overview" size="full" role="visitor" />
	<main class="m-world__page" style="padding-top: 28px">
		<div class="m-world__layout">
			<div style="display: flex; flex-direction: column; gap: 40px; min-width: 0">
				<section aria-labelledby="h-about" style="display: flex; flex-direction: column; gap: 16px">
					<h2 id="h-about" class="h-section m-flanked">About the Reach</h2>
					<div class="m-prose m-prose--lead">
						<p>
							The old Duke is dying in Harrowgate, and every house in the Reach has come to watch.
							They have come for the Feast of Lanterns, they say: the spring rite when the river
							carries a light for each of the dead. They have also come to count the banners, and
							this year there is one too few.
						</p>
						<p>
							The Gilded Reach is a game of courtesy and knives. Most scenes are dinners, letters,
							audiences and walks along the walls; when blades come out, they matter. You play a
							noble, a servant of a noble, or someone the nobles need: a scholar of the Archive, a
							smuggler of the Lantern Quay, a priest of the Rite.
						</p>
					</div>
				</section>

				<section aria-labelledby="h-table" style="display: flex; flex-direction: column; gap: 12px">
					<h2 id="h-table" class="h-section">At the table</h2>
					<dl class="m-datalist__root m-datalist__root--variant_ruled" style="margin: 0">
						<div class="m-datalist__item m-datalist__item--variant_ruled">
							<dt class="m-datalist__label m-datalist__label--variant_ruled">Genre</dt>
							<dd class="m-datalist__value m-datalist__value--variant_ruled">
								Court intrigue, low magic
							</dd>
						</div>
						<div class="m-datalist__item m-datalist__item--variant_ruled">
							<dt class="m-datalist__label m-datalist__label--variant_ruled">System</dt>
							<dd class="m-datalist__value m-datalist__value--variant_ruled">
								Freeform, d20 checks for contested actions
							</dd>
						</div>
						<div class="m-datalist__item m-datalist__item--variant_ruled">
							<dt class="m-datalist__label m-datalist__label--variant_ruled">Pace</dt>
							<dd class="m-datalist__value m-datalist__value--variant_ruled">
								One post a week in each of your scenes
							</dd>
						</div>
						<div class="m-datalist__item m-datalist__item--variant_ruled">
							<dt class="m-datalist__label m-datalist__label--variant_ruled">Posts</dt>
							<dd class="m-datalist__value m-datalist__value--variant_ruled">
								300 to 800 words, third person, past tense
							</dd>
						</div>
						<div class="m-datalist__item m-datalist__item--variant_ruled">
							<dt class="m-datalist__label m-datalist__label--variant_ruled">Players</dt>
							<dd class="m-datalist__value m-datalist__value--variant_ruled">7 of 9 seats</dd>
						</div>
						<div class="m-datalist__item m-datalist__item--variant_ruled">
							<dt class="m-datalist__label m-datalist__label--variant_ruled">Running since</dt>
							<dd class="m-datalist__value m-datalist__value--variant_ruled">September 2025</dd>
						</div>
					</dl>
				</section>

				<section
					aria-labelledby="h-content"
					style="display: flex; flex-direction: column; gap: 12px"
				>
					<h2 id="h-content" class="h-section">Content</h2>
					<div
						style="display: grid; grid-template-columns: max-content minmax(0, 1fr); gap: 12px 20px; align-items: start; font-size: 14px"
					>
						<span style="font-weight: 600; padding-top: 4px">In the story</span>
						<div style="display: flex; flex-wrap: wrap; gap: 6px">
							<span class="m-chip m-chip--kind_fact"><span>Violence</span></span><span
								class="m-chip m-chip--kind_fact"><span>Death and grief</span></span
							><span class="m-chip m-chip--kind_fact"><span>Political cruelty</span></span><span
								class="m-chip m-chip--kind_fact"><span>Alcohol</span></span
							>
						</div>
						<span style="font-weight: 600; padding-top: 4px">Off screen only</span>
						<div style="display: flex; flex-wrap: wrap; gap: 6px">
							<span class="m-chip m-chip--kind_fact"
								><span class="m-icon" aria-hidden="true">visibility_off</span><span>Torture</span
								></span
							><span class="m-chip m-chip--kind_fact"
								><span class="m-icon" aria-hidden="true">visibility_off</span><span
									>Romance beyond a kiss</span
								></span
							>
						</div>
						<span style="font-weight: 600; padding-top: 4px">Never</span>
						<div style="display: flex; flex-wrap: wrap; gap: 6px">
							<span class="m-chip m-chip--kind_fact m-chip--tone_danger"
								><span class="m-icon" aria-hidden="true">block</span><span>Sexual violence</span
								></span
							><span class="m-chip m-chip--kind_fact m-chip--tone_danger"
								><span class="m-icon" aria-hidden="true">block</span><span>Harm to children</span
								></span
							>
						</div>
					</div>
				</section>

				<section aria-labelledby="h-rules" style="display: flex; flex-direction: column; gap: 12px">
					<h2 id="h-rules" class="h-section">Table rules</h2>
					<div class="m-accordion__root m-accordion__root--variant_framed">
						{#each data.rules as r (r.title)}
							<details class="m-accordion__item m-accordion__item--variant_framed" open={r.open}>
								<summary class="m-accordion__trigger m-accordion__trigger--variant_framed"
									><span class="m-icon" aria-hidden="true" style="color: var(--ink-muted)"
										>{r.icon}</span
									><span class="m-accordion__title m-accordion__title--variant_framed"
										>{r.title}</span
									><span
										class="m-icon m-accordion__indicator m-accordion__indicator--variant_framed"
										aria-hidden="true">expand_more</span
									></summary
								>
								<div
									class="m-accordion__content m-accordion__content--variant_framed"
									style="font-family: var(--font-reading); font-size: 16px; line-height: 1.55"
								>
									{r.body}
								</div>
							</details>
						{/each}
					</div>
				</section>

				<section aria-labelledby="h-roles" style="display: flex; flex-direction: column; gap: 12px">
					<h2 id="h-roles" class="h-section">Open roles</h2>
					<div
						style="display: grid; grid-template-columns: repeat(auto-fill, minmax(min(220px, 100%), 1fr)); gap: 12px"
					>
						{#each data.roles as r (r.title)}
							<article
								class="m-ornamented m-pad"
								style="display: flex; flex-direction: column; gap: 8px; background-color: var(--surface-raised); border: var(--border-width, 1px) solid var(--frame-line, var(--line)); border-radius: var(--frame-radius, var(--radius-lg)); corner-shape: var(--frame-shape, var(--corner-shape)); box-shadow: var(--elevation-1), var(--edge-light)"
							>
								<span class="m-chip m-chip--kind_fact" style="align-self: flex-start"
									><span class="m-icon" aria-hidden="true">{r.icon}</span><span>{r.kind}</span
									></span
								>
								<h3
									style="margin: 0; font-family: var(--font-display); font-weight: var(--display-weight); font-size: calc(20px * var(--display-adjust, 1)); line-height: 1.2"
								>
									{r.title}
								</h3>
								<p
									style="margin: 0; font-family: var(--font-reading); font-size: 15px; line-height: 1.5; color: var(--ink)"
								>
									{r.body}
								</p>
								<span
									style="margin-top: auto; padding-top: 4px; font-size: 13px; color: var(--ink-muted)"
									>{r.status}</span
								>
							</article>
						{/each}
					</div>
				</section>

				<section aria-labelledby="h-read" style="display: flex; flex-direction: column; gap: 8px">
					<h2 id="h-read" class="h-section">Read before you apply</h2>
					<div style="display: flex; flex-direction: column">
						<a
							class="m-scene-row__root m-scene-row__root--unread_false"
							href={resolve('/w/[world]/play/[scene]', {
								world: 'gilded-reach',
								scene: 'feast-of-lanterns'
							})}
							style="grid-template-columns: 3px minmax(0, 1fr) auto 5ch"
						>
							<span class="m-scene-row__marker" aria-hidden="true"></span>
							<span class="m-scene-row__main"
								><span class="m-scene-row__title">The Feast of Lanterns</span><span
									class="m-scene-row__where"
									><span class="m-meta"
										><span class="m-icon m-icon--sm" aria-hidden="true">location_on</span><span
											>Harrowgate Keep, the great hall</span
										></span
									><span class="m-meta"
										><span class="m-icon m-icon--sm" aria-hidden="true">auto_stories</span><span
											>A Crown of Ash</span
										></span
									></span
								></span
							>
							<span class="m-scene-row__state"
								><span class="m-meta" style="font-size: 13px; color: var(--ink-muted)"
									><span class="m-icon m-icon--sm" aria-hidden="true">forum</span><span
										>24 posts</span
									></span
								></span
							>
							<time class="m-scene-row__time">2h</time>
						</a>
						<a
							class="m-scene-row__root m-scene-row__root--unread_false"
							href="#contraband"
							style="grid-template-columns: 3px minmax(0, 1fr) auto 5ch"
						>
							<span class="m-scene-row__marker" aria-hidden="true"></span>
							<span class="m-scene-row__main"
								><span class="m-scene-row__title">Contraband at Low Tide</span><span
									class="m-scene-row__where"
									><span class="m-meta"
										><span class="m-icon m-icon--sm" aria-hidden="true">location_on</span><span
											>Lantern Quay</span
										></span
									><span class="m-meta"
										><span class="m-icon m-icon--sm" aria-hidden="true">auto_stories</span><span
											>A Crown of Ash</span
										></span
									></span
								></span
							>
							<span class="m-scene-row__state"
								><span class="m-meta" style="font-size: 13px; color: var(--ink-muted)"
									><span class="m-icon m-icon--sm" aria-hidden="true">forum</span><span
										>9 posts</span
									></span
								></span
							>
							<time class="m-scene-row__time">1d</time>
						</a>
					</div>
				</section>
			</div>

			<aside
				class="m-world__rail"
				aria-label="Joining"
				style="display: flex; flex-direction: column; gap: 16px"
			>
				<div
					class="m-ornamented m-pad"
					style="display: flex; flex-direction: column; gap: 12px; background-color: var(--surface-raised); border: var(--border-width, 1px) solid var(--frame-line, var(--line)); border-radius: var(--frame-radius, var(--radius-lg)); corner-shape: var(--frame-shape, var(--corner-shape)); box-shadow: var(--elevation-2), var(--edge-light)"
				>
					<span class="m-flag m-flag--kind_done" style="align-self: flex-start"
						><span class="m-icon m-icon--sm" aria-hidden="true">how_to_reg</span>Recruiting</span
					>
					<div style="display: grid; gap: 8px; font-size: 14px">
						<span class="m-meta"
							><span class="m-icon m-icon--sm" aria-hidden="true">person_add</span><span
								>2 seats open</span
							></span
						>
						<span class="m-meta"
							><span class="m-icon m-icon--sm" aria-hidden="true">event</span><span
								>Applications close April 30</span
							></span
						>
						<span class="m-meta"
							><span class="m-icon m-icon--sm" aria-hidden="true">gavel</span><span
								>Reviewed by Aurelia and Bram</span
							></span
						>
					</div>
					<a
						class="m-button m-button--variant_filled"
						href={resolve('/w/[world]/apply', { world: 'gilded-reach' })}
						style="justify-content: center"
						><span class="m-icon" aria-hidden="true">person_add</span><span>Apply to join</span></a
					>
				</div>
				<div style="display: flex; flex-direction: column; gap: 10px">
					<h3 style="margin: 0; font: 600 13px/18px var(--font-ui); color: var(--ink-muted)">
						Game masters
					</h3>
					<div style="display: flex; gap: 10px; align-items: center">
						<span
							class="m-avatar m-avatar--kind_account m-avatar--size_sm"
							aria-hidden="true"
							style="--avatar-hue: 262"><span aria-hidden="true">AB</span></span
						><span style="display: grid; font-size: 14px"
							><span style="font-weight: 600">Aurelia</span><span
								style="font-size: 12.5px; color: var(--ink-muted)">@aurelia.bsky.social</span
							></span
						>
					</div>
					<div style="display: flex; gap: 10px; align-items: center">
						<span
							class="m-avatar m-avatar--kind_account m-avatar--size_sm"
							aria-hidden="true"
							style="--avatar-hue: 96"><span aria-hidden="true">BW</span></span
						><span style="display: grid; font-size: 14px"
							><span style="font-weight: 600">Bram</span><span
								style="font-size: 12.5px; color: var(--ink-muted)">@bram.writes</span
							></span
						>
					</div>
				</div>
				<div style="display: flex; flex-direction: column; gap: 10px">
					<h3 style="margin: 0; font: 600 13px/18px var(--font-ui); color: var(--ink-muted)">
						Tags
					</h3>
					<div style="display: flex; flex-wrap: wrap; gap: 6px">
						<span class="m-chip m-chip--kind_fact"><span>Court intrigue</span></span><span
							class="m-chip m-chip--kind_fact"><span>Low magic</span></span
						><span class="m-chip m-chip--kind_fact"><span>Slow burn</span></span><span
							class="m-chip m-chip--kind_fact"><span>Letters</span></span
						>
					</div>
				</div>
				<button
					class="m-button m-button--variant_text m-button--size_sm"
					type="button"
					style="align-self: flex-start; margin-inline-start: -8px"
					><span class="m-icon" aria-hidden="true">data_object</span><span>View World record</span
					></button
				>
			</aside>
		</div>
	</main>
</div>

<style>
	.h-section {
		margin: 0;
		font-family: var(--font-display);
		font-weight: var(--display-weight);
		font-size: calc(26px * var(--display-adjust, 1));
		line-height: 1.15;
		text-transform: var(--display-case);
		letter-spacing: var(--display-tracking);
		text-wrap: balance;
	}
</style>
