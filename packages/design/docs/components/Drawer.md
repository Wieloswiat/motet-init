# Drawer

A modal panel from the side, or a bottom sheet on phones, for secondary work that keeps its context.

- `Drawer({ title, content, footer })` returns a `dialog`; call `openModal()`.
- Native modal dialog: focus is contained, Escape closes, a click on the scrim closes. It slides in on the spatial spring.
- Use for character sheets, scene details and filters on small screens.
