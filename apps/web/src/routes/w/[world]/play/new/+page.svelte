<script lang="ts">
	import NewScene from '#lib/screens/NewScene.svelte';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
</script>

<NewScene data={data.screen} />
