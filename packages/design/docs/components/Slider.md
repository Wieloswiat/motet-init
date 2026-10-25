# Slider

A value on a range, with the value always printed.

- `Slider({ label, min, max, step, value, unit, format, marks })`.
- A native range input: arrows, Page keys, Home and End all work, and `aria-valuetext` carries the formatted value.
- Use for settings where the exact number matters less than the feel (text size, art strength). Use NumberInput when people type exact values.
