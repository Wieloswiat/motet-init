# Select

Pick one value from a list too long for radios.

- `Select({ label, value, options: [{ value, label, description, icon, group, disabled }], placeholder, name, help })`; emits `change` with `{ value, option }`.
- A select-only combobox (ARIA 1.2): a button with `role=combobox` and a listbox popup. Focus stays on the button; the active option is `aria-activedescendant`.
- Keys: Down, Up, Enter or Space open; arrows, Home and End move; Enter, Space or Tab choose; Escape closes; typing jumps to the first match, open or closed.
- The popup is a top-layer popover on the frosted material, as wide as the trigger at least.
