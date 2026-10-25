<script lang="ts">
	import WorldVisitor from '#lib/screens/WorldVisitor.svelte';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
</script>

<WorldVisitor data={data.screen} />
