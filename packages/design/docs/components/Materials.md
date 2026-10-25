# Materials

Finish, grain, edge light, elevation and the content sheet.

All are static layers behind text, a few percent of luminance. Grain has five levels: none, fine, medium, coarse and heavy; each raises the contrast floor of muted text a little. Finish is for surfaces the size of a card or smaller; page-sized surfaces take grain only, because a gradient stretched over a page reads as fog. Borders are always one solid colour. A World with `shadow: 0` separates with `line-strong` borders instead.

The content sheet is `solid`, `veil` or `frosted`. Translucent sheets take the lowest opacity at which muted text and links keep 4.6:1 (body text 7:1) over the art's measured range, never below 0.72 (veil) or 0.6 (frosted). They fall back to solid in high contrast, under reduced transparency, without `backdrop-filter` and without art. Forced colours remove every layer and keep borders.
