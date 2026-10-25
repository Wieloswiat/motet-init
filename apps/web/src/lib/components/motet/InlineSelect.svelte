<!--
	A choice inside a sentence: "While I'm away, GMs may move [Lady Maren and
	Anwen] along." Several values read as words. This port shows the summary
	only; the listbox comes with the screens' behaviour.
-->
<script lang="ts">
	interface Option {
		value: string;
		label: string;
		/** The word the sentence uses, such as "Lady Maren" for "Lady Maren of Ost". */
		short: string | null;
	}

	let {
		label,
		options,
		value,
		pid,
		multiple = true,
		all,
		none
	}: {
		label: string;
		options: readonly Option[];
		/** The picked values; a single choice holds one. */
		value: readonly string[];
		pid: string;
		multiple?: boolean;
		/** The words for every option picked. */
		all: string;
		/** The words for none picked. */
		none: string;
	} = $props();

	const word = (picked: string): string => {
		const option = options.find((o) => o.value === picked);

		return option ? (option.short ?? option.label) : picked;
	};

	const summary = $derived.by(() => {
		if (value.length === 0) {
			return none;
		}

		if (!multiple) {
			return word(value[0] ?? '');
		}

		if (value.length === options.length) {
			return all;
		}

		const names = value.map(word);
		const [first = '', ...rest] = names;

		return new Intl.ListFormat('en', { style: 'long', type: 'conjunction' }).format(
			names.length > 2 ? [first, `${rest.length} more`] : names
		);
	});
</script>

<span class="m-inline-select__root"
	><button
		class="m-inline-select__trigger"
		type="button"
		aria-haspopup="listbox"
		aria-expanded="false"
		aria-controls="{pid}-list"
		aria-label="{label}: {summary}"
		><span class="m-inline-select__value">{summary}</span><span
			class="m-icon m-inline-select__indicator"
			aria-hidden="true">expand_more</span
		></button
	></span
>
