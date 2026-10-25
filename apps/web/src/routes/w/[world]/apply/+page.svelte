<script lang="ts">
	import WorldApply from '#lib/screens/WorldApply.svelte';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
</script>

<WorldApply data={data.screen} />
