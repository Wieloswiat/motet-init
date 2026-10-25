<script lang="ts">
	import Settings from '#lib/screens/Settings.svelte';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
</script>

<Settings data={data.screen} />
