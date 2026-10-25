<script lang="ts">
	import Characters from '#lib/screens/Characters.svelte';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
</script>

<Characters data={data.screen} />
