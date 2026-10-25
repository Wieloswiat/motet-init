Motet is the design system of a play-by-forum platform where every World (a game, run by its GMs) looks like its own place, while reading, writing, identity and navigation work the same everywhere. It has two layers. **Motet's core** is quiet, material-aware flat UI after Fluent 2 and Material 3. **World expression** is a small set of declarative axes (palette, material, shape, ornament, type, imagery, icons, motion, density) that a World sets and Motet compiles into the same tokens every component reads.

Build every surface from these rules. Look at **ScenePage** and **WorldGallery** first: they show one set of components under ten very different Worlds, three of them modelled on existing play-by-forum games. **Game UI lessons** explains where the World axes come from, and **Component inventory** maps every component to its Ark UI, Chakra UI and Radix counterpart. The [Motet Theme Creator](https://claude.ai/artifact/Vo1KybPbrkLnu2Pu7W1sC2) builds a World's theme record with these components.

## Principles

1. **The work leads.** Posts, the composer and the current Scene take the space. A masthead introduces a World once; it does not sit between a reader and the story. Never open a working screen with a pitch, a welcome or a dashboard of counts.
2. **The character speaks; the account is always there.** A post leads with the character's name and portrait in the World's voice, and its second line is always the writing account, in platform style. No theme can hide or restyle it.
3. **Worlds own atmosphere, Motet owns meaning.** A World may recolour, reshape and decorate a Reply button. It may not rename it, move it, or make danger, focus, selection, permission or publication state look like something else.
4. **Readers win.** Mode, reading face and size, contrast, density, motion, transparency and decoration are the reader's. "Use Motet appearance" is always one step away, in chrome no World can touch.
5. **Subtle material, never skeuomorphism.** Grain, sheen, edge light and translucency are a few percent of luminance. They sit behind text, never animate, and disappear in forced colours.
6. **Ownership is shown by function, not slogans.** "View record", "Export", the account handle on every post and "Draft saved on this device" do the work. The interface never describes its own architecture.

## Using the tokens

- Paint with roles, never raw colours. `surface` is the page a World's content sits on, and the whole of Motet's own pages (see **Page**); `surface-quiet` holds OOC posts, quotes, Scene events (rolls, tool results) and the side panels of Motet's pages; `surface-raised` holds cards, the composer and dialogs; `canvas` sits only behind a single-task card such as sign-in and never carries text.
- Set text in `ink`. Set metadata, handles and epithets in `ink-muted`; nothing is fainter than `ink-muted`.
- Use `line` for decorative separation and `line-strong` for any boundary that carries meaning (inputs, chips, outline buttons).
- Use `accent` only as a fill (the one filled button per view, the tab indicator); put `on-accent` on it. Use `accent-ink` for links and text buttons, `accent-soft` with `on-accent-soft` for tonal buttons, selected chips and the "Your turn" flag.
- Use `ornament` only for World decoration: divider motifs, corner pieces, segment marks, drop caps and heading flanks. A World may also draw its frame lines in it (`shape.tone: ornament`).
- `danger`, `caution`, `positive` and `info` are platform-reserved. Each comes as a fill, `on-…`, `…-ink` and `…-soft`, and always appears with an icon and a word.
- Focus is `focus`: a 2px solid ring with a 2px gap. The engine regenerates it per World and holds it at 3:1 on every surface.
- Space on the 4px scale (`space-1` … `space-16`). Gaps say relationship: 4–8px inside a group, 16–32px between groups.
- Round with `radius-control` (buttons, inputs), `radius-sm` (chips, flags), `radius-md` (menu items, OOC posts, Scene events), `radius-lg` (menus) and `radius-xl` (dialogs). Framed surfaces (cards, framed posts, the composer) use `frame-radius` with `frame-shape`, which carry the World's profile. Motet draws corners as squircles via `corner-shape`; browsers without it draw them round.
- Lift with `elevation-1` (resting cards), `elevation-2` (character cards), `elevation-3` (the composer) and `elevation-4` (menus, dialogs), each combined with `edge-light`.
- Time functional motion with the spring pairs `ease-spatial-*` / `dur-spatial-*` (position, size, corner morph) and `ease-effects-*` / `dur-effects-*` (colour, opacity). Decorative entrances use `ease-decor` / `dur-decor`. State changes that move things go through `Motet.transition()` as view transitions; see **Motion and view transitions**.

Every one of these is overridden on a World scope by `Motet.applyWorld()`. Components never need to know which World they are in.

## Content fundamentals

- **Sentence case, verb first, short.** "Post", "Save draft", "Switch World", "Discard draft". Buttons say what happens.
- **Name things as the product does:** World, Scene, Arc, Location, Post, Character, Card, Account, Draft, In character, Out of character.
- **Structured fields, not sentences.** Show "Velka Crossing" with a place icon and "The Long Winter" with a story icon as two fields. Never chain facts with middots or bullets ("4 places · 80 replies").
- **No self-description.** No taglines, no "built on ATProto", no "No ads. No algorithms." Say what a control does in its label or tooltip and nothing about the stack.
- **Say it once.** An error names the problem and stops: "Enter a name.", not "Enter a name. Your other answers are kept." Don't reassure about things that did not go wrong, and don't repeat a label in its help text. Help text appears only when it adds something.
- **Honest states, in the right place.** "Draft saved on this device", "Offline. Draft saved on this device", "Not sent". Never "Sent" before it is. A state that is always present (draft sync) is an icon with its words in the tooltip and accessible name, not a sentence beside the button.
- **Second person only where it is the reader's state:** "Your turn". Elsewhere name the thing: "Response requested", "Application".
- **No emoji in interface copy.** Icons come from Material Symbols. Authors may use emoji in their own posts.
- **Polish and English** are interface languages. Labels must survive 30% growth; Polish names (Wiktor „Kret” Zając) must wrap, not truncate.

## Visual foundations

- **Colour** comes from 1–3 seeds through OKLCH roles with contrast targets (see Foundations). Motet's own seed is a petrol blue (`accent`) over cool slate neutrals. It is deliberately neither purple-blue nor warm paper.
- **Type** has three jobs. IBM Plex Sans (`--font-ui`) sets every control, label and metadata and never changes per World. Source Serif 4 (`--font-reading`) sets prose at 18px / 1.6 within 68ch, and the reader may swap it. A World sets only the display face: titles, scene headings and, if it chooses, character names.
- **Material**: surfaces are flat fills with optional finish (`matte`, `satin`, `sheen`), grain (0–4) and edge light (0–2). Borders are always one solid colour. Translucency is reserved for chrome islands, menus and, where a World chooses it, the content sheet, at an opacity the engine has proved legible over that World's art.
- **Shape**: Worlds choose a corner (`round`, `squircle`, `bevel`, `square`), a radius scale and a frame profile: `even`, `cut` (one chamfered corner) or `clipped` (two opposite chamfers). Asymmetric profiles stand in for ornament in hard-edged styles, after Cyberpunk 2077 and Destiny. Controls take the corner, never the profile.
- **Imagery**: a World has one image, never a header picture plus a separate background. By default (`continuous`) the art is fixed behind the whole page: the top of the page is a window onto it, and the content sheet rises over it with the World's edge (`fade`, `torn`, `burned`, `arch`, `straight`). A fade spans the full width; the other edges make a column, and the art shows in the margins. The World's title can stand on the art, or step aside when the art carries its own wordmark (`imagery.title`). `backdrop` keeps only a band of art under the chrome; `header` puts the art at the top only. The sheet has no border or radius, and text never sits on art.
- **Borders over shadows** when in doubt. Shadows are Fluent-style key plus ambient pairs; a World may turn them off (`material.shadow: 0`) and rely on `line-strong`.
- **No ambient motion.** Nothing pulses, shimmers or loops. Decorative motion plays once on entry and is removed under reduced motion.

## Iconography

Use Material Symbols only, from the variable font (`.m-icon`, ligature names such as `reply`, `casino`, `front_hand`). A World picks the style (`outlined`, `rounded`, `sharp`) and weight (200–600); dark modes use grade −25. `FILL 1` is reserved for selected and pressed states. Never draw a status as a coloured dot; use an icon and a word. The font files in `fonts/` are subset to the icons this system uses: add names to the subset before using new ones.

## Accessibility floor

WCAG 2.2 AA on whole rendered combinations, not just tokens. Text keeps 4.5:1 and boundaries 3:1 in every theme and every World; the engine clamps authored colours that miss. Coarse pointers get 44px targets (small controls extend an invisible hit area). Focus is never obscured by chrome: content reserves `chrome-offset` and `scroll-padding-top`. Two high-contrast themes (7:1 text) ship beside light and dark. Forced colours remove material, grain, ornament and art, and borders take over.

## Never

Purple-to-blue gradients, glow on dark sections, gradient or one-sided borders that read as missing edges, status dots in pills, pulsing badges, dot-separated metadata, coloured left-border cards, emoji as icons, landing-page heroes inside the app, uppercase eyebrow labels over headings, text over images, icon-only primary actions without names, grey text below `ink-muted`, and copy that describes the technology.
