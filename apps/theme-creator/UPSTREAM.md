# Theme Creator, exported

The Theme Creator edits a World's theme record and previews it on sample
screens. It was designed outside this repository and exported as a static
site. This app serves that export through Vite, with the engine, demo
content and styles taken from `@motet/design` instead of the export's copies.
It is not wired to Motet: a theme is kept in the browser only.

## Source

- `Motet Theme Creator.zip`, 2,080,366 bytes, SHA-256
  `f9e60c23d5c4d725dcc810bab66fd3315c0cf5dde57bee2e4f879dbd73dc3046`.
- Copied on 2026-10-25.

## What came from where

| Here                       | In the export                                |
| -------------------------- | -------------------------------------------- |
| `index.html`, `frame.html` | the same files, without their inline scripts |
| `src/main.js`              | `index.html`'s inline script                 |
| `src/frame.js`             | `frame.html`'s inline script                 |

The export's `motet/bundle.js`, `motet/worlds.js` and `motet/tokens.css` are
older copies of the files in `@motet/design` (packages/design/UPSTREAM.md);
the editor runs on the newer engine unchanged. Its `motet/bundle.css` is
rebuilt here from the design system's Panda preset.

## Left out

- `motet/fonts/`: not committed until each font's licence is confirmed
  (ADR 0009).
- `motet/mglawica-bg.jpg`: the Mgławica Mocy forum's own art, not ours to
  publish.

## Local changes

- The pages load `/src/styles.css` and one module script each, in place of
  the export's `<link>` and `<script>` tags.
- `src/main.js` and `src/frame.js` start with an import of `src/motet.js`,
  which loads the engine and the demo in the export's order. The rest of each
  file is the inline script, byte for byte.
