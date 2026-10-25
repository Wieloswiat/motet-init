<script lang="ts">
	import Chrome from '#lib/components/motet/Chrome.svelte';
	import WorldHeader from '#lib/components/motet/WorldHeader.svelte';
	import { world } from '#lib/motet/world.js';
	import type { LoreData } from '#lib/demo/screens/lore.js';

	let { data }: { data: LoreData } = $props();
</script>

<svelte:head>
	<title>Lore</title>
</svelte:head>

<div {@attach world(data.worldRef)} class="m-world__root">
	<Chrome world="vellum" account="iga" inbox={data.inboxCount} />
	<WorldHeader current="Lore" size="slim" role="player" />
	<main class="m-world__page" style="padding-top: 24px">
		<div style="display: flex; flex-wrap: wrap; gap: 28px; align-items: flex-start">
			<nav
				aria-label="Lore pages"
				style="flex: 1 1 200px; max-width: 100%; display: flex; flex-direction: column; gap: 12px"
			>
				<div class="m-input__root">
					<span class="m-input__addon" data-side="start" aria-hidden="true"
						><span class="m-icon" aria-hidden="true">search</span></span
					><input
						class="m-input__control"
						type="search"
						placeholder="Search lore"
						aria-label="Search lore"
					/>
				</div>
				<ul class="m-tree__root" role="tree" aria-label="Lore">
					{#each data.tree as g (g.label)}
						<li
							class="m-tree__item"
							role="treeitem"
							aria-level="1"
							aria-expanded={g.expanded}
							aria-selected="false"
							style="--level: 1"
						>
							<button class="m-tree__row treebtn" type="button"
								><span class="m-tree__toggle" aria-hidden="true"
									><span class="m-icon" aria-hidden="true">chevron_right</span></span
								><span class="m-icon m-tree__icon" aria-hidden="true">{g.icon}</span><span
									class="m-tree__label">{g.label}</span
								><span class="m-tree__count">{g.count}</span></button
							>
							{#if g.open}
								<ul class="m-tree__group" role="group">
									{#each g.pages as p (p.label)}
										<li
											class="m-tree__item"
											role="treeitem"
											aria-level="2"
											aria-selected={p.selected}
											data-leaf=""
											style="--level: 2"
										>
											<button class="m-tree__row treebtn" type="button"
												><span class="m-tree__toggle" aria-hidden="true"></span><span
													class="m-tree__label">{p.label}</span
												>{#if p.stub}<span class="m-tree__count">stub</span>{/if}</button
											>
										</li>
									{/each}
								</ul>
							{/if}
						</li>
					{/each}
				</ul>
			</nav>

			<article
				style="flex: 999 1 480px; min-width: 0; display: flex; flex-direction: column; gap: 14px"
			>
				<nav class="m-breadcrumb__root" aria-label="Breadcrumb">
					<ol class="m-breadcrumb__list">
						<li class="m-breadcrumb__item">
							<a class="m-breadcrumb__link" href="#lore">Lore</a><span
								class="m-icon m-breadcrumb__separator"
								aria-hidden="true">chevron_right</span
							>
						</li>
						<li class="m-breadcrumb__item">
							<a class="m-breadcrumb__link" href="#lore">{data.page.group}</a><span
								class="m-icon m-breadcrumb__separator"
								aria-hidden="true">chevron_right</span
							>
						</li>
						<li class="m-breadcrumb__item">
							<span class="m-breadcrumb__current" aria-current="page">{data.page.title}</span>
						</li>
					</ol>
				</nav>
				<h1
					class="m-flanked"
					style="margin: 0; font-family: var(--font-display); font-weight: var(--display-weight); font-size: calc(38px * var(--display-adjust, 1)); line-height: 1.1; text-transform: var(--display-case); letter-spacing: var(--display-tracking); text-wrap: balance"
				>
					{data.page.title}
				</h1>
				<div
					style="display: flex; flex-wrap: wrap; gap: 8px 16px; align-items: center; font-size: 13px; color: var(--ink-muted)"
				>
					<span class="m-meta"
						><span class="m-icon m-icon--sm" aria-hidden="true">history</span><span
							>{data.page.edited}</span
						></span
					>
					<span class="m-meta"
						><span class="m-icon m-icon--sm" aria-hidden="true">{data.page.visIcon}</span><span
							>{data.page.vis}</span
						></span
					>
					<span style="flex: 1"></span>
					<button class="m-button m-button--variant_text m-button--size_sm" type="button"
						><span class="m-icon" aria-hidden="true">history</span><span>History</span></button
					>
					<button class="m-button m-button--variant_outline m-button--size_sm" type="button"
						><span class="m-icon" aria-hidden="true">edit</span><span>Suggest an edit</span></button
					>
				</div>

				{#if data.is.keep}
					<div class="m-vt-pane m-prose m-prose--lead" style="max-width: none">
						<p>
							Harrowgate Keep stands on the last high ground before the river Ferrow spreads into
							the marshes. It has been the seat of House Vael for eleven generations and the place
							every lord of the Reach comes to when something needs deciding.
						</p>
						<h2 id="sec-hall" class="h-art">The great hall</h2>
						<p>
							The hall is older than the rest of the Keep. Its beams are hung with the banners of
							every sworn house, in the order they swore, and a missing banner is noticed before a
							missing guest. During the Feast of Lanterns the hall is lit only by lanterns, one for
							each house’s dead.
						</p>
						<h2 id="sec-archive" class="h-art">The Archive</h2>
						<p>
							Below the chapel, three rooms of shelves and one of locked chests. The brothers of the
							Archive keep the household rolls, the treaties and the oaths. Anyone may read the
							rolls. Only the Keeper reads the chests.
						</p>
						<h2 id="sec-who" class="h-art">Who lives there</h2>
						<p>
							The Duke and his household, a garrison of sixty under Ser Jaume Tallow, the brothers
							of the Archive, and during the Feast every lord who can afford a room and several who
							cannot.
						</p>
					</div>
					<div class="m-collapsible__root m-collapsible__root--variant_spoiler">
						<button
							class="m-collapsible__trigger m-collapsible__trigger--variant_spoiler"
							type="button"
							aria-expanded={data.spoilerOpen}
							aria-controls="sp"
							><span class="m-icon" aria-hidden="true">visibility_off</span><span
								class="m-collapsible__label m-collapsible__label--variant_spoiler">Spoiler</span
							><span class="m-collapsible__topic m-collapsible__topic--variant_spoiler"
								>Revealed in The Steward’s Ledger</span
							><span class="m-collapsible__action m-collapsible__action--variant_spoiler"
								>{data.spoilerAction}</span
							></button
						>
						{#if data.spoilerOpen}<div
								class="m-collapsible__content m-collapsible__content--variant_spoiler"
								id="sp"
								role="region"
								style="font-family: var(--font-reading); font-size: 16px; line-height: 1.55"
							>
								The Keep’s garrison has not been paid since autumn. The steward has been paying it
								from the Feast accounts.
							</div>{/if}
					</div>
				{/if}

				{#if data.is.rite}
					<div class="m-vt-pane m-prose m-prose--lead" style="max-width: none">
						<p>
							On the first new moon of spring every household in the Reach sets a paper lantern on
							the river for each of its dead. The lanterns are lit at dusk from a single flame
							carried down from the Keep, and the river takes them to the sea.
						</p>
						<h2 id="sec-rite" class="h-art">The rite</h2>
						<p>
							The head of a household names each of its dead aloud before the lantern goes into the
							water. It is bad luck to name someone who is not dead, and worse luck to forget
							someone who is.
						</p>
						<h2 id="sec-court" class="h-art">At court</h2>
						<p>
							At Harrowgate the rite has become the Feast of Lanterns: three days of dinners,
							audiences and quiet bargains, ending on the Quay at dusk. Which houses walk down to
							the river together is watched more closely than any treaty.
						</p>
					</div>
				{/if}

				{#if data.is.checks}
					<div class="m-vt-pane m-prose" style="max-width: none">
						<p>
							Roll when the outcome is uncertain and someone wants something else to happen: another
							character, the GM, or the world. Never roll to find out whether your own character
							tries something.
						</p>
						<h2 id="sec-roll" class="h-art">How to roll</h2>
						<p>
							Roll d20 and add your trait if it applies, from the composer’s dice button. The GM
							sets the difficulty before or after, but always says it. Ties go to whoever is
							defending.
						</p>
						<h2 id="sec-traits" class="h-art">Traits</h2>
					</div>
					<div style="overflow-x: auto">
						<table class="lore-table">
							<thead
								><tr
									><th scope="col">Trait</th><th scope="col">Covers</th><th scope="col">Adds</th
									></tr
								></thead
							>
							<tbody>
								<tr><td>Silver tongue</td><td>Persuading, lying, reading a room</td><td>+3</td></tr>
								<tr><td>Steel</td><td>Fighting, enduring, frightening</td><td>+3</td></tr>
								<tr><td>Letters</td><td>Records, ciphers, law and history</td><td>+3</td></tr>
								<tr><td>Shadows</td><td>Sneaking, stealing, going unnoticed</td><td>+3</td></tr>
								<tr><td>Faith</td><td>The Rite, the dead, the priesthood</td><td>+3</td></tr>
							</tbody>
						</table>
					</div>
				{/if}

				{#if data.is.stub}
					<div
						class="m-vt-pane"
						style="display: flex; flex-direction: column; gap: 12px; align-items: flex-start; padding: 24px; border: 1px dashed var(--line-strong); border-radius: var(--radius-lg)"
					>
						<p style="margin: 0; font-family: var(--font-reading); font-size: 17px">
							Nobody has written this page yet.
						</p>
						<button class="m-button m-button--variant_tonal m-button--size_sm" type="button"
							><span class="m-icon" aria-hidden="true">edit</span><span>Suggest a first draft</span
							></button
						>
					</div>
				{/if}

				{#if data.hasMentions}
					<section
						aria-labelledby="h-mentions"
						style="display: flex; flex-direction: column; gap: 6px; margin-top: 16px; padding-top: 16px; border-top: 1px solid var(--line)"
					>
						<h2
							id="h-mentions"
							style="margin: 0; font: 600 13px/18px var(--font-ui); color: var(--ink-muted)"
						>
							Mentioned in
						</h2>
						<div style="display: flex; flex-wrap: wrap; gap: 8px">
							{#each data.page.mentions as m (m.title)}<a
									class="m-chip m-chip--kind_fact"
									href={m.href}
									style="text-decoration: none"
									><span class="m-icon" aria-hidden="true">forum</span><span>{m.title}</span></a
								>{/each}
						</div>
					</section>
				{/if}
			</article>

			<aside
				aria-label="About this page"
				style="flex: 1 1 220px; max-width: 100%; display: flex; flex-direction: column; gap: 20px; position: sticky; top: calc(var(--chrome-offset, 72px) + 8px)"
			>
				{#if data.hasInfo}
					<div
						class="m-ornamented m-pad"
						style="display: flex; flex-direction: column; gap: 10px; background-color: var(--surface-raised); border: var(--border-width, 1px) solid var(--frame-line, var(--line)); border-radius: var(--frame-radius, var(--radius-lg)); corner-shape: var(--frame-shape, var(--corner-shape))"
					>
						<div
							aria-hidden="true"
							style="height: 120px; border-radius: var(--radius-md); background-image: var(--art); background-size: cover; background-position: 70% 40%"
						></div>
						<dl class="m-datalist__root m-datalist__root--variant_stacked" style="margin: 0">
							{#each data.page.info as i, j (j)}
								<div class="m-datalist__item m-datalist__item--variant_stacked">
									<dt class="m-datalist__label m-datalist__label--variant_stacked">{i.k}</dt>
									<dd class="m-datalist__value m-datalist__value--variant_stacked">{i.v}</dd>
								</div>
							{/each}
						</dl>
					</div>
				{/if}
				{#if data.hasToc}
					<nav aria-label="On this page" style="display: flex; flex-direction: column; gap: 6px">
						<h2
							style="margin: 0 0 4px; display: flex; gap: 6px; align-items: center; font: 600 13px/18px var(--font-ui); color: var(--ink-muted)"
						>
							<span class="m-icon m-icon--sm" aria-hidden="true">toc</span>On this page
						</h2>
						{#each data.page.toc as t (t.label)}<a
								class="toc"
								href={t.href}
								style="font-size: 14px; line-height: 20px">{t.label}</a
							>{/each}
					</nav>
				{/if}
			</aside>
		</div>
	</main>
</div>

<style>
	.h-art {
		margin: 1.4em 0 0.35em;
		font-family: var(--font-display);
		font-weight: var(--display-weight);
		font-size: calc(24px * var(--display-adjust, 1));
		line-height: 1.2;
		text-transform: var(--display-case);
		letter-spacing: var(--display-tracking);
		scroll-margin-top: calc(var(--chrome-offset, 72px) + 12px);
	}
	@layer reset {
		.treebtn {
			background: none;
			border: 0;
			font: inherit;
			color: inherit;
			cursor: pointer;
			width: 100%;
			text-align: start;
		}
	}
	.toc {
		text-decoration: none;
		color: var(--ink-muted);
	}
	.toc:hover {
		color: var(--ink);
	}
	.lore-table {
		width: 100%;
		border-collapse: collapse;
		font-family: var(--font-ui);
		font-size: 14px;
	}
	.lore-table th,
	.lore-table td {
		text-align: start;
		padding: 8px 10px;
		border-bottom: 1px solid var(--line);
		vertical-align: top;
	}
	.lore-table th {
		font-weight: 600;
		color: var(--ink-muted);
		font-size: 13px;
	}
</style>
