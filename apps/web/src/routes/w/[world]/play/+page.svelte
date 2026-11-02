<script lang="ts">
	import { resolve } from '$app/paths';
	import { decodeScene } from './records.js';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
	let worldParam = $derived(encodeURIComponent(data.world.did));
</script>

<svelte:head>
	<title>Scenes · Motet</title>
</svelte:head>

<main class="m-world__page">
	<nav aria-label="World navigation">
		<a href={resolve('/w/[world]', { world: worldParam })}
			>{data.world.displayName ?? data.world.handle ?? data.world.did}</a
		>
	</nav>
	<header class="scene-list-heading">
		<div>
			<h1>Scenes</h1>
			<p>Public scenes in this World.</p>
		</div>
		{#if data.session}
			<a
				class="m-button m-button--variant_filled"
				href={resolve('/w/[world]/play/new', { world: worldParam })}>Open scene</a
			>
		{:else}
			<a href={resolve(`/sign-in?returnTo=${encodeURIComponent(`/w/${worldParam}/play/new`)}`)}>
				Sign in to open a scene
			</a>
		{/if}
	</header>

	{#if data.listing.scenes.length === 0}
		<p>No scenes found.</p>
	{:else}
		<ul class="scene-list" aria-label="Scenes">
			{#each data.listing.scenes as record (record.uri)}
				{@const scene = decodeScene(record, data.world.did)}
				<li>
					{#if scene}
						<a href={resolve('/w/[world]/play/[scene]', {
							world: worldParam,
							scene: encodeURIComponent(scene.key)
						})}>{scene.title}</a>
						<span>{scene.state}</span>
					{:else}
						<span>A scene could not be displayed because its record is invalid.</span>
					{/if}
				</li>
			{/each}
		</ul>
	{/if}

	{#if data.listing.cursor}
		<a href={resolve(`/w/[world]/play?cursor=${encodeURIComponent(data.listing.cursor)}`, {
			world: worldParam
		})}>More scenes</a>
	{/if}
</main>

<style>
	.scene-list-heading {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		justify-content: space-between;
		gap: 1rem;
	}

	.scene-list {
		list-style: none;
		padding: 0;
	}

	.scene-list li {
		display: flex;
		flex-wrap: wrap;
		justify-content: space-between;
		gap: 1rem;
		padding: 1rem 0;
		border-bottom: 1px solid var(--line);
	}
</style>
