# Combobox

A text field that filters a list as you type.

- `Combobox({ label, options, value, placeholder, leading, allowCustom, empty })`; options may carry `group`, `icon`, `avatar`, `description` and `keywords`.
- Matching ignores case and diacritics, Polish ł included; the matched part is marked with an underline and weight, not colour alone.
- Keys: typing filters and opens; Down and Up move; Enter picks; Escape closes, or clears when closed; Alt+Down opens without moving.
- Without `allowCustom`, leaving the field restores the last picked value.
