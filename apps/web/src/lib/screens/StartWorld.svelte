<script lang="ts">
	import { beforeNavigate } from '$app/navigation';
	import { resolve } from '$app/paths';
	import {
		at_motet_pilot_beginworld as beginWorld,
		at_motet_pilot_finishworld as finishWorld,
		at_motet_pilot_getworld as getWorld
	} from '@motet/lexicons';
	import { onDestroy } from 'svelte';
	import { procedure, query, XrpcError } from '#lib/api.js';
	import { didKey, exportBackup, generate, sign, verifyBackup } from '#lib/owner-key/index.js';
	import { signGenesis } from '#lib/plc/index.js';

	let { signedIn }: { signedIn: boolean } = $props();

	type Phase =
		| 'details'
		| 'creating'
		| 'creation-error'
		| 'backup'
		| 'publishing'
		| 'publication-error'
		| 'ready';

	type Attempt = {
		key: CryptoKeyPair;
		op: string;
		rotationKey: string;
		reservation?: beginWorld.BeginWorldOutput;
		finish?: finishWorld.FinishWorldInput;
		expectedDid?: string;
	};

	let phase = $state<Phase>('details');
	let name = $state('');
	let handle = $state('');
	let passphrase = $state('');
	let verifyPassphrase = $state('');
	let backupFile = $state<File | null>(null);
	let created = $state<finishWorld.FinishWorldOutput | null>(null);
	let busy = $state(false);
	let downloaded = $state(false);
	let canEdit = $state(false);
	let message = $state('');
	let attempt = $state.raw<Attempt | null>(null);
	let disposed = false;
	let polling: AbortController | null = null;

	onDestroy(() => {
		disposed = true;
		polling?.abort();
	});

	const currentStep = $derived(
		phase === 'backup'
			? 2
			: phase === 'publishing' || phase === 'publication-error' || phase === 'ready'
				? 3
				: 1
	);

	async function createWorld(): Promise<void> {
		if (busy || (phase !== 'details' && phase !== 'creation-error')) return;

		busy = true;
		phase = 'creating';
		message = '';

		try {
			if (attempt === null) {
				const key = await generate();

				attempt = {
					key,
					op: crypto.randomUUID(),
					rotationKey: await didKey(key.publicKey)
				};

				if (disposed) return;
			}

			const current = attempt;

			if (current.reservation === undefined) {
				current.reservation = await procedure<
					beginWorld.BeginWorldInput,
					beginWorld.BeginWorldOutput
				>(beginWorld.BeginWorldNsid, {
					handle: handle.trim(),
					name: name.trim(),
					visibility: 'public',
					op: current.op,
					rotationKey: current.rotationKey
				});
			}

			if (disposed) return;

			if (current.finish === undefined) {
				const result = await signGenesis(
					current.reservation.operation,
					current.reservation.signingBytes,
					current.rotationKey,
					handle.trim(),
					current.reservation.tenorUrl,
					(bytes) => sign(current.key.privateKey, bytes)
				);

				current.expectedDid = result.did;
				current.finish = {
					reservation: current.reservation.reservation,
					signedOperation: result.signedOperation
				};
			}

			if (disposed) return;

			// Retrying this exact signed reservation is safe even if the first response was lost.
			const result = await procedure<finishWorld.FinishWorldInput, finishWorld.FinishWorldOutput>(
				finishWorld.FinishWorldNsid,
				current.finish
			);

			if (result.did !== current.expectedDid) {
				throw new Error('The returned World DID does not match the signed genesis.');
			}

			created = result;

			phase = 'backup';
		} catch (error) {
			canEdit = attempt?.finish === undefined;
			const detail = error instanceof XrpcError ? ` ${error.message}` : '';

			message = canEdit
				? `World creation could not continue.${detail} Retry or edit the details.`
				: `World creation could not be confirmed.${detail} Your owner key and signed request remain on this page. Retry the same request; do not start another World.`;

			phase = 'creation-error';
		} finally {
			busy = false;
		}
	}

	function editDetails(): void {
		if (!canEdit || busy) return;

		attempt = null;
		message = '';
		phase = 'details';
	}

	async function downloadBackup(): Promise<void> {
		if (busy || created === null || attempt === null) return;

		busy = true;
		message = '';

		try {
			const backup = await exportBackup({
				key: attempt.key,
				worldDid: created.did,
				passphrase
			});

			const url = URL.createObjectURL(new Blob([backup], { type: 'application/json' }));
			const link = document.createElement('a');

			link.href = url;
			link.download = `${created.handle}-owner-key.json`;
			link.click();
			window.setTimeout(() => URL.revokeObjectURL(url), 60_000);
			downloaded = true;
		} catch {
			message =
				'Could not prepare the encrypted backup. Your key remains on this page; try the download again.';
		} finally {
			busy = false;
		}
	}

	async function waitForWorld(): Promise<void> {
		if (busy || created === null || (phase !== 'publishing' && phase !== 'publication-error'))
			return;

		busy = true;
		phase = 'publishing';
		message = '';
		const world = created;
		const deadline = Date.now() + 60_000;
		const controller = new AbortController();
		const timeout = window.setTimeout(() => controller.abort(), 60_000);

		const appviewFetch: typeof fetch = (input, init) =>
			fetch(input, { ...init, signal: controller.signal });

		polling = controller;

		try {
			while (!disposed && !controller.signal.aborted && Date.now() < deadline) {
				try {
					const result = await query<getWorld.GetWorldOutput>(appviewFetch, getWorld.GetWorldNsid, {
						actor: world.handle
					});

					if (result.did === world.did) {
						phase = 'ready';

						return;
					}
				} catch (cause) {
					if (!(cause instanceof XrpcError && cause.status === 404)) throw cause;
				}

				const remaining = deadline - Date.now();

				if (remaining > 0) {
					await new Promise<void>((resolve) =>
						window.setTimeout(resolve, Math.min(1_500, remaining))
					);
				}
			}

			if (!disposed) {
				message =
					'Your verified World is not visible in the AppView yet. Wait a moment and retry the lookup.';
				phase = 'publication-error';
			}
		} catch {
			if (!disposed) {
				message = controller.signal.aborted
					? 'Your verified World is not visible in the AppView yet. Wait a moment and retry the lookup.'
					: 'The AppView could not check your World. Your backup is safe; retry the lookup.';
				phase = 'publication-error';
			}
		} finally {
			window.clearTimeout(timeout);
			polling = null;
			busy = false;
		}
	}

	async function confirmBackup(): Promise<void> {
		if (busy || backupFile === null || created === null || attempt === null) return;

		busy = true;
		message = '';
		let verified = false;

		try {
			await verifyBackup(await backupFile.text(), verifyPassphrase, {
				worldDid: created.did,
				didKey: attempt.rotationKey
			});

			attempt = null;
			passphrase = '';
			verifyPassphrase = '';
			backupFile = null;
			phase = 'publishing';
			verified = true;
		} catch {
			message = 'Could not verify backup. Check the file and passphrase.';
		} finally {
			busy = false;
		}

		if (verified) await waitForWorld();
	}

	function warnBeforeUnload(event: BeforeUnloadEvent): void {
		if ((attempt !== null || phase === 'creating') && phase !== 'ready') {
			event.preventDefault();
			event.returnValue = '';
		}
	}

	beforeNavigate(({ cancel, willUnload }) => {
		if (
			(attempt !== null || phase === 'creating') &&
			phase !== 'ready' &&
			!willUnload &&
			!window.confirm(
				'Your owner key has not been verified. Leaving this page may lose it. Leave anyway?'
			)
		) {
			cancel();
		}
	});
</script>

<svelte:window onbeforeunload={warnBeforeUnload} />
<svelte:head>
	<title>Start a World</title>
</svelte:head>

<div class="m-page__root">
	<main class="m-page__main">
		<div class="cw">
			<div class="cw-main">
				<div class="heading">
					<a class="m-button m-button--variant_text m-button--size_sm" href={resolve('/')}>
						Home
					</a>
					<h1>Start a World</h1>
				</div>

				{#if !signedIn && phase === 'details'}
					<section class="cw-card" aria-labelledby="sign-in-heading">
						<h2 class="cw-h" id="sign-in-heading">Sign in to start a World</h2>
						<p class="cw-note">Your account will be the World’s first owner.</p>
						<a
							class="m-button m-button--variant_filled action"
							href={resolve('/sign-in?returnTo=%2Fnew-world')}>Sign in</a
						>
					</section>
				{:else}
					<div class="m-steps__root">
						<ol class="m-steps__list" aria-label="World setup progress">
							{#each ['The World', 'Back up the key', 'Ready'] as title, index (title)}
								<li
									class="m-steps__step"
									data-state={currentStep > index + 1
										? 'complete'
										: currentStep === index + 1
											? 'current'
											: 'upcoming'}
									aria-current={currentStep === index + 1 ? 'step' : undefined}
								>
									<span class="m-steps__marker" aria-hidden="true">{index + 1}</span>
									<span class="m-steps__text"><span class="m-steps__title">{title}</span></span>
								</li>
							{/each}
						</ol>
					</div>

					{#if phase === 'details'}
						<section class="cw-card m-vt-pane" aria-labelledby="world-details-heading">
							<h2 class="cw-h" id="world-details-heading">The World</h2>
							<div class="m-field__root">
								<label class="m-field__label" for="world-name">World name</label>
								<div class="m-input__root">
									<input
										class="m-input__control"
										id="world-name"
										type="text"
										bind:value={name}
										maxlength="640"
										disabled={attempt !== null}
										required
									/>
								</div>
							</div>
							<div class="m-field__root">
								<label class="m-field__label" for="world-handle">World handle</label>
								<div class="m-input__root">
									<input
										class="m-input__control"
										id="world-handle"
										type="text"
										bind:value={handle}
										placeholder="your-world.motet.test"
										autocomplete="off"
										autocapitalize="none"
										disabled={attempt !== null}
										spellcheck="false"
										aria-describedby="world-handle-help"
										required
									/>
								</div>
								<p class="m-field__help" id="world-handle-help">
									Enter the full handle under your tenor’s domain. On the local tenor, use a name
									ending in .motet.test.
								</p>
							</div>
							<div class="m-field__root">
								<span class="m-field__label">Visibility</span>
								<p class="cw-note">Public. Anyone can read public scenes.</p>
							</div>
							<div class="m-field__root">
								<label class="m-field__label" for="backup-passphrase">Backup passphrase</label>
								<div class="m-input__root">
									<input
										class="m-input__control"
										id="backup-passphrase"
										type="password"
										bind:value={passphrase}
										autocomplete="new-password"
										aria-describedby="backup-passphrase-help"
										required
									/>
								</div>
								<p class="m-field__help" id="backup-passphrase-help">
									Choose a passphrase you can keep separate from the backup file. It never leaves
									this browser.
								</p>
							</div>
							<div class="actions">
								<button
									class="m-button m-button--variant_filled"
									type="button"
									onclick={createWorld}
									disabled={busy || !name.trim() || !handle.trim() || !passphrase}
									>Create World</button
								>
							</div>
						</section>
					{:else if phase === 'creating'}
						<div class="cw-card" role="status">
							<h2 class="cw-h">Creating your World</h2>
							<p class="cw-note">
								Keep this page open while your browser signs the World’s identity.
							</p>
						</div>
					{:else if phase === 'creation-error'}
						<div class="cw-card">
							<h2 class="cw-h">Creation needs attention</h2>
							<p role="alert">{message}</p>
							<div class="actions">
								{#if canEdit}<button
										class="m-button m-button--variant_text"
										type="button"
										onclick={editDetails}>Edit details</button
									>{/if}
								<button
									class="m-button m-button--variant_filled"
									type="button"
									onclick={createWorld}
									disabled={busy}>Retry creation</button
								>
							</div>
						</div>
					{:else if phase === 'backup' && created !== null}
						<section class="cw-card" aria-labelledby="backup-heading">
							<h2 class="cw-h" id="backup-heading">Back up the owner key</h2>
							<p>
								Your World has been created. Download its encrypted owner key and store it somewhere
								safe. Keep the passphrase in a separate place.
							</p>
							<p class="cw-note">World DID: <span class="identifier">{created.did}</span></p>
							<button
								class="m-button m-button--variant_outline action"
								type="button"
								onclick={downloadBackup}
								disabled={busy}
								>{busy && !downloaded ? 'Preparing backup…' : 'Download encrypted backup'}</button
							>
							{#if downloaded}
								<div class="verify">
									<h3>Check your backup</h3>
									<p class="cw-note">
										Re-import the downloaded file and enter its passphrase before continuing.
									</p>
									<div class="m-field__root">
										<label class="m-field__label" for="backup-file">Backup file</label>
										<input
											id="backup-file"
											type="file"
											accept=".json,application/json"
											required
											onchange={(event) => {
												backupFile = event.currentTarget.files?.[0] ?? null;
											}}
										/>
									</div>
									<div class="m-field__root">
										<label class="m-field__label" for="verify-passphrase">Backup passphrase</label>
										<div class="m-input__root">
											<input
												class="m-input__control"
												id="verify-passphrase"
												type="password"
												bind:value={verifyPassphrase}
												autocomplete="off"
												required
											/>
										</div>
									</div>
									<button
										class="m-button m-button--variant_filled action"
										type="button"
										onclick={confirmBackup}
										disabled={busy || backupFile === null || !verifyPassphrase}
										>{busy ? 'Verifying…' : 'Verify backup'}</button
									>
								</div>
							{/if}
							{#if message}<p role="alert">{message}</p>{/if}
						</section>
					{:else if phase === 'publishing'}
						<div class="cw-card" role="status">
							<h2 class="cw-h">Finding your World</h2>
							<p>
								Your backup is verified. Waiting for the AppView to find your World may take a
								moment.
							</p>
						</div>
					{:else if phase === 'publication-error'}
						<div class="cw-card">
							<h2 class="cw-h">World lookup needs attention</h2>
							<p role="alert">{message}</p>
							<button
								class="m-button m-button--variant_filled action"
								type="button"
								onclick={waitForWorld}
								disabled={busy}>Retry lookup</button
							>
						</div>
					{:else if phase === 'ready' && created !== null}
						<section class="cw-card" aria-labelledby="ready-heading">
							<h2 class="cw-h" id="ready-heading">World ready</h2>
							<p>
								Your encrypted backup was verified. Keep the file and passphrase safe; they are
								needed to recover your World.
							</p>
							<p class="cw-note">@{created.handle}</p>
							<a
								class="m-button m-button--variant_filled action"
								href={resolve('/w/[world]', { world: created.handle })}>Go to World</a
							>
						</section>
					{/if}
				{/if}
			</div>
			{#if signedIn && phase !== 'ready'}
				<aside class="cw-side" aria-labelledby="owner-key-heading">
					<h2 id="owner-key-heading">Your owner key</h2>
					<p>
						Your browser creates the key that can recover this World. Motet and your tenor never
						receive its private half.
					</p>
					<p>
						Keep the encrypted backup and its passphrase in separate safe places. If you lose the
						key, you can lose control of the World’s identity.
					</p>
				</aside>
			{/if}
		</div>
	</main>
</div>

<style>
	.cw {
		display: flex;
		flex-wrap: wrap;
		gap: 24px 40px;
		align-items: flex-start;
	}
	.cw-main {
		flex: 999 1 520px;
		min-width: 0;
		display: flex;
		flex-direction: column;
		gap: 20px;
		max-width: 720px;
	}
	.heading {
		display: grid;
		gap: 4px;
	}
	.heading a {
		justify-self: start;
		margin-inline-start: -8px;
	}
	.heading h1 {
		margin: 0;
		font: 600 28px/34px var(--font-ui);
	}
	.cw-card {
		display: flex;
		flex-direction: column;
		gap: 20px;
		padding: 24px;
		background-color: var(--surface-raised);
		border: 1px solid var(--line);
		border-radius: var(--radius-xl);
		corner-shape: var(--corner-shape);
		box-shadow: var(--elevation-1), var(--edge-light);
	}
	.cw-card p {
		margin: 0;
	}
	.cw-h {
		margin: 0;
		font: 600 22px/28px var(--font-ui);
	}
	.cw-note {
		margin: 0;
		font-size: 13px;
		line-height: 18px;
		color: var(--ink-muted);
	}
	.actions {
		display: flex;
		gap: 8px;
		justify-content: flex-end;
		flex-wrap: wrap;
		border-top: 1px solid var(--line);
		padding-top: 16px;
	}
	.action {
		align-self: flex-start;
	}
	.verify {
		display: flex;
		flex-direction: column;
		gap: 14px;
		border-top: 1px solid var(--line);
		padding-top: 20px;
	}
	.verify h3 {
		margin: 0;
		font: 600 17px/24px var(--font-ui);
	}
	.identifier {
		overflow-wrap: anywhere;
	}
	.cw-side {
		flex: 1 1 300px;
		min-width: 0;
		display: flex;
		flex-direction: column;
		gap: 10px;
		color: var(--ink-muted);
	}
	.cw-side h2 {
		margin: 0;
		font: 600 15px/20px var(--font-ui);
		color: var(--ink);
	}
	.cw-side p {
		margin: 0;
		font-size: 14px;
		line-height: 1.5;
	}
	@media (min-width: 1000px) {
		.cw-side {
			position: sticky;
			top: calc(var(--chrome-offset, 72px) + 16px);
			max-width: 380px;
		}
	}
</style>
