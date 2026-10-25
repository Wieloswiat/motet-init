# RadioGroup

One choice from a short visible set, in a `fieldset` with a legend.

**Use** for two to six options that all need to be seen. **Use Select** for longer lists, **Segmented** for switching views.

- `RadioGroup({ legend, name, value, items: [{ value, label, description, icon, disabled }], variant: 'list' | 'cards', onChange })`.
- Radios stay round in every World: the circle carries the meaning.
- `cards` gives each option a bordered surface, an icon and a sentence; the selected card takes the accent border and a soft fill.
- Native radios: arrow keys move the choice, Tab leaves the group.
