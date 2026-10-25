# Steps

Progress through a fixed sequence: creating a character, setting up a World.

- `Steps({ items: [{ title, description }], current, orientation: 'auto' | 'vertical', label })`.
- Horizontal from 560px of container width, vertical below. Completed steps show a check, the current one a ring; each step's state is in words for screen readers, and the current one has `aria-current="step"`.
