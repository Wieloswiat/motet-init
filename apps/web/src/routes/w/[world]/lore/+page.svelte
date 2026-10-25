<script lang="ts">
	import Lore from '#lib/screens/Lore.svelte';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
</script>

<Lore data={data.screen} />
