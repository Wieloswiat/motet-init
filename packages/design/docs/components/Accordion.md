# Accordion

Sections a reader opens one or several at a time: house rules, lore chapters, settings.

- `Accordion({ items: [{ title, meta, icon, content, open }], multiple, variant: 'flush' | 'framed' })`.
- Native `details` and `summary`. Single-open groups use the `name` attribute. Find-in-page opens a section that holds the match.
- Opening animates the height on the spatial spring where `::details-content` is supported; elsewhere it opens at once.
- `framed` takes the World's frame profile; `flush` sits on the page with hairlines.
