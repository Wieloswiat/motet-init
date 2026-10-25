# 0012. The exported design system, and demo screens in the web app

Status: Provisional. Amends [0009](0009-web-stack.md): the design package
section below replaces 0009's description of `@motet/design`; the rest of
0009 stands.

## Context

Motet's design system, its app screens and the Theme Creator are designed
outside this repository, in claude.ai design artifacts. The repository had
only the tokens (as CSS variables) and a thin Panda preset that named them.
The exports bring much more: a Panda v2 preset with recipes for every
component, a script engine that compiles a World's theme record into custom
properties and ornaments, demo content, and twenty screens written as
templates with sample data.

Nothing behind the screens exists yet: no AppView reads, no sign-in, no
posting. The user asked for the design in the repository now, working or
not, so that the app grows from it.

## Decision

- **The design system's preset replaces the thin preset.** `@motet/design`
  copies the export's `panda/` as it is (packages/design/UPSTREAM.md lists the
  source version and the local changes). The app and the Theme Creator build
  their CSS from it with Panda; the export's prebuilt `bundle.css` is not
  copied.
- **Every recipe is emitted.** The ported screens write recipe classes
  (`m-button`, `m-chip__root`) in markup, which Panda can't extract, so
  `staticCss: { recipes: '*' }`. Remove it once components call the recipes
  from TypeScript.
- **The engine runs in the browser.** A World's look comes from
  `Motet.mountWorld`, the export's engine, loaded lazily on the client. A
  server-rendered page shows Motet's look until it hydrates, then the World's.
  That is acceptable for demo screens. Before Worlds are real, compile the
  record on the server (the engine is plain functions over a record) so the
  first paint is already the World's.
- **The screens are a one-time port, owned here from now on.** Each screen
  became a Svelte component in `apps/web/src/lib/screens/`, with its sample
  data as a TypeScript module in `src/lib/demo/screens/`, which the route's
  `load` returns. A converter did the bulk work; it is not kept, because the
  screens will be edited by hand as they gain behaviour, and a re-export
  would overwrite that work. The shared parts (chrome, World header, pickers,
  report dialog) were ported by hand. The screens are static: buttons do
  nothing, and only a few menus and toggles hold local state.
- **Routes follow the app's shape, not the export's order.** `/` is the
  shelves home, World pages live under `/w/[world]`, and `/design` lists every
  screen next to the theme switch. Only the demo World, scene and character
  resolve; anything else is a 404.
- **The Theme Creator is its own app** (`apps/theme-creator`), the exported
  static editor served by Vite and sharing `@motet/design`. Its script stays
  as exported, out of lint, so that a new export can replace it.
- **Webfonts stay out**, as 0009 decided, and so does the Mgławica Mocy art
  the export borrowed from that forum.

## Consequences

- The web app's look matches the design artifacts, and each screen is a
  starting point for the real feature.
- The vendored preset, engine and demo, and the Theme Creator's script, are
  excluded from lint and formatting and are not edited in place except for the
  changes their UPSTREAM.md lists.
- Until the fonts land, text uses system fonts and icons show their ligature
  names.
- Playwright checks that every screen loads under the production Content
  Security Policy without errors and that a World page paints its own art.
  These say nothing about features the screens only depict.

## Revisit when

A screen gets real data or behaviour (drop its demo module), the recipes are
called from TypeScript (drop `staticCss`), or the server renders Worlds'
themes (stop loading the engine for first paint).
