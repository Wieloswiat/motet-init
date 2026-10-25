<script lang="ts">
	import Members from '#lib/screens/Members.svelte';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
</script>

<Members data={data.screen} />
