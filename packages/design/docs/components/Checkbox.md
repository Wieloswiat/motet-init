# Checkbox

Independent on/off choices. In a group, a parent checkbox shows the mixed state when only some children are on.

**Use** for settings submitted with a form and for multi-select lists. **Use a Switch** for settings that apply at once.

- `Checkbox({ label, description, checked, indeterminate, disabled, name, value, onChange })`; `CheckboxGroup({ legend, help, allLabel, items, name })`.
- A native `input type=checkbox` inside its `label`, so the whole row toggles it. The box takes the World's corner up to 6px; the check is drawn in `on-accent`.
- Description text is linked with `aria-describedby`. Forced colours draw it with system colours.
