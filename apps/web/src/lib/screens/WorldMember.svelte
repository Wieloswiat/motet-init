<script lang="ts">
	import { resolve } from '$app/paths';
	import Chrome from '#lib/components/motet/Chrome.svelte';
	import WorldHeader from '#lib/components/motet/WorldHeader.svelte';
	import { world } from '#lib/motet/world.js';
	import type { WorldMemberData } from '#lib/demo/screens/world-member.js';

	let { data }: { data: WorldMemberData } = $props();
</script>

<svelte:head>
	<title>World overview, member</title>
</svelte:head>

<div {@attach world(data.worldRef)} class="m-world__root">
	<Chrome world="vellum" account="iga" inbox={data.inboxCount} />
	<WorldHeader current="Overview" size="full" role="player" />
	<main class="m-world__page" style="padding-top: 28px">
		<div class="m-world__layout">
			<div style="display: flex; flex-direction: column; gap: 40px; min-width: 0">
				<section aria-labelledby="h-yours" style="display: flex; flex-direction: column; gap: 8px">
					<div style="display: flex; gap: 12px; align-items: baseline; flex-wrap: wrap">
						<h2 id="h-yours" class="h-section m-flanked">Your scenes</h2>
						<a
							class="m-button m-button--variant_text m-button--size_sm"
							href={resolve('/w/[world]/play', { world: 'gilded-reach' })}
							style="margin-inline-start: auto"
							><span class="m-icon" aria-hidden="true">forum</span><span>All scenes</span></a
						>
					</div>
					<div style="display: flex; flex-direction: column">
						{#each data.mine as s (s.title)}
							<a class={s.rootClass} href={s.href}>
								<span class={s.markerClass} aria-hidden="true"></span>
								<span class="m-scene-row__main">
									<span class={s.titleClass}
										>{s.title}{#if s.unread}<span class="m-visually-hidden">, has unread posts</span
											>{/if}</span
									>
									<span class="m-scene-row__where">
										<span class="m-meta"
											><span class="m-icon m-icon--sm" aria-hidden="true">location_on</span><span
												>{s.where}</span
											></span
										>
										<span class="m-meta"
											><span class="m-icon m-icon--sm" aria-hidden="true">theater_comedy</span><span
												>{s.as}</span
											></span
										>
									</span>
								</span>
								<span class="m-scene-row__state">
									{#if s.hasFlag}<span class={s.flagClass}
											><span class="m-icon m-icon--sm" aria-hidden="true">{s.flagIcon}</span
											>{s.flagLabel}</span
										>{/if}
									{#if s.hasCount}<span class="m-count" title={s.countTip}>{s.count}</span>{/if}
								</span>
								<span class="m-scene-row__cast" title={s.castTip}>
									{#each s.cast as c (c)}<span
											class="m-avatar m-avatar--kind_character m-avatar--size_xs"
											aria-hidden="true"><img src={c} alt="" /></span
										>{/each}
								</span>
								<time class="m-scene-row__time">{s.time}</time>
							</a>
						{/each}
					</div>
				</section>

				<section aria-labelledby="h-gms" style="display: flex; flex-direction: column; gap: 12px">
					<div style="display: flex; gap: 12px; align-items: baseline; flex-wrap: wrap">
						<h2 id="h-gms" class="h-section">From the GMs</h2>
						<a
							class="m-button m-button--variant_text m-button--size_sm"
							href={resolve('/w/[world]/play', { world: 'gilded-reach' })}
							style="margin-inline-start: auto"
							><span class="m-icon" aria-hidden="true">campaign</span><span>Announcements</span></a
						>
					</div>
					<article class="notice" aria-labelledby="ann-title">
						<header
							style="display: grid; grid-template-columns: auto minmax(0, 1fr) auto; gap: 2px 10px; align-items: center"
						>
							<span
								class="m-avatar m-avatar--kind_account m-avatar--size_sm"
								aria-hidden="true"
								style="--avatar-hue: 262; grid-row: span 2"><span aria-hidden="true">AB</span></span
							>
							<span
								style="display: flex; flex-wrap: wrap; gap: 4px 8px; align-items: center; font-size: 14px"
								><span style="font-weight: 600">Aurelia</span><span style="color: var(--ink-muted)"
									>@aurelia.bsky.social</span
								><span class="m-role"
									><span class="m-icon" aria-hidden="true">shield_person</span>GM</span
								></span
							>
							<time
								style="font-size: 13px; color: var(--ink-muted); grid-row: span 2; align-self: start"
								>Mar 9</time
							>
							<h3 id="ann-title" style="margin: 0; font: 600 15px/22px var(--font-ui)">
								The Feast ends with round 6
							</h3>
						</header>
						<div class="m-prose m-prose--ui" style="font-size: 15px">
							<p>
								The Feast of Lanterns closes when the last lantern goes out, after round 6. Then the
								court rides north for the Duke’s funeral, and arc two starts in Ost. If your
								character has business to finish in Harrowgate, finish it at the Feast.
							</p>
						</div>
						<div
							style="display: flex; flex-wrap: wrap; gap: 4px 8px; align-items: center; margin-inline-start: -12px"
						>
							<a class="m-button m-button--variant_text m-button--size_sm" href="#announcement"
								><span class="m-icon" aria-hidden="true">reply</span><span
									>Reply in Announcements</span
								></a
							>
							<span
								class="m-meta"
								style="margin-inline-start: auto; font-size: 13px; color: var(--ink-muted)"
								><span class="m-icon m-icon--sm" aria-hidden="true">forum</span><span
									>5 replies</span
								></span
							>
						</div>
					</article>
				</section>

				<section aria-labelledby="h-else" style="display: flex; flex-direction: column; gap: 8px">
					<h2 id="h-else" class="h-section">Elsewhere in the Reach</h2>
					<div style="display: flex; flex-direction: column">
						{#each data.others as s (s.title)}
							<a class={s.rootClass} href={s.href}>
								<span class={s.markerClass} aria-hidden="true"></span>
								<span class="m-scene-row__main">
									<span class={s.titleClass}
										>{s.title}{#if s.unread}<span class="m-visually-hidden">, has unread posts</span
											>{/if}</span
									>
									<span class="m-scene-row__where">
										<span class="m-meta"
											><span class="m-icon m-icon--sm" aria-hidden="true">{s.whereIcon}</span><span
												>{s.where}</span
											></span
										>
										{#if s.hasArc}<span class="m-meta"
												><span class="m-icon m-icon--sm" aria-hidden="true">auto_stories</span><span
													>{s.arc}</span
												></span
											>{/if}
									</span>
								</span>
								<span class="m-scene-row__state">
									{#if s.hasFlag}<span class={s.flagClass}
											><span class="m-icon m-icon--sm" aria-hidden="true">{s.flagIcon}</span
											>{s.flagLabel}</span
										>{/if}
									{#if s.hasCount}<span class="m-count" title={s.countTip}>{s.count}</span>{/if}
								</span>
								<span class="m-scene-row__cast" title={s.castTip}>
									{#each s.cast as c (c)}<span
											class="m-avatar m-avatar--kind_character m-avatar--size_xs"
											aria-hidden="true"><img src={c} alt="" /></span
										>{/each}
								</span>
								<time class="m-scene-row__time">{s.time}</time>
							</a>
						{/each}
					</div>
				</section>
			</div>

			<aside
				class="m-world__rail"
				aria-label="You in this World"
				style="display: flex; flex-direction: column; gap: 24px"
			>
				<div style="display: flex; flex-direction: column; gap: 4px">
					<h3 class="h-rail" style="margin-bottom: 4px">Your characters</h3>
					{#each data.chars as c (c.href)}
						<a class="charrow" href={c.href}>
							<span
								class="m-avatar m-avatar--kind_character m-avatar--size_md"
								aria-hidden="true"
								style="grid-row: span 2"><img src={c.portrait} alt="" /></span
							>
							<span style="font-weight: 600; font-size: 14.5px">{c.name}</span>
							<span class="m-meta" style="font-size: 13px"
								><span class="m-icon m-icon--sm" aria-hidden="true">location_on</span><span
									>{c.where}</span
								></span
							>
						</a>
					{/each}
				</div>

				<div style="display: flex; flex-direction: column; gap: 8px">
					<h3 class="h-rail">Next deadline</h3>
					<div style="display: grid; gap: 4px; font-size: 14px">
						<span style="font-weight: 600">Round 3 closes Friday, 18:00</span>
						<span class="m-meta" style="font-size: 13px"
							><span class="m-icon m-icon--sm" aria-hidden="true">schedule</span><span
								>Your time, CET. Posts after it wait for round 4.</span
							></span
						>
					</div>
				</div>

				<nav aria-labelledby="h-pinned" style="display: flex; flex-direction: column; gap: 2px">
					<h3 id="h-pinned" class="h-rail" style="margin-bottom: 6px">Pinned</h3>
					{#each data.pinned as p (p.label)}
						<a class="railnav" href={p.href}
							><span class="m-icon" aria-hidden="true" style="color: var(--ink-muted)"
								>{p.icon}</span
							><span style="flex: 1">{p.label}</span>{#if p.hasNote}<span
									style="font-size: 12.5px; color: var(--ink-muted)">{p.note}</span
								>{/if}</a
						>
					{/each}
				</nav>

				<div style="display: flex; flex-direction: column; gap: 10px">
					<h3 class="h-rail">Game masters</h3>
					<a
						class="charrow"
						href={resolve('/profile/[handle]', { handle: 'aurelia.bsky.social' })}
						style="padding-block: 4px"
						><span
							class="m-avatar m-avatar--kind_account m-avatar--size_sm"
							aria-hidden="true"
							style="--avatar-hue: 262; grid-row: span 2"><span aria-hidden="true">AB</span></span
						><span style="font-weight: 600; font-size: 14px">Aurelia</span><span
							style="font-size: 12.5px; color: var(--ink-muted)">@aurelia.bsky.social</span
						></a
					>
					<a class="charrow" href="#bram" style="padding-block: 4px"
						><span
							class="m-avatar m-avatar--kind_account m-avatar--size_sm"
							aria-hidden="true"
							style="--avatar-hue: 96; grid-row: span 2"><span aria-hidden="true">BW</span></span
						><span style="font-weight: 600; font-size: 14px">Bram</span><span
							style="font-size: 12.5px; color: var(--ink-muted)">@bram.writes</span
						></a
					>
				</div>
			</aside>
		</div>
	</main>
</div>

<style>
	.h-section {
		margin: 0;
		font-family: var(--font-display);
		font-weight: var(--display-weight);
		font-size: calc(26px * var(--display-adjust, 1));
		line-height: 1.15;
		text-transform: var(--display-case);
		letter-spacing: var(--display-tracking);
		text-wrap: balance;
	}
	.h-rail {
		margin: 0;
		font: 600 13px/18px var(--font-ui);
		color: var(--ink-muted);
	}
	.railnav {
		display: flex;
		align-items: center;
		gap: 10px;
		min-height: 36px;
		padding-inline: 8px;
		margin-inline: -8px;
		border-radius: var(--radius-md);
		font-size: 14px;
		text-decoration: none;
		color: var(--ink);
	}
	.railnav:hover {
		background-color: var(--surface-quiet);
	}
	.railnav:focus-visible {
		outline: 2px solid var(--focus);
		outline-offset: 2px;
	}
	.charrow {
		display: grid;
		grid-template-columns: auto minmax(0, 1fr);
		gap: 2px 12px;
		align-items: center;
		padding: 8px;
		margin-inline: -8px;
		border-radius: var(--radius-md);
		text-decoration: none;
		color: var(--ink);
	}
	.charrow:hover {
		background-color: var(--surface-quiet);
	}
	.charrow:focus-visible {
		outline: 2px solid var(--focus);
		outline-offset: 2px;
	}
	.notice {
		display: grid;
		gap: 10px;
		padding: 16px 18px;
		background-color: var(--surface-quiet);
		border: 1px solid var(--line);
		border-radius: var(--radius-md);
		corner-shape: var(--corner-shape);
	}
</style>
