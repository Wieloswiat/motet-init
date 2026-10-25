# Motet design system, exported

The design system is designed outside this repository, in a claude.ai design
artifact, and copied in here. This record identifies the copied version, so
that the next export can replace these files and keep the local changes.

## Source

- Artifact: "Motet Design", <https://claude.ai/artifact/B5oJDfYzEoAoo6SNQpVcSQ>
  (id `51a6e4a4-bc69-42ce-92f6-d8bb07e9401d`), version `1792936696-7d04`. Its
  last change is dated 2026-10-16 ("View transitions: Motet.transition() …").
- Copied on 2026-10-25 by reading the artifact's published files.

## What came from where

Paths in the artifact are under `project/`.

| Here                                 | In the artifact                        |
| ------------------------------------ | -------------------------------------- |
| `src/panda/` (preset and recipes)    | `panda/`                               |
| `src/engine/motet.js`                | `components/bundle.js`                 |
| `src/engine/motet.d.ts`              | `components/index.d.ts`                |
| `src/demo/worlds.js`                 | `components/demo/worlds.js`            |
| `src/tokens.json`                    | `tokens.json`                          |
| `docs/` (README, 01–11, components/) | the same files                         |
| `src/tokens.css`                     | the Theme Creator's `motet/tokens.css` |

`components/bundle.css` is not copied: `apps/web` and `apps/theme-creator`
build the same CSS from `src/panda/` with Panda.

`src/demo/worlds.d.ts` is ours: the export has no types for the demo.

## Left out

- **Webfonts.** The Theme Creator export ships `fonts/*.woff2`. They are not
  committed until each font's licence is confirmed (ADR 0009). `src/fonts.css` keeps the
  `@font-face` rules for when they are; until then the stacks fall back to
  system fonts and Material Symbols icons show their ligature names.
- **`mglawica-bg.jpg`.** The Mgławica Mocy forum's own header art, included
  in the export only to review the sample with the art's author. It is not
  ours to publish.

## Local changes

Reapply these after a new export:

- `src/panda/recipes/controls.ts`: in the `button` and `iconButton` bases, a
  key written before `...stateLayer` was silently overwritten by the spread
  (TypeScript error TS2783). The pressed radius and the hover ink are now
  merged into the state layer's own `&:active:not(:disabled)` and
  `@media (hover: hover)` entries, so both apply as the export intended.
- `src/engine/motet.d.ts`: declares `defaultRecord` and `presets`, which the
  engine exports but the declarations missed, and types `window.Motet` as the
  module.
- `src/demo/worlds.js`: the Mgławica Mocy World has no art, since the image is
  left out (see above).
