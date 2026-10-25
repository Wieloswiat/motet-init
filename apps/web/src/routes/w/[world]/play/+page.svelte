<script lang="ts">
	import WorldPlay from '#lib/screens/WorldPlay.svelte';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
</script>

<WorldPlay data={data.screen} />
