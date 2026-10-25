<!--
	Reports a post or an account. Each reason is one
	com.atproto.moderation.createReport reasonType, and the reader picks
	which moderation services get a copy. Sending is not wired yet.
-->
<script lang="ts">
	let {
		kind,
		world,
		account,
		handle,
		who = '',
		excerpt = '',
		portrait = '',
		initials = '',
		hue = 262,
		targetGm = false
	}: {
		kind: 'post' | 'account';
		/** The World the post is in; empty for an account. */
		world: string;
		/** Display name of the account that wrote the post, or is reported. */
		account: string;
		handle: string;
		/** The character who spoke, for a post. */
		who?: string;
		excerpt?: string;
		portrait?: string;
		initials?: string;
		hue?: number;
		/** The post's author is a GM of the World, so only the other GMs see the report. */
		targetGm?: boolean;
	} = $props();

	type Reason = 'rules' | 'harass' | 'sexual' | 'spam' | 'imperson' | 'danger' | 'other';

	type Service = 'world' | 'motet' | 'pbf';

	const inWorld = $derived(kind === 'post' && world !== '');

	const reasons = $derived<{ id: Reason; label: string }[]>([
		...(inWorld
			? [{ id: 'rules' as const, label: `Breaks ${world}’s rules or content lines` }]
			: []),
		{ id: 'harass', label: 'Harassment, hate or trolling' },
		{ id: 'sexual', label: 'Unwanted sexual content' },
		{ id: 'spam', label: 'Spam or a scam' },
		{ id: 'imperson', label: 'Pretending to be someone else' },
		{ id: 'danger', label: 'Threats, or someone may be in danger' },
		{ id: 'other', label: 'Something else' }
	]);

	const services = $derived<{ id: Service; label: string; icon: string; hint: string }[]>([
		...(inWorld
			? [
					{
						id: 'world' as const,
						label: world,
						icon: 'shield_person',
						hint: targetGm
							? `This World’s moderation. Only Bram sees it: ${account} is a GM here.`
							: 'This World’s moderation, run by its GMs. They can hide posts and act here.'
					}
				]
			: []),
		{
			id: 'motet',
			label: 'Motet moderation',
			icon: 'shield',
			hint: 'Motet’s rules, in every World and for every account.'
		},
		{
			id: 'pbf',
			label: 'PBF Safety Collective',
			icon: 'policy',
			hint: 'A moderation service you use.'
		}
	]);

	// A service's labeler record may list the reason types it takes.
	const takes = (service: Service, reason: Reason): boolean =>
		service !== 'pbf' || ['harass', 'sexual', 'danger', 'other'].includes(reason);

	// Sensible first picks per reason; the reader can change every one.
	const defaults = (reason: Reason): Service[] => {
		if (reason === 'rules') {
			return ['world'];
		}

		if (reason === 'spam' || reason === 'imperson') {
			return ['motet'];
		}

		if (reason === 'other') {
			return inWorld ? ['world'] : ['motet'];
		}

		return inWorld ? ['world', 'motet'] : ['motet'];
	};

	let reason: Reason | null = $state(null);
	let chosen: Service[] = $state([]);
	let note = $state('');

	function pick(next: Reason): void {
		reason = next;
		chosen = defaults(next);
	}

	function flip(service: Service): void {
		chosen = chosen.includes(service) ? chosen.filter((s) => s !== service) : [...chosen, service];
	}
</script>

<div
	style="position: fixed; inset: 0; z-index: 80; display: grid; place-items: center; padding: 16px; background-color: var(--scrim); font-family: var(--font-ui); color: var(--ink)"
>
	<div
		class="m-dialog__root"
		role="dialog"
		aria-modal="true"
		aria-labelledby="rd-title"
		style="box-sizing: border-box; width: min(560px, 100%); max-height: calc(100vh - 32px); overflow: auto; margin: 0"
	>
		<div style="display: flex; flex-direction: column; gap: 18px">
			<div class="m-dialog__header" style="margin: 0">
				<span class="m-icon" aria-hidden="true" style="color: var(--danger-ink)">report</span>
				<h2 class="m-dialog__title" id="rd-title">
					{kind === 'post' ? 'Report this post' : `Report @${handle}`}
				</h2>
			</div>

			<div
				style="display: grid; grid-template-columns: auto minmax(0, 1fr); gap: 2px 12px; align-items: center; padding: 12px 14px; background-color: var(--surface-quiet); border-radius: var(--radius-md)"
			>
				<span
					class="m-avatar m-avatar--size_md m-avatar--kind_{kind === 'post'
						? 'character'
						: 'account'}"
					aria-hidden="true"
					style="grid-row: span 2"
					style:--avatar-hue={hue}
				>
					{#if kind === 'post'}<img src={portrait} alt="" />{:else}<span aria-hidden="true"
							>{initials}</span
						>{/if}
				</span>
				<span style="display: flex; gap: 8px; align-items: baseline; flex-wrap: wrap; min-width: 0"
					><span style="font-weight: 600">{kind === 'post' ? who : account}</span><span
						style="font-size: 13px; color: var(--ink-muted)">@{handle}</span
					></span
				>
				<span
					style="font-size: 13.5px; line-height: 19px; color: var(--ink-muted); overflow: hidden; display: -webkit-box; -webkit-line-clamp: 2; -webkit-box-orient: vertical"
					>{kind === 'post'
						? excerpt
						: `Account on ${handle.split('.').slice(1).join('.') || handle}`}</span
				>
			</div>

			<fieldset class="m-fieldset__root" style="margin: 0">
				<legend class="m-fieldset__legend">What’s wrong?</legend>
				<div class="m-radio__root" style="margin-top: 4px">
					{#each reasons as r (r.id)}
						<label class="m-radio__item"
							><input
								class="m-radio__control"
								type="radio"
								name="rd-reason"
								checked={r.id === reason}
								onchange={() => pick(r.id)}
							/><span class="m-radio__text"><span class="m-radio__label">{r.label}</span></span
							></label
						>
					{/each}
				</div>
			</fieldset>

			{#if reason === 'danger'}
				<div
					class="m-alert__root m-alert__root--tone_caution m-alert__root--variant_inline"
					role="note"
				>
					<span class="m-icon m-alert__icon m-alert__icon--tone_caution" aria-hidden="true"
						>warning</span
					>
					<div class="m-alert__content">
						<div class="m-alert__description">
							If someone is in danger right now, contact local emergency services. Reports here are
							read by people, not in real time.
						</div>
					</div>
					<span></span>
				</div>
			{/if}

			{#if reason !== null}
				{@const current = reason}
				<fieldset class="m-fieldset__root" style="margin: 0">
					<legend class="m-fieldset__legend">Send it to</legend>
					<div style="display: flex; flex-direction: column; margin-top: 4px">
						{#each services as s (s.id)}
							<label class="m-checkbox__root"
								><input
									class="m-checkbox__control"
									type="checkbox"
									checked={chosen.includes(s.id) && takes(s.id, current)}
									disabled={!takes(s.id, current)}
									onchange={() => flip(s.id)}
								/><span class="m-checkbox__text"
									><span
										class="m-checkbox__label"
										style="display: inline-flex; gap: 6px; align-items: center"
										><span class="m-icon m-icon--sm" aria-hidden="true">{s.icon}</span
										>{s.label}</span
									><span class="m-checkbox__description"
										>{takes(s.id, current)
											? s.hint
											: 'Takes reports of harassment, sexual content and safety only.'}</span
									></span
								></label
							>
						{/each}
					</div>
					<div class="m-fieldset__help" style="margin-top: 6px">
						Each one gets its own copy. The person you report isn’t told who sent it.
					</div>
				</fieldset>

				<div class="m-textarea__root">
					<label class="m-field__label" for="rd-note">Anything to add?</label><textarea
						class="m-textarea__control m-textarea__control--face_ui"
						id="rd-note"
						rows="2"
						style="--rows: 2"
						placeholder="Optional"
						maxlength="2000"
						aria-describedby="rd-count"
						bind:value={note}></textarea>
					<div class="m-textarea__footer">
						<span></span><span
							class="m-textarea__counter"
							id="rd-count"
							data-state={[...note].length > 1800 ? 'near' : undefined}
							>{[...note].length} / 2000</span
						>
					</div>
				</div>
			{/if}

			<div class="m-dialog__actions" style="margin: 0">
				<button class="m-button m-button--variant_text m-button--tone_neutral" type="button"
					><span>Cancel</span></button
				>
				<button
					class="m-button m-button--variant_filled m-button--tone_danger"
					type="button"
					disabled={reason === null || chosen.length === 0}><span>Send report</span></button
				>
			</div>
		</div>
	</div>
</div>
