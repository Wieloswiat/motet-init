<script lang="ts">
	import { resolve } from '$app/paths';
	import Chrome from '#lib/components/motet/Chrome.svelte';
	import LanguagePicker from '#lib/components/motet/LanguagePicker.svelte';
	import { world } from '#lib/motet/world.js';
	import type { StartWorldData } from '#lib/demo/screens/start-world.js';

	let { data }: { data: StartWorldData } = $props();
</script>

<svelte:head>
	<title>Start a World</title>
</svelte:head>

<div {@attach world(data.pageRef)} class="m-page__root">
	<Chrome account="iga" inbox={data.inboxCount} />
	<main class="m-page__main">
		{#if data.building}
			<div class="cw">
				<div class="cw-main">
					<div style="display: grid; gap: 4px">
						<a
							class="m-button m-button--variant_text m-button--size_sm"
							href={resolve('/')}
							style="justify-self: start; margin-inline-start: -8px"
							><span class="m-icon" aria-hidden="true">arrow_back</span><span>Home</span></a
						>
						<h1 style="margin: 0; font: 600 28px/34px var(--font-ui)">Start a World</h1>
					</div>
					<div class="m-steps__root">
						<ol class="m-steps__list">
							{#each data.steps as s (s.title)}
								<li class="m-steps__step" data-state={s.state} aria-current={s.current}>
									<button
										class="stepbtn"
										type="button"
										style="display: contents; background: none; border: 0; padding: 0; font: inherit; color: inherit; cursor: pointer"
									>
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

					<form class="cw-card m-vt-pane">
						{#if data.is.world}
							<div style="display: flex; flex-direction: column; gap: 18px">
								<h2 class="cw-h">The World</h2>
								<div class="m-field__root">
									<label class="m-field__label" for="cw-name">Name</label>
									<div class="m-input__root">
										<input
											class="m-input__control"
											id="cw-name"
											type="text"
											value={data.name}
											placeholder="The name players will see"
										/>
									</div>
								</div>
								<div class="m-field__root">
									<label class="m-field__label" for="cw-handle">Handle</label>
									<div class="m-input__root">
										<span class="m-input__addon" data-side="start" aria-hidden="true"
											><span class="m-icon" aria-hidden="true">alternate_email</span></span
										><input
											class="m-input__control"
											id="cw-handle"
											type="text"
											value={data.handleStem}
											aria-describedby="cw-handle-help"
											spellcheck="false"
										/><span
											class="m-input__addon"
											data-side="end"
											style="padding-inline-end: 12px; color: var(--ink-muted)">.motet.at</span
										>
									</div>
									<div class="m-field__help" id="cw-handle-help">
										The World's own account. Its moderation and records come from it.
									</div>
								</div>
								<div class="m-textarea__root">
									<label class="m-field__label" for="cw-desc">In a few lines</label><textarea
										class="m-textarea__control"
										id="cw-desc"
										rows="3"
										style="--rows: 3"
										maxlength="300"
										aria-describedby="cw-desc-count"
										value={data.desc}></textarea>
									<div class="m-textarea__footer">
										<span></span><span class="m-textarea__counter" id="cw-desc-count"
											>{data.descCount}</span
										>
									</div>
								</div>
								<div
									style="display: grid; grid-template-columns: repeat(auto-fit, minmax(min(240px, 100%), 1fr)); gap: 16px"
								>
									<LanguagePicker
										multiple={false}
										label="Main language"
										value={[data.lang]}
										leading="language"
										placeholder="Pick a language"
										pid="cw-lang"
									/>
									<LanguagePicker
										multiple
										label="Players may also write in"
										value={data.also}
										placeholder="Add a language"
										pid="cw-also"
									/>
								</div>
							</div>
						{/if}

						{#if data.is.table}
							<div style="display: flex; flex-direction: column; gap: 18px">
								<h2 class="cw-h">The table</h2>
								<div class="m-field__root">
									<span class="m-field__label" id="cw-genre">Genre</span>
									<div
										role="group"
										aria-labelledby="cw-genre"
										style="display: flex; flex-wrap: wrap; gap: 8px"
									>
										{#each data.genres as g (g.label)}<button
												class="m-chip m-chip--kind_filter"
												type="button"
												aria-pressed={g.on}><span>{g.label}</span></button
											>{/each}
									</div>
								</div>
								<div class="m-field__root">
									<span class="m-field__label" id="cw-sys">Rules</span>
									<div
										class="m-segmented__root"
										role="radiogroup"
										aria-labelledby="cw-sys"
										style="align-self: flex-start; width: fit-content"
									>
										{#each data.systems as o (o.label)}<button
												class="m-segmented__item"
												type="button"
												role="radio"
												aria-checked={o.on}>{o.label}</button
											>{/each}
									</div>
									{#if data.namedSystem}<div
											class="m-input__root"
											style="margin-top: 8px; max-width: 360px"
										>
											<input
												class="m-input__control"
												type="text"
												aria-label="Which game system"
												placeholder="Which one?"
												value="Mothership 1e"
											/>
										</div>{/if}
								</div>
								<div class="m-field__root">
									<span class="m-field__label" id="cw-pace">Pace</span>
									<div
										class="m-segmented__root"
										role="radiogroup"
										aria-labelledby="cw-pace"
										style="align-self: flex-start; width: fit-content"
									>
										{#each data.paces as o (o.label)}<button
												class="m-segmented__item"
												type="button"
												role="radio"
												aria-checked={o.on}>{o.label}</button
											>{/each}
									</div>
								</div>
								<div style="display: flex; flex-direction: column; gap: 4px">
									<h3 class="cw-sub">Content</h3>
									{#each data.content as c (c.label)}
										<div class="cw-row" style="align-items: flex-start; flex-wrap: wrap">
											<span
												style="display: flex; gap: 8px; align-items: center; font-size: 14px; font-weight: 600; min-width: 140px"
												><span
													class="m-icon m-icon--sm"
													aria-hidden="true"
													style="color: var(--ink-muted)">{c.icon}</span
												>{c.label}</span
											>
											<span
												style="display: flex; flex-wrap: wrap; gap: 6px; flex: 1 1 260px; justify-content: flex-end"
											>
												{#each c.items as i (i)}<span class={c.chipClass}><span>{i}</span></span
													>{/each}
												<button
													class="m-button m-button--variant_text m-button--size_sm"
													type="button"
													><span class="m-icon" aria-hidden="true">add</span><span>Add</span
													></button
												>
											</span>
										</div>
									{/each}
								</div>
							</div>
						{/if}

						{#if data.is.look}
							<div style="display: flex; flex-direction: column; gap: 18px">
								<div style="display: grid; gap: 4px">
									<h2 class="cw-h">The look</h2>
									<p class="cw-note">
										Start from one of these. Everything can change later, in Manage.
									</p>
								</div>
								<div class="themes" role="radiogroup" aria-label="Starting theme">
									{#each data.themes as t (t.label)}
										<button class="themebtn" type="button" role="radio" aria-checked={t.on}>
											<span class="tile" {@attach world(t.ref)}
												><span class="tile-art" aria-hidden="true"></span><span
													class="tile-aa"
													aria-hidden="true">Aa<span class="tile-sw"></span></span
												></span
											>
											<span style="display: grid; gap: 1px; padding: 0 4px 4px"
												><span style="font-size: 14px; font-weight: 600">{t.label}</span><span
													style="font-size: 12.5px; line-height: 16px; color: var(--ink-muted)"
													>{t.hint}</span
												></span
											>
										</button>
									{/each}
								</div>
								<div style="display: flex; flex-wrap: wrap; gap: 8px; align-items: center">
									<button class="m-button m-button--variant_outline" type="button"
										><span class="m-icon" aria-hidden="true">add_photo_alternate</span><span
											>Use your own art</span
										></button
									>
									<a
										class="m-button m-button--variant_text"
										href="https://claude.ai/artifact/Vo1KybPbrkLnu2Pu7W1sC2"
										><span class="m-icon" aria-hidden="true">palette</span><span
											>Fine-tune in the Theme Creator</span
										><span class="m-icon" aria-hidden="true">open_in_new</span></a
									>
								</div>
							</div>
						{/if}

						{#if data.is.people}
							<div style="display: flex; flex-direction: column; gap: 18px">
								<h2 class="cw-h">People and moderation</h2>
								<div class="m-field__root">
									<label class="m-field__label" for="cw-gms">Other GMs</label>
									<div class="m-tags__root">
										{#each data.gms as g, i (i)}<span class="m-tags__tag"
												><span>@{g.handle}</span><button
													class="m-tags__remove"
													type="button"
													aria-label={g.removeLabel}
													><span class="m-icon" aria-hidden="true">close</span></button
												></span
											>{/each}
										<input
											class="m-tags__control"
											id="cw-gms"
											type="text"
											placeholder="Add a handle"
											spellcheck="false"
											aria-describedby="cw-gms-help"
										/>
									</div>
									<div class="m-field__help" id="cw-gms-help">
										They're asked first, and act for the World once they accept.
									</div>
								</div>
								<div class="m-field__root">
									<span class="m-field__label" id="cw-join">Who can join</span>
									<div
										class="m-radio__root m-radio__root--variant_cards"
										role="radiogroup"
										aria-labelledby="cw-join"
									>
										{#each data.joins as j (j.label)}
											<label class="m-radio__item m-radio__item--variant_cards"
												><input
													class="m-radio__control m-radio__control--variant_cards"
													type="radio"
													name="cw-join"
													checked={j.on}
												/><span class="m-radio__text m-radio__text--variant_cards"
													><span class="m-radio__label m-radio__label--variant_cards"
														>{j.label}</span
													><span class="m-radio__description">{j.hint}</span></span
												><span
													class="m-icon m-radio__icon m-radio__icon--variant_cards"
													aria-hidden="true">{j.icon}</span
												></label
											>
										{/each}
									</div>
								</div>
								<div class="m-field__root">
									<span class="m-field__label" id="cw-read">Who can read scenes</span>
									<div
										class="m-segmented__root"
										role="radiogroup"
										aria-labelledby="cw-read"
										style="align-self: flex-start; width: fit-content"
									>
										{#each data.reads as o (o.label)}<button
												class="m-segmented__item"
												type="button"
												role="radio"
												aria-checked={o.on}>{o.label}</button
											>{/each}
									</div>
								</div>

								<div style="display: flex; flex-direction: column; gap: 6px; padding-top: 4px">
									<h3 class="cw-sub">Moderation</h3>
									<p class="cw-note">
										{data.name} moderates itself, as @{data.handle}. Reports about posts here reach
										its GMs, and its labels apply to everyone reading it in Motet.
									</p>
									<div class="m-field__root" style="margin-top: 8px">
										<span class="m-field__label" id="cw-takes">Reports it takes</span>
										<div
											role="group"
											aria-labelledby="cw-takes"
											style="display: grid; grid-template-columns: repeat(auto-fill, minmax(min(220px, 100%), 1fr)); gap: 0 16px"
										>
											{#each data.takes as t (t.label)}
												<label class="m-checkbox__root"
													><input
														class="m-checkbox__control"
														type="checkbox"
														checked={t.on}
														disabled={t.locked}
													/><span class="m-checkbox__text"
														><span class="m-checkbox__label">{t.label}</span></span
													></label
												>
											{/each}
										</div>
									</div>
									<div style="display: flex; flex-direction: column; margin-top: 8px">
										<span class="m-field__label">Services it also follows</span>
										<div class="cw-row">
											<span style="display: grid; gap: 1px"
												><span style="font-weight: 600; font-size: 14px">Motet moderation</span
												><span class="cw-note">Motet's rules, in every World</span></span
											>
											<span
												class="m-meta"
												style="font-size: 13px; color: var(--ink-muted); flex: none"
												><span class="m-icon m-icon--sm" aria-hidden="true">lock</span><span
													>Always on</span
												></span
											>
										</div>
										<div class="cw-row">
											<span style="display: grid; gap: 1px"
												><span style="font-weight: 600; font-size: 14px">PBF Safety Collective</span
												><span class="cw-note"
													>Harassment and safety reports across play-by-forum games</span
												></span
											>
											<button
												class="m-switch__root"
												type="button"
												role="switch"
												aria-checked={data.pbfAttr}
												aria-label="Follow PBF Safety Collective"
												><span class="m-switch__thumb"></span></button
											>
										</div>
									</div>
								</div>
							</div>
						{/if}

						{#if data.is.review}
							<div style="display: flex; flex-direction: column; gap: 16px">
								<h2 class="cw-h">Review</h2>
								<dl
									class="m-datalist__root m-datalist__root--variant_ruled"
									style="margin: 0; --label-w: 170px"
								>
									{#each data.summary as r, i (i)}
										<div class="m-datalist__item m-datalist__item--variant_ruled">
											<dt class="m-datalist__label m-datalist__label--variant_ruled">{r.k}</dt>
											<dd
												class="m-datalist__value m-datalist__value--variant_ruled"
												style="display: flex; justify-content: space-between; gap: 12px; align-items: baseline"
											>
												<span>{r.v}</span><button
													class="m-button m-button--variant_text m-button--size_sm"
													type="button"
													aria-label={r.editLabel}><span>Change</span></button
												>
											</dd>
										</div>
									{/each}
								</dl>
								<p class="cw-note">
									This creates the World's account, @{data.handle}. You act for it now; other GMs do
									once they accept.
								</p>
							</div>
						{/if}

						<div
							style="display: flex; gap: 8px; justify-content: space-between; flex-wrap: wrap; border-top: 1px solid var(--line); margin-top: 4px; padding-top: 16px"
						>
							{#if data.hasBack}<button class="m-button m-button--variant_text" type="button"
									><span class="m-icon" aria-hidden="true">arrow_back</span><span>Back</span
									></button
								>{/if}
							{#if data.noBack}<span></span>{/if}
							{#if data.notLast}<button
									class="m-button m-button--variant_filled"
									type="button"
									disabled={data.blocked}
									><span>{data.nextLabel}</span><span class="m-icon" aria-hidden="true"
										>arrow_forward</span
									></button
								>{/if}
							{#if data.isLast}<button class="m-button m-button--variant_filled" type="submit"
									><span class="m-icon" aria-hidden="true">add</span><span>Create World</span
									></button
								>{/if}
						</div>
					</form>
				</div>

				<aside class="cw-side" aria-label="Preview">
					<span style="font: 600 13px/18px var(--font-ui); color: var(--ink-muted)"
						>How it looks</span
					>
					<div class="pv m-vt m-vt-box" {@attach world(data.previewRef)} style="--vt: m-preview">
						<span class="pv-art" aria-hidden="true"></span>
						<div style="display: flex; flex-direction: column; gap: 12px; padding: 0 18px 18px">
							<span
								class="m-avatar m-avatar--kind_world m-avatar--size_lg"
								aria-hidden="true"
								style="margin-top: -28px; box-shadow: 0 0 0 4px var(--surface)"
								><span aria-hidden="true">{data.initials}</span></span
							>
							<div style="display: grid; gap: 6px">
								<h2 class="pv-title">{data.nameShown}</h2>
								<div
									style="display: flex; flex-wrap: wrap; gap: 4px 12px; font-size: 13px; color: var(--ink-muted)"
								>
									<span class="m-meta"
										><span class="m-icon m-icon--sm" aria-hidden="true">shield_person</span><span
											>{data.gmLine}</span
										></span
									>
									<span class="m-meta"
										><span class="m-icon m-icon--sm" aria-hidden="true">{data.joinIcon}</span><span
											>{data.joinState}</span
										></span
									>
									<span class="m-meta"
										><span class="m-icon m-icon--sm" aria-hidden="true">translate</span><span
											>{data.langName}</span
										></span
									>
								</div>
							</div>
							<p class="pv-desc">{data.descShown}</p>
							<div style="display: flex; gap: 8px; flex-wrap: wrap">
								<span class="m-button m-button--variant_filled m-button--size_sm" aria-hidden="true"
									><span class="m-icon" aria-hidden="true">{data.joinIcon}</span><span
										>{data.joinAction}</span
									></span
								>
								<span class="m-button m-button--variant_text m-button--size_sm" aria-hidden="true"
									><span class="m-icon" aria-hidden="true">bookmark_add</span><span>Follow</span
									></span
								>
							</div>
						</div>
					</div>
					<p class="cw-note">Readers can always switch to Motet's own appearance.</p>
				</aside>
			</div>
		{/if}

		{#if data.done}
			<div
				class="m-vt-pane"
				style="display: flex; flex-direction: column; gap: 24px; max-width: 880px; margin-inline: auto"
			>
				<div
					class="pv m-vt m-vt-box"
					{@attach world(data.heroRef)}
					style="--vt: m-preview; --pv-art: 200px; --pv-title: 40px"
				>
					<span class="pv-art" aria-hidden="true"></span>
					<div
						style="display: flex; gap: 18px; align-items: flex-end; flex-wrap: wrap; padding: 0 28px 24px"
					>
						<span
							class="m-avatar m-avatar--kind_world m-avatar--size_xl"
							aria-hidden="true"
							style="--avatar-size: 88px; margin-top: -44px; box-shadow: 0 0 0 4px var(--surface)"
							><span aria-hidden="true">{data.initials}</span></span
						>
						<div style="display: grid; gap: 6px; min-width: 0; flex: 1 1 280px">
							<h1 class="pv-title">{data.nameShown}</h1>
							<div
								style="display: flex; flex-wrap: wrap; gap: 4px 12px; font-size: 13px; color: var(--ink-muted)"
							>
								<span class="m-meta"
									><span class="m-icon m-icon--sm" aria-hidden="true">shield_person</span><span
										>{data.gmLine}</span
									></span
								>
								<span class="m-meta"
									><span class="m-icon m-icon--sm" aria-hidden="true">alternate_email</span><span
										>{data.handle}</span
									></span
								>
								<span class="m-meta"
									><span class="m-icon m-icon--sm" aria-hidden="true">{data.joinIcon}</span><span
										>{data.joinState}</span
									></span
								>
							</div>
						</div>
						<a
							class="m-button m-button--variant_filled"
							href={resolve('/w/[world]', { world: 'gilded-reach' })}
							><span>Open the World</span><span class="m-icon" aria-hidden="true"
								>arrow_forward</span
							></a
						>
					</div>
				</div>
				<section aria-labelledby="h-next" class="cw-card" style="gap: 4px">
					<h2 id="h-next" class="cw-h" style="margin-bottom: 8px">
						What it needs before anyone plays
					</h2>
					<div class="next">
						<span class="m-icon" aria-hidden="true" style="color: var(--ink-muted)"
							>location_on</span
						><span style="display: grid; gap: 1px"
							><span style="font-weight: 600">A first location</span><span class="cw-note"
								>Scenes live in locations. Start with where the story opens.</span
							></span
						><a
							class="m-button m-button--variant_tonal m-button--size_sm"
							href={resolve('/w/[world]/play', { world: 'gilded-reach' })}>Add a location</a
						>
					</div>
					<div class="next">
						<span class="m-icon" aria-hidden="true" style="color: var(--ink-muted)">gavel</span
						><span style="display: grid; gap: 1px"
							><span style="font-weight: 600">Table rules</span><span class="cw-note"
								>Pace and content are set. Write the rest in your own words.</span
							></span
						><a
							class="m-button m-button--variant_tonal m-button--size_sm"
							href={resolve('/w/[world]/manage', { world: 'gilded-reach' })}>Write rules</a
						>
					</div>
					<div class="next">
						<span class="m-icon" aria-hidden="true" style="color: var(--ink-muted)">campaign</span
						><span style="display: grid; gap: 1px"
							><span style="font-weight: 600">A recruiting call</span><span class="cw-note"
								>It appears on the notice board once you open applications.</span
							></span
						><a
							class="m-button m-button--variant_tonal m-button--size_sm"
							href={resolve('/w/[world]/manage', { world: 'gilded-reach' })}>Write the call</a
						>
					</div>
					<div class="next">
						<span class="m-icon" aria-hidden="true" style="color: var(--ink-muted)"
							>hourglass_top</span
						><span style="display: grid; gap: 1px"
							><span style="font-weight: 600">{data.gmAsked}</span><span class="cw-note"
								>You'll see it in your inbox when they answer.</span
							></span
						><span></span>
					</div>
				</section>
			</div>
		{/if}
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
	.cw-side {
		flex: 1 1 300px;
		min-width: 0;
		display: flex;
		flex-direction: column;
		gap: 10px;
	}
	@media (min-width: 1000px) {
		.cw-side {
			position: sticky;
			top: calc(var(--chrome-offset, 72px) + 16px);
			max-width: 380px;
		}
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
	.cw-h {
		margin: 0;
		font: 600 22px/28px var(--font-ui);
	}
	.cw-sub {
		margin: 0;
		font: 600 15px/20px var(--font-ui);
	}
	.cw-note {
		margin: 0;
		font-size: 13px;
		line-height: 18px;
		color: var(--ink-muted);
	}
	.cw-row {
		display: flex;
		gap: 16px;
		align-items: center;
		justify-content: space-between;
		padding-block: 12px;
		border-bottom: 1px solid var(--line);
	}
	.cw-row:last-child {
		border-bottom: 0;
	}
	.themes {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(min(168px, 100%), 1fr));
		gap: 12px;
	}
	@layer reset {
		.themebtn {
			background: none;
			border: 0;
			padding: 0;
			font: inherit;
			color: inherit;
			cursor: pointer;
			text-align: start;
		}
	}
	.themebtn {
		display: flex;
		flex-direction: column;
		gap: 8px;
		min-width: 0;
		padding: 6px;
		border-radius: var(--radius-lg);
		box-shadow: inset 0 0 0 1px var(--line);
	}
	.themebtn:hover {
		background-color: var(--surface-quiet);
	}
	.themebtn[aria-checked='true'] {
		box-shadow: inset 0 0 0 2px var(--accent);
		background-color: var(--accent-soft);
	}
	.themebtn:focus-visible {
		outline: 2px solid var(--focus);
		outline-offset: 2px;
	}
	.tile {
		display: flex;
		flex-direction: column;
		overflow: hidden;
		border-radius: calc(var(--radius-lg) - 4px);
		background-color: var(--surface);
		color: var(--ink);
		pointer-events: none;
	}
	.tile-art {
		display: block;
		height: 58px;
		background-image: var(--art);
		background-size: cover;
		background-position: var(--art-focus, 50% 50%);
	}
	.tile-aa {
		height: 44px;
		box-sizing: border-box;
		padding: 6px 10px 8px;
		font-family: var(--font-display);
		font-weight: var(--display-weight);
		font-size: calc(22px * var(--display-adjust, 1));
		line-height: 1.1;
		text-transform: var(--display-case);
		letter-spacing: var(--display-tracking);
		display: flex;
		align-items: center;
		gap: 8px;
	}
	.tile-sw {
		display: inline-block;
		width: 14px;
		height: 14px;
		border-radius: 50%;
		background-color: var(--accent);
		box-shadow: 0 0 0 2px var(--surface);
	}
	.pv {
		display: flex;
		flex-direction: column;
		overflow: hidden;
		background-color: var(--surface);
		color: var(--ink);
		border: 1px solid var(--line);
		border-radius: var(--radius-lg);
		corner-shape: var(--corner-shape);
		box-shadow: var(--elevation-2), var(--edge-light);
	}
	.pv-art {
		display: block;
		height: var(--pv-art, 96px);
		background-image: var(--art);
		background-size: cover;
		background-position: var(--art-focus, 50% 50%);
	}
	.pv-title {
		margin: 0;
		font-family: var(--font-display);
		font-weight: var(--display-weight);
		font-size: calc(var(--pv-title, 24px) * var(--display-adjust, 1));
		line-height: 1.12;
		text-transform: var(--display-case);
		letter-spacing: var(--display-tracking);
		text-wrap: balance;
		overflow-wrap: anywhere;
	}
	.pv-desc {
		margin: 0;
		font-family: var(--font-reading);
		font-size: 15px;
		line-height: 1.55;
		color: var(--ink);
	}
	.next {
		display: grid;
		grid-template-columns: 24px minmax(0, 1fr) auto;
		gap: 4px 12px;
		align-items: center;
		padding: 12px 0;
		border-bottom: 1px solid var(--line);
	}
	.next:last-child {
		border-bottom: 0;
	}
</style>
