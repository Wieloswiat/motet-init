<script lang="ts">
	import WorldMember from '#lib/screens/WorldMember.svelte';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
</script>

<WorldMember data={data.screen} />
