<script lang="ts">
	import Profile from '#lib/screens/Profile.svelte';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
</script>

<Profile data={data.screen} />
