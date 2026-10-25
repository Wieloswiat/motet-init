<script lang="ts">
	import { resolve } from '$app/paths';
	import Chrome from '#lib/components/motet/Chrome.svelte';
	import WorldHeader from '#lib/components/motet/WorldHeader.svelte';
	import { world } from '#lib/motet/world.js';
	import type { WorldPlayData } from '#lib/demo/screens/world-play.js';

	let { data }: { data: WorldPlayData } = $props();
</script>

<svelte:head>
	<title>Locations and scenes</title>
</svelte:head>

<div {@attach world(data.worldRef)} class="m-world__root">
	<Chrome world="vellum" account="iga" inbox={data.inboxCount} />
	<WorldHeader current="Play" size="slim" role="player" />
	<main class="m-world__page" style="padding-top: 24px">
		<div
			style="display: flex; flex-wrap: wrap; gap: 12px; align-items: center; justify-content: space-between; margin-bottom: 20px"
		>
			<div
				class="m-scroll-row"
				role="group"
				aria-label="Show"
				style="flex: 1 1 320px; min-width: 0"
			>
				{#each data.filters as f (f.label)}
					<button class="m-chip m-chip--kind_filter" type="button" aria-pressed={f.pressed}
						><span class="m-icon" aria-hidden="true">{f.icon}</span><span>{f.label}</span
						>{#if f.hasCount}<span style="font-weight: 600; opacity: .75">{f.count}</span
							>{/if}</button
					>
				{/each}
			</div>
			<a
				class="m-button m-button--variant_filled"
				href={resolve('/w/[world]/play/new', { world: 'gilded-reach' })}
				><span class="m-icon" aria-hidden="true">add</span><span>New scene</span></a
			>
		</div>
		<div class="m-world__layout">
			<div style="display: flex; flex-direction: column; gap: 32px; min-width: 0">
				{#each data.locations as l (l.id)}
					<section
						id={l.id}
						class="m-vt"
						aria-labelledby={l.hid}
						style="display: flex; flex-direction: column; gap: 6px; scroll-margin-top: calc(var(--chrome-offset, 72px) + 12px); --vt: m-{l.id}"
					>
						<div style="display: flex; gap: 12px; align-items: baseline; flex-wrap: wrap">
							<span
								class="m-icon"
								aria-hidden="true"
								style="color: var(--ornament); align-self: center">{l.icon}</span
							>
							<h2 id={l.hid} class="h-loc">{l.name}</h2>
							{#if l.hasLore}<a
									class="m-button m-button--variant_text m-button--size_sm"
									href={resolve('/w/[world]/lore', { world: 'gilded-reach' })}
									style="margin-inline-start: auto"
									><span class="m-icon" aria-hidden="true">menu_book</span><span>Lore</span></a
								>{/if}
						</div>
						<p
							style="margin: 0 0 6px; font-family: var(--font-reading); font-size: 15.5px; line-height: 1.5; color: var(--ink-muted); max-width: 68ch"
						>
							{l.blurb}
						</p>
						<div style="display: flex; flex-direction: column">
							{#each l.scenes as s (s.title)}
								<a class={s.rootClass} href={s.href} style="--vt: {s.vtn}">
									<span class={s.markerClass} aria-hidden="true"></span>
									<span class="m-scene-row__main">
										<span class={s.titleClass}
											>{s.title}{#if s.unread}<span class="m-visually-hidden"
													>, has unread posts</span
												>{/if}</span
										>
										<span class="m-scene-row__where">
											<span class="m-meta"
												><span class="m-icon m-icon--sm" aria-hidden="true">{s.whereIcon}</span
												><span>{s.where}</span></span
											>
											{#if s.hasArc}<span class="m-meta"
													><span class="m-icon m-icon--sm" aria-hidden="true">auto_stories</span
													><span>{s.arc}</span></span
												>{/if}
											{#if s.private}<span class="m-meta"
													><span class="m-icon m-icon--sm" aria-hidden="true">lock</span><span
														>Cast only</span
													></span
												>{/if}
										</span>
									</span>
									<span class="m-scene-row__state">
										{#if s.hasFlag}<span class={s.flagClass}
												><span class="m-icon m-icon--sm" aria-hidden="true">{s.flagIcon}</span
												>{s.flagLabel}</span
											>{/if}
										{#if s.hasCount}<span class="m-count" title={s.countTip}>{s.count}</span>{/if}
									</span>
									<span class="m-scene-row__cast" title={s.castTip}>
										{#each s.cast as c (c)}<span
												class="m-avatar m-avatar--kind_character m-avatar--size_xs"
												aria-hidden="true"><img src={c} alt="" /></span
											>{/each}
									</span>
									<time class="m-scene-row__time">{s.time}</time>
								</a>
							{/each}
						</div>
					</section>
				{/each}
				{#if data.empty}
					<p style="margin: 0; padding: 24px 0; color: var(--ink-muted); font-size: 15px">
						No scenes here.
					</p>
				{/if}
			</div>

			<aside
				class="m-world__rail"
				aria-label="Board"
				style="display: flex; flex-direction: column; gap: 20px"
			>
				<nav aria-label="Locations" style="display: flex; flex-direction: column; gap: 2px">
					<h3 style="margin: 0 0 6px; font: 600 13px/18px var(--font-ui); color: var(--ink-muted)">
						Locations
					</h3>
					{#each data.index as i (i.name)}
						<a
							class="locnav"
							href={i.href}
							style="display: flex; align-items: center; gap: 10px; min-height: 36px; padding-inline: 8px; margin-inline: -8px; border-radius: var(--radius-md); font-size: 14px"
							><span class="m-icon" aria-hidden="true" style="color: var(--ink-muted)"
								>{i.icon}</span
							><span style="flex: 1">{i.name}</span><span
								style="font-size: 12.5px; color: var(--ink-muted)">{i.count}</span
							></a
						>
					{/each}
				</nav>
				<div style="display: flex; flex-direction: column; gap: 8px">
					<h3 style="margin: 0; font: 600 13px/18px var(--font-ui); color: var(--ink-muted)">
						Arcs
					</h3>
					<div style="display: grid; gap: 2px; font-size: 14px">
						<span style="font-weight: 600">A Crown of Ash</span><span
							style="font-size: 12.5px; color: var(--ink-muted)">Chapter one, since September</span
						>
					</div>
					<div style="display: grid; gap: 2px; font-size: 14px">
						<span style="font-weight: 600">Before the Feast</span><span
							style="font-size: 12.5px; color: var(--ink-muted)">Prologue, archived</span
						>
					</div>
				</div>
			</aside>
		</div>
	</main>
</div>

<style>
	.h-loc {
		margin: 0;
		font-family: var(--font-display);
		font-weight: var(--display-weight);
		font-size: calc(24px * var(--display-adjust, 1));
		line-height: 1.15;
		text-transform: var(--display-case);
		letter-spacing: var(--display-tracking);
	}
	.locnav {
		text-decoration: none;
		color: var(--ink);
	}
	.locnav:hover {
		background-color: var(--surface-quiet);
	}
</style>
