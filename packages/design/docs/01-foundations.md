# Foundations

Every value below is produced by one function, `Motet.compileWorld(record, { mode, contrast, reader })`. Motet's own appearance is just `Motet.defaultRecord` compiled for light, dark and the two high-contrast themes; that output is `tokens.json`.

## Colour: seeds to roles

A World gives a `primary` seed, optionally `secondary` (ornament) and `neutral`, a `scheme`, a chroma ceiling and a temperature. The engine works in OKLCH and assigns each role by **contrast target**, not by fixed lightness steps (after Adobe Leonardo), then gamut-maps to sRGB.

| Role | Built from | Duty (standard / high contrast) |
| --- | --- | --- |
| `canvas`, `surface`, `surface-quiet`, `surface-raised`, `surface-overlay` | neutral hue at fixed lightness per mode, shifted by `depth`; tint and mica mix toward the accent or art | grounds |
| `ink` | neutral | 12:1 / 15:1 on every ground |
| `ink-muted` | neutral | 4.95:1 / 7.6:1 on every ground, plus 0.04, 0.12, 0.24 or 0.36 for grain levels 1–4 |
| `line` / `line-strong` | neutral | 1.3:1 (decorative) / 3:1 on surface and raised |
| `accent` | primary; chroma by scheme | 3:1 boundary on surfaces, and its label `on-accent` 4.6:1 |
| `accent-ink` | primary | 4.75:1 on every ground |
| `accent-soft` / `on-accent-soft` | primary | label 4.75:1 on the soft fill |
| `ornament` | secondary | 3:1 on surface and raised |
| `focus` | primary hue, high chroma | 3.6:1 on every ground and on `accent-soft` |
| `danger`, `caution`, `positive`, `info` | reserved hues 27°, 78°, 185°, 248° | fills carry 4.6:1 labels; `…-ink` 4.75:1 |

Schemes follow Material's dynamic-colour variants: `neutral` (accent chroma ≤ 0.06), `tonal` (default), `vibrant` and `expressive` (up to 1.4× the ceiling), `fidelity` (keeps the seed's own lightness and chroma), `monochrome`. Temperature at or beyond ±0.3 moves the neutral family to warm stone (hue ≈ 68°) or cool slate (≈ 238°); it never rotates the accent.

**Reserved hues step aside.** If a World's accent sits within 32° of danger red, danger moves to 4° or 52°, so a red World never has red "danger" that looks like its brand. Success is teal (185°), off the red–green axis.

**Authored overrides** (`palette.overrides.light|dark`) are accepted per role. An override that misses its duty is walked in lightness until it passes, and the theme editor says so. Nothing a World authors can lower a contrast floor.

## Material

| Parameter | Values | What the engine writes |
| --- | --- | --- |
| `finish` | `matte`, `satin`, `sheen` | `--finish`: none; a 0.5 white fade over the top 42% (light) or 3.5% (dark); a 162° highlight band |
| `grain` | 0–4 | `--grain`: a 220px fractal-noise SVG tile at 2/4/6/9% (light) or 2.8/5.5/8.5/12% (dark) opacity for fine, medium, coarse and heavy, behind text. Page-sized surfaces take grain but never a finish gradient |
| `edge` | 0–2 | `--edge-light`: inset 1px top highlight and bottom shade |
| `shadow` | 0–2 | `--elevation-1…4`: Fluent 2 key and ambient pairs; 0 removes them, 2 deepens them by 30% |
| `tint` | 0–12 | mixes the accent into the neutrals |
| `mica` | true / false | tints surfaces 7% toward the art's dominant colour; still opaque |
| `sheet` | `solid`, `veil`, `frosted` | the content sheet over the World's art. `veil` and `frosted` are translucent like the chrome, at the lowest opacity that keeps muted text and links at 4.6:1 and body text at 7:1 over the art's measured range (pure black and white if unmeasured); veil at least 0.72 with 16px blur, frosted at least 0.6 with 32px blur and grain. Translucent sheets have straight edges and a hairline in `--frame-line` |

Fluent's taxonomy maps to Motet as: **solid** (all content), **mica** (art-tinted but opaque surfaces), **veil** (chrome islands, 0.84+ opacity, 10px blur), **frosted** (menus and other transient surfaces, 24px blur) and **smoke** (`scrim` under dialogs). Content is translucent only on a `veil` or `frosted` sheet whose opacity the engine has proved legible against the art. High contrast, reduced transparency, missing `backdrop-filter` and art-less pages all fall back to `solid`. Cards, the composer, rolls and OOC posts stay opaque on every sheet.

Borders are one solid colour (`--frame-line`: `line`, `line-strong`, or the ornament colour when `shape.tone` is `ornament`). There are no gradient borders: a border that fades along one side reads as a missing edge, not as depth.

## Shape

Radii follow Material 3's steps: `radius-xs` 4, `radius-sm` 8, `radius-md` 12, `radius-lg` 16, `radius-xl` 28, multiplied by the World's scale (0–150%). Corners are `round`, `squircle`, `bevel` or `square`. Squircle Worlds draw 1.35× larger radii through `corner-shape: squircle`, which reads as the same softness. Bevel needs `corner-shape` (Chromium 139+); elsewhere it falls back to round corners at 35% of the radius.

**Frame profiles** shape the World's framed surfaces (cards, framed posts, the composer, character cards) and nothing else:

| Profile | `frame-radius` | `frame-shape` | Reads as |
| --- | --- | --- | --- |
| `even` | `radius-lg` on all corners | the World's corner | Motet default |
| `cut` | small corners, one chamfer bottom right | `round round bevel round` | a cut steel plate (Ashfall) |
| `clipped` | chamfers top left and bottom right | `bevel round bevel round` | a HUD panel (Neon Meridian) |

The chamfer is 16px × scale, at least 10px; the small corners are at most 4px. Asymmetry is the decoration, so it stays on large surfaces: controls take the World's corner but never a profile. Without `corner-shape` support, cut and clipped degrade to small even corners. A World can add a **segment mark**: a 44 × 3px bar in the ornament colour riding the top edge of each framed surface, 24px from the leading corner (`ornament.set: segment`, level 2).

## Typography

| Role | Face | Size / line | Owner |
| --- | --- | --- | --- |
| Interface | IBM Plex Sans (wdth 75–100, wght 100–700) | 14/20 controls, 13/18 labels and metadata, 16/22 titles, 20/26 headings, 28/34 Motet page titles | Motet |
| Reading | Source Serif 4 (opsz 8–60), or Plex Sans | 18/28.8, max 68ch, paragraphs spaced 0.85em or indented 1.6em | reader, World suggests |
| Display | World's choice from the vetted list | masthead 28–44px, scene 30px, names 17px, each × the face's adjust | World |
| Code | IBM Plex Mono | 13/18 | Motet |

The vetted display faces all carry Polish: Oxanium (technical SF), Big Shoulders Stencil (industrial, post-apocalyptic), Cormorant Garamond (literary fantasy, adjust 1.16), Grenze Gotisch (gothic horror), Archivo at wdth 78 (contemporary strategy), IBM Plex Mono (diegetic terminals, adjust 0.9), plus Plex Sans and Source Serif. Orbitron is excluded: its Google Fonts build has no Polish diacritics. A World that sets its display face in mono still reads prose in a proportional face.

Dice faces use **DiceFont** by Franco Ponticelli (font under SIL OFL 1.1, CSS under MIT): d2, d4, d6, d8, d10 (0–10), d12, d20 and Fate, at `--font-dice`. It has no d100; percentile rolls show numbers.

## Spacing and density

The 4px scale runs `space-1` (4) to `space-16` (64). Density changes control height (36 compact, 40 standard, 44 comfortable) and the rhythm between posts (`--post-gap` 24/32/40). Coarse pointers always get 44px.

## Motion

Springs come from Material 3 Expressive and are written as (duration, bounce), the form Apple uses, converted with stiffness = (2π / duration)² and damping ratio = 1 − bounce. The engine samples each to a CSS `linear()` curve and reports its settle time.

| Spring | Standard (duration s, bounce) | Expressive |
| --- | --- | --- |
| spatial fast / default / slow | 0.168, 0.1 / 0.237, 0.1 / 0.363, 0.1 | 0.222, 0.4 / 0.322, 0.2 / 0.444, 0.2 |
| effects fast / default / slow | 0.102, 0 / 0.157, 0 / 0.222, 0 | same |

These come from community ports of Google's Compose tokens (Material 3's site needs JavaScript); check them against the Compose `MotionScheme` source before freezing. A World picks a **character**, never curves: `standard`; `crisp` (no bounce, 15% faster); `soft` (expressive springs, bounce capped at 0.15); `eerie` (no bounce, decorative entrances 2.4× slower). Functional motion is capped at 420ms. Reduced motion sets spatial and decorative durations to 0 and keeps short fades.

Use spatial springs for position, size and corner morphs (a pressed button's radius drops to 55%). Use effects springs for colour and opacity. Use view transitions for route and composer continuity.

## Icons

Material Symbols, one variable font per style. Axes: FILL 0/1 (1 = selected), wght 200–600 (World), GRAD 0 light / −25 dark, opsz 20 at 16px and 24 at 20–24px.
