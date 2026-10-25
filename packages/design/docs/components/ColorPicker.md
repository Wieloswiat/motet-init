# ColorPicker

Pick a colour in OKLCH with its gamut and its contrast duties in view. Used for World seeds and exact role colours in the theme creator.

- `ColorPicker({ label, value, help, placeholder, clearable, swatches, against, inline, footer })`. Emits `input` while dragging and `change` when a value is committed, both with `{ value, lch }`; `value` is `#rrggbb`, or `null` after clearing.
- `against: [{ label, hex, min }]` lists the grounds the colour must work on. Each prints its ratio with the word for its state ("needs 3:1"), and the plane greys out every colour that would miss one, so the passing region is the vivid one.
- The plane is lightness (up) by chroma (right) at one hue. Colours past sRGB are hatched and the edge is drawn; dragging stops at the edge. When a hue change pushes a colour past it, a note says how far chroma was limited.
- Keyboard: the plane's thumb is one slider; Up and Down move lightness, Left and Right chroma, Shift for larger steps. Hue is a native range. L, C and H can be typed, and hex can be typed or pasted into the field without opening the panel.
- `setValue(hex)`, `setAgainst(list)` and `setHelp(text, tone)` update it from outside without emitting. In a popup it closes on Escape and returns focus to the swatch.
