<script lang="ts">
	import Scene from '#lib/screens/Scene.svelte';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
</script>

<Scene data={data.screen} />
