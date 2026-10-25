<script lang="ts">
	import { resolve } from '$app/paths';
	import Chrome from '#lib/components/motet/Chrome.svelte';
	import LanguagePicker from '#lib/components/motet/LanguagePicker.svelte';
	import { world } from '#lib/motet/world.js';
	import type { SearchData } from '#lib/demo/screens/search.js';

	let { data }: { data: SearchData } = $props();
</script>

<svelte:head>
	<title>Search results</title>
</svelte:head>

<div {@attach world(data.pageRef)} class="m-page__root">
	<Chrome account="iga" inbox={data.inboxCount} />
	<main class="m-page__main">
		<div style="display: flex; flex-direction: column; gap: 20px">
			<form role="search" style="display: flex; gap: 12px; align-items: flex-end; flex-wrap: wrap">
				<div class="m-field__root" style="flex: 1 1 420px; min-width: 0">
					<label class="m-field__label m-visually-hidden" for="q">Search</label>
					<div class="m-input__root" style="--control-h: 48px; font-size: 16px">
						<span class="m-input__addon" data-side="start" aria-hidden="true"
							><span class="m-icon" aria-hidden="true">search</span></span
						>
						<input
							class="m-input__control"
							id="q"
							type="search"
							value={data.query}
							style="font-size: 16px"
						/>
					</div>
				</div>
				<button
					class="m-button m-button--variant_outline"
					type="button"
					aria-expanded={data.panelOpen}
					aria-controls="adv"
					style="min-height: 48px"
					><span class="m-icon" aria-hidden="true">tune</span><span>Filters</span
					>{#if data.hasActive}<span class="m-count">{data.activeCount}</span>{/if}</button
				>
			</form>

			<nav class="m-tabs__root" aria-label="Result types" style="margin-inline: -12px">
				{#each data.tabs as t (t.label)}
					<button class="m-tabs__tab tabbtn" type="button" aria-current={t.current}
						><span class="m-icon" aria-hidden="true">{t.icon}</span>{t.label}<span
							style="font-weight: 500; color: var(--ink-muted)">{t.count}</span
						></button
					>
				{/each}
			</nav>

			<div style="display: flex; flex-wrap: wrap; gap: 24px 32px; align-items: flex-start">
				{#if data.panelOpen}
					<aside
						class="m-page__panel"
						id="adv"
						aria-label="Filters"
						style="flex: 1 1 300px; gap: 20px; padding: 20px"
					>
						<div style="display: flex; justify-content: space-between; align-items: center">
							<h2 style="margin: 0; font: 600 15px/20px var(--font-ui)">Filters</h2>
							<button
								class="m-button m-button--variant_text m-button--size_sm"
								type="button"
								disabled={data.noActive}><span>Clear all</span></button
							>
						</div>

						<fieldset class="m-fieldset__root" style="margin: 0">
							<legend class="m-fieldset__legend">Search in</legend>
							<div class="m-radio__root">
								{#each data.scopes as o (o.value)}
									<label class="m-radio__item"
										><input
											class="m-radio__control"
											type="radio"
											name="scope"
											value={o.value}
											checked={o.on}
										/><span class="m-radio__text"
											><span class="m-radio__label">{o.label}</span></span
										></label
									>
								{/each}
							</div>
							{#if data.oneWorld}
								<div class="m-select__root" style="margin-top: 10px">
									<span class="m-field__label m-visually-hidden" id="w-l">World</span>
									<button
										class="m-select__trigger"
										type="button"
										aria-haspopup="listbox"
										aria-labelledby="w-l w-v"
										style="width: 100%"
										><span
											class="m-select__value"
											id="w-v"
											style="display: flex; gap: 8px; align-items: center"
											><span
												class="m-avatar m-avatar--kind_world m-avatar--size_xs"
												aria-hidden="true"><img src={data.marks.vellum} alt="" /></span
											>The Gilded Reach</span
										><span class="m-icon m-select__indicator" aria-hidden="true">expand_more</span
										></button
									>
								</div>
							{/if}
						</fieldset>

						<div class="m-field__root">
							<label class="m-field__label" for="by">Written by</label>
							<div class="m-input__root">
								<span class="m-input__addon" data-side="start" aria-hidden="true"
									><span class="m-icon" aria-hidden="true">alternate_email</span></span
								><input
									class="m-input__control"
									id="by"
									type="text"
									placeholder="handle"
									value={data.by}
								/>
							</div>
						</div>
						<div class="m-field__root">
							<label class="m-field__label" for="as">Speaking as</label>
							<div class="m-input__root">
								<span class="m-input__addon" data-side="start" aria-hidden="true"
									><span class="m-icon" aria-hidden="true">theater_comedy</span></span
								><input class="m-input__control" id="as" type="text" placeholder="Character name" />
							</div>
						</div>

						<div style="display: flex; flex-direction: column; gap: 8px">
							<span class="m-field__label" id="lanes-l">Posts</span>
							<div style="display: flex; align-items: center; gap: 12px">
								<div
									class="m-lanes__root"
									role="group"
									aria-labelledby="lanes-l"
									data-value={data.lane}
								>
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
										title="Out of character"
										><span class="m-icon" aria-hidden="true">chat</span></button
									>
								</div>
								<span style="font-size: 14px; color: var(--ink-muted)" aria-live="polite"
									>{data.laneText}</span
								>
							</div>
						</div>

						<div
							style="display: grid; grid-template-columns: repeat(auto-fit, minmax(min(120px, 100%), 1fr)); gap: 12px"
						>
							<div class="m-field__root">
								<label class="m-field__label" for="from">From</label>
								<div class="m-input__root">
									<input
										class="m-input__control"
										id="from"
										type="text"
										placeholder="Any date"
									/><span class="m-input__addon" data-side="end" aria-hidden="true"
										><span class="m-icon" aria-hidden="true">calendar_month</span></span
									>
								</div>
							</div>
							<div class="m-field__root">
								<label class="m-field__label" for="to">To</label>
								<div class="m-input__root">
									<input class="m-input__control" id="to" type="text" placeholder="Any date" /><span
										class="m-input__addon"
										data-side="end"
										aria-hidden="true"
										><span class="m-icon" aria-hidden="true">calendar_month</span></span
									>
								</div>
							</div>
						</div>

						<LanguagePicker
							label="Language"
							placeholder="Any language"
							value={data.langCodes}
							pid="search-lang"
						/>

						<div class="m-field__root">
							<span class="m-field__label">Genre</span>
							<div class="m-tags__root">
								{#each data.genreTags as g (g.label)}
									<span class="m-tags__tag"
										><span>{g.label}</span><button
											class="m-tags__remove"
											type="button"
											aria-label={g.remove}
											><span class="m-icon" aria-hidden="true">close</span></button
										></span
									>
								{/each}
								<input
									class="m-tags__control"
									type="text"
									aria-label="Add a genre"
									placeholder="Add a genre"
								/>
							</div>
						</div>

						<fieldset class="m-fieldset__root" style="margin: 0">
							<legend class="m-fieldset__legend">Options</legend>
							<div
								class="m-fieldset__content"
								style="display: flex; flex-direction: column; gap: 10px"
							>
								{#each data.options as o (o.label)}
									<label class="m-checkbox__root"
										><input class="m-checkbox__control" type="checkbox" checked={o.on} /><span
											class="m-checkbox__text"
											><span class="m-checkbox__label">{o.label}</span></span
										></label
									>
								{/each}
							</div>
						</fieldset>

						<details
							class="m-accordion__item"
							style="border-top: 1px solid var(--line); padding-top: 8px"
						>
							<summary class="m-accordion__trigger" style="padding-inline: 0"
								><span class="m-accordion__title" style="font-size: 14px">Search operators</span
								><span class="m-icon m-accordion__indicator" aria-hidden="true">expand_more</span
								></summary
							>
							<div
								style="display: grid; grid-template-columns: auto minmax(0, 1fr); gap: 8px 12px; font-size: 13px; line-height: 18px; color: var(--ink-muted); padding-block: 4px 8px"
							>
								<code class="m-kbd" style="justify-self: start">"exact words"</code><span
									>Exact phrase</span
								>
								<code class="m-kbd" style="justify-self: start">by:@handle</code><span
									>Written by an account</span
								>
								<code class="m-kbd" style="justify-self: start">as:"Brother Cael"</code><span
									>Spoken by a character</span
								>
								<code class="m-kbd" style="justify-self: start">world:gilded-reach</code><span
									>In one World</span
								>
								<code class="m-kbd" style="justify-self: start">lane:ooc</code><span
									>Out of character only</span
								>
							</div>
						</details>

						<button
							class="m-button m-button--variant_tonal"
							type="button"
							style="align-self: flex-start"
							><span class="m-icon" aria-hidden="true">bookmark_add</span><span
								>Save this search</span
							></button
						>
					</aside>
				{/if}
				<section
					aria-label="Results"
					style="flex: 999 1 560px; min-width: 0; display: flex; flex-direction: column; gap: 28px"
				>
					<div
						style="display: flex; justify-content: space-between; align-items: center; gap: 12px; flex-wrap: wrap"
					>
						<p style="margin: 0; font-size: 14px; color: var(--ink-muted)">{data.summary}</p>
						<div class="m-select__root" style="display: flex; align-items: center; gap: 8px">
							<span class="m-field__label" id="sort-l" style="margin: 0">Sort</span>
							<button
								class="m-select__trigger"
								type="button"
								aria-haspopup="listbox"
								aria-labelledby="sort-l sort-v"
								style="min-width: 150px"
								><span class="m-select__value" id="sort-v">Relevance</span><span
									class="m-icon m-select__indicator"
									aria-hidden="true">expand_more</span
								></button
							>
						</div>
					</div>

					{#if data.show.worlds}
						<div
							class="m-vt m-vt-box"
							style="--vt: m-sr-worlds; display: flex; flex-direction: column; gap: 10px"
						>
							<h2
								style="margin: 0; font: 600 15px/20px var(--font-ui); display: flex; gap: 8px; align-items: center"
							>
								<span class="m-icon" aria-hidden="true" style="color: var(--ink-muted)">public</span
								>Worlds
							</h2>
							<a
								{@attach world(data.worldRef)}
								href={resolve('/w/[world]/visitor', { world: 'gilded-reach' })}
								style="display: grid; grid-template-columns: 120px minmax(0, 1fr); min-width: 0; text-decoration: none; color: var(--ink); background-color: var(--surface); background-image: var(--grain); border: 1px solid var(--line); border-radius: var(--radius-lg); corner-shape: var(--corner-shape); overflow: hidden"
							>
								<span
									aria-hidden="true"
									style="background-image: var(--art); background-size: cover; background-position: var(--art-focus, 50% 50%); min-height: 112px"
								></span>
								<span style="display: flex; flex-direction: column; gap: 6px; padding: 14px 16px">
									<span style="display: flex; gap: 10px; align-items: center"
										><span
											class="m-avatar m-avatar--kind_world m-avatar--size_sm"
											aria-hidden="true"><img src={data.marks.vellum} alt="" /></span
										><span
											style="font-family: var(--font-display); font-weight: var(--display-weight); font-size: calc(20px * var(--display-adjust, 1))"
											>The Gilded Reach</span
										></span
									>
									<span style="font-family: var(--font-reading); font-size: 15px; line-height: 1.5"
										>Court intrigue at Harrowgate Keep, where the dead are honoured each spring with
										the <mark>Lantern</mark> Rite.</span
									>
									<span
										style="display: flex; flex-wrap: wrap; gap: 4px 14px; font-size: 13px; color: var(--ink-muted)"
										><span class="m-meta"
											><span class="m-icon m-icon--sm" aria-hidden="true">how_to_reg</span><span
												>Recruiting</span
											></span
										><span class="m-meta"
											><span class="m-icon m-icon--sm" aria-hidden="true">translate</span><span
												>English</span
											></span
										><span class="m-meta"
											><span class="m-icon m-icon--sm" aria-hidden="true">person</span><span
												>You play here</span
											></span
										></span
									>
								</span>
							</a>
						</div>
					{/if}

					{#if data.show.posts}
						<div
							class="m-vt m-vt-box"
							style="--vt: m-sr-posts; display: flex; flex-direction: column; gap: 4px"
						>
							<div style="display: flex; justify-content: space-between; align-items: baseline">
								<h2
									style="margin: 0 0 6px; font: 600 15px/20px var(--font-ui); display: flex; gap: 8px; align-items: center"
								>
									<span class="m-icon" aria-hidden="true" style="color: var(--ink-muted)"
										>forum</span
									>Posts
								</h2>
								{#if data.isAll}<button
										class="m-button m-button--variant_text m-button--size_sm"
										type="button"><span>All 14 posts</span></button
									>{/if}
							</div>
							{#each data.posts as p (p.name)}
								<a
									class="hit"
									href={p.href}
									style="display: grid; grid-template-columns: 40px minmax(0, 1fr); gap: 4px 12px; padding: 12px; margin-inline: -12px; text-decoration: none; color: var(--ink)"
								>
									{#if p.isCharacter}<span
											class="m-avatar m-avatar--kind_character m-avatar--size_md"
											aria-hidden="true"
											style="grid-row: span 3"><img src={p.portrait} alt="" /></span
										>{/if}{#if p.isAccount}<span
											class="m-avatar m-avatar--kind_account m-avatar--size_md"
											aria-hidden="true"
											style="--avatar-hue: 262; grid-row: span 3"
											><span aria-hidden="true">AB</span></span
										>{/if}
									<span style="display: flex; flex-wrap: wrap; gap: 2px 10px; align-items: baseline"
										><span style="font-weight: 600">{p.name}</span><span
											style="font-size: 13px; color: var(--ink-muted)">{p.handle}</span
										></span
									>
									<span
										style="font-family: var(--font-reading); font-size: 15.5px; line-height: 1.55"
										>{p.pre}<mark>{p.match}</mark>{p.post}</span
									>
									<span
										style="display: flex; flex-wrap: wrap; gap: 4px 14px; font-size: 13px; color: var(--ink-muted)"
										><span class="m-meta"
											><span
												class="m-avatar m-avatar--kind_world m-avatar--size_xs"
												aria-hidden="true"><img src={p.mark} alt="" /></span
											><span>{p.world}</span></span
										><span class="m-meta"
											><span class="m-icon m-icon--sm" aria-hidden="true">forum</span><span
												>{p.scene}</span
											></span
										><span class="m-meta"
											><span class="m-icon m-icon--sm" aria-hidden="true">{p.laneIcon}</span><span
												>{p.lane}</span
											></span
										><span class="m-meta"
											><span class="m-icon m-icon--sm" aria-hidden="true">schedule</span><span
												>{p.when}</span
											></span
										></span
									>
								</a>
							{/each}
						</div>
					{/if}

					{#if data.show.scenes}
						<div
							class="m-vt m-vt-box"
							style="--vt: m-sr-scenes; display: flex; flex-direction: column"
						>
							<h2
								style="margin: 0 0 6px; font: 600 15px/20px var(--font-ui); display: flex; gap: 8px; align-items: center"
							>
								<span class="m-icon" aria-hidden="true" style="color: var(--ink-muted)"
									>auto_stories</span
								>Scenes
							</h2>
							{#each data.scenes as s, i (i)}
								<a
									class="m-scene-row__root m-scene-row__root--unread_false"
									href={s.href}
									style="grid-template-columns: 3px minmax(0, 1fr) auto"
								>
									<span
										class="m-scene-row__marker m-scene-row__marker--unread_false"
										aria-hidden="true"
									></span>
									<span class="m-scene-row__main"
										><span class="m-scene-row__title">{s.pre}<mark>{s.match}</mark>{s.post}</span
										><span class="m-scene-row__where"
											><span class="m-meta"
												><span
													class="m-avatar m-avatar--kind_world m-avatar--size_xs"
													aria-hidden="true"><img src={s.mark} alt="" /></span
												><span>{s.world}</span></span
											><span class="m-meta"
												><span class="m-icon m-icon--sm" aria-hidden="true">location_on</span><span
													>{s.where}</span
												></span
											></span
										></span
									>
									<span class="m-scene-row__state"
										>{#if s.archived}<span class="m-chip m-chip--kind_fact"
												><span class="m-icon" aria-hidden="true">archive</span><span>Archived</span
												></span
											>{/if}</span
									>
								</a>
							{/each}
						</div>
					{/if}

					{#if data.show.lore}
						<div
							class="m-vt m-vt-box"
							style="--vt: m-sr-lore; display: flex; flex-direction: column; gap: 4px"
						>
							<h2
								style="margin: 0 0 6px; font: 600 15px/20px var(--font-ui); display: flex; gap: 8px; align-items: center"
							>
								<span class="m-icon" aria-hidden="true" style="color: var(--ink-muted)"
									>menu_book</span
								>Lore
							</h2>
							<a
								class="hit"
								href={resolve('/w/[world]/lore', { world: 'gilded-reach' })}
								style="display: grid; gap: 4px; padding: 12px; margin-inline: -12px; text-decoration: none; color: var(--ink)"
							>
								<span style="font-size: 13px; color: var(--ink-muted)"
									>The Gilded Reach › Faith</span
								>
								<span style="font-weight: 600; font-size: 16px">The <mark>Lantern</mark> Rite</span>
								<span style="font-family: var(--font-reading); font-size: 15px; line-height: 1.5"
									>On the first new moon of spring every household sets a paper <mark>lantern</mark> on
									the river for each of its dead.</span
								>
							</a>
							<a
								class="hit"
								href={resolve('/w/[world]/lore', { world: 'gilded-reach' })}
								style="display: grid; gap: 4px; padding: 12px; margin-inline: -12px; text-decoration: none; color: var(--ink)"
							>
								<span style="font-size: 13px; color: var(--ink-muted)"
									>The Gilded Reach › Places</span
								>
								<span style="font-weight: 600; font-size: 16px"><mark>Lantern</mark> Quay</span>
								<span style="font-family: var(--font-reading); font-size: 15px; line-height: 1.5"
									>The river port below Harrowgate. Customs closes at dusk; the smugglers do not.</span
								>
							</a>
						</div>
					{/if}

					{#if data.show.characters}
						<div
							class="m-vt m-vt-box"
							style="--vt: m-sr-characters; display: flex; flex-direction: column; gap: 4px"
						>
							<h2
								style="margin: 0 0 6px; font: 600 15px/20px var(--font-ui); display: flex; gap: 8px; align-items: center"
							>
								<span class="m-icon" aria-hidden="true" style="color: var(--ink-muted)">group</span
								>Characters
							</h2>
							<a
								class="hit"
								href={resolve('/w/[world]/characters/[character]', {
									world: 'gilded-reach',
									character: 'lady-maren'
								})}
								style="display: grid; grid-template-columns: 40px minmax(0, 1fr); gap: 2px 12px; padding: 12px; margin-inline: -12px; text-decoration: none; color: var(--ink); align-items: center"
							>
								<span
									class="m-avatar m-avatar--kind_character m-avatar--size_md"
									aria-hidden="true"
									style="grid-row: span 2"><img src={data.portraits.wren} alt="" /></span
								>
								<span style="font-weight: 600">Wren of the Quay</span>
								<span style="font-size: 13px; color: var(--ink-muted)"
									><mark>Lantern</mark>-keeper’s apprentice, smuggler. The Gilded Reach, played by
									@ola.codes</span
								>
							</a>
						</div>
					{/if}

					{#if data.show.people}
						<div
							class="m-vt m-vt-box"
							style="--vt: m-sr-people; display: flex; flex-direction: column; gap: 4px"
						>
							<h2
								style="margin: 0 0 6px; font: 600 15px/20px var(--font-ui); display: flex; gap: 8px; align-items: center"
							>
								<span class="m-icon" aria-hidden="true" style="color: var(--ink-muted)">person</span
								>People
							</h2>
							<div
								class="hit"
								style="display: grid; grid-template-columns: 40px minmax(0, 1fr) auto; gap: 2px 12px; padding: 12px; margin-inline: -12px; align-items: center"
							>
								<span
									class="m-avatar m-avatar--kind_account m-avatar--size_md"
									aria-hidden="true"
									style="--avatar-hue: 48; grid-row: span 2"
									><span aria-hidden="true">LF</span></span
								>
								<span style="font-weight: 600"><mark>Lantern</mark> Fox</span>
								<a
									class="m-button m-button--variant_outline m-button--size_sm"
									href="#profile"
									style="grid-row: span 2"><span>Profile</span></a
								>
								<span style="font-size: 13px; color: var(--ink-muted)"
									>@lanternfox.bsky.social. Plays in 2 Worlds</span
								>
							</div>
						</div>
					{/if}
				</section>
			</div>
		</div>
	</main>
</div>

<style>
	:not(.m-listbox__optionLabel) > mark {
		background-color: var(--accent-soft);
		color: var(--on-accent-soft);
		border-radius: 3px;
		padding-inline: 2px;
	}
	.hit {
		border-radius: var(--radius-md);
	}
	.hit:hover {
		background-color: color-mix(in oklab, var(--ink) 4%, transparent);
	}
	@layer reset {
		.tabbtn {
			background: none;
			border: 0;
			cursor: pointer;
			font: inherit;
		}
	}
</style>
