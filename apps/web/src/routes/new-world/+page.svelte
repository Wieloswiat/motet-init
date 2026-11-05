<script lang="ts">
	import StartWorld from '#lib/screens/StartWorld.svelte';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
</script>

<StartWorld signedIn={data.session !== null} />
