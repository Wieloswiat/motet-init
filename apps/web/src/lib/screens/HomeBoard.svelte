<script lang="ts">
	import { resolve } from '$app/paths';
	import Chrome from '#lib/components/motet/Chrome.svelte';
	import LanguagePicker from '#lib/components/motet/LanguagePicker.svelte';
	import { world } from '#lib/motet/world.js';
	import type { HomeBoardData } from '#lib/demo/screens/home-board.js';

	let { data }: { data: HomeBoardData } = $props();
</script>

<svelte:head>
	<title>Home, notice board</title>
</svelte:head>

<div {@attach world(data.pageRef)} class="m-page__root">
	<Chrome account="nadia" inbox={data.inboxCount} />
	<main class="m-page__main">
		<div style="display: flex; flex-wrap: wrap; gap: 32px; align-items: flex-start">
			<div
				style="flex: 999 1 560px; min-width: 0; display: flex; flex-direction: column; gap: 40px"
			>
				<section aria-labelledby="h-calls" style="display: flex; flex-direction: column; gap: 16px">
					<div
						style="display: flex; align-items: center; justify-content: space-between; gap: 12px; flex-wrap: wrap"
					>
						<h2 id="h-calls" style="margin: 0; font: 600 20px/28px var(--font-ui)">
							Recruiting calls
						</h2>
						<div style="flex: 0 1 280px; min-width: 200px">
							<LanguagePicker
								label="Languages"
								hideLabel
								placeholder="Any language"
								value={data.langCodes}
								pid="calls-lang"
							/>
						</div>
					</div>
					{#each data.calls as c (c.key)}
						<article
							{@attach world(c.ref)}
							style="display: flex; flex-direction: column; min-width: 0; color: var(--ink); background-color: var(--surface); background-image: var(--grain); border: 1px solid var(--line); border-radius: var(--radius-lg); corner-shape: var(--corner-shape); overflow: hidden; box-shadow: var(--elevation-1), var(--edge-light)"
						>
							<div
								aria-hidden="true"
								style="height: 64px; background-image: var(--art); background-size: cover; background-position: var(--art-focus, 50% 50%)"
							></div>
							<div style="display: flex; flex-direction: column; gap: 12px; padding: 0 20px 18px">
								<div style="display: flex; gap: 12px; align-items: flex-end">
									<span
										class="m-avatar m-avatar--kind_world m-avatar--size_md"
										aria-hidden="true"
										style="margin-top: -20px; box-shadow: 0 0 0 3px var(--surface)"
										><img src={c.mark} alt="" /></span
									>
									<span
										style="display: flex; flex-wrap: wrap; align-items: baseline; gap: 4px 12px; min-width: 0; flex: 1"
									>
										<span
											style="font-family: var(--font-display); font-weight: var(--display-weight); font-size: calc(17px * var(--display-adjust, 1)); text-transform: var(--display-case); letter-spacing: var(--display-tracking)"
											>{c.world}</span
										>
										<span
											style="display: inline-flex; gap: 6px; align-items: center; font-size: 13px; line-height: 18px; color: var(--ink-muted)"
											>{c.by}<span class="m-role"
												><span class="m-icon" aria-hidden="true">shield_person</span>GM</span
											></span
										>
									</span>
									<time style="font-size: 13px; color: var(--ink-muted); white-space: nowrap"
										>{c.when}</time
									>
								</div>
								<h3
									style="margin: 0; font-family: var(--font-display); font-weight: var(--display-weight); font-size: calc(24px * var(--display-adjust, 1)); line-height: 1.15; text-transform: var(--display-case); letter-spacing: var(--display-tracking); text-wrap: balance"
								>
									{c.title}
								</h3>
								<p
									style="margin: 0; max-width: 68ch; font-family: var(--font-reading); font-size: 16px; line-height: 1.55; text-wrap: pretty"
								>
									{c.body}
								</p>
								<div
									style="display: flex; flex-wrap: wrap; gap: 6px 16px; font-size: 13px; line-height: 18px; color: var(--ink-muted)"
								>
									<span class="m-meta"
										><span class="m-icon m-icon--sm" aria-hidden="true">person_add</span><span
											>{c.seats}</span
										></span
									>
									<span class="m-meta"
										><span class="m-icon m-icon--sm" aria-hidden="true">event</span><span
											>{c.closes}</span
										></span
									>
									<span class="m-meta"
										><span class="m-icon m-icon--sm" aria-hidden="true">schedule</span><span
											>{c.pace}</span
										></span
									>
									<span class="m-meta"
										><span class="m-icon m-icon--sm" aria-hidden="true">casino</span><span
											>{c.system}</span
										></span
									>
									<span class="m-meta"
										><span class="m-icon m-icon--sm" aria-hidden="true">translate</span><span
											>{c.lang}</span
										></span
									>
								</div>
								<div style="display: flex; gap: 8px; flex-wrap: wrap">
									<a class="m-button m-button--variant_tonal m-button--size_sm" href={c.apply}
										><span class="m-icon" aria-hidden="true">person_add</span><span>Apply</span></a
									>
									<a class="m-button m-button--variant_text m-button--size_sm" href={c.about}
										><span>About this World</span></a
									>
								</div>
							</div>
						</article>
					{/each}
				</section>

				<section aria-labelledby="h-read" style="display: flex; flex-direction: column; gap: 16px">
					<h2 id="h-read" style="margin: 0; font: 600 20px/28px var(--font-ui)">Read a scene</h2>
					<div
						style="display: grid; grid-template-columns: repeat(auto-fill, minmax(min(300px, 100%), 1fr)); gap: 16px"
					>
						{#each data.reads as r (r.name)}
							<a
								{@attach world(r.ref)}
								href="#read"
								style="display: flex; flex-direction: column; gap: 12px; min-width: 0; padding: 16px 18px 18px; text-decoration: none; color: var(--ink); background-color: var(--surface); background-image: var(--grain); border: 1px solid var(--line); border-radius: var(--radius-lg); corner-shape: var(--corner-shape); box-shadow: var(--elevation-1), var(--edge-light)"
							>
								<span
									style="display: flex; gap: 8px; align-items: center; font-size: 13px; line-height: 18px; color: var(--ink-muted)"
									><span class="m-avatar m-avatar--kind_world m-avatar--size_xs" aria-hidden="true"
										><img src={r.mark} alt="" /></span
									>{r.world}</span
								>
								<span
									style="font-family: var(--font-display); font-weight: var(--display-weight); font-size: calc(20px * var(--display-adjust, 1)); line-height: 1.15; text-transform: var(--display-case); letter-spacing: var(--display-tracking)"
									>{r.title}</span
								>
								<span style="display: flex; gap: 10px; align-items: center">
									<span
										class="m-avatar m-avatar--kind_character m-avatar--size_sm"
										aria-hidden="true"><img src={r.portrait} alt="" /></span
									>
									<span style="display: grid; min-width: 0">
										<span class="m-post__name" style="font-size: calc(15px * var(--name-adjust, 1))"
											>{r.name}</span
										>
										<span style="font-size: 12.5px; line-height: 16px; color: var(--ink-muted)"
											>{r.epithet}</span
										>
									</span>
								</span>
								<span
									class="excerpt"
									style="font-family: var(--font-reading); font-size: 15.5px; line-height: 1.55; color: var(--ink)"
									>{r.text}</span
								>
								<span
									style="display: inline-flex; gap: 4px; align-items: center; font-size: 13px; font-weight: 600; color: var(--accent-ink)"
									>Read the scene<span class="m-icon m-icon--sm" aria-hidden="true"
										>arrow_forward</span
									></span
								>
							</a>
						{/each}
					</div>
				</section>
			</div>

			<aside
				aria-label="Your play"
				style="flex: 1 1 300px; min-width: 0; display: flex; flex-direction: column; gap: 20px"
			>
				<section
					class="m-page__panel"
					aria-labelledby="h-turn"
					style="padding: 16px 16px 10px; gap: 8px"
				>
					<h2 id="h-turn" style="margin: 0 0 4px; font: 600 15px/20px var(--font-ui)">Your turn</h2>
					<a
						class="rowlink"
						href="#hollow-plebania"
						style="display: grid; grid-template-columns: auto minmax(0, 1fr); gap: 2px 10px; align-items: center; padding: 8px; margin-inline: -8px; border-radius: var(--radius-md); text-decoration: none; color: var(--ink)"
					>
						<span
							class="m-avatar m-avatar--kind_world m-avatar--size_sm"
							aria-hidden="true"
							style="grid-row: span 2"><img src={data.marks.hollow} alt="" /></span
						>
						<span style="font-weight: 600">Plebania nocą</span>
						<span
							style="display: flex; gap: 8px; align-items: center; font-size: 13px; color: var(--ink-muted)"
							>Hollowmere<span class="m-flag m-flag--kind_turn"
								><span class="m-icon m-icon--sm" aria-hidden="true">front_hand</span>Your turn</span
							></span
						>
					</a>
				</section>
				<section class="m-page__panel" aria-labelledby="h-worlds" style="padding: 16px 16px 10px">
					<h2 id="h-worlds" style="margin: 0 0 8px; font: 600 15px/20px var(--font-ui)">
						Your Worlds
					</h2>
					<a
						class="rowlink"
						href="#hollow"
						style="display: grid; grid-template-columns: auto minmax(0, 1fr); gap: 0 10px; align-items: center; padding: 8px; margin-inline: -8px; border-radius: var(--radius-md); text-decoration: none; color: var(--ink)"
					>
						<span
							class="m-avatar m-avatar--kind_world m-avatar--size_sm"
							aria-hidden="true"
							style="grid-row: span 2"><img src={data.marks.hollow} alt="" /></span
						>
						<span style="font-weight: 600">Hollowmere</span>
						<span style="font-size: 13px; color: var(--ink-muted)">Ida Wilk</span>
					</a>
					<a
						class="rowlink"
						href="#neon"
						style="display: grid; grid-template-columns: auto minmax(0, 1fr); gap: 0 10px; align-items: center; padding: 8px; margin-inline: -8px; border-radius: var(--radius-md); text-decoration: none; color: var(--ink)"
					>
						<span
							class="m-avatar m-avatar--kind_world m-avatar--size_sm"
							aria-hidden="true"
							style="grid-row: span 2"><img src={data.marks.neon} alt="" /></span
						>
						<span style="font-weight: 600">Neon Meridian</span>
						<span
							style="display: inline-flex; gap: 4px; align-items: center; font-size: 13px; color: var(--ink-muted)"
							><span class="m-icon m-icon--sm" aria-hidden="true">visibility</span>Watching</span
						>
					</a>
					<div
						style="display: flex; flex-wrap: wrap; gap: 4px; margin-top: 4px; margin-inline-start: -8px"
					>
						<a class="m-button m-button--variant_text m-button--size_sm" href={resolve('/')}
							><span>All your Worlds</span></a
						>
						<a
							class="m-button m-button--variant_text m-button--size_sm"
							href={resolve('/new-world')}
							><span class="m-icon" aria-hidden="true">add</span><span>Start a World</span></a
						>
					</div>
				</section>
			</aside>
		</div>
	</main>
</div>

<style>
	.excerpt {
		display: -webkit-box;
		-webkit-line-clamp: 5;
		line-clamp: 5;
		-webkit-box-orient: vertical;
		overflow: hidden;
	}
	.rowlink:hover {
		background-color: color-mix(in oklab, var(--ink) 6%, transparent);
	}
</style>
