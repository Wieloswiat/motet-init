<script lang="ts">
	import { resolve } from '$app/paths';
	import Chrome from '#lib/components/motet/Chrome.svelte';
	import WorldHeader from '#lib/components/motet/WorldHeader.svelte';
	import { world } from '#lib/motet/world.js';
	import type { WorldApplyData } from '#lib/demo/screens/world-apply.js';

	let { data }: { data: WorldApplyData } = $props();
</script>

<svelte:head>
	<title>Application and character</title>
</svelte:head>

<div {@attach world(data.worldRef)} class="m-world__root">
	<Chrome world="vellum" account="nadia" inbox={data.inboxCount} />
	<WorldHeader current="Overview" size="slim" role="applicant" />
	<main class="m-world__page" style="padding-top: 28px">
		<div
			style="max-width: 780px; margin-inline: auto; display: flex; flex-direction: column; gap: 24px"
		>
			<div style="display: flex; align-items: center; gap: 12px; flex-wrap: wrap">
				<a
					class="m-button m-button--variant_text m-button--size_sm"
					href={resolve('/w/[world]/visitor', { world: 'gilded-reach' })}
					style="margin-inline-start: -8px"
					><span class="m-icon" aria-hidden="true">arrow_back</span><span>Overview</span></a
				>
			</div>
			<h1 class="h-step" style="font-size: calc(32px * var(--display-adjust, 1))">
				Apply to The Gilded Reach
			</h1>

			{#if data.notSent}
				<div class="m-steps__root">
					<ol class="m-steps__list">
						{#each data.steps as s (s.title)}
							<li class="m-steps__step" data-state={s.state} aria-current={s.current}>
								<button class="stepbtn" type="button" disabled={s.locked} style="display: contents">
									<span class="m-steps__marker" aria-hidden="true"
										>{#if s.done}<span class="m-icon" aria-hidden="true">check</span
											>{/if}{#if s.notDone}{s.n}{/if}</span
									>
									<span class="m-steps__text"><span class="m-steps__title">{s.title}</span></span>
								</button>
							</li>
						{/each}
					</ol>
				</div>

				<form
					class="m-ornamented m-pad m-vt-pane"
					style="display: flex; flex-direction: column; gap: 22px; background-color: var(--surface-raised); border: var(--border-width, 1px) solid var(--frame-line, var(--line)); border-radius: var(--frame-radius, var(--radius-lg)); corner-shape: var(--frame-shape, var(--corner-shape)); box-shadow: var(--elevation-1), var(--edge-light)"
				>
					{#if data.is.rules}
						<div style="display: flex; flex-direction: column; gap: 20px">
							<h2 class="h-step">Before you start</h2>
							<ul
								style="margin: 0; padding: 0; list-style: none; display: grid; gap: 12px; font-family: var(--font-reading); font-size: 16px; line-height: 1.5"
							>
								<li style="display: grid; grid-template-columns: 24px minmax(0, 1fr); gap: 12px">
									<span class="m-icon" aria-hidden="true" style="color: var(--ink-muted)"
										>schedule</span
									><span
										>One post a week in each of your scenes, or a word in Table talk when you can’t.</span
									>
								</li>
								<li style="display: grid; grid-template-columns: 24px minmax(0, 1fr); gap: 12px">
									<span class="m-icon" aria-hidden="true" style="color: var(--ink-muted)"
										>theater_comedy</span
									><span>Write only your own character.</span>
								</li>
								<li style="display: grid; grid-template-columns: 24px minmax(0, 1fr); gap: 12px">
									<span class="m-icon" aria-hidden="true" style="color: var(--ink-muted)"
										>block</span
									><span
										>Never: sexual violence, harm to children. Off screen only: torture, romance
										beyond a kiss.</span
									>
								</li>
							</ul>
							<a
								class="m-button m-button--variant_text m-button--size_sm"
								href={resolve('/w/[world]/visitor', { world: 'gilded-reach' })}
								style="align-self: flex-start; margin-inline-start: -8px"
								><span>Read the full table rules</span></a
							>
							<label class="m-checkbox__root"
								><input class="m-checkbox__control" type="checkbox" checked={data.agreed} /><span
									class="m-checkbox__text"
									><span class="m-checkbox__label"
										>I’ve read the table rules and the content lines</span
									></span
								></label
							>
							<fieldset class="m-fieldset__root" style="margin: 0">
								<legend class="m-fieldset__legend">How often can you post?</legend>
								<div class="m-radio__root">
									{#each data.paces as p (p.label)}
										<label class="m-radio__item"
											><input
												class="m-radio__control"
												type="radio"
												name="pace"
												checked={p.on}
											/><span class="m-radio__text"
												><span class="m-radio__label">{p.label}</span></span
											></label
										>
									{/each}
								</div>
							</fieldset>
						</div>
					{/if}

					{#if data.is.character}
						<div style="display: flex; flex-direction: column; gap: 20px">
							<h2 class="h-step">Your character</h2>
							<fieldset class="m-fieldset__root" style="margin: 0">
								<legend class="m-fieldset__legend">Role</legend>
								<div class="m-radio__root m-radio__root--variant_cards">
									{#each data.roles as r (r.label)}
										<label class="m-radio__item m-radio__item--variant_cards"
											><input
												class="m-radio__control m-radio__control--variant_cards"
												type="radio"
												name="role"
												checked={r.on}
											/><span class="m-radio__text m-radio__text--variant_cards"
												><span class="m-radio__label m-radio__label--variant_cards">{r.label}</span
												><span class="m-radio__description">{r.hint}</span></span
											><span
												class="m-icon m-radio__icon m-radio__icon--variant_cards"
												aria-hidden="true">{r.icon}</span
											></label
										>
									{/each}
								</div>
							</fieldset>
							<div style="display: flex; flex-wrap: wrap; gap: 20px; align-items: flex-start">
								<div class="m-upload__root" style="flex: 0 1 180px; min-width: 0">
									<span class="m-field__label">Portrait</span>
									<label
										class="m-upload__dropzone"
										for="portrait"
										style="aspect-ratio: 3 / 4; max-width: 100%; padding: 12px; justify-content: center"
									>
										<input type="file" id="portrait" accept="image/*" />
										{#if data.hasPortrait}<img
												src={data.portrait}
												alt="Portrait of {data.name}"
												style="width: 100%; height: 100%; object-fit: cover; border-radius: var(--radius-md)"
											/>{/if}
										{#if data.noPortrait}<span class="m-icon m-upload__icon" aria-hidden="true"
												>add_photo_alternate</span
											><span class="m-upload__title">Add a portrait</span><span
												class="m-upload__hint">3:4, at least 600px tall</span
											>{/if}
									</label>
								</div>
								<div
									style="flex: 1 1 320px; min-width: 0; display: flex; flex-direction: column; gap: 16px"
								>
									<div class="m-field__root">
										<label class="m-field__label" for="cname">Name</label>
										<div class="m-input__root">
											<input
												class="m-input__control"
												id="cname"
												type="text"
												value={data.name}
												aria-invalid={data.nameInvalid}
												aria-describedby="cname-help"
											/>
										</div>
										{#if data.nameMissing}<div
												class="m-field__help"
												id="cname-help"
												data-tone="danger"
											>
												<span class="m-icon" aria-hidden="true">error</span><span
													>Enter a name.</span
												>
											</div>{/if}
									</div>
									<div class="m-field__root">
										<label class="m-field__label" for="cep">Epithet</label>
										<div class="m-input__root">
											<input
												class="m-input__control"
												id="cep"
												type="text"
												value={data.epithet}
												aria-describedby="cep-help"
											/>
										</div>
										<div class="m-field__help" id="cep-help">
											<span>Shown under the name on every post</span>
										</div>
									</div>
									<div
										style="display: grid; grid-template-columns: repeat(auto-fit, minmax(min(150px, 100%), 1fr)); gap: 16px"
									>
										<div class="m-select__root">
											<span class="m-field__label" id="house-l">House</span><button
												class="m-select__trigger"
												type="button"
												aria-haspopup="listbox"
												aria-labelledby="house-l house-v"
												><span class="m-select__value" id="house-v">{data.house}</span><span
													class="m-icon m-select__indicator"
													aria-hidden="true">expand_more</span
												></button
											>
										</div>
										<div class="m-field__root">
											<label class="m-field__label" for="age">Age</label>
											<div class="m-input__root">
												<span class="m-input__action"
													><button
														class="m-icon-button m-icon-button--variant_standard"
														type="button"
														aria-label="Decrease"
														><span class="m-icon" aria-hidden="true">remove</span></button
													></span
												><input
													class="m-input__control"
													id="age"
													type="text"
													inputmode="numeric"
													value={data.age}
													readonly
													style="text-align: center"
												/><span class="m-input__action"
													><button
														class="m-icon-button m-icon-button--variant_standard"
														type="button"
														aria-label="Increase"
														><span class="m-icon" aria-hidden="true">add</span></button
													></span
												>
											</div>
										</div>
									</div>
								</div>
							</div>
							<fieldset class="m-fieldset__root" style="margin: 0">
								<legend class="m-fieldset__legend">Trait</legend>
								<div class="m-fieldset__help" style="margin-bottom: 8px">
									Added to your d20 checks. Pick the one your character leans on.
								</div>
								<div style="display: flex; flex-wrap: wrap; gap: 8px">
									{#each data.traits as t (t.label)}
										<button
											class="m-chip m-chip--kind_filter"
											type="button"
											aria-pressed={t.pressed}
											><span class="m-icon" aria-hidden="true">{t.icon}</span><span>{t.label}</span
											></button
										>
									{/each}
								</div>
							</fieldset>
							<div class="m-textarea__root">
								<label
									class="m-field__label"
									for="secret"
									style="display: flex; gap: 8px; align-items: center"
									>A secret your character keeps <span
										class="m-chip m-chip--kind_note"
										style="min-height: 22px"
										><span class="m-icon" aria-hidden="true">lock</span><span
											>Only GMs see this</span
										></span
									></label
								>
								<textarea
									class="m-textarea__control m-textarea__control--face_reading"
									id="secret"
									rows="3"
									style="--rows: 3"
									>She tore the banner down herself, before dawn, and does not yet know why her father let her.</textarea
								>
							</div>
						</div>
					{/if}

					{#if data.is.story}
						<div style="display: flex; flex-direction: column; gap: 20px">
							<h2 class="h-step">Story</h2>
							<div class="m-textarea__root">
								<label class="m-field__label" for="bg">Background</label>
								<textarea
									class="m-textarea__control m-textarea__control--face_reading"
									id="bg"
									rows="8"
									style="--rows: 8; --max-rows: 16"
									value={data.bg}></textarea>
								<div class="m-textarea__footer">
									<span class="m-textarea__counter" data-state="ok">{data.bgCount}</span>
								</div>
							</div>
							<div class="m-textarea__root">
								<label class="m-field__label" for="first"
									>How your character arrives at Harrowgate <span
										style="font-weight: 400; color: var(--ink-muted)">(optional)</span
									></label
								>
								<textarea
									class="m-textarea__control m-textarea__control--face_reading"
									id="first"
									rows="5"
									style="--rows: 5"
									placeholder="Write it as a first post, in character."></textarea>
								<div class="m-field__help">
									<span>Becomes your first post if you are accepted.</span>
								</div>
							</div>
						</div>
					{/if}

					{#if data.is.review}
						<div style="display: flex; flex-direction: column; gap: 20px">
							<h2 class="h-step">Review</h2>
							<div style="display: flex; flex-wrap: wrap; gap: 20px; align-items: flex-start">
								<article
									class="m-character__root m-ornamented m-marked"
									style="flex: 1 1 400px; min-width: 0"
								>
									<div class="m-character__portrait"><img src={data.portrait} alt="" /></div>
									<div class="m-character__body">
										<h3 class="m-character__name">{data.name}</h3>
										<span class="m-character__epithet">{data.epithet}</span>
										<dl class="m-character__facts">
											<dt>House</dt>
											<dd>{data.house}</dd>
											<dt>Age</dt>
											<dd>{data.age}</dd>
											<dt>Trait</dt>
											<dd>{data.traitLabel}</dd>
										</dl>
										<div class="m-character__owner">
											<span
												class="m-avatar m-avatar--kind_account m-avatar--size_xs"
												aria-hidden="true"
												style="--avatar-hue: 32"><span aria-hidden="true">NP</span></span
											><span>Played by @nadia.pds.example</span>
										</div>
									</div>
									<div class="m-character__foot">
										<span class="m-flag m-flag--kind_caution"
											><span class="m-icon m-icon--sm" aria-hidden="true">hourglass_top</span
											>Awaiting review</span
										>
									</div>
								</article>
								<dl
									class="m-datalist__root m-datalist__root--variant_ruled"
									style="flex: 1 1 240px; min-width: 0; margin: 0"
								>
									<div class="m-datalist__item m-datalist__item--variant_ruled">
										<dt class="m-datalist__label m-datalist__label--variant_ruled">Role</dt>
										<dd class="m-datalist__value m-datalist__value--variant_ruled">
											{data.roleLabel}
										</dd>
									</div>
									<div class="m-datalist__item m-datalist__item--variant_ruled">
										<dt class="m-datalist__label m-datalist__label--variant_ruled">Posting</dt>
										<dd class="m-datalist__value m-datalist__value--variant_ruled">
											{data.paceLabel}
										</dd>
									</div>
									<div class="m-datalist__item m-datalist__item--variant_ruled">
										<dt class="m-datalist__label m-datalist__label--variant_ruled">Background</dt>
										<dd class="m-datalist__value m-datalist__value--variant_ruled">
											{data.bgCount}
										</dd>
									</div>
									<div class="m-datalist__item m-datalist__item--variant_ruled">
										<dt class="m-datalist__label m-datalist__label--variant_ruled">Secret</dt>
										<dd
											class="m-datalist__value m-datalist__value--variant_ruled"
											style="display: flex; gap: 6px; align-items: center"
										>
											<span class="m-icon m-icon--sm" aria-hidden="true">lock</span>GMs only
										</dd>
									</div>
								</dl>
							</div>
						</div>
					{/if}

					<div
						style="display: flex; align-items: center; gap: 12px; flex-wrap: wrap; padding-top: 16px; border-top: 1px solid var(--line)"
					>
						{#if data.hasBack}<button class="m-button m-button--variant_text" type="button"
								><span class="m-icon" aria-hidden="true">arrow_back</span><span>Back</span></button
							>{/if}
						<span style="flex: 1"></span>
						<span
							class="m-composer__status"
							role="status"
							aria-label="Draft saved on this device"
							title="Draft saved on this device"
							><span class="m-icon" aria-hidden="true">cloud_done</span></span
						>
						{#if data.notLast}<button
								class="m-button m-button--variant_filled"
								type="button"
								disabled={data.blocked}
								><span>{data.nextLabel}</span><span class="m-icon" aria-hidden="true"
									>arrow_forward</span
								></button
							>{/if}
						{#if data.isLast}<button class="m-button m-button--variant_filled" type="submit"
								><span class="m-icon" aria-hidden="true">send</span><span>Send application</span
								></button
							>{/if}
					</div>
				</form>
			{/if}

			{#if data.sent}
				<div
					class="m-ornamented m-pad"
					style="display: flex; flex-direction: column; gap: 18px; align-items: flex-start; background-color: var(--surface-raised); border: var(--border-width, 1px) solid var(--frame-line, var(--line)); border-radius: var(--frame-radius, var(--radius-lg)); corner-shape: var(--frame-shape, var(--corner-shape))"
				>
					<span class="m-flag m-flag--kind_done"
						><span class="m-icon m-icon--sm" aria-hidden="true">check_circle</span>Application sent</span
					>
					<h2 class="h-step">{data.name} is with the GMs</h2>
					<p
						style="margin: 0; font-family: var(--font-reading); font-size: 17px; line-height: 1.55; max-width: 60ch"
					>
						Aurelia and Bram review applications before April 30. Their answer arrives in your
						Inbox. You can change the application until one of them opens it.
					</p>
					<div style="display: flex; gap: 8px; flex-wrap: wrap">
						<button class="m-button m-button--variant_tonal" type="button"
							><span class="m-icon" aria-hidden="true">edit</span><span>Edit application</span
							></button
						>
						<a class="m-button m-button--variant_text" href={resolve('/board')}
							><span>Back to Motet</span></a
						>
					</div>
				</div>
			{/if}
		</div>
	</main>
</div>

<style>
	.h-step {
		margin: 0;
		font-family: var(--font-display);
		font-weight: var(--display-weight);
		font-size: calc(26px * var(--display-adjust, 1));
		line-height: 1.15;
		text-transform: var(--display-case);
		letter-spacing: var(--display-tracking);
	}
	@layer reset {
		.stepbtn {
			background: none;
			border: 0;
			padding: 0;
			font: inherit;
			color: inherit;
			cursor: pointer;
			text-align: start;
		}
	}
</style>
