# InlineSelect

A choice made inside a sentence, after Apple Shortcuts' parameters and human-readable filter bars: "While I'm away, GMs may move **Lady Maren and Anwen** along."

- Use it when the sentence explains a setting better than a label could, and the setting has a small set of values. Keep ordinary fields everywhere else; never chain several selects into a paragraph.
- `InlineSelect({ label, options, value, multiple, all, none, placeholder, groupLabel, maxNames })`; options may carry `short`, the word the sentence uses ("Lady Maren" for "Lady Maren of Ost"), plus `icon`, `avatar` and `description`.
- With `multiple`, the value is summarised in words: `none` when nothing is picked, `all` when everything is, otherwise a list in the reader's language ("Lady Maren and Anwen", "Lady Maren and 2 more"). `all` and `none` also appear as quick options at the top of the list.
- The trigger is a button with the summary as its text and `label` in its accessible name. Keys follow Select: Down, Enter or Space opens; Down and Up move; Enter or Space picks (toggles with `multiple`, which stays open); Escape closes and returns focus.
- The list is the shared listbox in the top layer; picked options carry a check, never colour alone.
