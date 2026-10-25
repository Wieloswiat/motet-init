# Dialog

A modal confirmation for decisions that cannot be undone, over the smoke `scrim`.

The title states the consequence ("Discard this draft?"); the body adds only what the title can't, in one sentence; the safe action comes first, the destructive one is a filled danger button with a specific verb. Use native `dialog` with `showModal()`; focus returns to the trigger.
