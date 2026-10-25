# Theme record

A World's appearance is a record in its author's repository: data, never CSS. This section is a draft for the lexicon, not a published schema.

## Why not only standard.mode

`place.mode.standard.theme` (v1, frozen) is an intent-based look: `control`, `action` and `surface` at three priorities, each with `background`, `foreground` and `border`, plus three text priorities and a list of fonts, all as CSS strings. Its contract layers app, author and viewer looks value by value, and leaves readability to authors. That is a good common denominator, and too little for a World:

| Motet needs | standard.mode v1 |
| --- | --- |
| Seeds and a scheme that generate both modes and the high-contrast themes | one record per expression key, fixed values |
| Material, shape, ornament, imagery, icons, motion, density, composition | not defined (motion and density are "possible later aspects") |
| Focus, danger, selection, unread and turn signals that a theme cannot redefine | not modelled |
| Contrast guarantees | explicitly out of scope |
| Display face separate from interface and reading faces | three text priorities only |

So Motet keeps its own compact record and **also writes a standard.mode record** compiled from it, so any mode-aware app shows the World's colours and type faithfully. The Motet record points at it.

## The record (draft)

`at.motet.theme`, one per reusable theme. A World's adoption (`at.motet.world.appearance`) references a theme by strong ref and adds the World's own name, mark, art and adjustments, so a theme update never discards local identity. ATProto records have no floats: every fraction is an integer.

```json
{
  "$type": "at.motet.theme",
  "name": "Ashfall",
  "palette": { "primary": "#b5532c", "secondary": "#c9a24a", "scheme": "tonal",
               "chroma": 75, "temperature": 65, "depth": { "light": -1, "dark": 0 }, "mode": "dark",
               "overrides": { "dark": { "ornament": "#c9a24a" } } },
  "material": { "finish": "matte", "grain": 4, "tint": 6, "edge": 0, "shadow": 1 },
  "shape": { "corner": "square", "scale": 0, "profile": "cut", "border": "heavy", "tone": "neutral" },
  "ornament": { "set": "rule", "level": 1, "edge": "burned",
                "custom": { "motif": { "vb": [112, 20], "p": [{ "d": "M30 2h52l4 4v8l-4 4H30l-4-4V6z" }] } } },
  "type": { "display": "big-shoulders-stencil", "case": "upper", "tracking": 40, "weight": 800,
            "names": "display", "reading": "serif", "numerals": "tabular", "paragraphs": "spaced" },
  "icons": { "style": "sharp", "weight": 500 },
  "motion": { "character": "crisp" },
  "density": "standard",
  "composition": { "post": "card", "portrait": "gutter", "names": "plate", "plate": "ink" },
  "imagery": { "placement": "continuous", "treatments": { "vignette": 90 }, "focus": [50, 60], "title": "page",
               "dominant": "#8a4b2c", "range": ["#37221b", "#e9c698"],
               "art": { "$type": "blob", "mimeType": "image/webp" } },
  "standard": { "uri": "at://did:plc:…/place.mode.standard.theme/at.motet.world.ashfall", "cid": "bafy…" },
  "engine": 2,
  "createdAt": "2026-10-15T00:00:00Z"
}
```

- `chroma` is thousandths of OKLCH chroma; `temperature`, each of `treatments`, `scale` and `tint` are percentages; `tracking` is thousandths of an em; `focus` is percent coordinates.
- `dominant` and `range` are computed once from the art when the theme is published: the dominant colour for mica tinting, and the darkest and lightest colours after a 32px blur, so a translucent sheet can be as clear as that art allows instead of assuming pure black and white.
- `engine` pins the compiler version. A newer engine renders an older record with its pinned rules until the author republishes, and the editor shows the difference first. Engine 2 split `grain` into five levels (engine 1's 1–3 are 2–4), made `strength` the treatment's own intensity, and dropped the `ledger` composition (read as `open`). Treatments now combine in `imagery.treatments`; an older `treatment` + `strength` pair reads as that one treatment.
- Unknown fields are ignored without effect; invalid known fields fall back to Motet's value for that field, not for the whole record.
- Ornament paths pass `Motet.validateOrnament()` before publishing and again before rendering.
- Wherever the engine moves a colour away from what the record asks for, it says so in `palette.adjustments` (role, kind, before, after, the ratio it was keeping, and a sentence). Editors print these beside the control that caused them instead of silently showing a different colour.

## The companion standard.mode record

`Motet.toStandardMode(record, { mode })` produces it. Record key: the World's expression, `at.motet.world.<world-rkey>`, in the World's preferred mode.

| standard.mode intent | Motet role |
| --- | --- |
| `control` | `surface-raised` / `ink` / `line-strong` |
| `action.primary` | `accent` / `on-accent` / `accent` |
| `action.secondary` | `accent-soft` / `on-accent-soft` |
| `action.auxiliary` | `surface` / `accent-ink` / `line-strong` |
| `surface.primary` (interrupts the page) | `surface-raised` / `ink` / `line` |
| `surface.secondary` (over the page) | `surface-overlay` / `ink` / `line` |
| `surface.auxiliary` (the page) | `surface` / `ink` / `line` |
| `text.primary` / `secondary` / `auxiliary` | reading 18px, interface 15px, display face 13px |

Because the Mode contract lets a viewer save a look at the same expression key, a reader could override one World's colours from any mode-aware tool. Motet treats a viewer's mode look as one more reader preference ("Use my mode.place look"), applied after the contrast clamp, never below it.

## DTCG and at-design-tokens

`Motet.toDTCG(record)` exports the resolved light and dark roles, radii and durations as DTCG 2025.10 tokens for design tools. It is an export, not the record: DTCG is a format for values, while Motet's record is a set of decisions the engine turns into values. `designtxt/at-design-tokens` stores DTCG files as untyped ATProto records with numbers as strings; it is an experimental proof of concept with little adoption so far, so Motet does not depend on it. Its records would be trivial to emit from `toDTCG` if it gains traction.
