<script lang="ts">
	import Inbox from '#lib/screens/Inbox.svelte';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
</script>

<Inbox data={data.screen} />
