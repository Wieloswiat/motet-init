<script lang="ts">
	import { resolve } from '$app/paths';
	import Chrome from '#lib/components/motet/Chrome.svelte';
	import ReportDialog from '#lib/components/motet/ReportDialog.svelte';
	import { world } from '#lib/motet/world.js';
	import type { ProfileData } from '#lib/demo/screens/profile.js';

	let { data }: { data: ProfileData } = $props();
</script>

<svelte:head>
	<title>Profile</title>
</svelte:head>

<div {@attach world(data.pageRef)} class="m-page__root">
	<Chrome account="iga" inbox={data.inboxCount} />
	<main class="m-page__main" style="--page-max: 1040px">
		<div style="display: flex; flex-direction: column; gap: 28px">
			<header style="display: flex; flex-wrap: wrap; gap: 20px 28px; align-items: flex-start">
				<span
					class="m-avatar m-avatar--kind_account m-avatar--size_xl"
					aria-hidden="true"
					style={data.person.hueStyle}><span aria-hidden="true">{data.person.initials}</span></span
				>
				<div
					style="flex: 1 1 340px; min-width: 0; display: flex; flex-direction: column; gap: 10px"
				>
					<div
						style="display: flex; flex-wrap: wrap; gap: 8px 16px; align-items: flex-start; justify-content: space-between"
					>
						<div style="display: grid; gap: 2px; min-width: 0">
							<h1 style="margin: 0; font: 600 28px/34px var(--font-ui)">{data.person.display}</h1>
							<span style="font-size: 15px; color: var(--ink-muted); overflow-wrap: anywhere"
								>@{data.person.handle}</span
							>
						</div>
						<div style="display: flex; gap: 8px; align-items: center">
							{#if data.own}
								<a class="m-button m-button--variant_tonal" href={resolve('/settings')}
									><span class="m-icon" aria-hidden="true">edit</span><span>Edit profile</span></a
								>
							{/if}
							{#if data.other}
								<span style="position: relative; display: inline-flex">
									<button
										class="m-icon-button m-icon-button--variant_tonal"
										type="button"
										aria-label="More for {data.person.display}"
										aria-haspopup="menu"
										aria-expanded={data.menuOpen}
										><span class="m-icon" aria-hidden="true">more_horiz</span></button
									>
									{#if data.menuShown}
										<div style="position: fixed; inset: 0; z-index: 7"></div>
										<div
											class="m-menu__root"
											role="menu"
											aria-label="More for {data.person.display}"
											style="position: absolute; top: 48px; right: 0; z-index: 8; min-width: 250px"
										>
											<button class="m-menu__item" type="button" role="menuitem"
												><span class="m-icon" aria-hidden="true">content_copy</span>Copy handle</button
											>
											<a class="m-menu__item" role="menuitem" href="#record"
												><span class="m-icon" aria-hidden="true">data_object</span>View record</a
											>
											<hr class="m-menu__separator" />
											<button class="m-menu__item" type="button" role="menuitem"
												><span class="m-icon" aria-hidden="true">volume_off</span
												>{data.muteLabel}</button
											>
											<button class="m-menu__item" type="button" role="menuitem"
												><span class="m-icon" aria-hidden="true">block</span
												>{data.blockLabel}</button
											>
											<button class="m-menu__item" type="button" role="menuitem" data-tone="danger"
												><span class="m-icon" aria-hidden="true">report</span>Report account</button
											>
										</div>
									{/if}
								</span>
							{/if}
						</div>
					</div>
					<div
						style="display: flex; flex-wrap: wrap; gap: 6px 18px; font-size: 13.5px; color: var(--ink-muted)"
					>
						{#if data.person.hasPronouns}<span class="m-meta"
								><span class="m-icon m-icon--sm" aria-hidden="true">person</span><span
									>{data.person.pronouns}</span
								></span
							>{/if}
						<span class="m-meta"
							><span class="m-icon m-icon--sm" aria-hidden="true">translate</span><span
								>{data.person.langs}</span
							></span
						>
						<span class="m-meta"
							><span class="m-icon m-icon--sm" aria-hidden="true">calendar_month</span><span
								>{data.person.since}</span
							></span
						>
						<span class="m-meta" title="Account host"
							><span class="m-icon m-icon--sm" aria-hidden="true">dns</span><span
								>{data.person.host}</span
							></span
						>
					</div>
					<p style="margin: 0; max-width: 64ch; font-size: 15px; line-height: 23px">
						{data.person.bio}
					</p>
					{#if data.hasShared}
						<div
							style="display: flex; flex-wrap: wrap; gap: 8px; align-items: center; font-size: 13.5px; color: var(--ink-muted)"
						>
							<span>You’re both in</span>
							{#each data.shared as w (w.href)}<a
									class="m-chip m-chip--kind_fact"
									href={w.href}
									style="text-decoration: none; color: var(--ink)"
									><span
										class="m-avatar m-avatar--kind_world m-avatar--size_xs"
										aria-hidden="true"
										style="margin-inline-start: -6px"><img src={w.mark} alt="" /></span
									><span>{w.name}</span></a
								>{/each}
						</div>
					{/if}
					{#if data.hasState}
						<div style="display: flex; gap: 12px; flex-wrap: wrap">
							{#each data.states as x, i (i)}<span class="m-flag m-flag--kind_draft"
									><span class="m-icon m-icon--sm" aria-hidden="true">{x.icon}</span>{x.label}</span
								>{/each}
						</div>
					{/if}
				</div>
			</header>

			<nav class="m-tabs__root" aria-label="Profile">
				{#each data.tabs as t (t.label)}<button
						class="m-tabs__tab tabbtn"
						type="button"
						aria-current={t.current}
						><span class="m-icon" aria-hidden="true">{t.icon}</span>{t.label}<span
							style="font-weight: 500; color: var(--ink-muted)">{t.count}</span
						></button
					>{/each}
			</nav>

			{#if data.is.characters}
				<div
					class="m-vt-pane"
					style="display: grid; grid-template-columns: repeat(auto-fill, minmax(min(300px, 100%), 1fr)); gap: 16px"
				>
					{#each data.characters as c (c.name)}
						<article
							style="display: flex; flex-direction: column; gap: 12px; padding: 16px; background-color: var(--surface-raised); border: 1px solid var(--line); border-radius: var(--radius-lg); corner-shape: var(--corner-shape)"
						>
							<div style="display: flex; gap: 12px; align-items: center">
								<span class="m-avatar m-avatar--kind_character m-avatar--size_lg" aria-hidden="true"
									><img src={c.portrait} alt="" /></span
								>
								<span style="display: grid; gap: 2px; min-width: 0"
									><span style="font: 600 17px/22px var(--font-ui)">{c.name}</span
									>{#if c.hasKind}<span style="font-size: 13px; color: var(--ink-muted)"
											>{c.kind}</span
										>{/if}</span
								>
							</div>
							<div style="display: flex; flex-direction: column; border-top: 1px solid var(--line)">
								{#each c.cards as k, i (i)}
									<a
										class="prow"
										href={k.href}
										style="display: grid; grid-template-columns: auto minmax(0, 1fr) auto; gap: 10px; align-items: center; padding: 10px 8px; margin-inline: -8px; border-radius: var(--radius-md)"
									>
										<span class="m-avatar m-avatar--kind_world m-avatar--size_sm" aria-hidden="true"
											><img src={k.mark} alt="" /></span
										>
										<span style="display: grid; min-width: 0"
											><span style="font-weight: 600">{k.world}</span><span
												style="font-size: 12.5px; color: var(--ink-muted)">{k.note}</span
											></span
										>
										<span class={k.flagClass}
											><span class="m-icon m-icon--sm" aria-hidden="true">{k.flagIcon}</span
											>{k.flag}</span
										>
									</a>
								{/each}
							</div>
						</article>
					{/each}
				</div>
			{/if}

			{#if data.is.worlds}
				<div class="m-vt-pane" style="display: flex; flex-direction: column; gap: 24px">
					{#if data.own}<a
							class="m-button m-button--variant_tonal"
							href={resolve('/new-world')}
							style="align-self: flex-start"
							><span class="m-icon" aria-hidden="true">add</span><span>Start a World</span></a
						>{/if}
					{#each data.worldGroups as g (g.title)}
						<section style="display: flex; flex-direction: column">
							<h2 style="margin: 0 0 6px; font: 600 15px/20px var(--font-ui)">{g.title}</h2>
							{#each g.items as w (w.name)}
								<a
									class="prow"
									href={w.href}
									style="display: grid; grid-template-columns: auto minmax(0, 1fr) auto; gap: 14px; align-items: center; padding: 12px 8px; margin-inline: -8px; border-bottom: 1px solid var(--line)"
								>
									<span class="m-avatar m-avatar--kind_world m-avatar--size_md" aria-hidden="true"
										><img src={w.mark} alt="" /></span
									>
									<span style="display: grid; gap: 2px; min-width: 0"
										><span style="font-weight: 600; font-size: 15px">{w.name}</span><span
											style="display: flex; flex-wrap: wrap; gap: 4px 14px; font-size: 13px; color: var(--ink-muted)"
											><span class="m-meta"
												><span class="m-icon m-icon--sm" aria-hidden="true">{w.roleIcon}</span><span
													>{w.role}</span
												></span
											><span class="m-meta"
												><span class="m-icon m-icon--sm" aria-hidden="true">{w.stateIcon}</span
												><span>{w.state}</span></span
											></span
										></span
									>
									<span style="font-size: 13px; color: var(--ink-muted)">{w.since}</span>
								</a>
							{/each}
						</section>
					{/each}
				</div>
			{/if}

			{#if data.is.posts}
				<div class="m-vt-pane" style="display: flex; flex-direction: column">
					{#each data.posts as p, i (i)}
						<a
							class="prow"
							href={p.href}
							style="display: grid; grid-template-columns: 40px minmax(0, 1fr); gap: 4px 14px; padding: 14px 8px; margin-inline: -8px; border-bottom: 1px solid var(--line); border-radius: var(--radius-md)"
						>
							<span class={p.avatarClass} aria-hidden="true" style="grid-row: span 3; {p.hue}"
								>{#if p.hasPortrait}<img src={p.portrait} alt="" />{/if}{#if p.noPortrait}<span
										aria-hidden="true">{p.initials}</span
									>{/if}</span
							>
							<span style="display: flex; flex-wrap: wrap; gap: 4px 10px; align-items: baseline"
								><span style="font-weight: 600">{p.speaker}</span><span
									style="font-size: 13px; color: var(--ink-muted)">{p.laneLabel}</span
								></span
							>
							<span
								style="font-family: var(--font-reading); font-size: 16px; line-height: 1.5; overflow: hidden; display: -webkit-box; -webkit-line-clamp: 2; -webkit-box-orient: vertical"
								>{p.text}</span
							>
							<span
								style="display: flex; flex-wrap: wrap; gap: 4px 14px; font-size: 13px; color: var(--ink-muted)"
								><span class="m-meta"
									><span class="m-avatar m-avatar--kind_world m-avatar--size_xs" aria-hidden="true"
										><img src={p.mark} alt="" /></span
									><span>{p.world}</span></span
								><span class="m-meta"
									><span class="m-icon m-icon--sm" aria-hidden="true">forum</span><span
										>{p.scene}</span
									></span
								><span class="m-meta"
									><span class="m-icon m-icon--sm" aria-hidden="true">schedule</span><span
										>{p.when}</span
									></span
								></span
							>
						</a>
					{/each}
					<p style="margin: 16px 0 0; font-size: 13.5px; color: var(--ink-muted)">
						Posts in scenes you can’t read don’t appear here.
					</p>
				</div>
			{/if}
		</div>
	</main>
	{#if data.reporting}
		<ReportDialog
			kind="account"
			world=""
			account={data.person.display}
			handle={data.person.handle}
			initials={data.person.initials}
			hue={data.person.hue}
		/>
	{/if}
	{#if data.hasToast}
		<ol class="m-toast__region" aria-label="Notifications">
			<li class="m-toast__root" role="status" data-tone="neutral">
				<span class="m-icon m-toast__icon" aria-hidden="true">{data.toastIcon}</span><span
					class="m-toast__message">{data.toastText}</span
				><button
					class="m-icon-button m-icon-button--variant_standard m-icon-button--size_sm m-toast__close"
					type="button"
					aria-label="Dismiss"><span class="m-icon" aria-hidden="true">close</span></button
				>
			</li>
		</ol>
	{/if}
</div>

<style>
	.prow {
		text-decoration: none;
		color: var(--ink);
	}
	.prow:hover {
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
