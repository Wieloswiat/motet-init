<script lang="ts">
	import SignIn from '#lib/screens/SignIn.svelte';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
</script>

<SignIn returnTo={data.returnTo} />
