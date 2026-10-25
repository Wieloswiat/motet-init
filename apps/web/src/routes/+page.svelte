<script lang="ts">
	import HomeShelves from '#lib/screens/HomeShelves.svelte';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
</script>

<HomeShelves data={data.screen} />
