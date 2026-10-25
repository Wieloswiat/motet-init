<!--
	A World's masthead and tabs. The screen around it mounts the World's
	theme; the header only reads it. Visitors get Follow and Apply; players
	get their settings for this World.
-->
<script lang="ts">
	import { resolve } from '$app/paths';
	import { marks } from '#lib/demo/art.js';

	type Tab = 'Overview' | 'Play' | 'Characters' | 'Lore' | 'Members' | 'Manage' | 'Settings';

	let {
		current,
		size,
		role
	}: {
		current: Tab;
		/** `full` on the Overview, `slim` above everything else. */
		size: 'full' | 'slim';
		role: 'visitor' | 'applicant' | 'player' | 'gm';
	} = $props();

	const world = 'gilded-reach';

	// Members land on what is theirs; visitors and applicants on what the World is.
	const tabs = $derived([
		{
			icon: 'explore',
			label: 'Overview',
			href:
				role === 'player' || role === 'gm'
					? resolve('/w/[world]', { world })
					: resolve('/w/[world]/visitor', { world })
		},
		{ icon: 'forum', label: 'Play', href: resolve('/w/[world]/play', { world }) },
		{ icon: 'group', label: 'Characters', href: resolve('/w/[world]/characters', { world }) },
		{ icon: 'menu_book', label: 'Lore', href: resolve('/w/[world]/lore', { world }) },
		{ icon: 'groups', label: 'Members', href: resolve('/w/[world]/members', { world }) },
		...(role === 'gm'
			? [{ icon: 'shield_person', label: 'Manage', href: resolve('/w/[world]/manage', { world }) }]
			: [])
	]);

	let following = $state(false);
</script>

<div class="m-world__backdrop" aria-hidden="true"></div>
<header class="m-masthead__root m-masthead__root--size_{size}">
	<div class="m-masthead__art m-masthead__art--size_{size}">
		<span class="m-masthead__scrim m-masthead__scrim--size_{size}" aria-hidden="true"></span>
	</div>
	<div class="m-masthead__inner m-masthead__inner--size_{size}">
		<div class="m-masthead__identity m-masthead__identity--size_{size}">
			<span
				class="m-avatar m-avatar--kind_world m-masthead__mark m-masthead__mark--size_{size}"
				aria-hidden="true"><img src={marks.vellum} alt="" /></span
			>
			<div class="m-masthead__titles m-masthead__titles--size_{size}">
				<h1 class="m-masthead__title m-masthead__title--size_{size}">The Gilded Reach</h1>
				<div class="m-masthead__byline m-masthead__byline--size_{size}">
					<span class="m-meta"
						><span class="m-icon m-icon--sm" aria-hidden="true">shield_person</span><span
							>Aurelia, Bram</span
						></span
					>
					<span class="m-meta"
						><span class="m-icon m-icon--sm" aria-hidden="true">how_to_reg</span><span
							>Recruiting</span
						></span
					>
					<span class="m-meta"
						><span class="m-icon m-icon--sm" aria-hidden="true">translate</span><span>English</span
						></span
					>
				</div>
			</div>
		</div>
		<!-- The tabs scroll and the settings button stays beside them; visitors' buttons may wrap under. -->
		<div
			class="m-masthead__nav m-masthead__nav--size_{size}"
			style:flex-wrap={role === 'visitor' ? 'wrap' : 'nowrap'}
			style="display: flex; align-items: center; gap: 8px 12px"
		>
			<nav class="m-tabs__root" aria-label="World" style="flex: 1 1 auto; min-width: 0">
				{#each tabs as t (t.label)}
					<a
						class="m-tabs__tab"
						href={t.href}
						aria-current={t.label === current ? 'page' : undefined}
						><span class="m-icon" aria-hidden="true">{t.icon}</span>{t.label}</a
					>
				{/each}
			</nav>
			{#if role === 'visitor'}
				<div style="display: flex; gap: 8px; align-items: center; padding-inline: 12px">
					<button
						class="m-button m-button--variant_text"
						type="button"
						aria-pressed={following}
						onclick={() => (following = !following)}
						><span class="m-icon" aria-hidden="true">{following ? 'check' : 'bookmark_add'}</span
						><span>{following ? 'Following' : 'Follow'}</span></button
					>
					<a class="m-button m-button--variant_filled" href={resolve('/w/[world]/apply', { world })}
						><span class="m-icon" aria-hidden="true">person_add</span><span>Apply to join</span></a
					>
				</div>
			{/if}
			{#if role === 'player'}
				<div
					style="display: flex; gap: 4px; align-items: center; padding-inline: 4px 12px; flex: none"
				>
					<a
						class="m-icon-button m-icon-button--variant_standard"
						href={resolve('/w/[world]/settings', { world })}
						aria-label="Your settings in this World"
						title="Your settings in this World"
						aria-current={current === 'Settings' ? 'page' : undefined}
						><span class="m-icon" aria-hidden="true">tune</span></a
					>
				</div>
			{/if}
		</div>
	</div>
</header>
