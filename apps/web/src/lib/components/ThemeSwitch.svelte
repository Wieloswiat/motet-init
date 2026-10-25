<!--
	Chooses the reader's theme. A World can still restyle the subtree it
	governs; this sets the base the reader sees everywhere else.
-->
<script lang="ts" module>
	export const themes = ['light', 'dark', 'contrast-light', 'contrast-dark'] as const;

	export type ThemeName = (typeof themes)[number];
</script>

<script lang="ts">
	import { SegmentGroup } from '@ark-ui/svelte/segment-group';
	import { css } from '#styled/css';

	let { value = $bindable('light') }: { value?: ThemeName } = $props();

	function selectTheme(event: SegmentGroup.ValueChangeDetails): void {
		// SAFETY: Ark emits the values of the items rendered from themes above.
		value = event.value as ThemeName;
	}

	const item = css({
		paddingInline: '3',
		paddingBlock: '1',
		borderRadius: 'control',
		color: 'ink',
		cursor: 'pointer',
		_checked: { background: 'accent', color: 'on-accent' }
	});
</script>

<SegmentGroup.Root
	{value}
	onValueChange={selectTheme}
	class={css({ display: 'flex', gap: '1', background: 'surface-quiet', padding: '1' })}
>
	<SegmentGroup.Label class={css({ srOnly: true })}>Theme</SegmentGroup.Label>
	{#each themes as theme (theme)}
		<SegmentGroup.Item value={theme} class={item}>
			<SegmentGroup.ItemText>{theme}</SegmentGroup.ItemText>
			<SegmentGroup.ItemControl />
			<SegmentGroup.ItemHiddenInput />
		</SegmentGroup.Item>
	{/each}
</SegmentGroup.Root>
