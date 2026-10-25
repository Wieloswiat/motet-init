<script lang="ts">
	import HomeBoard from '#lib/screens/HomeBoard.svelte';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
</script>

<HomeBoard data={data.screen} />
