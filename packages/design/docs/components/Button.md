# Button

The action control: filled for the one thing a view is for, tonal for the rest, outline for secondary choices, text inside dense content.

**Use** a verb in sentence case ("Post", "Save draft"). One filled button per view. **Avoid** icon-only buttons for primary actions, and links styled as buttons for navigation.

- Props: `variant` (`filled`, `tonal`, `outline`, `text`), `tone` (`accent`, `danger`, `neutral`), `size` (`sm`, `md`), `icon`, `trailingIcon`, `disabled`.
- States: hover and pressed are state layers (8% and 14% of the content colour); pressing morphs the corner radius to 55% on the spatial spring; disabled uses `surface-quiet` and `ink-muted` with no shadow.
- Worlds change fill colour, corner shape and edge light. Labels, order and meaning stay Motet's.
- Accessibility: native `button`; 40px tall (44px on coarse pointers, also for `sm` through an invisible hit area); focus ring `focus`.
