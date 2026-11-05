<script lang="ts">
	import { resolve } from '$app/paths';
	import ThemeSwitch, { type ThemeName } from '#lib/components/ThemeSwitch.svelte';
	import { css } from '#styled/css';

	let theme: ThemeName = $state('light');

	// These remaining design-export screens still show sample content.
	const world = 'gilded-reach';

	const screens = [
		{ href: resolve('/'), label: 'Home: shelves' },
		{ href: resolve('/board'), label: 'Home: notice board (alternative)' },
		{ href: resolve('/inbox'), label: 'Inbox' },
		{ href: resolve('/search'), label: 'Search' },
		{ href: resolve('/settings'), label: 'Settings' },
		{
			href: resolve('/profile/[handle]', { handle: 'aurelia.bsky.social' }),
			label: 'Profile: someone else'
		},
		{ href: resolve('/profile/[handle]', { handle: 'iga.bsky.social' }), label: 'Profile: yours' },
		{ href: resolve('/w/[world]/visitor', { world }), label: 'World overview: visitor' },
		{ href: resolve('/w/[world]/apply', { world }), label: 'Apply and create a character' },
		{ href: resolve('/w/[world]/characters', { world }), label: 'Characters' },
		{
			href: resolve('/w/[world]/characters/[character]', { world, character: 'lady-maren' }),
			label: 'Character sheet'
		},
		{ href: resolve('/w/[world]/lore', { world }), label: 'Lore' },
		{ href: resolve('/w/[world]/members', { world }), label: 'Members' },
		{ href: resolve('/w/[world]/settings', { world }), label: 'Your settings in this World' },
		{ href: resolve('/w/[world]/manage', { world }), label: 'GM view and World settings' }
	];
</script>

<svelte:head>
	<title>Motet design</title>
</svelte:head>

<main
	data-theme={theme}
	class={css({
		minHeight: '[100dvh]',
		background: 'canvas',
		color: 'ink',
		fontFamily: 'ui',
		padding: '8',
		display: 'grid',
		alignContent: 'start',
		gap: '4'
	})}
>
	<h1 class={css({ fontFamily: 'display', color: 'accent.ink' })}>Motet</h1>
	<ThemeSwitch bind:value={theme} />
	<h2>Demo screens</h2>
	<p>These design studies show sample content and do not use the AppView.</p>
	<nav aria-label="Demo screens">
		<ul class={css({ display: 'grid', gap: '2', paddingInlineStart: '6' })}>
			{#each screens as screen (screen.href)}
				<li><a class={css({ color: 'accent.ink' })} href={screen.href}>{screen.label}</a></li>
			{/each}
		</ul>
	</nav>
</main>
