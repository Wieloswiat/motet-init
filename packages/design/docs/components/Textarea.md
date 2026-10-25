# Textarea

Multi-line text that grows with its content up to `maxRows`, then scrolls.

- `Textarea({ label, help, value, rows, maxRows, maxLength, face: 'reading' | 'ui' })`.
- `reading` sets prose in the reader's reading face (bios, lore); `ui` is for notes and messages.
- Growth uses `field-sizing: content`, with a script fallback.
- The counter is always visible but only announced twice: when the last tenth starts and when the limit is passed. Going over turns the counter and border to `danger-ink` and sets `aria-invalid`; the text is never cut.
