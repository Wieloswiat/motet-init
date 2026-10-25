<script lang="ts">
	import WorldManage from '#lib/screens/WorldManage.svelte';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
</script>

<WorldManage data={data.screen} />
