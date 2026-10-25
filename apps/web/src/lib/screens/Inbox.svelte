<script lang="ts">
	import { resolve } from '$app/paths';
	import Chrome from '#lib/components/motet/Chrome.svelte';
	import { world } from '#lib/motet/world.js';
	import type { InboxData } from '#lib/demo/screens/inbox.js';

	let { data }: { data: InboxData } = $props();
</script>

<svelte:head>
	<title>Inbox</title>
</svelte:head>

<div {@attach world(data.pageRef)} class="m-page__root">
	<Chrome account="iga" inbox={data.needsCount} current="inbox" />
	<main class="m-page__main">
		<div style="display: flex; flex-wrap: wrap; gap: 24px 32px; align-items: flex-start">
			<nav
				class="m-page__panel sidepanel inbox-filters"
				aria-label="Inbox filters"
				style="flex: 1 1 220px"
			>
				{#each data.filters as f (f.label)}
					<button class="m-page__navItem" type="button" aria-current={f.current}>
						<span class="m-icon" aria-hidden="true">{f.icon}</span>
						<span>{f.label}</span>
						{#if f.hasCount}<span class="m-count">{f.count}</span>{/if}
					</button>
				{/each}
				<hr style="width: 100%; border: 0; border-top: 1px solid var(--line); margin: 6px 0" />
				<a class="m-page__navItem" href={resolve('/settings')}
					><span class="m-icon" aria-hidden="true">tune</span><span>Notification settings</span></a
				>
			</nav>

			<section
				aria-labelledby="h-inbox"
				style="flex: 999 1 560px; min-width: 0; display: flex; flex-direction: column; gap: 20px"
			>
				<div
					style="display: flex; align-items: center; justify-content: space-between; gap: 12px; flex-wrap: wrap"
				>
					<h1 id="h-inbox" style="margin: 0; font: 600 26px/32px var(--font-ui)">{data.heading}</h1>
					<button class="m-button m-button--variant_text" type="button" disabled={data.noneUnread}
						><span class="m-icon" aria-hidden="true">done_all</span><span>Mark all as read</span
						></button
					>
				</div>

				{#each data.groups as g (g.title)}
					<div style="display: flex; flex-direction: column">
						<h2
							style="margin: 0 0 4px; padding-inline: 14px; font: 600 13px/18px var(--font-ui); color: var(--ink-muted)"
						>
							{g.title}
						</h2>
						{#each g.items as it (it.id)}
							<div
								class="row m-vt m-vt-box"
								style="--vt: {it.vtn}; position: relative; display: grid; grid-template-columns: minmax(0, 1fr) auto; align-items: start; box-shadow: inset 0 -1px 0 var(--line)"
							>
								<a class={it.rootClass} href={it.href} style="box-shadow: none">
									<span class="m-inbox-item__world"
										><span
											class="m-avatar m-avatar--kind_world m-avatar--size_sm"
											aria-hidden="true"><img src={it.mark} alt="" /></span
										></span
									>
									<span class="m-inbox-item__main">
										<span class={it.kindClass}
											><span class="m-icon m-icon--sm" aria-hidden="true">{it.kindIcon}</span
											>{it.kindLabel}<span class="m-inbox-item__in">{it.worldName}</span></span
										>
										<span class={it.titleClass}>{it.title}</span>
										<span class="m-inbox-item__excerpt">{it.excerpt}</span>
									</span>
									<span class="m-inbox-item__side"><time>{it.time}</time></span>
								</a>
								<div style="display: flex; gap: 6px; align-items: center; padding: 12px 10px 0 0">
									<span class="m-role" title={it.roleTip}
										><span class="m-icon" aria-hidden="true">{it.roleIcon}</span>{it.role}</span
									>
									<button
										class="m-icon-button m-icon-button--variant_standard m-icon-button--size_sm"
										type="button"
										aria-haspopup="menu"
										aria-expanded={it.menuOpen}
										aria-label={it.levelTip}
										title={it.levelTip}
										><span class="m-icon" aria-hidden="true">{it.levelIcon}</span></button
									>
								</div>
								{#if it.menuOpen}
									<div
										class="m-menu__root"
										role="menu"
										aria-label="Notifications for this thread"
										style="position: absolute; right: 8px; top: 48px; z-index: 5; width: 300px"
									>
										<div
											style="padding: 6px 10px 4px; font: 600 12.5px/16px var(--font-ui); color: var(--menu-ink-muted, var(--ink-muted))"
										>
											{it.menuTitle}
										</div>
										{#each it.levels as lv (lv.label)}
											<button
												class="m-menu__item"
												type="button"
												role="menuitemradio"
												aria-checked={lv.checked}
												style="align-items: flex-start; padding-block: 8px"
											>
												<span class="m-icon" aria-hidden="true">{lv.icon}</span>
												<span style="display: grid; flex: 1"
													><span>{lv.label}</span><span
														style="font-size: 12.5px; line-height: 16px; color: var(--menu-ink-muted, var(--ink-muted))"
														>{lv.hint}</span
													></span
												>
												{#if lv.on}<span class="m-icon" aria-hidden="true">check</span>{/if}
											</button>
										{/each}
										<hr class="m-menu__separator" />
										<button class="m-menu__item" type="button" role="menuitem"
											><span class="m-icon" aria-hidden="true">{it.readIcon}</span
											>{it.readLabel}</button
										>
									</div>
								{/if}
							</div>
						{/each}
					</div>
				{/each}
				{#if data.empty}
					<p style="margin: 0; padding: 32px 14px; color: var(--ink-muted); font-size: 15px">
						Nothing here.
					</p>
				{/if}
			</section>
		</div>
	</main>
</div>

<style>
	@media (min-width: 900px) {
		.sidepanel {
			max-width: 280px;
		}
	}
	@media (max-width: 899px) {
		.inbox-filters {
			flex-direction: row !important;
			flex-basis: 100% !important;
			overflow-x: auto;
			scrollbar-width: none;
			padding: 4px !important;
			gap: 4px;
		}
		.inbox-filters .m-page__navItem {
			flex: 0 0 auto;
			width: auto !important;
			white-space: nowrap;
			gap: 8px;
		}
		.inbox-filters .m-page__navItem > .m-count {
			margin-inline-start: 4px;
		}
		.inbox-filters hr {
			display: none;
		}
	}
	.row:hover {
		background-color: color-mix(in oklab, var(--ink) 4%, transparent);
	}
</style>
