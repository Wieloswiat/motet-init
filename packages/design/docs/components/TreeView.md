# TreeView

A hierarchy a reader moves through with the keyboard: lore, an archive of arcs and scenes.

- `TreeView({ label, items: [{ id, label, icon, count, children, expanded, selected }] })`; emits `select` and `toggle`.
- One tab stop. Up and Down move through visible items; Right opens a branch or enters it; Left closes it or goes to the parent; Home and End; `*` opens all siblings; typing jumps to a name.
- Levels, positions and set sizes are exposed; guide lines show depth; the selected item takes the soft accent.
