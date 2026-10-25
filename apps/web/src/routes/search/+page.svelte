<script lang="ts">
	import Search from '#lib/screens/Search.svelte';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
</script>

<Search data={data.screen} />
