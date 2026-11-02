<script lang="ts">
	import { resolve } from '$app/paths';
	import Chrome from '#lib/components/motet/Chrome.svelte';
	import LanguagePicker from '#lib/components/motet/LanguagePicker.svelte';
	import { world } from '#lib/motet/world.js';
	import type { HomeShelvesData } from '#lib/demo/screens/home-shelves.js';

	let { data }: { data: HomeShelvesData } = $props();
</script>

<svelte:head>
	<title>Home, shelves</title>
</svelte:head>

<div {@attach world(data.pageRef)} class="m-page__root">
	<Chrome account="iga" inbox={data.inboxCount} />
	<main class="m-page__main">
		<p role="note">
			Design demo: the Worlds, activity and account menu on this page are examples, not your
			account.
		</p>
		<div style="display: flex; flex-direction: column; gap: 48px">
			<section aria-labelledby="h-yours" style="display: flex; flex-direction: column; gap: 16px">
				<div
					style="display: flex; align-items: baseline; justify-content: space-between; gap: 12px; flex-wrap: wrap"
				>
					<h2
						id="h-yours"
						style="margin: 0; font: 600 20px/28px var(--font-ui); text-wrap: balance"
					>
						Your Worlds
					</h2>
					<a class="m-button m-button--variant_text" href={resolve('/new-world')}
						><span class="m-icon" aria-hidden="true">add</span><span>Start a World</span></a
					>
				</div>
				<div
					style="display: grid; grid-template-columns: repeat(auto-fill, minmax(min(240px, 100%), 1fr)); gap: 16px"
				>
					{#each data.yours as w (w.key)}
						<a
							class="wcard"
							href={w.href}
							{@attach world(w.ref)}
							style="display: flex; flex-direction: column; min-width: 0; text-decoration: none; color: var(--ink); background-color: var(--surface); background-image: var(--grain); border: 1px solid var(--line); border-radius: var(--radius-lg); corner-shape: var(--corner-shape); overflow: hidden; box-shadow: var(--elevation-1), var(--edge-light)"
						>
							<span
								aria-hidden="true"
								style="display: block; height: 84px; background-image: var(--art); background-size: cover; background-position: var(--art-focus, 50% 50%)"
							></span>
							<span style="display: flex; gap: 12px; align-items: flex-start; padding: 0 14px">
								<span
									class="m-avatar m-avatar--kind_world m-avatar--size_md"
									aria-hidden="true"
									style="margin-top: -18px; box-shadow: 0 0 0 3px var(--surface)"
									><img src={w.mark} alt="" /></span
								>
								<span style="display: grid; gap: 2px; min-width: 0; padding-top: 8px">
									<span
										style="font-family: var(--font-display); font-weight: var(--display-weight); font-size: calc(19px * var(--display-adjust, 1)); line-height: 1.15; text-transform: var(--display-case); letter-spacing: var(--display-tracking)"
										>{w.name}</span
									>
									<span
										style="display: flex; gap: 6px; align-items: center; font-size: 13px; line-height: 18px; color: var(--ink-muted)"
										><span class="m-icon m-icon--sm" aria-hidden="true">{w.asIcon}</span
										>{w.as}</span
									>
								</span>
							</span>
							<span
								style="display: flex; gap: 8px; align-items: center; flex-wrap: wrap; padding: 12px 14px 14px; margin-top: auto; min-height: 24px"
							>
								{#if w.hasFlag}
									<span class={w.flagClass}
										><span class="m-icon m-icon--sm" aria-hidden="true">{w.flagIcon}</span
										>{w.flagLabel}</span
									>
								{/if}
								{#if w.hasNote}
									<span class="m-meta" style="font-size: 13px; color: var(--ink-muted)"
										><span class="m-icon m-icon--sm" aria-hidden="true">{w.noteIcon}</span><span
											>{w.note}</span
										></span
									>
								{/if}
							</span>
						</a>
					{/each}
				</div>
			</section>

			<section aria-labelledby="h-continue" style="display: flex; flex-direction: column; gap: 8px">
				<h2 id="h-continue" style="margin: 0 0 4px; font: 600 20px/28px var(--font-ui)">
					Continue
				</h2>
				<div style="display: flex; flex-direction: column">
					<a
						class="m-scene-row__root m-scene-row__root--unread_true"
						href={resolve('/w/[world]/play/[scene]', {
							world: 'gilded-reach',
							scene: 'feast-of-lanterns'
						})}
					>
						<span class="m-scene-row__marker m-scene-row__marker--unread_true" aria-hidden="true"
						></span>
						<span class="m-scene-row__main m-scene-row__main--unread_true">
							<span class="m-scene-row__title m-scene-row__title--unread_true"
								>The Feast of Lanterns<span class="m-visually-hidden">, has unread posts</span
								></span
							>
							<span class="m-scene-row__where m-scene-row__where--unread_true">
								<span class="m-meta"
									><span class="m-avatar m-avatar--kind_world m-avatar--size_xs" aria-hidden="true"
										><img src={data.marks.vellum} alt="" /></span
									><span>The Gilded Reach</span></span
								>
								<span class="m-meta"
									><span class="m-icon m-icon--sm" aria-hidden="true">location_on</span><span
										>Harrowgate Keep, the great hall</span
									></span
								>
							</span>
						</span>
						<span class="m-scene-row__state m-scene-row__state--unread_true"
							><span class="m-flag m-flag--kind_turn"
								><span class="m-icon m-icon--sm" aria-hidden="true">front_hand</span>Your turn</span
							><span class="m-count" title="3 unread">3</span></span
						>
						<span
							class="m-scene-row__cast m-scene-row__cast--unread_true"
							title="Cast: Lady Maren of Ost, Brother Cael"
							><span class="m-avatar m-avatar--kind_character m-avatar--size_xs" aria-hidden="true"
								><img src={data.portraits.maren} alt="" /></span
							><span class="m-avatar m-avatar--kind_character m-avatar--size_xs" aria-hidden="true"
								><img src={data.portraits.cael} alt="" /></span
							></span
						>
						<time class="m-scene-row__time m-scene-row__time--unread_true">2h</time>
					</a>
					<a class="m-scene-row__root m-scene-row__root--unread_true" href="#kepler-silent-hours">
						<span class="m-scene-row__marker m-scene-row__marker--unread_true" aria-hidden="true"
						></span>
						<span class="m-scene-row__main m-scene-row__main--unread_true">
							<span class="m-scene-row__title m-scene-row__title--unread_true"
								>Silent Hours<span class="m-visually-hidden">, has unread posts</span></span
							>
							<span class="m-scene-row__where m-scene-row__where--unread_true">
								<span class="m-meta"
									><span class="m-avatar m-avatar--kind_world m-avatar--size_xs" aria-hidden="true"
										><img src={data.marks.kepler} alt="" /></span
									><span>Kepler Relay</span></span
								>
								<span class="m-meta"
									><span class="m-icon m-icon--sm" aria-hidden="true">location_on</span><span
										>Comms deck, ring B</span
									></span
								>
							</span>
						</span>
						<span class="m-scene-row__state m-scene-row__state--unread_true"
							><span class="m-count" title="2 unread">2</span></span
						>
						<span
							class="m-scene-row__cast m-scene-row__cast--unread_true"
							title="Cast: Ensign Petra Lind, Oyelaran, Horvat"
							><span class="m-avatar m-avatar--kind_character m-avatar--size_xs" aria-hidden="true"
								><img src={data.portraits.petra} alt="" /></span
							><span class="m-avatar m-avatar--kind_character m-avatar--size_xs" aria-hidden="true"
								><img src={data.portraits.oyelaran} alt="" /></span
							><span class="m-avatar m-avatar--kind_character m-avatar--size_xs" aria-hidden="true"
								><img src={data.portraits.horvat} alt="" /></span
							></span
						>
						<time class="m-scene-row__time m-scene-row__time--unread_true">1d</time>
					</a>
					<a class="m-scene-row__root m-scene-row__root--unread_false" href="#letters-from-ost">
						<span class="m-scene-row__marker m-scene-row__marker--unread_false" aria-hidden="true"
						></span>
						<span class="m-scene-row__main m-scene-row__main--unread_false">
							<span class="m-scene-row__title m-scene-row__title--unread_false"
								>Letters from Ost</span
							>
							<span class="m-scene-row__where m-scene-row__where--unread_false">
								<span class="m-meta"
									><span class="m-avatar m-avatar--kind_world m-avatar--size_xs" aria-hidden="true"
										><img src={data.marks.vellum} alt="" /></span
									><span>The Gilded Reach</span></span
								>
								<span class="m-meta"
									><span class="m-icon m-icon--sm" aria-hidden="true">location_on</span><span
										>The Northern March</span
									></span
								>
							</span>
						</span>
						<span class="m-scene-row__state m-scene-row__state--unread_false"
							><span class="m-flag m-flag--kind_draft"
								><span class="m-icon m-icon--sm" aria-hidden="true">draft</span>Draft</span
							></span
						>
						<span
							class="m-scene-row__cast m-scene-row__cast--unread_false"
							title="Cast: Lady Maren of Ost"
							><span class="m-avatar m-avatar--kind_character m-avatar--size_xs" aria-hidden="true"
								><img src={data.portraits.maren} alt="" /></span
							></span
						>
						<time class="m-scene-row__time m-scene-row__time--unread_false">Mar 2</time>
					</a>
				</div>
			</section>

			<section
				aria-labelledby="h-discover"
				style="display: flex; flex-direction: column; gap: 20px"
			>
				<div
					style="display: flex; align-items: baseline; justify-content: space-between; gap: 12px; flex-wrap: wrap"
				>
					<h2 id="h-discover" style="margin: 0; font: 600 20px/28px var(--font-ui)">Discover</h2>
					<a class="m-button m-button--variant_text" href={resolve('/search')}
						><span>Browse all Worlds</span><span class="m-icon" aria-hidden="true"
							>arrow_forward</span
						></a
					>
				</div>
				<div style="display: flex; flex-wrap: wrap; gap: 12px 16px; align-items: center">
					<div role="group" aria-label="Genre" style="display: flex; flex-wrap: wrap; gap: 8px">
						{#each data.genres as g (g.label)}
							<button class="m-chip m-chip--kind_filter" type="button" aria-pressed={g.pressed}
								>{#if g.on}<span class="m-icon" aria-hidden="true">check</span>{/if}<span
									>{g.label}</span
								></button
							>
						{/each}
					</div>
					<span aria-hidden="true" style="width: 1px; height: 24px; background-color: var(--line)"
					></span>
					<div style="flex: 0 1 300px; min-width: 220px">
						<LanguagePicker
							label="Languages"
							hideLabel
							placeholder="Any language"
							value={data.langCodes}
							pid="discover-lang"
						/>
					</div>
					<button
						class="m-chip m-chip--kind_filter"
						type="button"
						aria-pressed={data.recruitingPressed}
						><span class="m-icon" aria-hidden="true">how_to_reg</span><span>Recruiting</span
						></button
					>
				</div>

				{#each data.shelves as s (s.title)}
					<div style="display: flex; flex-direction: column; gap: 12px">
						<h3
							style="margin: 0; display: flex; gap: 8px; align-items: center; font: 600 15px/20px var(--font-ui); color: var(--ink)"
						>
							<span class="m-icon" aria-hidden="true" style="color: var(--ink-muted)">{s.icon}</span
							>{s.title}
						</h3>
						<div
							style="display: grid; grid-template-columns: repeat(auto-fill, minmax(min(300px, 100%), 1fr)); gap: 16px"
						>
							{#each s.items as w (w.key)}
								<a
									class="wcard"
									href={w.href}
									{@attach world(w.ref)}
									style="display: flex; flex-direction: column; min-width: 0; text-decoration: none; color: var(--ink); background-color: var(--surface); background-image: var(--grain); border: 1px solid var(--line); border-radius: var(--radius-lg); corner-shape: var(--corner-shape); overflow: hidden; box-shadow: var(--elevation-1), var(--edge-light)"
								>
									<span
										aria-hidden="true"
										style="display: block; height: 112px; background-image: var(--art); background-size: cover; background-position: var(--art-focus, 50% 50%)"
									></span>
									<span style="display: flex; gap: 12px; align-items: flex-start; padding: 0 16px">
										<span
											class="m-avatar m-avatar--kind_world m-avatar--size_md"
											aria-hidden="true"
											style="margin-top: -18px; box-shadow: 0 0 0 3px var(--surface)"
											><img src={w.mark} alt="" /></span
										>
										<span style="display: grid; gap: 2px; min-width: 0; padding-top: 8px">
											<span
												style="font-family: var(--font-display); font-weight: var(--display-weight); font-size: calc(21px * var(--display-adjust, 1)); line-height: 1.15; text-transform: var(--display-case); letter-spacing: var(--display-tracking)"
												>{w.name}</span
											>
										</span>
									</span>
									<span
										style="padding: 10px 16px 0; font-family: var(--font-reading); font-size: 15.5px; line-height: 1.5; color: var(--ink); text-wrap: pretty"
										>{w.pitch}</span
									>
									<span
										style="display: flex; flex-wrap: wrap; gap: 6px 14px; padding: 12px 16px 0; font-size: 13px; line-height: 18px; color: var(--ink-muted)"
									>
										<span class="m-meta"
											><span class="m-icon m-icon--sm" aria-hidden="true">{w.stateIcon}</span><span
												>{w.state}</span
											></span
										>
										<span class="m-meta"
											><span class="m-icon m-icon--sm" aria-hidden="true">translate</span><span
												>{w.lang}</span
											></span
										>
										{#if w.hasPace}<span class="m-meta"
												><span class="m-icon m-icon--sm" aria-hidden="true">schedule</span><span
													>{w.pace}</span
												></span
											>{/if}
									</span>
									<span style="display: flex; flex-wrap: wrap; gap: 6px; padding: 12px 16px 0">
										{#each w.tags as t (t)}<span class="m-chip m-chip--kind_fact"
												><span>{t}</span></span
											>{/each}
									</span>
									<span
										style="display: flex; gap: 8px; align-items: center; padding: 14px 16px 16px; margin-top: auto; font-size: 13px; line-height: 18px; color: var(--ink-muted)"
									>
										{#if w.hasPeople}
											<span style="display: flex">
												{#each w.people as p, i (i)}<span
														class="m-avatar m-avatar--kind_account m-avatar--size_xs"
														aria-hidden="true"
														style="{p.hue}; box-shadow: 0 0 0 2px var(--surface); margin-inline-end: -4px"
														><span aria-hidden="true">{p.initial}</span></span
													>{/each}
											</span>
											<span style="padding-inline-start: 6px">{w.peopleText}</span>
										{/if}
										{#if w.noPeople}
											<span class="m-meta"
												><span class="m-icon m-icon--sm" aria-hidden="true">shield_person</span
												><span>{w.gms}</span></span
											>
										{/if}
									</span>
								</a>
							{/each}
						</div>
					</div>
				{/each}
				{#if data.nothing}
					<div
						style="display: flex; flex-direction: column; align-items: flex-start; gap: 12px; padding: 24px; border: 1px dashed var(--line-strong); border-radius: var(--radius-lg)"
					>
						<p style="margin: 0; font-size: 15px">No Worlds match these filters.</p>
						<button class="m-button m-button--variant_tonal m-button--size_sm" type="button"
							><span>Clear filters</span></button
						>
					</div>
				{/if}
			</section>
		</div>
	</main>
</div>

<style>
	.wcard {
		transition:
			box-shadow var(--dur-effects-fast) var(--ease-effects-fast),
			translate var(--dur-spatial-fast) var(--ease-spatial-fast);
	}
	.wcard:hover {
		box-shadow: var(--elevation-2), var(--edge-light);
	}
	.wcard:focus-visible {
		outline: 2px solid var(--focus);
		outline-offset: 2px;
	}
</style>
