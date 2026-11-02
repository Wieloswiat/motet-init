<script lang="ts">
	import { resolve } from '$app/paths';
	import { untrack } from 'svelte';
	import {
		at_motet_pilot_createpost,
		at_motet_pilot_getscene,
		type at_motet_pilot_querydefs
	} from '@motet/lexicons';
	import { procedure, query, XrpcError } from '#lib/api.js';
	import { decodePost, decodeScene } from '../records.js';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
	let worldParam = $derived(encodeURIComponent(data.world.did));

	let scenePage = $state<at_motet_pilot_getscene.GetSceneOutput | null>(
		untrack(() => data.scenePage)
	);

	let posts = $state<at_motet_pilot_querydefs.AdmittedPost[]>(
		untrack(() => data.scenePage?.posts ?? [])
	);

	let cursor = $state<string | undefined>(untrack(() => data.scenePage?.cursor));
	let scene = $derived(scenePage ? decodeScene(scenePage.scene, data.world.did) : null);
	let sceneLoading = $state(false);
	let sceneError = $state('');
	let moreLoading = $state(false);
	let moreError = $state('');
	let body = $state('');
	let register = $state('in-character');
	let submitting = $state(false);
	let postStatus = $state('');
	let sceneController: AbortController | null = null;
	let postController: AbortController | null = null;
	let moreController: AbortController | null = null;

	const pollBudget = 20_000;
	const pollInterval = 1_000;

	$effect(() => {
		const initialPage = data.scenePage;
		const actor = data.world.did;
		const sceneKey = data.sceneKey;
		scenePage = initialPage;
		posts = initialPage?.posts ?? [];
		cursor = initialPage?.cursor;
		sceneError = '';
		moreError = '';
		postStatus = '';
		body = '';
		submitting = false;
		moreLoading = false;

		if (!initialPage) startScenePoll(actor, sceneKey);

		return () => {
			sceneController?.abort();
			postController?.abort();
			moreController?.abort();
		};
	});

	function pause(ms: number, signal: AbortSignal): Promise<void> {
		return new Promise((resolve) => {
			if (signal.aborted) {
				resolve();

				return;
			}

			const timer = window.setTimeout(finish, ms);

			function finish(): void {
				window.clearTimeout(timer);
				signal.removeEventListener('abort', finish);
				resolve();
			}

			signal.addEventListener('abort', finish, { once: true });
		});
	}

	function getPage(
		actor: string,
		sceneKey: string,
		pageCursor: string | undefined,
		signal: AbortSignal,
		deadline: number
	): Promise<at_motet_pilot_getscene.GetSceneOutput> {
		const boundedFetch: typeof fetch = (input, init) =>
			fetch(input, {
				...init,
				signal: AbortSignal.any([signal, AbortSignal.timeout(Math.max(1, deadline - Date.now()))])
			});

		return query<at_motet_pilot_getscene.GetSceneOutput>(
			boundedFetch,
			at_motet_pilot_getscene.GetSceneNsid,
			{ actor, scene: sceneKey, limit: 100, cursor: pageCursor }
		);
	}

	function startScenePoll(actor: string, sceneKey: string): void {
		sceneController?.abort();
		const controller = new AbortController();
		sceneController = controller;
		sceneLoading = true;
		sceneError = '';
		void waitForScene(actor, sceneKey, controller.signal);
	}

	async function waitForScene(actor: string, sceneKey: string, signal: AbortSignal): Promise<void> {
		const deadline = Date.now() + pollBudget;

		while (!signal.aborted && Date.now() < deadline) {
			try {
				const result = await getPage(actor, sceneKey, undefined, signal, deadline);

				if (signal.aborted) return;
				scenePage = result;
				posts = result.posts;
				cursor = result.cursor;
				sceneLoading = false;

				return;
			} catch (cause) {
				if (signal.aborted) return;

				if (Date.now() >= deadline) break;

				if (!(cause instanceof XrpcError && cause.status === 404)) {
					sceneError = cause instanceof Error ? cause.message : 'Could not load this scene.';
					sceneLoading = false;

					return;
				}
			}

			await pause(Math.min(pollInterval, Math.max(0, deadline - Date.now())), signal);
		}

		if (!signal.aborted) {
			sceneLoading = false;
			sceneError =
				'The scene is not available yet. It may still be appearing, or it may not exist. Try again.';
		}
	}

	async function loadMore(): Promise<void> {
		if (!cursor || moreLoading) return;
		moreController?.abort();
		const controller = new AbortController();
		moreController = controller;
		moreLoading = true;
		moreError = '';

		try {
			const result = await getPage(
				data.world.did,
				data.sceneKey,
				cursor,
				controller.signal,
				Date.now() + pollBudget
			);

			if (controller.signal.aborted) return;
			posts = [...posts, ...result.posts];
			cursor = result.cursor;
		} catch (cause) {
			if (!controller.signal.aborted)
				moreError = cause instanceof Error ? cause.message : 'Could not load more posts.';
		} finally {
			if (!controller.signal.aborted) moreLoading = false;
		}
	}

	async function waitForPost(
		actor: string,
		sceneKey: string,
		uri: string,
		cid: string,
		admitted: boolean,
		signal: AbortSignal
	): Promise<void> {
		const deadline = Date.now() + pollBudget;
		let lastError = '';

		while (!signal.aborted && Date.now() < deadline) {
			try {
				let pageCursor: string | undefined;
				let found = false;
				const collected: at_motet_pilot_querydefs.AdmittedPost[] = [];

				do {
					const result = await getPage(actor, sceneKey, pageCursor, signal, deadline);

					if (signal.aborted) return;
					collected.push(...result.posts);
					found = result.posts.some((post) => post.uri === uri && post.cid === cid);

					if (result.cursor && result.cursor === pageCursor)
						throw new Error('Invalid post cursor.');
					pageCursor = result.cursor;
					scenePage = result;
				} while (pageCursor && !found && Date.now() < deadline);

				posts = collected;
				cursor = pageCursor;

				if (found) {
					postStatus = 'Admitted. Your post is now in the scene.';

					return;
				}

				lastError = '';
			} catch (cause) {
				if (signal.aborted) return;
				lastError = cause instanceof Error ? cause.message : 'Could not check the scene.';
			}

			await pause(Math.min(pollInterval, Math.max(0, deadline - Date.now())), signal);
		}

		if (!signal.aborted) {
			const detail = lastError ? ` Last check: ${lastError}` : '';
			postStatus = admitted
				? `Admitted, but the post has not appeared in the scene yet. Check back later; do not post it again.${detail}`
				: `Pending admission. The post has not appeared yet; it may still be pending or may have been refused. Check back later; do not post it again.${detail}`;
		}
	}

	async function createPost(event: SubmitEvent): Promise<void> {
		event.preventDefault();

		if (!data.session || !scene || submitting || moreLoading || !body.trim()) return;
		postController?.abort();
		const controller = new AbortController();
		postController = controller;
		submitting = true;
		postStatus = '';

		try {
			const result = await procedure<
				at_motet_pilot_createpost.CreatePostInput,
				at_motet_pilot_createpost.CreatePostOutput
			>(at_motet_pilot_createpost.CreatePostNsid, {
				world: data.world.did,
				scene: data.sceneKey,
				body,
				register
			});

			if (controller.signal.aborted) return;

			if (result.status === 'refused') {
				postStatus = `Refused: ${result.reason ?? 'No reason was provided.'}`;

				return;
			}

			if (result.status !== 'admitted' && result.status !== 'pending') {
				postStatus =
					'The AppView returned an unrecognized admission status. Check the scene before posting again.';

				return;
			}

			body = '';
			postStatus =
				result.status === 'admitted'
					? 'Admitted. Waiting for the post to appear in the scene.'
					: 'Pending admission. Waiting for the post to appear in the scene.';
			await waitForPost(
				data.world.did,
				data.sceneKey,
				result.uri,
				result.cid,
				result.status === 'admitted',
				controller.signal
			);
		} catch (cause) {
			if (!controller.signal.aborted)
				postStatus =
					cause instanceof Error ? `Could not post: ${cause.message}` : 'Could not post.';
		} finally {
			if (!controller.signal.aborted) submitting = false;
		}
	}
</script>

<svelte:head>
	<title>{scene ? `${scene.title} · Motet` : 'Scene · Motet'}</title>
</svelte:head>

<main class="m-world__page scene-page">
	<a href={resolve('/w/[world]/play', { world: worldParam })}>Back to scenes</a>

	{#if !scenePage}
		<h1>Scene is appearing</h1>
		{#if sceneLoading}<p role="status">Waiting for this scene to appear in the AppView…</p>{/if}
		{#if sceneError}
			<p role="alert">{sceneError}</p>
			<button type="button" onclick={() => startScenePoll(data.world.did, data.sceneKey)}
				>Try again</button
			>
		{/if}
	{:else if !scene}
		<h1>Scene unavailable</h1>
		<p role="alert">This scene cannot be displayed because its record is invalid.</p>
	{:else}
		<header>
			<p>{data.world.displayName ?? data.world.handle ?? data.world.did}</p>
			<h1>{scene.title}</h1>
			<p>{scene.state}</p>
		</header>
		<section aria-labelledby="cast-heading">
			<h2 id="cast-heading">Cast</h2>
			<ul>
				{#each scene.cast as member, index (`${member}:${index}`)}
					<li><span>{member}</span>{member === scene.openedBy ? ' — opener' : ''}</li>
				{/each}
			</ul>
		</section>

		<section aria-labelledby="posts-heading">
			<h2 id="posts-heading">Admitted posts</h2>
			{#if posts.length === 0}<p>No posts have been admitted yet.</p>{/if}
			{#each posts as post, index (`${post.uri}:${post.cid}:${index}`)}
				{@const content = decodePost(post)}
				<article class="admitted-post" aria-label={`Post by ${post.author}`}>
					<header>
						<strong>{post.author}</strong>
						<span>Place {post.place.seq + 1}</span>
						<span>Epoch {post.place.epoch}</span>
					</header>
					{#if content}
						<p>
							{content.register === 'out-of-character'
								? 'Out of character'
								: content.register === 'in-character'
									? 'In character'
									: content.register}
						</p>
						<pre>{content.body}</pre>
						<time datetime={content.createdAt}>{content.createdAt}</time>
					{:else}
						<p>This admitted post cannot be displayed because its record is invalid.</p>
					{/if}
				</article>
			{/each}
			{#if cursor}
				<button type="button" onclick={loadMore} disabled={moreLoading || submitting}>
					{moreLoading ? 'Loading…' : 'Load more posts'}
				</button>
			{/if}
			{#if moreError}<p role="alert">Could not load more posts: {moreError}</p>{/if}
		</section>

		<section aria-labelledby="compose-heading">
			<h2 id="compose-heading">Write a post</h2>
			{#if data.session}
				<form onsubmit={createPost}>
					<label for="post-register">Posting as</label>
					<select id="post-register" bind:value={register}>
						<option value="in-character">In character</option>
						<option value="out-of-character">Out of character</option>
					</select>
					<label for="post-body">Write a post</label>
					<textarea id="post-body" bind:value={body} maxlength="65536" rows="8" required></textarea>
					<p>Posts are shown as Typst source.</p>
					<button
						type="submit"
						class="m-button m-button--variant_filled"
						disabled={submitting || moreLoading}
					>
						{submitting ? 'Posting…' : 'Post'}
					</button>
				</form>
			{:else}
				<p>Sign in to write a post.</p>
				<a href={resolve(`/sign-in?returnTo=${encodeURIComponent(data.returnTo)}`)}>Sign in</a>
			{/if}
			{#if postStatus}<p role="status">{postStatus}</p>{/if}
		</section>
	{/if}
</main>

<style>
	.scene-page {
		max-width: 60rem;
		margin-inline: auto;
	}

	.scene-page section {
		margin-block: 2rem;
	}

	.admitted-post {
		padding: 1rem 0;
		border-top: 1px solid var(--line);
	}

	.admitted-post header {
		display: flex;
		flex-wrap: wrap;
		gap: 1rem;
	}

	.admitted-post pre {
		white-space: pre-wrap;
		overflow-wrap: anywhere;
		font: inherit;
	}

	form {
		display: grid;
		gap: 0.75rem;
	}
</style>
