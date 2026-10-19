<script lang="ts">
	import ThemeSwitch, { type ThemeName } from '#lib/components/ThemeSwitch.svelte';
	import { css } from '#styled/css';

	let theme: ThemeName = $state('light');
</script>

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
	<h1 class={css({ fontFamily: 'display', color: 'accent-ink' })}>Motet</h1>
	<ThemeSwitch bind:value={theme} />
</main>
