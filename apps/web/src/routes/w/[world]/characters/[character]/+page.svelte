<script lang="ts">
	import CharacterSheet from '#lib/screens/CharacterSheet.svelte';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
</script>

<CharacterSheet data={data.screen} />
