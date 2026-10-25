# Chrome

Motet's chrome is two corner islands, not a frame. Browsers frame web content to mark a security boundary; Motet has no such boundary to draw, so it takes as little territory as it can while staying recognisable in every World.

- **Left island:** the `motet` wordmark (Home) and the current World's mark with a switch chevron. The World's name is not repeated; the masthead and scene already show it.
- **Right island:** Search (a field on wide screens, an icon below 760px, `/` focuses it), the Inbox with a numeric count, and the account avatar. The account menu holds account switching, appearance for all Worlds, "Use Motet appearance here", reporting and sign-out.
- Content reserves `chrome-offset` (72px) at the top and uses it as `scroll-padding-top`, so the islands never cover a focused control or the first line of a post.

## Material

The islands default to **veil**: Motet's raised surface at the lowest opacity that keeps chrome text at 4.6:1 over pure black or pure white art (0.84 in practice), with a 10px blur and a 1.35 saturation lift, a 1px `chrome-line` outline and a soft lift. It is lighter than acrylic: art reads through as colour, not as shapes. **Solid** is used under reduced transparency (Motet's own setting, because Firefox and Safari don't yet support `prefers-reduced-transparency`) and when `backdrop-filter` is missing. **Frosted** (0.72+, 28px blur, grain) is available for comparison. The **Chrome** preview shows all three over light and dark art.

## What a World may change

Only a tint of the island fill toward its accent, at most 8% (default 4). Inside an island every World variable the controls read (`--accent`, `--radius-control`, `--corner-shape`, the surfaces, the icon style) is reset to Motet's, so a bevelled, stencilled World still gets a squircle chrome in Plex Sans with outlined icons. Menus opened from chrome use the opaque `chrome-bg-solid` fill.

## Global surfaces

Home, Inbox, Search and Account are Motet surfaces: no World theme is active there. Mixed-World lists show each World's mark in the standard identity slot and nothing else of its theme.

They are plain on purpose: one solid `surface` under the islands, with no frame, sheet or art (the **Page** pattern). The World cards and rows on them carry the colour. Side navigation and filters sit in a quiet `page.panel` box; only a single-task page such as sign-in puts one card on `canvas`.

## Recovery

"Use Motet appearance here" (per World) and the appearance switches (all Worlds, or this device) are always in the account menu, in chrome styling that no World can make unreadable. They change the reader's effective view only, never the World's stored design.
