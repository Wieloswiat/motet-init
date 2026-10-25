# HoverCard

A preview of a character or account while pointing at its name.

- `HoverCard({ trigger, content })` with `CharacterPreview({ name, epithet, portrait, facts, account })` as the usual content.
- Opens 500ms after the pointer arrives and closes 250ms after it leaves; keyboard focus opens it at once. Not used on touch.
- It holds no controls and is never the only way to the information: the link goes to the full card.
