<script lang="ts">
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { at_motet_pilot_openscene } from '@motet/lexicons';
	import { procedure } from '#lib/api.js';
	import { isSceneKey } from '../records.js';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
	let worldParam = $derived(encodeURIComponent(data.world.did));
	let title = $state('');
	let castDraft = $state('');
	let cast = $state<string[]>([]);
	let submitting = $state(false);
	let message = $state('');
	let opened = $state(false);
	let openedScene = $state<string | null>(null);
	let operation = $state<string | null>(null);

	function castMember(value: string): string {
		return value.trim().replace(/^@/, '');
	}

	function addCast(): void {
		const member = castMember(castDraft);

		if (!member) {
			castDraft = '';

			return;
		}

		if (
			cast.some((existing) => existing.toLowerCase() === member.toLowerCase()) ||
			member.toLowerCase() === data.session?.handle.toLowerCase() ||
			member === data.session?.did
		) {
			castDraft = '';

			return;
		}

		if (cast.length >= 99) {
			message = 'A scene can have at most 100 cast members, including you.';

			return;
		}

		cast = [...cast, member];
		castDraft = '';
		message = '';
	}

	async function openScene(event: SubmitEvent): Promise<void> {
		event.preventDefault();

		if (!data.session || submitting || opened) return;
		addCast();

		if (castDraft) return;
		const sceneTitle = title.trim();

		if (!sceneTitle) {
			message = 'Enter a scene title.';

			return;
		}

		operation ??= crypto.randomUUID();
		submitting = true;
		message = '';

		try {
			const result = await procedure<
				at_motet_pilot_openscene.OpenSceneInput,
				at_motet_pilot_openscene.OpenSceneOutput
			>(at_motet_pilot_openscene.OpenSceneNsid, {
				world: data.world.did,
				title: sceneTitle,
				cast,
				op: operation
			});

			opened = true;

			if (!isSceneKey(result.scene)) {
				message =
					'The scene was opened, but its address was invalid. Look for it in the scene list.';

				return;
			}

			openedScene = result.scene;
			await goto(
				resolve('/w/[world]/play/[scene]', {
					world: worldParam,
					scene: encodeURIComponent(openedScene)
				})
			);
		} catch (cause) {
			message = openedScene
				? 'The scene was opened, but navigation failed. Use the link below to open it.'
				: cause instanceof Error
					? `Could not open the scene: ${cause.message}`
					: 'Could not open the scene. Please try again.';

			if (!openedScene)
				message += ' Retry with the same details, or reload to start a different scene.';
		} finally {
			submitting = false;
		}
	}
</script>

<svelte:head>
	<title>Open a scene · Motet</title>
</svelte:head>

<main class="m-world__page scene-form">
	<a href={resolve('/w/[world]/play', { world: worldParam })}>Back to scenes</a>
	<h1>Open a scene</h1>
	<p>World: {data.world.displayName ?? data.world.handle ?? data.world.did}</p>

	{#if data.session}
		<form onsubmit={openScene}>
			<div class="m-field__root">
				<label class="m-field__label" for="scene-title">Scene title</label>
				<div class="m-input__root">
					<input
						class="m-input__control"
						id="scene-title"
						bind:value={title}
						maxlength="1200"
						disabled={operation !== null}
						required
					/>
				</div>
			</div>

			<section aria-labelledby="cast-heading">
				<h2 id="cast-heading">Cast</h2>
				<p>Anyone in the cast can post. You are always included as the opener.</p>
				<ul>
					<li><span>{data.session.handle}</span> (<span>{data.session.did}</span>) — opener</li>
					{#each cast as member (member)}
						<li>
							{member}
							<button
								type="button"
								aria-label={`Remove ${member} from cast`}
								disabled={operation !== null}
								onclick={() => (cast = cast.filter((selected) => selected !== member))}
								>Remove</button
							>
						</li>
					{/each}
				</ul>
				<div class="m-field__root">
					<label class="m-field__label" for="cast-member">Add cast member</label>
					<p class="m-field__help" id="cast-help">Enter a handle or DID.</p>
					<div class="cast-input">
						<input
							class="m-input__control"
							id="cast-member"
							bind:value={castDraft}
							aria-describedby="cast-help"
							autocomplete="off"
							disabled={operation !== null}
						/>
						<button
							type="button"
							class="m-button m-button--variant_outline"
							onclick={addCast}
							disabled={operation !== null}
						>
							Add to cast
						</button>
					</div>
				</div>
			</section>

			{#if message}<p role="alert">{message}</p>{/if}
			{#if openedScene}<a
					href={resolve('/w/[world]/play/[scene]', {
						world: worldParam,
						scene: encodeURIComponent(openedScene)
					})}>Go to scene</a
				>{/if}
			<button
				type="submit"
				class="m-button m-button--variant_filled"
				disabled={submitting || opened}
			>
				{submitting ? 'Opening scene…' : 'Open scene'}
			</button>
		</form>
	{:else}
		<p>Sign in to open a scene.</p>
		<a href={resolve(`/sign-in?returnTo=${encodeURIComponent(data.returnTo)}`)}>Sign in</a>
	{/if}
</main>

<style>
	.scene-form {
		max-width: 48rem;
		margin-inline: auto;
	}

	form {
		display: grid;
		gap: 1.5rem;
	}

	.cast-input {
		display: flex;
		flex-wrap: wrap;
		gap: 0.5rem;
	}

	.cast-input input {
		flex: 1 1 15rem;
	}
</style>
