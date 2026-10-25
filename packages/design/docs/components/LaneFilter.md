# LaneFilter

Which lanes a Scene shows: in character, out of character, or both.

- `LaneFilter({ value: 'both' | 'ic' | 'ooc', policy, labels })`; emits `change` with `{ value }`. `setValue(v)` sets it without emitting.
- Two toggle buttons (`aria-pressed`) in one split button, with the lane named in each tooltip and accessible name; a polite live region says what is now shown.
- One lane is always on. With one lane on, clicking it switches to the other and clicking the other shows both. With both on, `Motet.lanePolicy.fromBoth` decides: `'unselect'` (default) hides the clicked lane, `'solo'` keeps only the clicked lane. `Motet.nextLane(current, clicked, policy)` is the whole rule, so a product can change it in one place.
- On a Scene page the filter sits in the scene header on narrow screens and at the top of the sticky rail on wide ones; hidden posts take the dividers that only separated them.
