<script lang="ts">
	import { resolve } from '$app/paths';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
	let worldPath = $derived(encodeURIComponent(data.world.did));
</script>

<svelte:head>
	<title>{data.world.displayName ?? data.world.handle ?? data.world.did} · Motet</title>
</svelte:head>

<div class="m-world__root">
	<main class="m-world__page overview">
		<header class="world-heading">
			<a class="m-button m-button--variant_text m-button--size_sm home" href={resolve('/')}>Home</a>
			<h1>{data.world.displayName ?? data.world.handle ?? data.world.did}</h1>
			{#if data.world.handle}<p class="identity">@{data.world.handle}</p>{/if}
			<p class="identity did">{data.world.did}</p>
		</header>

		<section aria-labelledby="scenes-heading" class="scenes">
			<div class="section-heading">
				<h2 id="scenes-heading">Scenes</h2>
				<div class="section-actions">
					<a class="m-button m-button--variant_text m-button--size_sm" href={resolve('/w/[world]/play', { world: worldPath })}>All scenes</a>
					{#if data.session}
						<a class="m-button m-button--variant_filled m-button--size_sm" href={resolve('/w/[world]/play/new', { world: worldPath })}>Open scene</a>
					{/if}
				</div>
			</div>
			{#if data.scenesUnavailable}
				<p role="alert">Scenes could not be loaded from the AppView. Please try again.</p>
			{:else}
				{#if data.invalidScenes}<p role="alert">Some scene records could not be displayed because they were invalid.</p>{/if}
				{#if data.scenes.length > 0}
					<div class="scene-list">
						{#each data.scenes as scene (scene.key)}
							<a class="m-scene-row__root m-scene-row__root--unread_false scene-row" href={resolve('/w/[world]/play/[scene]', { world: worldPath, scene: encodeURIComponent(scene.key) })}>
								<span class="m-scene-row__main m-scene-row__main--unread_false">
									<span class="m-scene-row__title m-scene-row__title--unread_false">{scene.title}</span>
								</span>
								<span class="scene-state">{scene.state}</span>
							</a>
						{/each}
					</div>
				{:else if !data.invalidScenes}
					<p class="empty">This World has no public scenes yet.</p>
				{/if}
				{#if data.hasMoreScenes}<p class="empty">More scenes are available in <a href={resolve('/w/[world]/play', { world: worldPath })}>all scenes</a>.</p>{/if}
			{/if}
			{#if data.session}<p class="empty">Your World permissions determine whether you can open a scene.</p>{/if}
		</section>
	</main>
</div>

<style>
	.overview { display: flex; flex-direction: column; gap: 36px; padding-top: 28px; }
	.world-heading { display: flex; flex-direction: column; align-items: flex-start; gap: 6px; }
	.home { margin-inline-start: -8px; }
	.world-heading h1 { margin: 0; font-family: var(--font-display); font-weight: var(--display-weight); font-size: calc(36px * var(--display-adjust, 1)); line-height: 1.15; text-transform: var(--display-case); letter-spacing: var(--display-tracking); overflow-wrap: anywhere; }
	.identity { margin: 0; color: var(--ink-muted); font-size: 14px; }
	.did { overflow-wrap: anywhere; }
	.scenes { display: flex; flex-direction: column; gap: 12px; max-width: 820px; }
	.section-heading { display: flex; gap: 12px; align-items: baseline; justify-content: space-between; flex-wrap: wrap; }
	.section-heading h2 { margin: 0; font-family: var(--font-display); font-weight: var(--display-weight); font-size: calc(26px * var(--display-adjust, 1)); line-height: 1.15; }
	.section-actions { display: flex; gap: 8px; flex-wrap: wrap; }
	.scene-list { display: flex; flex-direction: column; }
	.scene-row { grid-template-columns: minmax(0, 1fr) auto; }
	.scene-state { color: var(--ink-muted); font-size: 13px; }
	.empty { margin: 0; color: var(--ink-muted); font-size: 14px; line-height: 1.5; }
</style>
