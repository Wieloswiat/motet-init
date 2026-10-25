<script lang="ts">
	import WorldSettings from '#lib/screens/WorldSettings.svelte';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
</script>

<WorldSettings data={data.screen} />
