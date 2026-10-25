<script lang="ts">
	import { resolve } from '$app/paths';
	import Chrome from '#lib/components/motet/Chrome.svelte';
	import LanguagePicker from '#lib/components/motet/LanguagePicker.svelte';
	import { world } from '#lib/motet/world.js';
	import type { SettingsData } from '#lib/demo/screens/settings.js';

	let { data }: { data: SettingsData } = $props();
</script>

<svelte:head>
	<title>Settings</title>
</svelte:head>

<div {@attach world(data.pageRef)} class="m-page__root">
	<Chrome account="iga" inbox={data.inboxCount} />
	<main class="m-page__main">
		<div
			class="m-drill"
			data-pane={data.pane}
			style="display: flex; flex-wrap: wrap; gap: 24px 40px; align-items: flex-start"
		>
			<nav
				class="m-page__panel sidepanel m-drill__nav m-vt"
				aria-labelledby="h-settings"
				style="flex: 1 1 220px; --vt: m-nav"
			>
				<h1 id="h-settings" style="margin: 2px 10px 8px; font: 600 20px/28px var(--font-ui)">
					Settings
				</h1>
				{#each data.sections as s (s.label)}
					<button class="m-page__navItem" type="button" aria-current={s.current}
						><span class="m-icon" aria-hidden="true">{s.icon}</span><span>{s.label}</span></button
					>
				{/each}
			</nav>

			<div
				class="m-drill__content m-vt-pane"
				style="flex: 999 1 560px; min-width: 0; display: flex; flex-direction: column; gap: 24px; max-width: 760px"
			>
				<button
					class="m-button m-button--variant_text m-button--size_sm m-drill__back"
					type="button"
					><span class="m-icon" aria-hidden="true">arrow_back</span><span>Settings</span></button
				>
				<h2 style="margin: 0; font: 600 22px/28px var(--font-ui)">{data.title}</h2>

				{#if data.is.account}
					<div style="display: flex; flex-direction: column; gap: 20px">
						<div
							style="display: flex; gap: 16px; align-items: center; padding: 16px; background-color: var(--surface-quiet); border-radius: var(--radius-lg)"
						>
							<span
								class="m-avatar m-avatar--kind_account m-avatar--size_lg"
								aria-hidden="true"
								style="--avatar-hue: 141"><span aria-hidden="true">IB</span></span
							>
							<div style="display: grid; gap: 2px; min-width: 0; flex: 1">
								<span style="font-weight: 600; font-size: 16px">Iga</span>
								<span style="color: var(--ink-muted); font-size: 14px">@iga.bsky.social</span>
							</div>
							<button class="m-button m-button--variant_outline m-button--size_sm" type="button"
								><span class="m-icon" aria-hidden="true">content_copy</span><span>Copy handle</span
								></button
							>
						</div>
						<dl class="m-datalist__root m-datalist__root--variant_ruled" style="margin: 0">
							<div class="m-datalist__item m-datalist__item--variant_ruled">
								<dt class="m-datalist__label m-datalist__label--variant_ruled">Account host</dt>
								<dd
									class="m-datalist__value m-datalist__value--variant_ruled"
									style="display: flex; gap: 8px; align-items: center"
								>
									<span class="m-icon m-icon--sm" aria-hidden="true">dns</span>bsky.social
								</dd>
							</div>
							<div class="m-datalist__item m-datalist__item--variant_ruled">
								<dt class="m-datalist__label m-datalist__label--variant_ruled">Account ID</dt>
								<dd
									class="m-datalist__value m-datalist__value--variant_ruled"
									style="font-family: var(--font-mono); font-size: 13px"
								>
									did:plc:7x3k2w4q9mvhfz6b1r8yn5ta
								</dd>
							</div>
							<div class="m-datalist__item m-datalist__item--variant_ruled">
								<dt class="m-datalist__label m-datalist__label--variant_ruled">Signed in</dt>
								<dd class="m-datalist__value m-datalist__value--variant_ruled">
									This browser, since Feb 12
								</dd>
							</div>
						</dl>
						<div class="m-field__root" style="max-width: 420px">
							<label class="m-field__label" for="email">Email for notifications</label>
							<div class="m-input__root">
								<span class="m-input__addon" data-side="start" aria-hidden="true"
									><span class="m-icon" aria-hidden="true">mail</span></span
								><input
									class="m-input__control"
									id="email"
									type="email"
									value="iga@example.com"
									autocomplete="email"
								/>
							</div>
							<div class="m-field__help">
								<span>Only used for the notifications you turn on.</span>
							</div>
						</div>
						<div style="display: flex; gap: 8px; flex-wrap: wrap">
							<a class="m-button m-button--variant_outline" href={resolve('/sign-in')}
								><span class="m-icon" aria-hidden="true">logout</span><span>Sign out</span></a
							>
							<button class="m-button m-button--variant_text" type="button"
								><span>Sign out everywhere</span></button
							>
						</div>
					</div>
				{/if}

				{#if data.is.profile}
					<div style="display: flex; flex-direction: column; gap: 20px; max-width: 560px">
						<div
							style="display: grid; grid-template-columns: repeat(auto-fit, minmax(min(200px, 100%), 1fr)); gap: 16px"
						>
							<div class="m-field__root">
								<label class="m-field__label" for="dn">Display name</label>
								<div class="m-input__root">
									<input class="m-input__control" id="dn" type="text" value="Iga" />
								</div>
							</div>
							<div class="m-field__root">
								<label class="m-field__label" for="pr">Pronouns</label>
								<div class="m-input__root">
									<input class="m-input__control" id="pr" type="text" value="she/her" />
								</div>
							</div>
						</div>
						<div class="m-textarea__root">
							<label class="m-field__label" for="bio">About you</label><textarea
								class="m-textarea__control m-textarea__control--face_ui"
								id="bio"
								rows="4"
								style="--rows: 4"
								>Court intrigue, slow burns, letters that arrive too late. Posting weekly, usually on Sundays.</textarea
							>
							<div class="m-textarea__footer">
								<span class="m-textarea__counter">94 / 300</span>
							</div>
						</div>
						<div class="setrow" style="border-top: 1px solid var(--line)">
							<div style="display: grid; gap: 2px">
								<span style="font-weight: 600">List the Worlds you play in</span><span
									style="font-size: 13px; color: var(--ink-muted)"
									>Shown on your profile with your characters</span
								>
							</div>
							<button
								class="m-switch__root"
								type="button"
								role="switch"
								aria-checked={data.sw.listWorlds}
								aria-label="List the Worlds you play in"
								><span class="m-switch__thumb"></span></button
							>
						</div>
						<div>
							<button class="m-button m-button--variant_filled" type="button"
								><span>Save profile</span></button
							>
						</div>
					</div>
				{/if}

				{#if data.is.language}
					<div style="display: flex; flex-direction: column; gap: 24px; max-width: 560px">
						<LanguagePicker
							multiple={false}
							label="Interface language"
							value={[data.uiLang]}
							leading="language"
							placeholder="Pick a language"
							help="Posts stay in the language they were written in."
							pid="ui-lang"
						/>
						<LanguagePicker
							label="Languages you read"
							value={data.readLangs}
							placeholder="Add a language"
							help="Discover and Search show these first."
							pid="read-lang"
						/>
						<div style="display: flex; flex-direction: column; gap: 20px">
							<div class="m-field__root" style="max-width: 320px">
								<label class="m-field__label" for="tz">Time zone</label>
								<div class="m-input__root">
									<span class="m-input__addon" data-side="start" aria-hidden="true"
										><span class="m-icon" aria-hidden="true">schedule</span></span
									><input
										class="m-input__control"
										id="tz"
										type="text"
										value="Europe/Warsaw (UTC+2)"
									/>
								</div>
							</div>
							<div style="display: grid; gap: 6px">
								<span class="m-field__label" id="df-l">Dates</span>
								<div
									class="m-segmented__root"
									role="radiogroup"
									aria-labelledby="df-l"
									style="width: fit-content"
								>
									{#each data.dateFormats as o (o.label)}<button
											class="m-segmented__item"
											type="button"
											role="radio"
											aria-checked={o.checked}>{o.label}</button
										>{/each}
								</div>
							</div>
						</div>
					</div>
				{/if}

				{#if data.is.reading}
					<div style="display: flex; flex-wrap: wrap; gap: 24px; align-items: flex-start">
						<div
							style="flex: 1 1 340px; min-width: 0; display: flex; flex-direction: column; gap: 18px"
						>
							<div style="display: grid; gap: 8px">
								<span class="m-field__label" id="mode-l">Mode</span>
								<div class="m-segmented__root" role="radiogroup" aria-labelledby="mode-l">
									{#each data.modes as o (o.label)}<button
											class="m-segmented__item"
											type="button"
											role="radio"
											aria-checked={o.checked}
											><span class="m-icon" aria-hidden="true">{o.icon}</span>{o.label}</button
										>{/each}
								</div>
							</div>
							<div style="display: grid; gap: 8px">
								<span class="m-field__label" id="face-l">Reading face</span>
								<div class="m-segmented__root" role="radiogroup" aria-labelledby="face-l">
									{#each data.faces as o (o.label)}<button
											class="m-segmented__item"
											type="button"
											role="radio"
											aria-checked={o.checked}>{o.label}</button
										>{/each}
								</div>
							</div>
							<div class="m-slider__root">
								<div class="m-slider__header">
									<label class="m-slider__label" for="ts">Text size</label><output
										class="m-slider__value"
										for="ts">{data.scaleText}</output
									>
								</div>
								<input
									class="m-slider__control"
									id="ts"
									type="range"
									min="85"
									max="150"
									step="5"
									value={data.scale}
									style={data.scaleFill}
								/>
							</div>
							<div style="display: grid; gap: 8px">
								<span class="m-field__label" id="den-l">Density</span>
								<div class="m-segmented__root" role="radiogroup" aria-labelledby="den-l">
									{#each data.densities as o (o.label)}<button
											class="m-segmented__item"
											type="button"
											role="radio"
											aria-checked={o.checked}>{o.label}</button
										>{/each}
								</div>
							</div>
							<div>
								{#each data.readingSwitches as r (r.label)}
									<div class="setrow">
										<div style="display: grid; gap: 2px">
											<span style="font-weight: 600">{r.label}</span><span
												style="font-size: 13px; color: var(--ink-muted)">{r.hint}</span
											>
										</div>
										<button
											class="m-switch__root"
											type="button"
											role="switch"
											aria-checked={r.on}
											aria-label={r.label}><span class="m-switch__thumb"></span></button
										>
									</div>
								{/each}
							</div>
						</div>
						<figure
							style="flex: 1 1 300px; min-width: 0; margin: 0; display: flex; flex-direction: column; gap: 8px; position: sticky; top: calc(var(--chrome-offset, 72px) + 8px)"
						>
							<figcaption style="font: 600 13px/18px var(--font-ui); color: var(--ink-muted)">
								Preview in The Gilded Reach
							</figcaption>
							<div
								{@attach world(data.previewRef)}
								style="padding: 20px; border: 1px solid var(--line); border-radius: var(--radius-lg); background-color: var(--surface); background-image: var(--grain); color: var(--ink); overflow: hidden"
							>
								<div class="m-world__stream">
									<article
										class="m-post__root m-post__root--lane_ic m-marked"
										aria-label="Brother Cael"
									>
										<div class="m-post__portrait m-post__portrait--lane_ic">
											<span
												class="m-avatar m-avatar--kind_character m-avatar--size_lg"
												aria-hidden="true"><img src={data.portraits.cael} alt="" /></span
											>
										</div>
										<div class="m-post__main m-post__main--lane_ic">
											<header class="m-post__header m-post__header--lane_ic">
												<div class="m-post__mini m-post__mini--lane_ic">
													<span
														class="m-avatar m-avatar--kind_character m-avatar--size_sm"
														aria-hidden="true"><img src={data.portraits.cael} alt="" /></span
													>
												</div>
												<div class="m-post__speaker m-post__speaker--lane_ic">
													<span class="m-post__name m-post__name--lane_ic">Brother Cael</span><span
														class="m-post__epithet m-post__epithet--lane_ic"
														>Keeper of the Archive</span
													>
												</div>
												<div class="m-post__author m-post__author--lane_ic">
													<span class="m-icon" aria-hidden="true">edit_note</span><span
														class="m-post__handle m-post__handle--lane_ic">Tom @tom.writes</span
													>
												</div>
											</header>
											<div class="m-post__body m-post__body--lane_ic m-prose">
												<p>
													Cael found her by the hearth, a cup of spiced wine he did not intend to
													drink held like an excuse. “The Corven banner,” he said quietly, before
													she could ask.
												</p>
											</div>
										</div>
									</article>
								</div>
							</div>
						</figure>
					</div>
				{/if}

				{#if data.is.notifications}
					<div style="display: flex; flex-direction: column; gap: 28px">
						<section
							aria-labelledby="n-roles"
							style="display: flex; flex-direction: column; gap: 4px"
						>
							<h3 id="n-roles" style="margin: 0 0 4px; font: 600 16px/22px var(--font-ui)">
								What to tell you about
							</h3>
							<p style="margin: 0 0 8px; font-size: 14px; color: var(--ink-muted)">
								Defaults for each role. A World or a single scene can override them.
							</p>
							{#each data.roleRows as r (r.label)}
								<div class="setrow" style="flex-wrap: wrap">
									<div style="display: flex; gap: 12px; align-items: flex-start; flex: 1 1 260px">
										<span
											class="m-icon"
											aria-hidden="true"
											style="color: var(--ink-muted); margin-top: 2px">{r.icon}</span
										>
										<div style="display: grid; gap: 2px">
											<span style="font-weight: 600">{r.label}</span><span
												style="font-size: 13px; color: var(--ink-muted)">{r.hint}</span
											>
										</div>
									</div>
									<div class="m-segmented__root" role="radiogroup" aria-label={r.label}>
										{#each r.options as o (o.label)}<button
												class="m-segmented__item"
												type="button"
												role="radio"
												aria-checked={o.checked}>{o.label}</button
											>{/each}
									</div>
								</div>
							{/each}
							<div class="setrow">
								<div style="display: flex; gap: 12px; align-items: flex-start">
									<span
										class="m-icon"
										aria-hidden="true"
										style="color: var(--ink-muted); margin-top: 2px">shield_person</span
									>
									<div style="display: grid; gap: 2px">
										<span style="font-weight: 600">Worlds you run</span><span
											style="font-size: 13px; color: var(--ink-muted)"
											>Applications, reports and scenes waiting on a GM. You run no Worlds yet.</span
										>
									</div>
								</div>
								<button
									class="m-switch__root"
									type="button"
									role="switch"
									aria-checked={data.sw.gm}
									aria-label="Worlds you run"><span class="m-switch__thumb"></span></button
								>
							</div>
						</section>
						<section aria-labelledby="n-how" style="display: flex; flex-direction: column">
							<h3 id="n-how" style="margin: 0 0 4px; font: 600 16px/22px var(--font-ui)">
								How to reach you
							</h3>
							<div class="setrow">
								<div style="display: flex; gap: 12px; align-items: flex-start">
									<span
										class="m-icon"
										aria-hidden="true"
										style="color: var(--ink-muted); margin-top: 2px">inbox</span
									>
									<div style="display: grid; gap: 2px">
										<span style="font-weight: 600">Inbox</span><span
											style="font-size: 13px; color: var(--ink-muted)">Always on</span
										>
									</div>
								</div>
								<button
									class="m-switch__root"
									type="button"
									role="switch"
									aria-checked="true"
									aria-disabled="true"
									aria-label="Inbox"><span class="m-switch__thumb"></span></button
								>
							</div>
							<div class="setrow" style="flex-wrap: wrap">
								<div style="display: flex; gap: 12px; align-items: flex-start; flex: 1 1 260px">
									<span
										class="m-icon"
										aria-hidden="true"
										style="color: var(--ink-muted); margin-top: 2px">mail</span
									>
									<div style="display: grid; gap: 2px">
										<span style="font-weight: 600">Email</span><span
											style="font-size: 13px; color: var(--ink-muted)">iga@example.com</span
										>
									</div>
								</div>
								<div style="display: flex; gap: 12px; align-items: center">
									{#if data.emailOn}
										<div class="m-segmented__root" role="radiogroup" aria-label="Email frequency">
											{#each data.digests as o (o.label)}<button
													class="m-segmented__item"
													type="button"
													role="radio"
													aria-checked={o.checked}>{o.label}</button
												>{/each}
										</div>
									{/if}
									<button
										class="m-switch__root"
										type="button"
										role="switch"
										aria-checked={data.sw.email}
										aria-label="Email"><span class="m-switch__thumb"></span></button
									>
								</div>
							</div>
							<div class="setrow">
								<div style="display: flex; gap: 12px; align-items: flex-start">
									<span
										class="m-icon"
										aria-hidden="true"
										style="color: var(--ink-muted); margin-top: 2px">smartphone</span
									>
									<div style="display: grid; gap: 2px">
										<span style="font-weight: 600">Push on this device</span><span
											style="font-size: 13px; color: var(--ink-muted)"
											>Your turn and mentions only</span
										>
									</div>
								</div>
								<button
									class="m-switch__root"
									type="button"
									role="switch"
									aria-checked={data.sw.push}
									aria-label="Push on this device"><span class="m-switch__thumb"></span></button
								>
							</div>
							<div class="setrow" style="flex-wrap: wrap">
								<div style="display: flex; gap: 12px; align-items: flex-start; flex: 1 1 260px">
									<span
										class="m-icon"
										aria-hidden="true"
										style="color: var(--ink-muted); margin-top: 2px">bedtime</span
									>
									<div style="display: grid; gap: 2px">
										<span style="font-weight: 600">Quiet hours</span><span
											style="font-size: 13px; color: var(--ink-muted)"
											>Push and email wait until morning. Europe/Warsaw</span
										>
									</div>
								</div>
								<div style="display: flex; gap: 8px; align-items: center">
									{#if data.quietOn}
										<div style="display: flex; gap: 6px; align-items: center">
											<div class="m-input__root" style="width: 92px">
												<input
													class="m-input__control"
													type="text"
													value="22:00"
													aria-label="Quiet from"
													inputmode="numeric"
													style="text-align: center"
												/>
											</div>
											<span aria-hidden="true" style="color: var(--ink-muted)">to</span>
											<div class="m-input__root" style="width: 92px">
												<input
													class="m-input__control"
													type="text"
													value="08:00"
													aria-label="Quiet until"
													inputmode="numeric"
													style="text-align: center"
												/>
											</div>
										</div>
									{/if}
									<button
										class="m-switch__root"
										type="button"
										role="switch"
										aria-checked={data.sw.quiet}
										aria-label="Quiet hours"><span class="m-switch__thumb"></span></button
									>
								</div>
							</div>
						</section>
						<section aria-labelledby="n-worlds" style="display: flex; flex-direction: column">
							<h3 id="n-worlds" style="margin: 0 0 4px; font: 600 16px/22px var(--font-ui)">
								Set differently in
							</h3>
							<a
								href={resolve('/w/[world]/settings', { world: 'gilded-reach' })}
								class="setrow"
								style="text-decoration: none; color: var(--ink)"
							>
								<span style="display: flex; gap: 12px; align-items: center"
									><span class="m-avatar m-avatar--kind_world m-avatar--size_sm" aria-hidden="true"
										><img src={data.marks.vellum} alt="" /></span
									><span style="display: grid"
										><span style="font-weight: 600">The Gilded Reach</span><span
											style="font-size: 13px; color: var(--ink-muted)"
											>All posts in your scenes, 1 scene muted</span
										></span
									></span
								>
								<span class="m-icon" aria-hidden="true" style="color: var(--ink-muted)"
									>chevron_right</span
								>
							</a>
							<a
								href="#kepler-settings"
								class="setrow"
								style="text-decoration: none; color: var(--ink)"
							>
								<span style="display: flex; gap: 12px; align-items: center"
									><span class="m-avatar m-avatar--kind_world m-avatar--size_sm" aria-hidden="true"
										><img src={data.marks.kepler} alt="" /></span
									><span style="display: grid"
										><span style="font-weight: 600">Kepler Relay</span><span
											style="font-size: 13px; color: var(--ink-muted)"
											>Your turn and mentions only</span
										></span
									></span
								>
								<span class="m-icon" aria-hidden="true" style="color: var(--ink-muted)"
									>chevron_right</span
								>
							</a>
						</section>
					</div>
				{/if}

				{#if data.is.writing}
					<div style="display: flex; flex-direction: column; gap: 22px; max-width: 600px">
						<fieldset class="m-fieldset__root" style="margin: 0">
							<legend class="m-fieldset__legend">Drafts</legend>
							<div class="m-radio__root">
								<label class="m-radio__item"
									><input
										class="m-radio__control"
										type="radio"
										name="drafts"
										checked={data.draftLocal}
									/><span class="m-radio__text"
										><span class="m-radio__label">On this device</span><span
											class="m-radio__description">Nothing leaves this browser until you post</span
										></span
									></label
								>
								<label class="m-radio__item"
									><input
										class="m-radio__control"
										type="radio"
										name="drafts"
										checked={data.draftSync}
									/><span class="m-radio__text"
										><span class="m-radio__label">On this device and your account</span><span
											class="m-radio__description"
											>Continue a draft on another device. Drafts are private records only you can
											read.</span
										></span
									></label
								>
							</div>
						</fieldset>
						<div style="display: grid; gap: 8px">
							<span class="m-field__label" id="ed-l">Editor</span>
							<div class="m-segmented__root" role="radiogroup" aria-labelledby="ed-l">
								{#each data.editors as o (o.label)}<button
										class="m-segmented__item"
										type="button"
										role="radio"
										aria-checked={o.checked}>{o.label}</button
									>{/each}
							</div>
						</div>
						<div class="m-select__root" style="max-width: 320px">
							<span class="m-field__label" id="sp-l">Spelling</span><button
								class="m-select__trigger"
								type="button"
								aria-haspopup="listbox"
								aria-labelledby="sp-l sp-v"
								><span class="m-select__value" id="sp-v">Match the World’s language</span><span
									class="m-icon m-select__indicator"
									aria-hidden="true">expand_more</span
								></button
							>
						</div>
						<div>
							<div class="setrow">
								<div style="display: grid; gap: 2px">
									<span style="font-weight: 600">Ask before leaving a draft</span><span
										style="font-size: 13px; color: var(--ink-muted)"
										>When a draft is not saved anywhere yet</span
									>
								</div>
								<button
									class="m-switch__root"
									type="button"
									role="switch"
									aria-checked={data.sw.askDraft}
									aria-label="Ask before leaving a draft"
									><span class="m-switch__thumb"></span></button
								>
							</div>
							<div class="setrow">
								<div style="display: grid; gap: 2px">
									<span style="font-weight: 600">Show word count</span><span
										style="font-size: 13px; color: var(--ink-muted)"
										>Under the composer, against the World’s post length</span
									>
								</div>
								<button
									class="m-switch__root"
									type="button"
									role="switch"
									aria-checked={data.sw.words}
									aria-label="Show word count"><span class="m-switch__thumb"></span></button
								>
							</div>
						</div>
					</div>
				{/if}

				{#if data.is.privacy}
					<div style="display: flex; flex-direction: column; gap: 24px; max-width: 640px">
						<fieldset class="m-fieldset__root" style="margin: 0">
							<legend class="m-fieldset__legend">Who can mention you</legend>
							<div class="m-radio__root">
								<label class="m-radio__item"
									><input class="m-radio__control" type="radio" name="mentions" /><span
										class="m-radio__text"><span class="m-radio__label">Anyone</span></span
									></label
								>
								<label class="m-radio__item"
									><input class="m-radio__control" type="radio" name="mentions" checked /><span
										class="m-radio__text"
										><span class="m-radio__label">People in your Worlds</span></span
									></label
								>
								<label class="m-radio__item"
									><input class="m-radio__control" type="radio" name="mentions" /><span
										class="m-radio__text"><span class="m-radio__label">Nobody</span></span
									></label
								>
							</div>
						</fieldset>
						<div class="setrow" style="border-block: 1px solid var(--line)">
							<div style="display: grid; gap: 2px">
								<span style="font-weight: 600">Show the Worlds you watch on your profile</span><span
									style="font-size: 13px; color: var(--ink-muted)"
									>Worlds you play in are listed from Profile</span
								>
							</div>
							<button
								class="m-switch__root"
								type="button"
								role="switch"
								aria-checked={data.sw.showWatch}
								aria-label="Show the Worlds you watch on your profile"
								><span class="m-switch__thumb"></span></button
							>
						</div>
					</div>
				{/if}

				{#if data.is.moderation}
					<div style="display: flex; flex-direction: column; gap: 28px; max-width: 680px">
						<section
							aria-labelledby="mod-services"
							style="display: flex; flex-direction: column; gap: 4px"
						>
							<h3 id="mod-services" style="margin: 0 0 4px; font: 600 16px/22px var(--font-ui)">
								Moderation services
							</h3>
							<p style="margin: 0 0 8px; font-size: 14px; color: var(--ink-muted)">
								They label posts and accounts for you and take your reports.
							</p>
							{#each data.services as v (v.name)}
								<div class="setrow">
									<span style="display: flex; gap: 12px; align-items: center; min-width: 0">
										<span
											class="m-avatar m-avatar--kind_account m-avatar--size_md"
											aria-hidden="true"
											style={v.hue}
											><span class="m-icon" aria-hidden="true" style="font-size: 20px"
												>{v.icon}</span
											></span
										>
										<span style="display: grid; gap: 1px; min-width: 0"
											><span style="font-weight: 600">{v.name}</span><span
												style="font-size: 13px; color: var(--ink-muted)">@{v.handle}</span
											><span style="font-size: 13px; color: var(--ink-muted)">{v.labels}</span
											></span
										>
									</span>
									{#if v.locked}<span
											class="m-meta"
											style="font-size: 13px; color: var(--ink-muted); flex: none"
											><span class="m-icon m-icon--sm" aria-hidden="true">lock</span><span
												>Always on in Motet</span
											></span
										>{/if}
									{#if v.optional}<button
											class="m-switch__root"
											type="button"
											role="switch"
											aria-checked={v.on}
											aria-label={v.switchLabel}><span class="m-switch__thumb"></span></button
										>{/if}
								</div>
							{/each}
							<div class="setrow">
								<span style="display: flex; gap: 12px; align-items: center; min-width: 0">
									<span
										class="m-avatar m-avatar--kind_account m-avatar--size_md"
										aria-hidden="true"
										style="--avatar-hue: 45"
										><span class="m-icon" aria-hidden="true" style="font-size: 20px">public</span
										></span
									>
									<span style="display: grid; gap: 1px; min-width: 0"
										><span style="font-weight: 600">Your Worlds</span><span
											style="font-size: 13px; color: var(--ink-muted)"
											>The Gilded Reach, Kepler Relay, The Salt Road</span
										><span style="font-size: 13px; color: var(--ink-muted)"
											>Each World moderates itself. Its labels apply while you read it.</span
										></span
									>
								</span>
								<span class="m-meta" style="font-size: 13px; color: var(--ink-muted); flex: none"
									><span class="m-icon m-icon--sm" aria-hidden="true">lock</span><span
										>Inside each World</span
									></span
								>
							</div>
							<form
								style="display: flex; gap: 8px; align-items: flex-end; flex-wrap: wrap; padding-top: 12px"
							>
								<div class="m-field__root" style="flex: 1 1 260px">
									<label class="m-field__label" for="svc">Add a service</label>
									<div class="m-input__root">
										<span class="m-input__addon" data-side="start" aria-hidden="true"
											><span class="m-icon" aria-hidden="true">alternate_email</span></span
										><input
											class="m-input__control"
											id="svc"
											type="text"
											placeholder="handle of a moderation service"
											value={data.svcDraft}
										/>
									</div>
								</div>
								<button class="m-button m-button--variant_tonal" type="submit"
									><span class="m-icon" aria-hidden="true">add_moderator</span><span>Add</span
									></button
								>
							</form>
						</section>

						<section aria-labelledby="mod-labels" style="display: flex; flex-direction: column">
							<h3 id="mod-labels" style="margin: 0 0 4px; font: 600 16px/22px var(--font-ui)">
								Labelled content
							</h3>
							{#each data.labelRows as l (l.label)}
								<div class="setrow" style="flex-wrap: wrap">
									<span style="display: grid; gap: 2px"
										><span style="font-weight: 600">{l.label}</span><span
											style="display: inline-flex; gap: 6px; align-items: center; font-size: 13px; color: var(--ink-muted)"
											><span class="m-icon m-icon--sm" aria-hidden="true">label</span
											>{l.source}</span
										></span
									>
									{#if l.open}<div class="m-segmented__root" role="radiogroup" aria-label={l.label}>
											{#each l.options as o (o.label)}<button
													class="m-segmented__item"
													type="button"
													role="radio"
													aria-checked={o.checked}>{o.label}</button
												>{/each}
										</div>{/if}
									{#if l.locked}<span
											class="m-meta"
											style="font-size: 13px; color: var(--ink-muted)"
											><span class="m-icon m-icon--sm" aria-hidden="true">lock</span><span
												>Always hidden</span
											></span
										>{/if}
								</div>
							{/each}
							<div class="setrow">
								<div style="display: grid; gap: 2px">
									<span style="font-weight: 600">Open content warnings without asking</span><span
										style="font-size: 13px; color: var(--ink-muted)">Spoilers stay closed</span
									>
								</div>
								<button
									class="m-switch__root"
									type="button"
									role="switch"
									aria-checked={data.sw.cw}
									aria-label="Open content warnings without asking"
									><span class="m-switch__thumb"></span></button
								>
							</div>
							<div class="m-field__root" style="padding-top: 12px">
								<label class="m-field__label" for="mw">Muted words</label>
								<div class="m-tags__root">
									<span class="m-tags__tag"
										><span>spiders</span><button
											class="m-tags__remove"
											type="button"
											aria-label="Remove spiders"
											><span class="m-icon" aria-hidden="true">close</span></button
										></span
									><span class="m-tags__tag"
										><span>drowning</span><button
											class="m-tags__remove"
											type="button"
											aria-label="Remove drowning"
											><span class="m-icon" aria-hidden="true">close</span></button
										></span
									><input class="m-tags__control" id="mw" type="text" placeholder="Add a word" />
								</div>
								<div class="m-field__help">
									<span>Posts with these words are folded behind a warning.</span>
								</div>
							</div>
						</section>

						<section aria-labelledby="mod-reports" style="display: flex; flex-direction: column">
							<h3 id="mod-reports" style="margin: 0 0 4px; font: 600 16px/22px var(--font-ui)">
								Your reports
							</h3>
							{#each data.myReports as r, i (i)}
								<div class="setrow" style="flex-wrap: wrap; align-items: flex-start">
									<span style="display: grid; gap: 3px; min-width: 0; flex: 1 1 280px">
										<span style="font-weight: 600">{r.what}</span>
										<span
											style="display: flex; flex-wrap: wrap; gap: 4px 14px; font-size: 13px; color: var(--ink-muted)"
											><span class="m-meta"
												><span class="m-icon m-icon--sm" aria-hidden="true">{r.toIcon}</span><span
													>{r.to}</span
												></span
											><span class="m-meta"
												><span class="m-icon m-icon--sm" aria-hidden="true">schedule</span><span
													>{r.when}</span
												></span
											></span
										>
									</span>
									<span class={r.flagClass}
										><span class="m-icon m-icon--sm" aria-hidden="true">{r.flagIcon}</span
										>{r.status}</span
									>
								</div>
							{/each}
						</section>

						<section aria-labelledby="blocked" style="display: flex; flex-direction: column">
							<h3 id="blocked" style="margin: 0 0 4px; font: 600 16px/22px var(--font-ui)">
								Blocked and muted accounts
							</h3>
							<div class="setrow">
								<span style="display: flex; gap: 12px; align-items: center"
									><span
										class="m-avatar m-avatar--kind_account m-avatar--size_sm"
										aria-hidden="true"
										style="--avatar-hue: 10"><span aria-hidden="true">SP</span></span
									><span style="display: grid"
										><span style="font-weight: 600">spamlord</span><span
											style="font-size: 13px; color: var(--ink-muted)">@spamlord.pds.example</span
										></span
									></span
								><span style="display: flex; gap: 12px; align-items: center"
									><span class="m-meta" style="font-size: 13px; color: var(--ink-muted)"
										><span class="m-icon m-icon--sm" aria-hidden="true">block</span><span
											>Blocked</span
										></span
									><button
										class="m-button m-button--variant_outline m-button--size_sm"
										type="button"><span>Unblock</span></button
									></span
								>
							</div>
							<div class="setrow">
								<span style="display: flex; gap: 12px; align-items: center"
									><span
										class="m-avatar m-avatar--kind_account m-avatar--size_sm"
										aria-hidden="true"
										style="--avatar-hue: 60"><span aria-hidden="true">LH</span></span
									><span style="display: grid"
										><span style="font-weight: 600">loudhailer</span><span
											style="font-size: 13px; color: var(--ink-muted)">@loudhailer.bsky.social</span
										></span
									></span
								><span style="display: flex; gap: 12px; align-items: center"
									><span class="m-meta" style="font-size: 13px; color: var(--ink-muted)"
										><span class="m-icon m-icon--sm" aria-hidden="true">volume_off</span><span
											>Muted</span
										></span
									><button
										class="m-button m-button--variant_outline m-button--size_sm"
										type="button"><span>Unmute</span></button
									></span
								>
							</div>
						</section>
					</div>
				{/if}

				{#if data.is.data}
					<div style="display: flex; flex-direction: column; gap: 24px; max-width: 640px">
						<div class="setrow" style="border-bottom: 1px solid var(--line)">
							<div style="display: grid; gap: 2px">
								<span style="font-weight: 600">Export everything</span><span
									style="font-size: 13px; color: var(--ink-muted)"
									>Posts, characters, drafts and settings as Markdown and JSON</span
								>
							</div>
							<button class="m-button m-button--variant_tonal" type="button"
								><span class="m-icon" aria-hidden="true">download</span><span>Export</span></button
							>
						</div>
						<div class="setrow" style="border-bottom: 1px solid var(--line)">
							<div style="display: grid; gap: 2px">
								<span style="font-weight: 600">Your records</span><span
									style="font-size: 13px; color: var(--ink-muted)"
									>142 posts, 3 characters, 1 profile</span
								>
							</div>
							<button class="m-button m-button--variant_outline" type="button"
								><span class="m-icon" aria-hidden="true">data_object</span><span>View records</span
								></button
							>
						</div>
						<div class="m-upload__root">
							<span class="m-field__label">Import from another forum</span><label
								class="m-upload__dropzone"
								for="imp"
								><input type="file" id="imp" accept=".zip,.json" /><span
									class="m-icon m-upload__icon"
									aria-hidden="true">upload</span
								><span class="m-upload__title">Drop an export from phpBB, NodeBB or Discourse</span
								><span class="m-upload__hint"
									>You choose which posts become yours before anything is published</span
								></label
							>
						</div>
						<div class="m-alert__root m-alert__root--tone_danger m-alert__root--variant_inline">
							<span class="m-icon m-alert__icon m-alert__icon--tone_danger" aria-hidden="true"
								>delete</span
							>
							<div class="m-alert__content">
								<p class="m-alert__title">Delete your Motet data</p>
								<div class="m-alert__description">
									Removes your posts, characters and drafts from your account. Your account itself
									stays.
								</div>
								<div class="m-alert__actions">
									<button
										class="m-button m-button--variant_outline m-button--tone_danger m-button--size_sm"
										type="button"><span>Delete Motet data</span></button
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
	.setrow {
		display: flex;
		gap: 16px;
		align-items: center;
		justify-content: space-between;
		padding-block: 14px;
		border-bottom: 1px solid var(--line);
	}
	.setrow:last-child {
		border-bottom: 0;
	}
	@media (min-width: 900px) {
		.sidepanel {
			max-width: 280px;
		}
	}
</style>
