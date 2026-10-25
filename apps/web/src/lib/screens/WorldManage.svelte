<script lang="ts">
	import { resolve } from '$app/paths';
	import Chrome from '#lib/components/motet/Chrome.svelte';
	import LanguagePicker from '#lib/components/motet/LanguagePicker.svelte';
	import WorldHeader from '#lib/components/motet/WorldHeader.svelte';
	import { world } from '#lib/motet/world.js';
	import type { WorldManageData } from '#lib/demo/screens/world-manage.js';

	let { data }: { data: WorldManageData } = $props();
</script>

<svelte:head>
	<title>Manage World</title>
</svelte:head>

<div {@attach world(data.worldRef)} class="m-world__root">
	<Chrome world="vellum" account="aurelia" inbox={data.inboxCount} />
	<WorldHeader current="Manage" size="slim" role="gm" />
	<main class="m-world__page" style="padding-top: 24px">
		<div
			class="m-drill"
			data-pane={data.pane}
			style="display: flex; flex-wrap: wrap; gap: 24px 36px; align-items: flex-start"
		>
			<nav
				class="m-page__panel sidepanel m-drill__nav m-vt"
				aria-label="Manage"
				style="flex: 1 1 248px; --vt: m-nav"
			>
				{#each data.sections as s (s.label)}
					<button class="m-page__navItem" type="button" aria-current={s.current}
						><span class="m-icon" aria-hidden="true">{s.icon}</span><span>{s.label}</span
						>{#if s.hasCount}<span class="m-count">{s.count}</span>{/if}</button
					>
				{/each}
			</nav>

			<div
				class="m-drill__content m-vt-pane"
				style="flex: 999 1 560px; min-width: 0; display: flex; flex-direction: column; gap: 24px"
			>
				<button
					class="m-button m-button--variant_text m-button--size_sm m-drill__back"
					type="button"
					><span class="m-icon" aria-hidden="true">arrow_back</span><span>Manage</span></button
				>

				{#if data.is.overview}
					<div style="display: flex; flex-direction: column; gap: 24px">
						<h1 class="h-m">Needs you</h1>
						<div style="display: flex; flex-direction: column">
							{#each data.tasks as t (t.title)}
								<div
									class="task"
									style="display: grid; grid-template-columns: 32px minmax(0, 1fr) auto; gap: 4px 14px; align-items: center; padding: 12px; border-bottom: 1px solid var(--line)"
								>
									<span
										class="m-icon"
										aria-hidden="true"
										style="grid-row: span 2; color: var(--ink-muted)">{t.icon}</span
									>
									<span style="display: flex; gap: 8px; align-items: center; flex-wrap: wrap"
										><span style="font: 600 12.5px/16px var(--font-ui); color: var(--ink-muted)"
											>{t.kind}</span
										><span style="font-weight: 600">{t.title}</span></span
									>
									<span
										style="grid-row: span 2; display: flex; gap: 6px; flex-wrap: wrap; justify-content: flex-end"
									>
										{#each t.actions as a (a.label)}<button class={a.cls} type="button"
												><span>{a.label}</span></button
											>{/each}
									</span>
									<span style="font-size: 14px; color: var(--ink-muted)">{t.detail}</span>
								</div>
							{/each}
						</div>
						<section
							aria-labelledby="h-away"
							style="display: flex; flex-direction: column; gap: 8px"
						>
							<h2 id="h-away" style="margin: 0; font: 600 15px/20px var(--font-ui)">Away</h2>
							<div style="display: flex; flex-wrap: wrap; gap: 8px 20px; font-size: 14px">
								<span class="m-meta"
									><span class="m-icon m-icon--sm" aria-hidden="true">bedtime</span><span
										>Bram, until Mar 27. Duke Aldric is yours.</span
									></span
								>
								<span class="m-meta"
									><span class="m-icon m-icon--sm" aria-hidden="true">bedtime</span><span
										>Marek, until Mar 18. GMs may move Ser Jaume.</span
									></span
								>
							</div>
						</section>
					</div>
				{/if}

				{#if data.is.moderation}
					<div style="display: flex; flex-direction: column; gap: 20px">
						<div
							style="display: flex; justify-content: space-between; align-items: center; gap: 12px; flex-wrap: wrap"
						>
							<h1 class="h-m">Moderation</h1>
							<div class="m-segmented__root" role="radiogroup" aria-label="Show reports">
								{#each data.repFilters as o (o.label)}<button
										class="m-segmented__item"
										type="button"
										role="radio"
										aria-checked={o.checked}
										>{o.label}<span style="font-weight: 500; opacity: .75">{o.count}</span></button
									>{/each}
							</div>
						</div>
						<div style="display: flex; flex-wrap: wrap; gap: 20px; align-items: flex-start">
							<div
								role="group"
								aria-label="Reports"
								style="flex: 1 1 240px; display: flex; flex-direction: column; gap: 4px"
							>
								{#each data.reports as r (r.icon)}
									<button
										class="appitem m-vt m-vt-box"
										type="button"
										aria-current={r.current}
										style="--vt: {r.vtn}; display: grid; grid-template-columns: 24px minmax(0, 1fr); gap: 2px 10px; padding: 10px; border-radius: var(--radius-md)"
									>
										<span class="m-icon" aria-hidden="true" style="grid-row: span 3; opacity: .75"
											>{r.icon}</span
										>
										<span style="font-weight: 600">{r.reason}</span>
										<span style="font-size: 13px; opacity: .85">{r.where}</span>
										<span style="font-size: 12.5px; opacity: .75">{r.when}</span>
									</button>
								{/each}
								{#if data.noReports}<p style="margin: 0; padding: 10px; color: var(--ink-muted)">
										Nothing here.
									</p>{/if}
							</div>

							{#if data.hasRep}
								<div
									style="flex: 999 1 440px; min-width: 0; display: flex; flex-direction: column; gap: 16px"
								>
									<div style="display: flex; flex-wrap: wrap; gap: 8px; align-items: center">
										<span class="m-chip m-chip--kind_fact"
											><span class="m-icon" aria-hidden="true">{data.rep.icon}</span><span
												>{data.rep.reason}</span
											></span
										>
										<span class={data.rep.flagClass}
											><span class="m-icon m-icon--sm" aria-hidden="true">{data.rep.flagIcon}</span
											>{data.rep.statusLabel}</span
										>
									</div>

									<article
										aria-label="Reported post"
										class="quietbox"
										style="display: grid; grid-template-columns: 40px minmax(0, 1fr); gap: 4px 12px"
									>
										<span
											class="m-avatar m-avatar--kind_account m-avatar--size_md"
											aria-hidden="true"
											style="grid-row: span 3; {data.rep.hue}"
											><span aria-hidden="true">{data.rep.initials}</span></span
										>
										<span
											style="display: flex; gap: 4px 10px; flex-wrap: wrap; align-items: baseline"
											><span style="font-weight: 600">{data.rep.author}</span><span
												style="font-size: 13px; color: var(--ink-muted)">@{data.rep.handle}</span
											></span
										>
										<span
											style="font-family: var(--font-reading); font-size: 16px; line-height: 1.5"
											>{data.rep.text}</span
										>
										<span
											style="display: flex; gap: 4px 14px; flex-wrap: wrap; font-size: 13px; color: var(--ink-muted)"
											><span class="m-meta"
												><span class="m-icon m-icon--sm" aria-hidden="true">forum</span><span
													>{data.rep.scene}</span
												></span
											><span class="m-meta"
												><span class="m-icon m-icon--sm" aria-hidden="true">history</span><span
													>Reported version, {data.rep.version}</span
												></span
											></span
										>
									</article>
									<div style="display: flex; gap: 8px; flex-wrap: wrap; margin-top: -6px">
										<a
											class="m-button m-button--variant_outline m-button--size_sm"
											href={resolve('/w/[world]/play/[scene]', {
												world: 'gilded-reach',
												scene: 'feast-of-lanterns'
											})}
											><span class="m-icon" aria-hidden="true">open_in_new</span><span
												>Go to post</span
											></a
										>
										<a
											class="m-button m-button--variant_text m-button--size_sm"
											href={resolve('/profile/[handle]', { handle: 'aurelia.bsky.social' })}
											><span class="m-icon" aria-hidden="true">person</span><span
												>{data.rep.author}’s profile</span
											></a
										>
									</div>

									<section
										aria-labelledby="h-rep"
										style="display: flex; flex-direction: column; gap: 8px"
									>
										<h2
											id="h-rep"
											style="margin: 0; font: 600 13px/18px var(--font-ui); color: var(--ink-muted)"
										>
											Report
										</h2>
										<div style="display: flex; gap: 10px; align-items: center; font-size: 14px">
											<span
												class="m-avatar m-avatar--kind_account m-avatar--size_xs"
												aria-hidden="true"
												style={data.rep.rhue}
												><span aria-hidden="true">{data.rep.rinitial}</span></span
											><span
												><strong>{data.rep.reporter}</strong>
												<span style="color: var(--ink-muted)">@{data.rep.reporterHandle}</span
												></span
											>
										</div>
										<p class="quietbox" style="margin: 0; font-size: 14px; line-height: 20px">
											{data.rep.note}
										</p>
										<p style="margin: 0; font-size: 13.5px; color: var(--ink-muted)">
											{data.rep.history}
										</p>
									</section>

									{#if data.rep.decided}
										<div class={data.rep.decisionClass} role="status">
											<span class="m-icon m-alert__icon" aria-hidden="true"
												>{data.rep.decisionIcon}</span
											>
											<div class="m-alert__content">
												<p class="m-alert__title">{data.rep.decisionTitle}</p>
												<div class="m-alert__description">{data.rep.decisionText}</div>
												<div class="m-alert__actions">
													<button
														class="m-button m-button--variant_text m-button--size_sm"
														type="button"><span>Undo</span></button
													>
												</div>
											</div>
											<span></span>
										</div>
									{/if}

									{#if data.rep.actionable}
										<div
											style="display: flex; flex-wrap: wrap; gap: 8px; padding-top: 12px; border-top: 1px solid var(--line)"
										>
											<button class="m-button m-button--variant_filled" type="button"
												><span class="m-icon" aria-hidden="true">visibility_off</span><span
													>Hide post</span
												></button
											>
											<button class="m-button m-button--variant_tonal" type="button"
												><span class="m-icon" aria-hidden="true">warning</span><span
													>Warn {data.rep.author}</span
												></button
											>
											<button class="m-button m-button--variant_text" type="button"
												><span>Dismiss</span></button
											>
											<span style="flex: 1"></span>
											<button
												class="m-button m-button--variant_outline"
												type="button"
												aria-expanded={data.escExpanded}
												aria-controls="esc"
												><span class="m-icon" aria-hidden="true">arrow_outward</span><span
													>Escalate</span
												></button
											>
										</div>
									{/if}

									{#if data.escOpen}
										<form
											id="esc"
											style="display: flex; flex-direction: column; gap: 14px; padding: 16px; border: 1px solid var(--line-strong); border-radius: var(--radius-lg)"
										>
											<h2 style="margin: 0; font: 600 16px/22px var(--font-ui)">
												Send it to a moderation service
											</h2>
											<div class="m-radio__root m-radio__root--variant_cards">
												{#each data.escTargets as t (t.label)}
													<label class="m-radio__item m-radio__item--variant_cards"
														><input
															class="m-radio__control m-radio__control--variant_cards"
															type="radio"
															name="esc-to"
															checked={t.on}
														/><span class="m-radio__text m-radio__text--variant_cards"
															><span class="m-radio__label m-radio__label--variant_cards"
																>{t.label}</span
															><span class="m-radio__description">{t.hint}</span></span
														><span
															class="m-icon m-radio__icon m-radio__icon--variant_cards"
															aria-hidden="true">{t.icon}</span
														></label
													>
												{/each}
											</div>
											{#if data.escOther}
												<div class="m-field__root">
													<label class="m-field__label" for="esc-h">Service handle</label>
													<div class="m-input__root">
														<span class="m-input__addon" data-side="start" aria-hidden="true"
															><span class="m-icon" aria-hidden="true">alternate_email</span></span
														><input
															class="m-input__control"
															id="esc-h"
															type="text"
															placeholder="moderation.example.org"
														/>
													</div>
												</div>
											{/if}
											<div class="m-select__root" style="max-width: 360px">
												<span class="m-field__label" id="esc-r-l">Reason</span><button
													class="m-select__trigger"
													type="button"
													aria-haspopup="listbox"
													aria-labelledby="esc-r-l esc-r-v"
													><span class="m-select__value" id="esc-r-v">{data.escReason}</span><span
														class="m-icon m-select__indicator"
														aria-hidden="true">expand_more</span
													></button
												>
											</div>
											<div style="display: flex; flex-direction: column">
												<label class="m-checkbox__root"
													><input
														class="m-checkbox__control"
														type="checkbox"
														checked={data.escNoteOn}
													/><span class="m-checkbox__text"
														><span class="m-checkbox__label"
															>Include {data.rep.reporter}’s note</span
														></span
													></label
												>
												<label class="m-checkbox__root"
													><input
														class="m-checkbox__control"
														type="checkbox"
														checked={data.escWhoOn}
													/><span class="m-checkbox__text"
														><span class="m-checkbox__label">Say who reported it</span><span
															class="m-checkbox__description"
															>Off unless they agree. The service sees the post either way.</span
														></span
													></label
												>
											</div>
											<div class="m-textarea__root">
												<label class="m-field__label" for="esc-note">Note for the service</label
												><textarea
													class="m-textarea__control m-textarea__control--face_ui"
													id="esc-note"
													rows="2"
													style="--rows: 2"
													placeholder="What should they know?"
													maxlength="2000"
													aria-describedby="esc-count"
													value={data.escNote}></textarea>
												<div class="m-textarea__footer">
													<span class="m-field__help" style="margin: 0"
														>Sent as The Gilded Reach, with a link to the post as it was reported.</span
													><span class="m-textarea__counter" id="esc-count">{data.escCount}</span>
												</div>
											</div>
											<div
												style="display: flex; gap: 8px; justify-content: flex-end; flex-wrap: wrap"
											>
												<button class="m-button m-button--variant_text" type="button"
													><span>Cancel</span></button
												>
												<button class="m-button m-button--variant_filled" type="submit"
													><span class="m-icon" aria-hidden="true">arrow_outward</span><span
														>{data.escSendLabel}</span
													></button
												>
											</div>
										</form>
									{/if}

									<section
										aria-labelledby="h-hist"
										style="display: flex; flex-direction: column; gap: 8px"
									>
										<h2
											id="h-hist"
											style="margin: 0; font: 600 13px/18px var(--font-ui); color: var(--ink-muted)"
										>
											History
										</h2>
										<ol style="list-style: none; margin: 0; padding: 0; display: grid; gap: 10px">
											{#each data.rep.timeline as t (t.icon)}
												<li
													style="display: grid; grid-template-columns: 20px minmax(0, 1fr) auto; gap: 10px; align-items: start; font-size: 14px; line-height: 20px"
												>
													<span
														class="m-icon m-icon--sm"
														aria-hidden="true"
														style="margin-top: 2px; color: var(--ink-muted)">{t.icon}</span
													><span>{t.text}</span><time
														style="font-size: 13px; color: var(--ink-muted)">{t.when}</time
													>
												</li>
											{/each}
										</ol>
									</section>
									<div class="m-textarea__root">
										<label class="m-field__label" for="gm-note">Note for other GMs</label><textarea
											class="m-textarea__control m-textarea__control--face_ui"
											id="gm-note"
											rows="2"
											style="--rows: 2"
											placeholder="Only GMs of this World see it"></textarea>
									</div>
								</div>
							{/if}
						</div>
						<section
							aria-labelledby="h-svc"
							class="quietbox"
							style="display: flex; flex-direction: column; gap: 12px; padding: 16px"
						>
							<div
								style="display: flex; gap: 12px; align-items: flex-start; justify-content: space-between; flex-wrap: wrap"
							>
								<div style="display: grid; gap: 2px">
									<h2 id="h-svc" style="margin: 0; font: 600 15px/20px var(--font-ui)">
										The Gilded Reach as a moderation service
									</h2>
									<span style="font-size: 13px; color: var(--ink-muted)"
										>Reports to this World and its labels come from its own account,
										@gilded-reach.motet.at. You and Bram act for it.</span
									>
								</div>
								<a class="m-button m-button--variant_text m-button--size_sm" href="#service-record"
									><span class="m-icon" aria-hidden="true">data_object</span><span>View record</span
									></a
								>
							</div>
							<dl
								class="m-datalist__root m-datalist__root--variant_plain"
								style="margin: 0; --label-w: 150px"
							>
								<div class="m-datalist__item m-datalist__item--variant_plain">
									<dt class="m-datalist__label m-datalist__label--variant_plain">
										Reports it takes
									</dt>
									<dd
										class="m-datalist__value m-datalist__value--variant_plain"
										style="display: flex; flex-wrap: wrap; gap: 6px"
									>
										<span class="m-chip m-chip--kind_fact"
											><span>Rules and content lines</span></span
										><span class="m-chip m-chip--kind_fact"><span>Harassment</span></span><span
											class="m-chip m-chip--kind_fact"><span>Sexual content</span></span
										><span class="m-chip m-chip--kind_fact"><span>Threats</span></span><span
											class="m-chip m-chip--kind_fact"><span>Something else</span></span
										>
									</dd>
								</div>
								<div class="m-datalist__item m-datalist__item--variant_plain">
									<dt class="m-datalist__label m-datalist__label--variant_plain">About</dt>
									<dd class="m-datalist__value m-datalist__value--variant_plain">
										Posts and accounts in this World
									</dd>
								</div>
								<div class="m-datalist__item m-datalist__item--variant_plain">
									<dt class="m-datalist__label m-datalist__label--variant_plain">
										Labels it applies
									</dt>
									<dd
										class="m-datalist__value m-datalist__value--variant_plain"
										style="display: flex; flex-wrap: wrap; gap: 6px"
									>
										<span class="m-chip m-chip--kind_fact"
											><span class="m-icon" aria-hidden="true">visibility_off</span><span
												>Hidden in this World</span
											></span
										><span class="m-chip m-chip--kind_fact"
											><span class="m-icon" aria-hidden="true">info</span><span
												>Needs a content note</span
											></span
										><span class="m-chip m-chip--kind_fact"
											><span class="m-icon" aria-hidden="true">auto_stories</span><span
												>Spoiler for an arc</span
											></span
										>
									</dd>
								</div>
							</dl>
							<p style="margin: 0; font-size: 13px; color: var(--ink-muted)">
								Motet applies these labels to everyone reading inside The Gilded Reach. Other apps
								apply them for people who subscribe to it.
							</p>
						</section>
					</div>
				{/if}

				{#if data.is.applications}
					<div style="display: flex; flex-direction: column; gap: 20px">
						<h1 class="h-m">Applications</h1>
						<div style="display: flex; flex-wrap: wrap; gap: 20px; align-items: flex-start">
							<div
								role="group"
								style="flex: 1 1 220px; display: flex; flex-direction: column; gap: 4px"
							>
								{#each data.apps as a (a.name)}
									<button
										class="appitem"
										type="button"
										aria-current={a.current}
										style="display: grid; grid-template-columns: 40px minmax(0, 1fr); gap: 2px 12px; align-items: center; padding: 10px; border-radius: var(--radius-md)"
									>
										<span
											class="m-avatar m-avatar--kind_character m-avatar--size_md"
											aria-hidden="true"
											style="grid-row: span 2"><img src={a.portrait} alt="" /></span
										>
										<span style="font-weight: 600">{a.name}</span>
										<span style="font-size: 12.5px; opacity: .8">{a.meta}</span>
									</button>
								{/each}
							</div>
							<div
								style="flex: 999 1 420px; min-width: 0; display: flex; flex-direction: column; gap: 16px"
							>
								{#if data.decided}
									<div class={data.decisionClass} role="status">
										<span class="m-icon m-alert__icon" aria-hidden="true">{data.decisionIcon}</span>
										<div class="m-alert__content">
											<p class="m-alert__title">{data.decisionTitle}</p>
											<div class="m-alert__description">{data.decisionText}</div>
											<div class="m-alert__actions">
												<button
													class="m-button m-button--variant_text m-button--size_sm"
													type="button"><span>Undo</span></button
												>
											</div>
										</div>
										<span></span>
									</div>
								{/if}
								<article class="m-character__root m-ornamented m-marked">
									<div class="m-character__portrait"><img src={data.app.portrait} alt="" /></div>
									<div class="m-character__body">
										<h3 class="m-character__name">{data.app.name}</h3>
										<span class="m-character__epithet">{data.app.epithet}</span>
										<dl class="m-character__facts">
											<dt>Role</dt>
											<dd>{data.app.role}</dd>
											<dt>House</dt>
											<dd>{data.app.house}</dd>
											<dt>Trait</dt>
											<dd>{data.app.trait}</dd>
											<dt>Posting</dt>
											<dd>{data.app.pace}</dd>
										</dl>
										<div class="m-character__owner">
											<span
												class="m-avatar m-avatar--kind_account m-avatar--size_xs"
												aria-hidden="true"
												style={data.app.hue}
												><span aria-hidden="true">{data.app.initials}</span></span
											><span>{data.app.owner}</span>
										</div>
									</div>
								</article>
								<div style="display: flex; flex-direction: column; gap: 6px">
									<h3
										style="margin: 0; font: 600 13px/18px var(--font-ui); color: var(--ink-muted)"
									>
										Background
									</h3>
									<p
										style="margin: 0; font-family: var(--font-reading); font-size: 16px; line-height: 1.55"
									>
										{data.app.bg}
									</p>
								</div>
								<div
									style="display: flex; flex-direction: column; gap: 6px; padding: 12px 14px; background-color: var(--surface-quiet); border-radius: var(--radius-md)"
								>
									<h3
										style="margin: 0; display: flex; gap: 6px; align-items: center; font: 600 13px/18px var(--font-ui); color: var(--ink-muted)"
									>
										<span class="m-icon m-icon--sm" aria-hidden="true">lock</span>Secret, for GMs
									</h3>
									<p
										style="margin: 0; font-family: var(--font-reading); font-size: 16px; line-height: 1.55"
									>
										{data.app.secret}
									</p>
								</div>
								{#if data.asking}
									<div class="m-textarea__root">
										<label class="m-field__label" for="ask">What should change?</label><textarea
											class="m-textarea__control m-textarea__control--face_ui"
											id="ask"
											rows="3"
											style="--rows: 3"
											>Lovely sheet. Could Tamsin know one thing about the banner that Isolde doesn’t? It gives them a reason to talk.</textarea
										>
									</div>
								{/if}
								<div
									style="display: flex; gap: 8px; flex-wrap: wrap; padding-top: 12px; border-top: 1px solid var(--line)"
								>
									<button
										class="m-button m-button--variant_filled"
										type="button"
										disabled={data.decided}
										><span class="m-icon" aria-hidden="true">check</span><span>Approve</span
										></button
									>
									<button
										class="m-button m-button--variant_tonal"
										type="button"
										disabled={data.decided}
										><span class="m-icon" aria-hidden="true">edit_note</span><span
											>{data.askLabel}</span
										></button
									>
									<span style="flex: 1"></span>
									<button
										class="m-button m-button--variant_text m-button--tone_danger"
										type="button"
										disabled={data.decided}><span>Decline</span></button
									>
								</div>
							</div>
						</div>
					</div>
				{/if}

				{#if data.is.recruiting}
					<div style="display: flex; flex-direction: column; gap: 24px">
						<h1 class="h-m">Recruiting and the join form</h1>
						<div
							style="display: flex; gap: 16px; align-items: center; justify-content: space-between; flex-wrap: wrap; padding: 16px; border: 1px solid var(--line); border-radius: var(--radius-lg)"
						>
							<div style="display: grid; gap: 2px">
								<span style="font-weight: 600">Accepting applications</span><span
									style="font-size: 13px; color: var(--ink-muted)"
									>Shown on the overview and in Discover</span
								>
							</div>
							<div style="display: flex; gap: 12px; align-items: center; flex-wrap: wrap">
								{#if data.recruitingOn}
									<div style="display: flex; gap: 8px; align-items: center">
										<div class="m-input__root" style="width: 140px">
											<input
												class="m-input__control"
												type="text"
												value="April 30"
												aria-label="Closes"
											/><span class="m-input__addon" data-side="end" aria-hidden="true"
												><span class="m-icon" aria-hidden="true">calendar_month</span></span
											>
										</div>
										<div class="m-input__root" style="width: 128px">
											<span class="m-input__action"
												><button
													class="m-icon-button m-icon-button--variant_standard"
													type="button"
													aria-label="Fewer seats"
													><span class="m-icon" aria-hidden="true">remove</span></button
												></span
											><input
												class="m-input__control"
												type="text"
												value={data.seats}
												aria-label="Seats"
												readonly
												style="text-align: center"
											/><span class="m-input__action"
												><button
													class="m-icon-button m-icon-button--variant_standard"
													type="button"
													aria-label="More seats"
													><span class="m-icon" aria-hidden="true">add</span></button
												></span
											>
										</div>
										<span style="font-size: 13px; color: var(--ink-muted)">seats</span>
									</div>
								{/if}
								<button
									class="m-switch__root"
									type="button"
									role="switch"
									aria-checked={data.recruitingAttr}
									aria-label="Accepting applications"><span class="m-switch__thumb"></span></button
								>
							</div>
						</div>
						<section
							aria-labelledby="h-fields"
							style="display: flex; flex-direction: column; gap: 8px"
						>
							<div
								style="display: flex; justify-content: space-between; align-items: baseline; flex-wrap: wrap; gap: 8px"
							>
								<h2 id="h-fields" style="margin: 0; font: 600 16px/22px var(--font-ui)">
									Application fields
								</h2>
								<a
									class="m-button m-button--variant_text m-button--size_sm"
									href={resolve('/w/[world]/apply', { world: 'gilded-reach' })}
									><span class="m-icon" aria-hidden="true">visibility</span><span
										>Preview the form</span
									></a
								>
							</div>
							{#each data.fields as f (f.label)}
								<div class="fbox m-vt m-vt-box" data-open={f.openAttr} style="--vt: {f.vtn}">
									<div class="fhead">
										{#if f.movable}<span
												class="m-icon"
												aria-hidden="true"
												style="color: var(--ink-muted); cursor: grab"
												title="Drag to reorder">drag_indicator</span
											>{/if}
										{#if f.fixed}<span
												class="m-icon"
												aria-hidden="true"
												style="color: var(--ink-muted)"
												title="Built in">lock</span
											>{/if}
										<button
											class="fbtn"
											type="button"
											aria-expanded={f.openAttr}
											aria-controls={f.editId}
											style="display: grid; min-width: 0"
											><span style="font-weight: 600">{f.label}</span><span
												style="font-size: 12.5px; color: var(--ink-muted)">{f.kind}</span
											></button
										>
										{#if f.private}<span class="m-chip m-chip--kind_note" style="min-height: 24px"
												><span class="m-icon" aria-hidden="true">lock</span><span>GMs only</span
												></span
											>{/if}
										{#if f.public}<span></span>{/if}
										<label
											class="freq"
											style="display: flex; gap: 8px; align-items: center; font-size: 13px; color: var(--ink-muted)"
											><span>Required</span><button
												class="m-switch__root"
												type="button"
												role="switch"
												aria-checked={f.required}
												aria-label={f.reqLabel}
												disabled={f.fixed}><span class="m-switch__thumb"></span></button
											></label
										>
										<button
											class="m-icon-button m-icon-button--variant_standard m-icon-button--size_sm"
											type="button"
											aria-label={f.editLabel}
											aria-expanded={f.openAttr}
											aria-controls={f.editId}
											><span class="m-icon" aria-hidden="true">{f.chev}</span></button
										>
									</div>
									{#if f.open}
										<div class="fedit" id={f.editId}>
											<div
												style="display: grid; grid-template-columns: repeat(auto-fit, minmax(min(220px, 100%), 1fr)); gap: 14px"
											>
												<div class="m-field__root">
													<label class="m-field__label" for={f.labelId}>Question</label>
													<div class="m-input__root">
														<input
															class="m-input__control"
															id={f.labelId}
															type="text"
															value={f.label}
														/>
													</div>
												</div>
												<div class="m-field__root">
													<label class="m-field__label" for={f.helpId}>Help under it</label>
													<div class="m-input__root">
														<input
															class="m-input__control"
															id={f.helpId}
															type="text"
															value={f.help}
															placeholder="Optional"
														/>
													</div>
												</div>
											</div>
											{#if f.canType}
												<div style="display: grid; gap: 6px">
													<span class="m-field__label" id={f.typeId}>Answer</span>
													<div
														class="m-segmented__root"
														role="radiogroup"
														aria-labelledby={f.typeId}
														style="width: fit-content; max-width: 100%; flex-wrap: wrap"
													>
														{#each f.types as t (t.label)}<button
																class="m-segmented__item"
																type="button"
																role="radio"
																aria-checked={t.checked}
																><span class="m-icon" aria-hidden="true">{t.icon}</span
																>{t.label}</button
															>{/each}
													</div>
												</div>
											{/if}
											{#if f.isChoice}
												<fieldset class="m-fieldset__root" style="margin: 0">
													<legend class="m-fieldset__legend">Options</legend>
													<div
														style="display: flex; flex-direction: column; gap: 6px; margin-top: 4px"
													>
														{#each f.opts as o (o.value)}
															<div style="display: flex; gap: 6px; align-items: center">
																<span
																	class="m-icon"
																	aria-hidden="true"
																	style="color: var(--ink-muted); cursor: grab">drag_indicator</span
																>
																<div class="m-input__root" style="max-width: 320px">
																	<input
																		class="m-input__control"
																		type="text"
																		value={o.value}
																		aria-label={o.aria}
																	/>
																</div>
																<button
																	class="m-icon-button m-icon-button--variant_standard m-icon-button--size_sm"
																	type="button"
																	aria-label={o.removeLabel}
																	><span class="m-icon" aria-hidden="true">close</span></button
																>
															</div>
														{/each}
														<button
															class="m-button m-button--variant_text m-button--size_sm"
															type="button"
															style="align-self: flex-start; margin-inline-start: 22px"
															><span class="m-icon" aria-hidden="true">add</span><span
																>Add an option</span
															></button
														>
													</div>
													<label class="m-checkbox__root"
														><input
															class="m-checkbox__control"
															type="checkbox"
															checked={f.multi}
														/><span class="m-checkbox__text"
															><span class="m-checkbox__label"
																>Applicants can pick more than one</span
															></span
														></label
													>
												</fieldset>
											{/if}
											{#if f.isLong}
												<div class="m-field__root" style="width: 180px">
													<label class="m-field__label" for={f.limitId}>Word limit</label>
													<div class="m-input__root">
														<span class="m-input__action"
															><button
																class="m-icon-button m-icon-button--variant_standard"
																type="button"
																aria-label="Fewer words"
																><span class="m-icon" aria-hidden="true">remove</span></button
															></span
														><input
															class="m-input__control"
															id={f.limitId}
															type="text"
															value={f.limit}
															readonly
															style="text-align: center"
														/><span class="m-input__action"
															><button
																class="m-icon-button m-icon-button--variant_standard"
																type="button"
																aria-label="More words"
																><span class="m-icon" aria-hidden="true">add</span></button
															></span
														>
													</div>
												</div>
											{/if}
											{#if f.canType}
												<div style="display: grid; gap: 6px">
													<span class="m-field__label" id={f.visId}>Who sees the answer</span>
													<div
														class="m-segmented__root"
														role="radiogroup"
														aria-labelledby={f.visId}
														style="width: fit-content"
													>
														{#each f.vis as t (t.label)}<button
																class="m-segmented__item"
																type="button"
																role="radio"
																aria-checked={t.checked}
																><span class="m-icon" aria-hidden="true">{t.icon}</span
																>{t.label}</button
															>{/each}
													</div>
												</div>
											{/if}
											{#if f.fixed}<p style="margin: 0; font-size: 13.5px; color: var(--ink-muted)">
													Every World asks for this, on the public card. You can reword it.
												</p>{/if}
											<div style="display: flex; gap: 8px; flex-wrap: wrap">
												{#if f.movable}<button
														class="m-button m-button--variant_text m-button--tone_danger m-button--size_sm"
														type="button"
														><span class="m-icon" aria-hidden="true">delete</span><span
															>Remove field</span
														></button
													>{/if}
												<span style="flex: 1"></span>
												<button
													class="m-button m-button--variant_tonal m-button--size_sm"
													type="button"><span>Done</span></button
												>
											</div>
										</div>
									{/if}
								</div>
							{/each}
							<div style="position: relative; align-self: flex-start">
								<button
									class="m-button m-button--variant_tonal m-button--size_sm"
									type="button"
									aria-haspopup="menu"
									aria-expanded={data.addOpen}
									><span class="m-icon" aria-hidden="true">add</span><span>Add a field</span
									></button
								>
								{#if data.addOpen}
									<div
										class="m-menu__root"
										role="menu"
										style="position: absolute; top: 40px; left: 0; z-index: 4"
									>
										{#each data.fieldTypes as t (t.label)}<button
												class="m-menu__item"
												type="button"
												role="menuitem"
												><span class="m-icon" aria-hidden="true">{t.icon}</span>{t.label}</button
											>{/each}
									</div>
								{/if}
							</div>
							{#if data.dirty}
								<div
									class="m-alert__root m-alert__root--tone_info m-alert__root--variant_inline"
									style="margin-top: 8px"
								>
									<span class="m-icon m-alert__icon m-alert__icon--tone_info" aria-hidden="true"
										>edit_note</span
									>
									<div class="m-alert__content">
										<p class="m-alert__title">Unpublished changes</p>
										<div class="m-alert__description">
											Applications already sent keep the form they were written for.
										</div>
										<div class="m-alert__actions">
											<button
												class="m-button m-button--variant_filled m-button--size_sm"
												type="button"><span>Publish form</span></button
											><button
												class="m-button m-button--variant_text m-button--size_sm"
												type="button"><span>Discard</span></button
											>
										</div>
									</div>
									<span></span>
								</div>
							{/if}
							{#if data.published}
								<div
									class="m-alert__root m-alert__root--tone_positive m-alert__root--variant_inline"
									role="status"
									style="margin-top: 8px"
								>
									<span class="m-icon m-alert__icon m-alert__icon--tone_positive" aria-hidden="true"
										>check_circle</span
									>
									<div class="m-alert__content">
										<p class="m-alert__title">Form published</p>
										<div class="m-alert__description">New applications use it from now on.</div>
									</div>
									<span></span>
								</div>
							{/if}
						</section>
					</div>
				{/if}

				{#if data.is.members}
					<div style="display: flex; flex-direction: column; gap: 16px">
						<div
							style="display: flex; justify-content: space-between; align-items: center; gap: 12px; flex-wrap: wrap"
						>
							<h1 class="h-m">Members and roles</h1>
							<button class="m-button m-button--variant_tonal" type="button"
								><span class="m-icon" aria-hidden="true">group_add</span><span>Invite</span></button
							>
						</div>
						<div style="display: flex; flex-direction: column; border-top: 1px solid var(--line)">
							{#each data.members as m (m.id)}
								<div
									style="display: grid; grid-template-columns: minmax(0, 1fr) auto auto; gap: 12px; align-items: center; padding: 10px 4px; border-bottom: 1px solid var(--line)"
								>
									<span style="display: flex; gap: 12px; align-items: center; min-width: 0"
										><span
											class="m-avatar m-avatar--kind_account m-avatar--size_sm"
											aria-hidden="true"
											style={m.hue}><span aria-hidden="true">{m.initials}</span></span
										><span style="display: grid; min-width: 0"
											><span style="font-weight: 600">{m.display}</span><span
												style="font-size: 12.5px; color: var(--ink-muted)">{m.sub}</span
											></span
										></span
									>
									<div class="m-segmented__root" role="radiogroup" aria-label={m.roleLabel}>
										{#each m.roles as r (r.label)}<button
												class="m-segmented__item"
												type="button"
												role="radio"
												aria-checked={r.checked}
												disabled={m.owner}>{r.label}</button
											>{/each}
									</div>
									<button
										class="m-icon-button m-icon-button--variant_standard m-icon-button--size_sm"
										type="button"
										aria-label={m.removeLabel}
										disabled={m.owner}
										><span class="m-icon" aria-hidden="true">person_remove</span></button
									>
								</div>
							{/each}
						</div>
					</div>
				{/if}

				{#if data.is.about}
					<div style="display: flex; flex-direction: column; gap: 20px; max-width: 680px">
						<h1 class="h-m">About and rules</h1>
						<div class="m-field__root">
							<label class="m-field__label" for="wname">Name</label>
							<div class="m-input__root">
								<input class="m-input__control" id="wname" type="text" value="The Gilded Reach" />
							</div>
						</div>
						<div class="m-textarea__root">
							<label class="m-field__label" for="wabout">About</label><textarea
								class="m-textarea__control m-textarea__control--face_reading"
								id="wabout"
								rows="5"
								style="--rows: 5"
								>The old Duke is dying in Harrowgate, and every house in the Reach has come to watch. They have come for the Feast of Lanterns, they say: the spring rite when the river carries a light for each of the dead.</textarea
							>
						</div>
						<div
							style="display: grid; grid-template-columns: repeat(auto-fit, minmax(min(200px, 100%), 1fr)); gap: 16px"
						>
							<LanguagePicker
								multiple={false}
								label="Main language"
								value={[data.worldLang]}
								pid="world-lang"
							/>
							<div class="m-select__root">
								<span class="m-field__label" id="vis-l">Who can read</span><button
									class="m-select__trigger"
									type="button"
									aria-haspopup="listbox"
									aria-labelledby="vis-l vis-v"
									><span class="m-select__value" id="vis-v">Anyone, scenes by choice</span><span
										class="m-icon m-select__indicator"
										aria-hidden="true">expand_more</span
									></button
								>
							</div>
						</div>
						<LanguagePicker
							label="Players may also write in"
							value={data.alsoLangs}
							placeholder="Add a language"
							help="Shown in Discover. Posts carry their own language."
							pid="also-lang"
						/>
						<div class="m-field__root">
							<label class="m-field__label" for="wtags">Tags</label>
							<div class="m-tags__root">
								<span class="m-tags__tag"
									><span>Court intrigue</span><button
										class="m-tags__remove"
										type="button"
										aria-label="Remove Court intrigue"
										><span class="m-icon" aria-hidden="true">close</span></button
									></span
								><span class="m-tags__tag"
									><span>Low magic</span><button
										class="m-tags__remove"
										type="button"
										aria-label="Remove Low magic"
										><span class="m-icon" aria-hidden="true">close</span></button
									></span
								><span class="m-tags__tag"
									><span>Slow burn</span><button
										class="m-tags__remove"
										type="button"
										aria-label="Remove Slow burn"
										><span class="m-icon" aria-hidden="true">close</span></button
									></span
								><input class="m-tags__control" id="wtags" type="text" placeholder="Add a tag" />
							</div>
						</div>
						<fieldset class="m-fieldset__root" style="margin: 0">
							<legend class="m-fieldset__legend">Content lines</legend>
							<div
								style="display: grid; grid-template-columns: max-content minmax(0, 1fr); gap: 10px 16px; align-items: center; font-size: 14px"
							>
								<span>In the story</span>
								<div class="m-tags__root">
									<span class="m-tags__tag"><span>Violence</span></span><span class="m-tags__tag"
										><span>Death and grief</span></span
									><span class="m-tags__tag"><span>Political cruelty</span></span><input
										class="m-tags__control"
										type="text"
										aria-label="Add to In the story"
									/>
								</div>
								<span>Off screen</span>
								<div class="m-tags__root">
									<span class="m-tags__tag"><span>Torture</span></span><span class="m-tags__tag"
										><span>Romance beyond a kiss</span></span
									><input class="m-tags__control" type="text" aria-label="Add to Off screen" />
								</div>
								<span>Never</span>
								<div class="m-tags__root">
									<span class="m-tags__tag"><span>Sexual violence</span></span><span
										class="m-tags__tag"><span>Harm to children</span></span
									><input class="m-tags__control" type="text" aria-label="Add to Never" />
								</div>
							</div>
						</fieldset>
						<div>
							<button class="m-button m-button--variant_filled" type="button"
								><span>Save</span></button
							>
						</div>
					</div>
				{/if}

				{#if data.is.theme}
					<div style="display: flex; flex-direction: column; gap: 20px; max-width: 680px">
						<h1 class="h-m">Theme</h1>
						<div
							style="display: grid; grid-template-columns: 200px minmax(0, 1fr); gap: 20px; align-items: center; padding: 16px; border: 1px solid var(--line); border-radius: var(--radius-lg)"
						>
							<div
								aria-hidden="true"
								style="height: 120px; border-radius: var(--radius-md); background-image: var(--art); background-size: cover; background-position: var(--art-focus, 50% 50%)"
							></div>
							<div style="display: flex; flex-direction: column; gap: 10px">
								<div style="display: flex; gap: 6px" aria-label="Palette">
									<span
										style="width: 28px; height: 28px; border-radius: 50%; background-color: var(--accent); box-shadow: inset 0 0 0 1px var(--line)"
									></span>
									<span
										style="width: 28px; height: 28px; border-radius: 50%; background-color: var(--ornament); box-shadow: inset 0 0 0 1px var(--line)"
									></span>
									<span
										style="width: 28px; height: 28px; border-radius: 50%; background-color: var(--surface); box-shadow: inset 0 0 0 1px var(--line-strong)"
									></span>
									<span
										style="width: 28px; height: 28px; border-radius: 50%; background-color: var(--ink); box-shadow: inset 0 0 0 1px var(--line)"
									></span>
								</div>
								<span style="font-family: var(--font-display); font-size: 22px"
									>Cormorant Garamond titles</span
								>
								<span style="font-size: 13px; color: var(--ink-muted)"
									>Filigree ornaments, arch edge, art behind the page. Light by default.</span
								>
							</div>
						</div>
						<div style="display: flex; gap: 8px; flex-wrap: wrap">
							<a
								class="m-button m-button--variant_filled"
								href="https://claude.ai/artifact/Vo1KybPbrkLnu2Pu7W1sC2"
								target="_blank"
								rel="noopener"
								><span class="m-icon" aria-hidden="true">palette</span><span
									>Open the theme editor</span
								><span class="m-icon" aria-hidden="true">open_in_new</span></a
							>
							<button class="m-button m-button--variant_text" type="button"
								><span class="m-icon" aria-hidden="true">history</span><span>Earlier versions</span
								></button
							>
						</div>
					</div>
				{/if}

				{#if data.is.danger}
					<div style="display: flex; flex-direction: column; gap: 16px; max-width: 680px">
						<h1 class="h-m">Archive or hand over</h1>
						<div
							class="setrow"
							style="display: flex; gap: 16px; align-items: center; justify-content: space-between; flex-wrap: wrap; padding: 16px; border: 1px solid var(--line); border-radius: var(--radius-lg)"
						>
							<div style="display: grid; gap: 2px; flex: 1 1 260px">
								<span style="font-weight: 600">Transfer ownership</span><span
									style="font-size: 13px; color: var(--ink-muted)"
									>Another GM becomes the owner. You stay a GM.</span
								>
							</div>
							<button class="m-button m-button--variant_outline" type="button"
								><span class="m-icon" aria-hidden="true">swap_horiz</span><span>Transfer</span
								></button
							>
						</div>
						<div
							class="setrow"
							style="display: flex; gap: 16px; align-items: center; justify-content: space-between; flex-wrap: wrap; padding: 16px; border: 1px solid var(--line); border-radius: var(--radius-lg)"
						>
							<div style="display: grid; gap: 2px; flex: 1 1 260px">
								<span style="font-weight: 600">Archive the World</span><span
									style="font-size: 13px; color: var(--ink-muted)"
									>Everything stays readable. Nobody can post. You can reopen it.</span
								>
							</div>
							<button class="m-button m-button--variant_outline" type="button"
								><span class="m-icon" aria-hidden="true">archive</span><span>Archive</span></button
							>
						</div>
						<div class="m-alert__root m-alert__root--tone_danger m-alert__root--variant_inline">
							<span class="m-icon m-alert__icon m-alert__icon--tone_danger" aria-hidden="true"
								>delete</span
							>
							<div class="m-alert__content">
								<p class="m-alert__title">Delete the World</p>
								<div class="m-alert__description">
									Removes the World, its locations and lore. Players keep their own posts and
									characters in their accounts.
								</div>
								<div class="m-alert__actions">
									<button
										class="m-button m-button--variant_outline m-button--tone_danger m-button--size_sm"
										type="button"><span>Delete World</span></button
									>
								</div>
							</div>
							<span></span>
						</div>
					</div>
				{/if}
			</div>
		</div>
	</main>
</div>

<style>
	.h-m {
		margin: 0;
		font-family: var(--font-display);
		font-weight: var(--display-weight);
		font-size: calc(26px * var(--display-adjust, 1));
		line-height: 1.15;
		text-transform: var(--display-case);
		letter-spacing: var(--display-tracking);
	}
	@media (min-width: 900px) {
		.sidepanel {
			max-width: 272px;
		}
	}
	.fbox {
		border: 1px solid var(--line);
		border-radius: var(--radius-md);
		background-color: var(--surface-raised);
	}
	.fbox[data-open='true'] {
		border-color: var(--line-strong);
		box-shadow: var(--elevation-1);
	}
	.fhead {
		display: grid;
		grid-template-columns: 24px minmax(0, 1fr) auto auto auto;
		gap: 12px;
		align-items: center;
		padding: 8px 8px 8px 12px;
	}
	.fedit {
		display: flex;
		flex-direction: column;
		gap: 16px;
		padding: 16px 16px 16px 48px;
		border-top: 1px solid var(--line);
	}
	@media (max-width: 600px) {
		.fedit {
			padding-inline-start: 16px;
		}
		.fhead {
			grid-template-columns: 24px minmax(0, 1fr) auto;
		}
		.fhead .freq {
			grid-column: 2 / -1;
		}
	}
	@layer reset {
		.fbtn {
			background: none;
			border: 0;
			font: inherit;
			color: inherit;
			cursor: pointer;
			text-align: start;
			padding: 4px 0;
		}
	}
	.quietbox {
		padding: 12px 14px;
		background-color: var(--surface-quiet);
		border-radius: var(--radius-md);
	}
	.task {
		text-decoration: none;
		color: var(--ink);
	}
	.task:hover {
		background-color: var(--surface-quiet);
	}
	.appitem {
		background: none;
		border: 0;
		font: inherit;
		color: inherit;
		cursor: pointer;
		text-align: start;
		width: 100%;
	}
	.appitem:hover {
		background-color: var(--surface-quiet);
	}
	.appitem[aria-current='true'] {
		background-color: var(--accent-soft);
		color: var(--on-accent-soft);
	}
	.frow {
		display: grid;
		grid-template-columns: 24px minmax(0, 1fr) auto auto;
		gap: 12px;
		align-items: center;
		padding: 10px 12px;
		border: 1px solid var(--line);
		border-radius: var(--radius-md);
		background-color: var(--surface-raised);
	}
</style>
