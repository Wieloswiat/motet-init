# Implementation

## Two halves

1. **Static styling is a Panda CSS v2 preset** (`panda/preset/`). Its tokens and semantic tokens are thin aliases of Motet's runtime properties (`colors.surface.raised` → `var(--surface-raised)`), its recipes and slot recipes are the components, and its conditions cover reduced motion, reduced transparency, coarse pointers and forced colours.
2. **World appearance is runtime data.** `Motet.applyWorld(element, record, { mode, contrast, reader })` writes custom properties and a few `data-*` attributes on a `[data-world]` element. The preset declares its token aliases on `[data-world]` as well as `:root` (`cssVarRoot`), so every recipe inside follows the World without a rebuild. Panda's own `themes` feature is not used for Worlds: Worlds are user data.

```ts
// panda.config.ts (Panda v2: the base preset is no longer implicit)
import { defineConfig } from '@pandacss/dev'
import base from '@pandacss/preset-base'
import motet, { motetCssVarRoot } from '@motet/design/panda'

export default defineConfig({
  presets: [base, motet],
  preflight: true,
  prefix: { cssVar: 'm', className: '' },
  cssVarRoot: motetCssVarRoot,
  staticCss: { recipes: '*' },
  include: ['./src/**/*.{svelte,ts}'],
})
```

`components/bundle.css` in this system is exactly that preset compiled with Panda 2.2.0 and `staticCss: { recipes: '*' }`. Class names are the recipe names: `m-button m-button--variant_filled`, `m-post__root m-post__root--lane_ooc`. Use Panda's generated recipe functions in Svelte, or the DOM factories in `bundle.js` anywhere else.

## The engine (`window.Motet`)

| Call | Does |
| --- | --- |
| `compileWorld(record, opts)` | returns `{ vars, attrs, palette, notes, adjustments }`; pure, runs on the server |
| `applyWorld(el, record, opts)` / `mountWorld(el, record, opts)` | writes the result on an element; `mountWorld` follows the page's light/dark theme, `mode: 'world'` uses the World's preferred mode. A World with parallax, in a browser without scroll-driven animations, gets one passive scroll listener that writes `--art-drift` |
| `buildPalette(record, mode)` and `audit(palette)` | the colour roles and every contrast duty with its ratio; `roleChecks` lists the duties |
| `palette.adjustments` | every place the engine moved a colour away from the record, as `{ role, kind, from, to, target, text }`: contrast walks, chroma held back by the scheme, gamut limits, the danger hue stepping away from a red accent, overrides that missed their duty |
| `color.*` | `fromHex`, `toHex`, `contrast`, `gamut`, `maxChroma(l, h)`, `luminance`, `mix`: the OKLCH math the engine uses |
| `spring({ duration, bounce })` | a `linear()` easing and its settle time |
| `transition(update, { types })` | runs a state change as a view transition when the browser can and the reader allows motion, otherwise at once; `types` name the change and are also written to `<html data-vt>` (see **Motion and view transitions**) |
| `validateOrnament(o)` | accepts path data only |
| `toStandardMode(record)`, `toDTCG(record)` | interchange |
| `Button`, `IconButton`, `Chip`, `Avatar`, `Post`, `DiceRoll`, `SceneEvent`, `SceneRow`, `InboxItem`, `CharacterCard`, `Masthead`, `Chrome`, `Composer`, `Segmented`, `Switch`, `LaneFilter`, `Tabs`, `Divider` | DOM factories that emit the recipe classes |
| `Field`, `TextInput`, `Checkbox`, `CheckboxGroup`, `Fieldset`, `RadioGroup`, `Textarea`, `PasswordInput`, `NumberInput`, `Select`, `Combobox`, `TagsInput`, `InlineSelect`, `Calendar`, `DatePicker`, `TimeField`, `Slider`, `FileUpload`, `ColorPicker` | form factories with their keyboard behaviour (see **Component inventory**) |
| `Accordion`, `Collapsible`, `TreeView`, `Pagination`, `Breadcrumb`, `Steps`, `Popover`, `HoverCard`, `CharacterPreview`, `Drawer`, `toast()`, `Alert`, `Progress`, `ProgressRing`, `Spinner`, `Skeleton`, `PostSkeleton`, `DataList` | disclosure, navigation, overlay and feedback factories |
| `treatmentsOf(imagery)` | the five treatment strengths of a record, reading older single-treatment records too |
| `nextLane(current, clicked, policy)`, `lanePolicy` | the lane filter's rule in one function, and its one setting |
| `popup.open(el, anchor, opts)` / `popup.close(el)` | the shared top-layer positioning every popup uses |
| `enhance(root)` | keyboard behaviour for segmented groups, switches, textareas and trees in server-rendered markup (`data-m`), tooltips from `data-tip` |

Reader preferences go in `opts.reader`: `plain` (Use Motet appearance), `decoration: 'off'`, `reduceTransparency`, `reduceMotion`, `contrast: 'high'`, `readingFont`, `textScale`, `density`, `coarse`, `chromeMaterial`.

**Server rendering.** Compile on the server and emit the variables as a stylesheet rule for `[data-world="…"]`, not inline styles, so a strict CSP needs no `unsafe-inline`. Cache by record CID, engine version, mode, contrast and the reader flags that change output.

## Browser support

| Feature | Use | Without it |
| --- | --- | --- |
| `corner-shape` (Chromium 139+), including per-corner values | squircle, bevel, cut and clipped profiles | round corners at a reduced radius; profiles become small even corners |
| `backdrop-filter` (Baseline) | chrome veil, menus | opaque fills |
| `prefers-reduced-transparency` (Chromium only) | follows the OS | Motet's own setting |
| OKLCH, `color-mix()` (Baseline) | state layers, menu fills | — |
| `linear()` (Baseline) | springs | — |
| Container queries (Baseline) | post, composer, row layouts | — |
| View transitions | composer expand, World switch | instant change |
| Popover API (Baseline 2024) | listboxes, calendar, popovers, hover cards in the top layer | fixed positioning with a z-index |
| `field-sizing: content` (Chromium) | textareas that grow | a script measures the height |
| `::details-content`, `interpolate-size` (Chromium) | accordion height animation | sections open at once |
| `hidden="until-found"` | find-in-page opens collapsed sections and spoilers | plain `hidden` |
| `mask`, `mask-composite` (Baseline) | ornaments, the art's edges, the content sheet's edge and side feather | — |
| `:has()` (Baseline) | the content sheet follows the masthead size | the full-size window |

## Core and product layers

The parts any future app can adopt unchanged: the colour, material, shape, type, motion and ornament compiler; tokens; controls (button, icon button, chip, avatar, segmented, switch, field, tabs, menu, dialog, tooltip, flag); the chrome islands; the World scope. The play-by-forum layer: post, composer, dice roll, scene row, inbox item, character card, masthead, the theme lexicon and the seven starting themes. Generic recipes never mention Worlds, Scenes or posts; another product supplies its own record type and patterns on the same scope mechanism.

## Files

- `components/bundle.js`: the engine and factories (one classic script).
- `components/bundle.css`: the compiled Panda preset, then `panda/preset/raw.css` (the `@view-transition` rule, which Panda can't emit). An app that builds its own stylesheet from the preset imports `raw.css` after it.
- `panda/preset/`: the preset source (tokens, global styles, recipes, generated defaults).
- `components/demo/worlds.js`: demo content and procedural placeholder art for the previews; not part of the system.
- `fonts/`: subset WOFF2 files (Latin plus Latin Extended-A, so Polish is covered); Material Symbols subset to the icons in use; DiceFont (SIL OFL 1.1) for dice faces.
