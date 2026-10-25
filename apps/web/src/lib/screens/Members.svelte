<script lang="ts">
	import { resolve } from '$app/paths';
	import Chrome from '#lib/components/motet/Chrome.svelte';
	import WorldHeader from '#lib/components/motet/WorldHeader.svelte';
	import { world } from '#lib/motet/world.js';
	import type { MembersData } from '#lib/demo/screens/members.js';

	let { data }: { data: MembersData } = $props();
</script>

<svelte:head>
	<title>Members</title>
</svelte:head>

<div {@attach world(data.worldRef)} class="m-world__root">
	<Chrome world="vellum" account="iga" inbox={data.inboxCount} />
	<WorldHeader current="Members" size="slim" role="player" />
	<main
		class="m-world__page"
		style="padding-top: 24px; display: flex; flex-direction: column; gap: 20px"
	>
		<div style="display: flex; flex-wrap: wrap; gap: 12px; align-items: center">
			<div class="m-input__root" style="flex: 1 1 220px; max-width: 300px">
				<span class="m-input__addon" data-side="start" aria-hidden="true"
					><span class="m-icon" aria-hidden="true">search</span></span
				><input
					class="m-input__control"
					type="search"
					placeholder="Find a member"
					aria-label="Find a member"
					value={data.query}
				/>
			</div>
			<div
				class="m-scroll-row"
				role="group"
				aria-label="Show"
				style="flex: 1 1 300px; min-width: 0"
			>
				{#each data.filters as f (f.label)}
					<button class="m-chip m-chip--kind_filter" type="button" aria-pressed={f.pressed}
						><span class="m-icon" aria-hidden="true">{f.icon}</span><span>{f.label}</span><span
							style="font-weight: 600; opacity: .75">{f.count}</span
						></button
					>
				{/each}
			</div>
		</div>

		<div
			role="table"
			aria-label="Members"
			style="display: flex; flex-direction: column; border-top: 1px solid var(--line)"
		>
			<div
				class="mhead"
				role="row"
				style="display: grid; grid-template-columns: minmax(0, 2fr) minmax(0, 1fr) minmax(0, 2fr) minmax(0, 1fr); gap: 16px; padding: 8px 12px; font: 600 12.5px/16px var(--font-ui); color: var(--ink-muted); border-bottom: 1px solid var(--line)"
			>
				<span role="columnheader">Member</span><span role="columnheader">Role</span><span
					role="columnheader">Characters</span
				><span role="columnheader">Last post</span>
			</div>
			{#each data.rows as m, i (i)}
				<div
					class="mrow mgrid m-vt m-vt-box"
					role="row"
					style="--vt: {m.vtn}; display: grid; grid-template-columns: minmax(0, 2fr) minmax(0, 1fr) minmax(0, 2fr) minmax(0, 1fr); gap: 16px; align-items: center; padding: 10px 12px; border-bottom: 1px solid var(--line)"
				>
					<span role="cell" style="display: flex; gap: 12px; align-items: center; min-width: 0">
						<span
							class="m-avatar m-avatar--kind_account m-avatar--size_md"
							aria-hidden="true"
							style={m.hue}><span aria-hidden="true">{m.initials}</span></span
						>
						<span style="display: grid; min-width: 0">
							<span style="display: flex; gap: 6px; align-items: center"
								><a
									class="plink"
									href={resolve('/profile/[handle]', { handle: 'aurelia.bsky.social' })}
									style="font-weight: 600; color: var(--ink); text-decoration: none">{m.display}</a
								>{#if m.you}<span style="font-size: 12.5px; color: var(--ink-muted)">you</span
									>{/if}</span
							>
							<span
								style="font-size: 13px; color: var(--ink-muted); overflow: hidden; text-overflow: ellipsis; white-space: nowrap"
								>@{m.handle}</span
							>
						</span>
					</span>
					<span role="cell" style="display: flex; flex-wrap: wrap; gap: 6px; align-items: center">
						<span class="m-role"
							><span class="m-icon" aria-hidden="true">{m.roleIcon}</span>{m.role}</span
						>
					</span>
					<span
						class="mwide"
						role="cell"
						style="display: flex; flex-wrap: wrap; gap: 6px 12px; align-items: center; min-width: 0"
					>
						{#each m.chars as c (c.name)}
							<a
								href={resolve('/w/[world]/characters/[character]', {
									world: 'gilded-reach',
									character: 'lady-maren'
								})}
								style="display: inline-flex; gap: 8px; align-items: center; text-decoration: none; color: var(--ink); min-width: 0"
								><span
									class="m-avatar m-avatar--kind_character m-avatar--size_xs"
									aria-hidden="true"><img src={c.portrait} alt="" /></span
								><span
									class="m-post__name"
									style="font-size: calc(14px * var(--name-adjust, 1)); white-space: nowrap"
									>{c.name}</span
								></a
							>
						{/each}
					</span>
					<span
						class="mwide"
						role="cell"
						style="display: grid; gap: 2px; font-size: 13px; color: var(--ink-muted)"
						><span>{m.last}</span>{#if m.away}<span
								class="m-meta"
								style="font-size: 12.5px; color: var(--ink)"
								><span class="m-icon m-icon--sm" aria-hidden="true">bedtime</span><span
									>{m.awayText}</span
								></span
							>{/if}</span
					>
				</div>
			{/each}
		</div>
		<div
			style="display: flex; gap: 12px; align-items: center; font-size: 14px; color: var(--ink-muted)"
		>
			<span style="display: flex"
				><span
					class="m-avatar m-avatar--kind_account m-avatar--size_xs"
					aria-hidden="true"
					style="--avatar-hue: 200; box-shadow: 0 0 0 2px var(--surface); margin-inline-end: -4px"
					><span aria-hidden="true">R</span></span
				><span
					class="m-avatar m-avatar--kind_account m-avatar--size_xs"
					aria-hidden="true"
					style="--avatar-hue: 20; box-shadow: 0 0 0 2px var(--surface); margin-inline-end: -4px"
					><span aria-hidden="true">M</span></span
				><span
					class="m-avatar m-avatar--kind_account m-avatar--size_xs"
					aria-hidden="true"
					style="--avatar-hue: 300; box-shadow: 0 0 0 2px var(--surface)"
					><span aria-hidden="true">J</span></span
				></span
			>
			<span>24 people watch this World without playing</span>
		</div>
	</main>
</div>

<style>
	.plink:hover {
		text-decoration: underline !important;
	}
	.mrow:hover {
		background-color: color-mix(in oklab, var(--surface-quiet) 70%, transparent);
	}
	@media (max-width: 720px) {
		.mgrid {
			grid-template-columns: minmax(0, 1fr) auto !important;
			row-gap: 8px !important;
		}
		.mgrid > .mwide {
			grid-column: 1 / -1;
			padding-inline-start: 52px;
		}
		.mhead {
			display: none !important;
		}
	}
</style>
