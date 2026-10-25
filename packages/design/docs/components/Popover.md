# Popover

A small non-modal panel anchored to the control that opened it.

- `Popover({ trigger, title, content, footer, width, align })`.
- A top-layer `role=dialog`. Opening moves focus to its first field; Escape, the close button or a click outside close it and return focus to the trigger.
- Use for short tasks next to their context (roll dice, edit a field). Use Dialog for decisions that block, Drawer for longer work.
