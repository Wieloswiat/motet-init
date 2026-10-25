<script lang="ts">
	import { resolve } from '$app/paths';
	import Chrome from '#lib/components/motet/Chrome.svelte';
	import ReportDialog from '#lib/components/motet/ReportDialog.svelte';
	import WorldHeader from '#lib/components/motet/WorldHeader.svelte';
	import { world } from '#lib/motet/world.js';
	import type { SceneData } from '#lib/demo/screens/scene.js';

	let { data }: { data: SceneData } = $props();
</script>

<svelte:head>
	<title>Scene</title>
</svelte:head>

<div {@attach world(data.worldRef)} class="m-world__root">
	<Chrome world="vellum" account="iga" inbox={data.inboxCount} />
	<WorldHeader current="Play" size="slim" role="player" />
	<main class="m-world__page">
		<div class="m-world__sceneHeader m-vt" style="--vt: m-scene-head">
			<a
				class="m-button m-button--variant_text m-button--size_sm"
				href={resolve('/w/[world]/play', { world: 'gilded-reach' })}
				style="margin-inline-start: -8px; margin-bottom: 8px"
				><span class="m-icon" aria-hidden="true">arrow_back</span><span>Harrowgate Keep</span></a
			>
			<h2 class="m-world__sceneTitle m-flanked">The Feast of Lanterns</h2>
			<div class="m-world__sceneFields">
				<span class="m-meta"
					><span class="m-icon m-icon--sm" aria-hidden="true">location_on</span><span
						>Harrowgate Keep, the great hall</span
					></span
				>
				<span class="m-meta"
					><span class="m-icon m-icon--sm" aria-hidden="true">auto_stories</span><span
						>A Crown of Ash</span
					></span
				>
				<div class="m-lanes__root" role="group" aria-label="Show posts" data-value={data.lane}>
					<button
						class="m-lanes__item"
						type="button"
						data-lane="ic"
						aria-pressed={data.icPressed}
						aria-label="In character"
						title="In character"
						><span class="m-icon" aria-hidden="true">theater_comedy</span></button
					>
					<button
						class="m-lanes__item"
						type="button"
						data-lane="ooc"
						aria-pressed={data.oocPressed}
						aria-label="Out of character"
						title="Out of character"><span class="m-icon" aria-hidden="true">chat</span></button
					>
				</div>
			</div>
		</div>
		<div class="m-world__layout">
			<div class="m-world__stream" data-show={data.lane}>
				{#each data.posts as p, i (i)}
					{#if p.newMark}<div class="newline m-vt" role="separator" style="--vt: m-newline">
							New since your last visit
						</div>{/if}
					{#if p.divider}<div class="m-divider__root m-vt" role="separator" style="--vt: {p.vtn}-d">
							<span class="m-divider__line"></span><span class="m-divider__motif"></span><span
								class="m-divider__line"
							></span>
						</div>{/if}
					{#if p.ev}
						<section class={p.evRoot} data-lane="ic" aria-label={p.aria} style="--vt: {p.vtn}">
							<div class={p.evMarker} aria-hidden="true">
								<span class="m-icon" aria-hidden="true">{p.icon}</span>
							</div>
							<div class={p.evCard}>
								<div class="m-event__header">
									<p class={p.evTitle}>
										{p.title}{#if p.hasExpr}<code>{p.expr}</code>{/if}{#if p.yourRoll}<span
												class="m-flag m-flag--kind_turn"
												><span class="m-icon m-icon--sm" aria-hidden="true">front_hand</span>Your
												roll</span
											>{/if}
									</p>
									<time class="m-event__time">{p.time}</time>
									{#if p.hasDetail}<div class="m-event__detail">{p.detail}</div>{/if}
								</div>
								{#if p.hasDice}
									<div class="m-event__body">
										<figure class="m-roll__root">
											<div class="m-roll__dice" role="list" aria-label="Dice">
												<span
													class={p.dieClass}
													role="listitem"
													title={p.dieTitle}
													style="--vt: m-die"
													><span class="m-die" aria-hidden="true">{p.glyph}</span><span
														class="m-visually-hidden">{p.result}</span
													></span
												>
											</div>
											<div class="m-roll__total">
												<span class="m-visually-hidden">Total </span>{p.total}
											</div>
											{#if p.hasOutcome}<div class="m-roll__outcome" data-tone={p.tone}>
													<span class="m-icon" aria-hidden="true">{p.outcomeIcon}</span>{p.outcome}
												</div>{/if}
										</figure>
									</div>
								{/if}
								{#if p.hasText}<div
										class="m-event__body"
										style="font-size: 14px; line-height: 20px"
									>
										{p.text}
									</div>{/if}
								{#if p.hasFooter}
									<div class="m-event__footer">
										<span class="m-event__source"
											><span class="m-icon" aria-hidden="true">{p.sourceIcon}</span>{p.source}</span
										>
										{#if p.canRoll}<div class="m-event__actions">
												<button
													class="m-button m-button--variant_filled m-button--size_sm"
													type="button"
													><span class="m-icon" aria-hidden="true">casino</span><span
														>Roll Composure</span
													></button
												>
											</div>{/if}
										{#if p.waiting}<span
												class="m-event__actions"
												style="font-size: 13px; color: var(--ink-muted)"
												>Waiting for Lady Maren</span
											>{/if}
									</div>
								{/if}
							</div>
						</section>
					{/if}
					{#if p.ic}
						<article class={p.rootClass} aria-label={p.name} style="--vt: {p.vtn}">
							<div class="m-post__portrait m-post__portrait--lane_ic">
								<span class="m-avatar m-avatar--kind_character m-avatar--size_lg" aria-hidden="true"
									><img src={p.portrait} alt="" /></span
								>
							</div>
							<div class="m-post__main m-post__main--lane_ic">
								<header class="m-post__header m-post__header--lane_ic">
									<div class="m-post__mini m-post__mini--lane_ic">
										<span
											class="m-avatar m-avatar--kind_character m-avatar--size_sm"
											aria-hidden="true"><img src={p.portrait} alt="" /></span
										>
									</div>
									<div class="m-post__speaker m-post__speaker--lane_ic">
										<span class="m-post__name m-post__name--lane_ic">{p.name}</span><span
											class="m-post__epithet m-post__epithet--lane_ic">{p.epithet}</span
										>
									</div>
									<div class="m-post__author m-post__author--lane_ic">
										<span class="m-icon" aria-hidden="true">edit_note</span><span
											class="m-post__handle m-post__handle--lane_ic">{p.by}</span
										>{#if p.gm}<span class="m-role"
												><span class="m-icon" aria-hidden="true">shield_person</span>GM</span
											>{/if}
									</div>
									<div class="m-post__meta m-post__meta--lane_ic">
										{#if p.unread}<span class="m-flag m-flag--kind_turn">New</span>{/if}
										{#if p.edited}<span class="m-meta" title="Edited Mar 10"
												><span class="m-icon m-icon--sm" aria-hidden="true">history</span><span
													class="m-visually-hidden">Edited</span
												></span
											>{/if}
										<time>{p.time}</time>
										<span style="position: relative; display: inline-flex">
											<button
												class="m-icon-button m-icon-button--variant_standard m-icon-button--size_sm"
												type="button"
												aria-label="Post actions"
												aria-haspopup="menu"
												aria-expanded={p.menuOpen}
												><span class="m-icon" aria-hidden="true">more_horiz</span></button
											>
											{#if p.menuShown}
												<div style="position: fixed; inset: 0; z-index: 7"></div>
												<div
													class="m-menu__root"
													role="menu"
													aria-label="Post actions"
													style="position: absolute; top: 36px; right: 0; z-index: 8; min-width: 264px"
												>
													{#each p.menu as m (m.label)}
														{#if m.sep}<hr class="m-menu__separator" />{/if}
														{#if m.isLink}<a class="m-menu__item" role="menuitem" href={m.href}
																><span class="m-icon" aria-hidden="true">{m.icon}</span>{m.label}</a
															>{/if}
														{#if m.isButton}<button
																class="m-menu__item"
																type="button"
																role="menuitem"
																data-tone={m.tone}
																><span class="m-icon" aria-hidden="true">{m.icon}</span
																>{m.label}</button
															>{/if}
													{/each}
												</div>
											{/if}
										</span>
									</div>
								</header>
								<div class={p.bodyClass} style="--vt: m-draft">
									{#each p.paras as t (t)}<p>{t}</p>{/each}
								</div>
								<footer class="m-post__actions m-post__actions--lane_ic">
									<button class="m-button m-button--variant_text m-button--size_sm" type="button"
										><span class="m-icon" aria-hidden="true">reply</span><span>Reply</span></button
									>
									<button class="m-button m-button--variant_text m-button--size_sm" type="button"
										><span class="m-icon" aria-hidden="true">format_quote</span><span>Quote</span
										></button
									>
								</footer>
							</div>
						</article>
					{/if}
					{#if p.ooc}
						<article
							class="m-post__root m-post__root--lane_ooc m-marked m-vt m-vt-box"
							aria-label={p.oocLabel}
							style="--vt: {p.vtn}"
						>
							<div class="m-post__portrait m-post__portrait--lane_ooc">
								<span
									class="m-avatar m-avatar--kind_account m-avatar--size_md"
									aria-hidden="true"
									style={p.hue}><span aria-hidden="true">{p.initials}</span></span
								>
							</div>
							<div class="m-post__main m-post__main--lane_ooc">
								<header class="m-post__header m-post__header--lane_ooc">
									<div class="m-post__mini m-post__mini--lane_ooc">
										<span
											class="m-avatar m-avatar--kind_account m-avatar--size_sm"
											aria-hidden="true"
											style={p.hue}><span aria-hidden="true">{p.initials}</span></span
										>
									</div>
									<div class="m-post__speaker m-post__speaker--lane_ooc">
										<span class="m-post__name m-post__name--lane_ooc">{p.name}</span>
									</div>
									<div class="m-post__author m-post__author--lane_ooc">
										<span class="m-post__handle m-post__handle--lane_ooc">{p.handle}</span
										>{#if p.gm}<span class="m-role"
												><span class="m-icon" aria-hidden="true">shield_person</span>GM</span
											>{/if}
									</div>
									<div class="m-post__meta m-post__meta--lane_ooc">
										{#if p.unread}<span class="m-flag m-flag--kind_turn">New</span>{/if}<time
											>{p.time}</time
										><span style="position: relative; display: inline-flex">
											<button
												class="m-icon-button m-icon-button--variant_standard m-icon-button--size_sm"
												type="button"
												aria-label="Post actions"
												aria-haspopup="menu"
												aria-expanded={p.menuOpen}
												><span class="m-icon" aria-hidden="true">more_horiz</span></button
											>
											{#if p.menuShown}
												<div style="position: fixed; inset: 0; z-index: 7"></div>
												<div
													class="m-menu__root"
													role="menu"
													aria-label="Post actions"
													style="position: absolute; top: 36px; right: 0; z-index: 8; min-width: 264px"
												>
													{#each p.menu as m (m.label)}
														{#if m.sep}<hr class="m-menu__separator" />{/if}
														{#if m.isLink}<a class="m-menu__item" role="menuitem" href={m.href}
																><span class="m-icon" aria-hidden="true">{m.icon}</span>{m.label}</a
															>{/if}
														{#if m.isButton}<button
																class="m-menu__item"
																type="button"
																role="menuitem"
																data-tone={m.tone}
																><span class="m-icon" aria-hidden="true">{m.icon}</span
																>{m.label}</button
															>{/if}
													{/each}
												</div>
											{/if}
										</span>
									</div>
								</header>
								<div class={p.oocBodyClass} style="--vt: m-draft">
									{#each p.paras as t (t)}<p>{t}</p>{/each}
								</div>
							</div>
						</article>
					{/if}
				{/each}

				<div style="margin-top: 24px; {data.slotStyle}">
					{#if data.expanded}<div class="composer-scrim" aria-hidden="true"></div>{/if}
					<section
						class="m-composer__root m-marked"
						aria-label="Reply"
						data-expanded={data.expandedAttr}
					>
						<div class="m-composer__head">
							<div style="position: relative">
								<button
									class="m-composer__speaker"
									type="button"
									aria-haspopup="listbox"
									aria-expanded={data.speakerOpen}
									aria-label={data.speakerLabel}
								>
									{#if data.asMaren}<span
											class="m-avatar m-avatar--kind_character m-avatar--size_sm"
											aria-hidden="true"><img src={data.portraits.maren} alt="" /></span
										>{/if}
									{#if data.asOoc}<span
											class="m-avatar m-avatar--kind_account m-avatar--size_sm"
											aria-hidden="true"
											style="--avatar-hue: 141"><span aria-hidden="true">IB</span></span
										>{/if}
									<span class="m-composer__speakerName">{data.speakerName}</span><span
										class="m-icon"
										aria-hidden="true">expand_more</span
									>
								</button>
								{#if data.speakerOpen}
									<div
										class="m-menu__root"
										role="listbox"
										aria-label="Speak as"
										style="position: absolute; top: 44px; left: 0; z-index: 4; min-width: 260px"
									>
										<button
											class="m-menu__item speakeropt"
											type="button"
											role="option"
											aria-selected={data.marenSel}
											aria-checked={data.marenSel}
											><span
												class="m-avatar m-avatar--kind_character m-avatar--size_xs"
												aria-hidden="true"><img src={data.portraits.maren} alt="" /></span
											><span style="display: grid; flex: 1"
												><span>Lady Maren of Ost</span><span
													style="font-size: 12.5px; color: var(--menu-ink-muted, var(--ink-muted))"
													>In character</span
												></span
											></button
										>
										<button
											class="m-menu__item speakeropt"
											type="button"
											role="option"
											aria-selected={data.oocSel}
											aria-checked={data.oocSel}
											><span
												class="m-avatar m-avatar--kind_account m-avatar--size_xs"
												aria-hidden="true"
												style="--avatar-hue: 141"><span aria-hidden="true">IB</span></span
											><span style="display: grid; flex: 1"
												><span>Iga</span><span
													style="font-size: 12.5px; color: var(--menu-ink-muted, var(--ink-muted))"
													>Out of character</span
												></span
											></button
										>
									</div>
								{/if}
							</div>
							<span class="m-composer__destination"
								><span class="m-icon" aria-hidden="true">forum</span><strong
									>The Feast of Lanterns</strong
								><span class="m-chip m-chip--kind_fact"><span>{data.laneChip}</span></span></span
							>
							<span class="m-composer__audience"
								><button class="m-button m-button--variant_text m-button--size_sm" type="button"
									><span class="m-icon" aria-hidden="true">group</span><span>World members</span
									><span class="m-icon" aria-hidden="true">expand_more</span></button
								><button
									class="m-icon-button m-icon-button--variant_standard m-icon-button--size_sm"
									type="button"
									aria-label={data.expandLabel}
									title={data.expandLabel}
									><span class="m-icon" aria-hidden="true">{data.expandIcon}</span></button
								></span
							>
						</div>
						{#if data.hasRoll}
							<div style="display: flex; align-items: center; gap: 8px; padding: 8px 14px 0">
								<span class="m-chip m-chip--kind_note"
									><span class="m-icon" aria-hidden="true">casino</span><span>{data.rollText}</span
									></span
								>
							</div>
						{/if}
						<textarea
							class={data.editorClass}
							style="--vt: m-draft"
							aria-label={data.editorLabel}
							placeholder={data.editorPlaceholder}
							value={data.draft}></textarea>
						<div class="m-composer__foot">
							<div class="m-composer__tools" role="toolbar" aria-label="Formatting">
								<button
									class="m-icon-button m-icon-button--variant_standard m-icon-button--size_sm"
									type="button"
									aria-label="Bold"
									><span class="m-icon" aria-hidden="true">format_bold</span></button
								>
								<button
									class="m-icon-button m-icon-button--variant_standard m-icon-button--size_sm"
									type="button"
									aria-label="Italic"
									><span class="m-icon" aria-hidden="true">format_italic</span></button
								>
								<button
									class="m-icon-button m-icon-button--variant_standard m-icon-button--size_sm"
									type="button"
									aria-label="Quote a post"
									><span class="m-icon" aria-hidden="true">format_quote</span></button
								>
								<button
									class="m-icon-button m-icon-button--variant_standard m-icon-button--size_sm"
									type="button"
									aria-label="Roll Composure, d20 plus 3"
									title="Roll Composure, d20 + 3"
									><span class="m-icon" aria-hidden="true">casino</span></button
								>
								<button
									class="m-icon-button m-icon-button--variant_standard m-icon-button--size_sm"
									type="button"
									aria-label="Add image"
									><span class="m-icon" aria-hidden="true">image</span></button
								>
							</div>
							<span class="m-composer__submit">
								<span style="font-size: 12.5px; color: var(--ink-muted)">{data.wordCount}</span>
								<span
									class="m-composer__status"
									role="status"
									aria-label="Draft saved on this device"
									title="Draft saved on this device"
									><span class="m-icon" aria-hidden="true">cloud_done</span></span
								>
								<button
									class="m-button m-button--variant_filled"
									type="button"
									disabled={data.cannotPost}
									><span class="m-icon" aria-hidden="true">send</span><span>Post</span></button
								>
							</span>
						</div>
					</section>
				</div>
			</div>

			<aside class="m-world__rail" aria-label="Scene">
				<div class="m-world__railLanes">
					<span>Posts</span>
					<div class="m-lanes__root" role="group" aria-label="Show posts" data-value={data.lane}>
						<button
							class="m-lanes__item"
							type="button"
							data-lane="ic"
							aria-pressed={data.icPressed}
							aria-label="In character"
							title="In character"
							><span class="m-icon" aria-hidden="true">theater_comedy</span></button
						>
						<button
							class="m-lanes__item"
							type="button"
							data-lane="ooc"
							aria-pressed={data.oocPressed}
							aria-label="Out of character"
							title="Out of character"><span class="m-icon" aria-hidden="true">chat</span></button
						>
					</div>
				</div>
				<div class="m-character__root m-ornamented m-marked" style="display: block">
					<h3 style="margin: 0 0 10px; font: 600 13px/18px var(--font-ui); color: var(--ink-muted)">
						Posting order
					</h3>
					<ol style="margin: 0; padding: 0; list-style: none; display: grid; gap: 10px">
						{#each data.order as o (o.name)}
							<li class="m-vt" style="display: flex; gap: 10px; align-items: center; --vt: {o.vtn}">
								<span class="m-avatar m-avatar--kind_character m-avatar--size_sm" aria-hidden="true"
									><img src={o.portrait} alt="" /></span
								>
								<a
									href={resolve('/w/[world]/characters/[character]', {
										world: 'gilded-reach',
										character: 'lady-maren'
									})}
									style="display: grid; gap: 2px; min-width: 0; flex: 1; text-decoration: none; color: var(--ink)"
								>
									<span
										class="m-post__name"
										style="font-size: calc(15px * var(--name-adjust, 1)); width: fit-content"
										>{o.name}</span
									>
									<span
										style="display: flex; flex-wrap: wrap; gap: 4px 8px; align-items: center; font: 400 12.5px/16px var(--font-ui); color: var(--ink-muted)"
										>{o.who}{#if o.now}<span
												class="m-flag m-flag--kind_turn m-vt"
												style="--vt: m-turn"
												><span class="m-icon m-icon--sm" aria-hidden="true">front_hand</span
												>{o.nowLabel}</span
											>{/if}</span
									>
								</a>
							</li>
						{/each}
					</ol>
					<div
						style="margin-top: 14px; display: grid; gap: 8px; font: 400 13px/18px var(--font-ui); color: var(--ink-muted)"
					>
						<span class="m-meta"
							><span class="m-icon m-icon--sm" aria-hidden="true">format_list_numbered</span><span
								>Round 3, in order</span
							></span
						>
						<span class="m-meta"
							><span class="m-icon m-icon--sm" aria-hidden="true">schedule</span><span
								>Posts due by Sunday</span
							></span
						>
						<span class="m-meta"
							><span class="m-icon m-icon--sm" aria-hidden="true">visibility</span><span
								>World members can read</span
							></span
						>
					</div>
					<div
						style="margin-top: 12px; display: flex; flex-direction: column; align-items: flex-start; gap: 2px"
					>
						<button
							class="m-button m-button--variant_text m-button--size_sm"
							type="button"
							style="margin-inline-start: -8px"
							><span class="m-icon" aria-hidden="true">skip_next</span><span>Pass this round</span
							></button
						>
						<button
							class="m-button m-button--variant_text m-button--size_sm"
							type="button"
							style="margin-inline-start: -8px"
							><span class="m-icon" aria-hidden="true">notifications_active</span><span
								>Notify me: all posts</span
							></button
						>
					</div>
				</div>
			</aside>
		</div>
	</main>
	{#if data.reporting}
		<ReportDialog
			kind="post"
			world="The Gilded Reach"
			who={data.report.who}
			handle={data.report.handle}
			account={data.report.account}
			excerpt={data.report.excerpt}
			portrait={data.report.portrait}
			targetGm={data.report.gm}
		/>
	{/if}
	{#if data.confirming}
		<div class="m-dialog__scrim">
			<div
				class="m-dialog__root"
				role="alertdialog"
				aria-modal="true"
				aria-labelledby="del-title"
				aria-describedby="del-body"
				style="margin: 0"
			>
				<div class="m-dialog__header">
					<span class="m-icon" aria-hidden="true" style="color: var(--danger-ink)">delete</span>
					<h2 class="m-dialog__title" id="del-title">Delete this post?</h2>
				</div>
				<div class="m-dialog__body" id="del-body">
					<p>
						It disappears from <strong>The Feast of Lanterns</strong> for everyone. Posts that quote it
						keep the quote.
					</p>
				</div>
				<div class="m-dialog__actions">
					<button class="m-button m-button--variant_text m-button--tone_neutral" type="button"
						><span>Keep post</span></button
					><button class="m-button m-button--variant_filled m-button--tone_danger" type="button"
						><span>Delete post</span></button
					>
				</div>
			</div>
		</div>
	{/if}
	{#if data.hasToast}
		<ol class="m-toast__region" aria-label="Notifications">
			<li class="m-toast__root" role="status" data-tone={data.toast.tone}>
				<span class="m-icon m-toast__icon" aria-hidden="true">{data.toast.icon}</span><span
					class="m-toast__message">{data.toast.text}</span
				>{#if data.toast.hasUndo}<button class="m-toast__action" type="button">Undo</button
					>{/if}<button
					class="m-icon-button m-icon-button--variant_standard m-icon-button--size_sm m-toast__close"
					type="button"
					aria-label="Dismiss"><span class="m-icon" aria-hidden="true">close</span></button
				>
			</li>
		</ol>
	{/if}
</div>

<style>
	.newline {
		display: flex;
		align-items: center;
		gap: 12px;
		margin-block: 8px;
		font: 600 12.5px/16px var(--font-ui);
		color: var(--accent-ink);
	}
	.newline::before,
	.newline::after {
		content: '';
		flex: 1;
		height: 1px;
		background-color: var(--accent);
	}
	@layer reset {
		.speakeropt {
			background: none;
			border: 0;
			font: inherit;
			color: inherit;
			cursor: pointer;
		}
	}
	.composer-scrim {
		position: fixed;
		inset: 0;
		z-index: 59;
		background-color: var(--scrim);
		animation: m-fade-in var(--dur-effects-default) var(--ease-effects-default);
	}
</style>
