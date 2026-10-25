<!--
	Picks one language, or several as tags. This port shows the chosen value
	only; the filtering listbox (matching that ignores case and diacritics)
	comes with Ark's Combobox when the screens gain behaviour.
-->
<script lang="ts" module>
	export const languages = [
		{ code: 'en', name: 'English', english: '' },
		{ code: 'pl', name: 'Polski', english: 'Polish' },
		{ code: 'de', name: 'Deutsch', english: 'German' },
		{ code: 'fr', name: 'Français', english: 'French' },
		{ code: 'es', name: 'Español', english: 'Spanish' },
		{ code: 'it', name: 'Italiano', english: 'Italian' },
		{ code: 'pt', name: 'Português', english: 'Portuguese' },
		{ code: 'cs', name: 'Čeština', english: 'Czech' },
		{ code: 'sk', name: 'Slovenčina', english: 'Slovak' },
		{ code: 'uk', name: 'Українська', english: 'Ukrainian' },
		{ code: 'lt', name: 'Lietuvių', english: 'Lithuanian' },
		{ code: 'nl', name: 'Nederlands', english: 'Dutch' },
		{ code: 'sv', name: 'Svenska', english: 'Swedish' },
		{ code: 'ja', name: '日本語', english: 'Japanese' }
	];
</script>

<script lang="ts">
	let {
		label,
		value,
		pid,
		multiple = true,
		hideLabel = false,
		placeholder,
		help = '',
		leading = 'translate'
	}: {
		label: string;
		/** Language codes; a single picker shows the first. */
		value: readonly string[];
		/** Prefix for the element ids, unique on the page. */
		pid: string;
		multiple?: boolean;
		hideLabel?: boolean;
		placeholder?: string;
		help?: string;
		/** The icon before a single picker's text. */
		leading?: string;
	} = $props();

	const nameOf = (code: string): string =>
		languages.find((language) => language.code === code)?.name ?? code;

	const helpId = $derived(help ? `${pid}-help` : undefined);
</script>

<div class="m-field__root" style="position: relative; min-width: 0">
	<label class={['m-field__label', hideLabel && 'm-visually-hidden']} for="{pid}-input"
		>{label}</label
	>
	{#if multiple}
		<div class="m-tags__root">
			{#each value as code (code)}
				<span class="m-tags__tag"
					><span>{nameOf(code)}</span><button
						class="m-tags__remove"
						type="button"
						tabindex="-1"
						aria-label="Remove {nameOf(code)}"
						><span class="m-icon" aria-hidden="true">close</span></button
					></span
				>
			{/each}
			<input
				class="m-tags__control"
				id="{pid}-input"
				type="text"
				role="combobox"
				aria-autocomplete="list"
				aria-expanded="false"
				aria-controls="{pid}-list"
				aria-describedby={helpId}
				placeholder={value.length ? '' : (placeholder ?? 'Add a language')}
				autocomplete="off"
				spellcheck="false"
				style={value.length ? 'min-width: 6ch; flex-basis: 6ch' : ''}
			/>
		</div>
	{:else}
		<div class="m-input__root">
			<span class="m-input__addon" data-side="start" aria-hidden="true"
				><span class="m-icon" aria-hidden="true">{leading}</span></span
			>
			<input
				class="m-input__control"
				id="{pid}-input"
				type="text"
				role="combobox"
				aria-autocomplete="list"
				aria-expanded="false"
				aria-controls="{pid}-list"
				aria-describedby={helpId}
				value={value[0] === undefined ? '' : nameOf(value[0])}
				placeholder={placeholder ?? 'Pick a language'}
				autocomplete="off"
				spellcheck="false"
			/>
			<span class="m-input__action"
				><button
					class="m-icon-button m-icon-button--variant_standard"
					type="button"
					tabindex="-1"
					aria-label="Show languages"
					aria-expanded="false"><span class="m-icon" aria-hidden="true">expand_more</span></button
				></span
			>
		</div>
	{/if}
	{#if help}
		<div class="m-field__help" id={helpId}>{help}</div>
	{/if}
</div>
