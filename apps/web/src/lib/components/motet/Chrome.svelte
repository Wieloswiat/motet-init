<!--
	Motet's chrome: two corner islands. Home and the World switcher on the
	left; Search, the Inbox and the account on the right. Inside an island
	every World variable is reset to Motet's, so no World can restyle it.
-->
<script lang="ts">
	import { resolve } from '$app/paths';
	import { accounts, type AccountKey } from '#lib/demo/accounts.js';
	import { marks } from '#lib/demo/art.js';

	let {
		world,
		account,
		inbox = 3,
		current
	}: {
		/** The World this page belongs to, or none on Motet's own pages. */
		world?: keyof typeof marks;
		account: AccountKey;
		/** Inbox items that need the reader. */
		inbox?: number;
		/** The Motet page this is, when the chrome links to it. */
		current?: 'inbox';
	} = $props();

	const person = $derived(accounts[account]);

	let open: 'world' | 'account' | null = $state(null);

	function toggle(menu: 'world' | 'account'): void {
		open = open === menu ? null : menu;
	}
</script>

<div class="m-chrome__root">
	<nav class="m-chrome__island" aria-label="Motet">
		<a class="m-chrome__home" href={resolve('/')} aria-label="Motet home">motet</a>
		<span class="m-chrome__sep" aria-hidden="true"></span>
		<div style="position: relative; display: flex">
			<button
				class="m-chrome__world"
				type="button"
				aria-haspopup="menu"
				aria-expanded={open === 'world'}
				aria-label={world ? 'Current World: The Gilded Reach. Switch World' : 'Switch World'}
				onclick={() => toggle('world')}
			>
				{#if world}
					<span class="m-avatar m-avatar--kind_world m-avatar--size_sm" aria-hidden="true"
						><img src={marks[world]} alt="" /></span
					>
				{:else}
					<span class="m-icon" aria-hidden="true" style="font-size: 22px">explore</span>
				{/if}
				<span class="m-icon" aria-hidden="true">unfold_more</span>
			</button>
			{#if open === 'world'}
				<div
					class="m-menu__root"
					role="menu"
					aria-label="Switch World"
					style="position: absolute; top: 48px; left: -6px; z-index: 2; min-width: 300px"
				>
					<div
						class="m-menu__label"
						style="padding: 6px 10px 4px; font: 600 12.5px/16px var(--font-ui); color: var(--menu-ink-muted, var(--ink-muted))"
					>
						Your Worlds
					</div>
					{#each person.worlds as w (w.key)}
						<a
							class="m-menu__item"
							role="menuitem"
							href={w.href}
							aria-current={w.key === world}
							style="min-height: 48px"
						>
							<span class="m-avatar m-avatar--kind_world m-avatar--size_sm" aria-hidden="true"
								><img src={marks[w.key]} alt="" /></span
							>
							<span style="display: grid; min-width: 0; flex: 1">
								<span style="font-weight: 600">{w.name}</span>
								<span
									style="font-size: 12.5px; line-height: 16px; color: var(--menu-ink-muted, var(--ink-muted))"
									>{w.as}</span
								>
							</span>
							{#if w.turn}
								<span class="m-flag m-flag--kind_turn"
									><span class="m-icon m-icon--sm" aria-hidden="true">front_hand</span>Your turn</span
								>
							{/if}
						</a>
					{/each}
					<hr class="m-menu__separator" />
					<a class="m-menu__item" role="menuitem" href={resolve('/search')}
						><span class="m-icon" aria-hidden="true">explore</span>Find a World</a
					>
					<a class="m-menu__item" role="menuitem" href={resolve('/new-world')}
						><span class="m-icon" aria-hidden="true">add</span>Start a World</a
					>
				</div>
			{/if}
		</div>
	</nav>
	<div class="m-chrome__island" role="toolbar" aria-label="Account and activity">
		<a
			class="m-chrome__search"
			href={resolve('/search')}
			aria-label="Search"
			style="text-decoration: none"
			><span class="m-icon" aria-hidden="true">search</span><span class="m-chrome__label"
				>Search</span
			><span class="m-chrome__kbd">/</span></a
		>
		<a
			class="m-icon-button m-icon-button--variant_standard"
			href={resolve('/inbox')}
			aria-label={inbox > 0 ? `Inbox, ${inbox} items need you` : 'Inbox'}
			aria-current={current === 'inbox' ? 'page' : undefined}
			style="text-decoration: none"
			><span class="m-icon" aria-hidden="true">inbox</span>{#if inbox > 0}<span
					class="m-icon-button__count"
					aria-hidden="true">{inbox}</span
				>{/if}</a
		>
		<div style="position: relative; display: flex">
			<button
				class="m-icon-button m-icon-button--variant_standard"
				type="button"
				aria-haspopup="menu"
				aria-expanded={open === 'account'}
				aria-label={`Account @${person.handle}`}
				onclick={() => toggle('account')}
			>
				<span
					class="m-avatar m-avatar--kind_account m-avatar--size_sm"
					aria-hidden="true"
					style:--avatar-hue={person.hue}><span aria-hidden="true">{person.initials}</span></span
				>
			</button>
			{#if open === 'account'}
				<div
					class="m-menu__root"
					role="menu"
					aria-label="Account"
					style="position: absolute; top: 48px; right: -6px; z-index: 2; min-width: 260px"
				>
					<div style="display: flex; gap: 10px; align-items: center; padding: 8px 10px 10px">
						<span
							class="m-avatar m-avatar--kind_account m-avatar--size_md"
							aria-hidden="true"
							style:--avatar-hue={person.hue}
							><span aria-hidden="true">{person.initials}</span></span
						>
						<span style="display: grid; min-width: 0">
							<span style="font-weight: 600">{person.display}</span>
							<span
								style="font-size: 12.5px; line-height: 16px; color: var(--menu-ink-muted, var(--ink-muted))"
								>@{person.handle}</span
							>
						</span>
					</div>
					<hr class="m-menu__separator" />
					<a
						class="m-menu__item"
						role="menuitem"
						href={resolve('/profile/[handle]', { handle: person.handle })}
						><span class="m-icon" aria-hidden="true">person</span>Profile</a
					>
					<a class="m-menu__item" role="menuitem" href={resolve('/settings')}
						><span class="m-icon" aria-hidden="true">settings</span>Settings</a
					>
				</div>
			{/if}
		</div>
	</div>
</div>
