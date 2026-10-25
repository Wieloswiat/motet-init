# TagsInput

A short list of values typed or picked from suggestions: content notes, scene tags.

- `TagsInput({ label, value, suggestions, placeholder, max, help })`; emits `change` with the full list.
- Enter or a comma adds; pasting "a, b, c" adds three; duplicates are ignored without diacritics or case.
- Backspace on an empty field selects the last tag, a second Backspace removes it; Left and Right move between tags.
- Each tag's remove button is named ("Remove Violence"); additions and removals are announced.
