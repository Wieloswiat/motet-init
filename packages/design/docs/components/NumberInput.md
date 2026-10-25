# NumberInput

A number with decrement and increment buttons and an optional unit.

- `NumberInput({ label, value, min, max, step, unit })`.
- `role=spinbutton` with `aria-valuenow`, min and max; the unit is part of `aria-valuetext` ("3 days").
- Keys: Arrow Up/Down step, Page Up/Down step ten times, Home/End jump to the limits. Typed values are clamped on blur or Enter.
- The buttons are out of the tab order and keep focus in the field; they disable at the limits.
