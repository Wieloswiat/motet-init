<script lang="ts">
	import { resolve } from '$app/paths';
	import Chrome from '#lib/components/motet/Chrome.svelte';
	import InlineSelect from '#lib/components/motet/InlineSelect.svelte';
	import WorldHeader from '#lib/components/motet/WorldHeader.svelte';
	import { world } from '#lib/motet/world.js';
	import type { WorldSettingsData } from '#lib/demo/screens/world-settings.js';

	let { data }: { data: WorldSettingsData } = $props();
</script>

<svelte:head>
	<title>Your settings in this World</title>
</svelte:head>

<div {@attach world(data.worldRef)} class="m-world__root">
	<Chrome world="vellum" account="iga" inbox={data.inboxCount} />
	<WorldHeader current="Settings" size="slim" role="player" />
	<main class="m-world__page" style="padding-top: 28px">
		<div style="max-width: 760px; display: flex; flex-direction: column; gap: 36px">
			<h1 class="h-set" style="font-size: calc(32px * var(--display-adjust, 1))">
				Your settings in The Gilded Reach
			</h1>

			<section
				class="m-vt"
				aria-labelledby="s-away"
				style="--vt: m-ws-away; display: flex; flex-direction: column; gap: 4px"
			>
				<div class="setrow" style="border-bottom: 0; padding-top: 0">
					<div style="display: grid; gap: 2px">
						<h2 id="s-away" class="h-set">Away</h2>
						<span style="font-size: 14px; color: var(--ink-muted)"
							>The table sees it next to your name, and you are skipped in posting order</span
						>
					</div>
					<button
						class="m-switch__root"
						type="button"
						role="switch"
						aria-checked={data.awayAttr}
						aria-labelledby="s-away"><span class="m-switch__thumb"></span></button
					>
				</div>
				{#if data.away}
					<div
						style="display: flex; flex-direction: column; gap: 16px; padding: 16px; background-color: var(--surface-quiet); border-radius: var(--radius-lg)"
					>
						<div class="m-field__root" style="width: 200px">
							<label class="m-field__label" for="until">Until</label>
							<div class="m-input__root">
								<input class="m-input__control" id="until" type="text" value="March 27" /><span
									class="m-input__addon"
									data-side="end"
									aria-hidden="true"
									><span class="m-icon" aria-hidden="true">calendar_month</span></span
								>
							</div>
						</div>
						<div class="sentence" style="font-size: 15px; line-height: 28px">
							While I’m away, GMs may move <InlineSelect
								label="Characters GMs may move"
								options={data.myChars}
								value={data.moveChars}
								multiple
								all="all my characters"
								none="none of my characters"
								pid="move"
							/> along.
						</div>
						<div class="m-textarea__root">
							<label class="m-field__label" for="awaynote">Note to the table</label><textarea
								class="m-textarea__control m-textarea__control--face_ui"
								id="awaynote"
								rows="2"
								style="--rows: 2"
								>Travelling for work, back on the 27th. Maren can sulk quietly in the Archive.</textarea
							>
						</div>
					</div>
				{/if}
			</section>

			<section
				class="m-vt"
				aria-labelledby="s-notify"
				style="--vt: m-ws-notify; display: flex; flex-direction: column; gap: 8px"
			>
				<h2 id="s-notify" class="h-set">Notifications</h2>
				<div class="setrow">
					<div style="display: grid; gap: 2px">
						<span style="font-weight: 600">Scenes you play in</span><span
							style="font-size: 13px; color: var(--ink-muted)">Your default is All posts</span
						>
					</div>
					<div class="m-segmented__root" role="radiogroup" aria-label="Scenes you play in">
						{#each data.levels as o (o.label)}<button
								class="m-segmented__item"
								type="button"
								role="radio"
								aria-checked={o.checked}>{o.label}</button
							>{/each}
					</div>
				</div>
				<div class="setrow">
					<div class="sentence" style="font-size: 15px; line-height: 28px">
						Also tell me about <InlineSelect
							label="Also tell me about"
							options={data.extraOpts}
							value={data.extraVals}
							multiple
							all="everything new here"
							none="nothing else"
							pid="extra"
						/>.
					</div>
				</div>
				<div style="display: flex; flex-direction: column; gap: 2px; padding-top: 8px">
					<span
						style="font: 600 13px/18px var(--font-ui); color: var(--ink-muted); margin-bottom: 4px"
						>Set differently</span
					>
					{#each data.overrides as o (o.id)}
						<div class="setrow" style="padding-block: 10px">
							<span style="display: grid; gap: 2px"
								><span style="font-weight: 600">{o.scene}</span><span
									style="display: inline-flex; gap: 6px; align-items: center; font-size: 13px; color: var(--ink-muted)"
									><span class="m-icon m-icon--sm" aria-hidden="true">{o.icon}</span>{o.level}</span
								></span
							>
							<button class="m-button m-button--variant_text m-button--size_sm" type="button"
								><span>Use World setting</span></button
							>
						</div>
					{/each}
					{#if data.noOverrides}<p style="margin: 0; font-size: 14px; color: var(--ink-muted)">
							Every scene follows the setting above.
						</p>{/if}
				</div>
			</section>

			<section
				class="m-vt"
				aria-labelledby="s-look"
				style="--vt: m-ws-look; display: flex; flex-direction: column; gap: 4px"
			>
				<h2 id="s-look" class="h-set">How this World looks to you</h2>
				<div class="setrow">
					<div style="display: grid; gap: 2px"><span style="font-weight: 600">Mode</span></div>
					<div class="m-segmented__root" role="radiogroup" aria-label="Mode">
						{#each data.modes as o (o.label)}<button
								class="m-segmented__item"
								type="button"
								role="radio"
								aria-checked={o.checked}
								><span class="m-icon" aria-hidden="true">{o.icon}</span>{o.label}</button
							>{/each}
					</div>
				</div>
				<div class="setrow">
					<div style="display: grid; gap: 2px">
						<span style="font-weight: 600">Use Motet appearance here</span><span
							style="font-size: 13px; color: var(--ink-muted)"
							>Keeps the World’s name and mark, drops its colours, type and art</span
						>
					</div>
					<button
						class="m-switch__root"
						type="button"
						role="switch"
						aria-checked={data.plainAttr}
						aria-label="Use Motet appearance here"><span class="m-switch__thumb"></span></button
					>
				</div>
				<div class="setrow">
					<div style="display: grid; gap: 2px">
						<span style="font-weight: 600">Decoration</span><span
							style="font-size: 13px; color: var(--ink-muted)">Ornaments, frames and art</span
						>
					</div>
					<button
						class="m-switch__root"
						type="button"
						role="switch"
						aria-checked={data.decoAttr}
						aria-label="Decoration"><span class="m-switch__thumb"></span></button
					>
				</div>
			</section>

			<section
				class="m-vt"
				aria-labelledby="s-chars"
				style="--vt: m-ws-chars; display: flex; flex-direction: column; gap: 12px"
			>
				<h2 id="s-chars" class="h-set">Your characters</h2>
				{#each data.myCards as c (c.label)}
					<div
						style="display: flex; gap: 14px; align-items: center; padding: 12px; border: 1px solid var(--line); border-radius: var(--radius-lg); flex-wrap: wrap"
					>
						<span class="m-avatar m-avatar--kind_character m-avatar--size_lg" aria-hidden="true"
							><img src={c.portrait} alt="" /></span
						>
						<span style="display: grid; flex: 1 1 200px"
							><span class="m-post__name" style="font-size: calc(18px * var(--name-adjust, 1))"
								>{c.label}</span
							><span style="font-size: 13px; color: var(--ink-muted)">{c.stats}</span></span
						>
						<a
							class="m-button m-button--variant_outline m-button--size_sm"
							href={resolve('/w/[world]/characters/[character]', {
								world: 'gilded-reach',
								character: 'lady-maren'
							})}><span class="m-icon" aria-hidden="true">edit</span><span>Edit card</span></a
						>
						<button class="m-button m-button--variant_text m-button--size_sm" type="button"
							><span class="m-icon" aria-hidden="true">archive</span><span>Retire</span></button
						>
					</div>
				{/each}
				<a
					class="m-button m-button--variant_text"
					href={resolve('/w/[world]/apply', { world: 'gilded-reach' })}
					style="align-self: flex-start; margin-inline-start: -8px"
					><span class="m-icon" aria-hidden="true">person_add</span><span
						>Apply with another character</span
					></a
				>
			</section>

			<div class="m-alert__root m-alert__root--tone_danger m-alert__root--variant_inline">
				<span class="m-icon m-alert__icon m-alert__icon--tone_danger" aria-hidden="true"
					>logout</span
				>
				<div class="m-alert__content">
					<p class="m-alert__title">Leave The Gilded Reach</p>
					<div class="m-alert__description">
						Your posts stay in their scenes and stay yours. Lady Maren and Anwen are retired and
						can’t be played here again without a new application.
					</div>
					<div class="m-alert__actions">
						<button
							class="m-button m-button--variant_outline m-button--tone_danger m-button--size_sm"
							type="button"><span>Leave World</span></button
						>
					</div>
				</div>
				<span></span>
			</div>
		</div>
	</main>
</div>

<style>
	.h-set {
		margin: 0;
		font-family: var(--font-display);
		font-weight: var(--display-weight);
		font-size: calc(24px * var(--display-adjust, 1));
		line-height: 1.15;
		text-transform: var(--display-case);
		letter-spacing: var(--display-tracking);
	}
	.setrow {
		display: flex;
		gap: 16px;
		align-items: center;
		justify-content: space-between;
		flex-wrap: wrap;
		padding-block: 14px;
		border-bottom: 1px solid var(--line);
	}
	.setrow:last-child {
		border-bottom: 0;
	}
</style>
