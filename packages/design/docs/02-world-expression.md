# World expression

A World is not a palette with a poster. It is a combination of nine bounded axes, each an enum or an integer range, compiled by Motet into the tokens every component already reads. Hold the content constant and change only the record: that is what **WorldGallery** shows.

## The axes

| Axis | Values | Guard-rail Motet keeps |
| --- | --- | --- |
| `palette` | `primary`, `secondary`, `neutral` seeds; `scheme`; `chroma` 0–200 (thousandths); `temperature` −100–100; `depth` per mode −2–2; preferred `mode`; per-role `overrides` | contrast duties; reserved hues move away from the accent |
| `material` | `finish`, `grain` 0–4, `edge` 0–2, `shadow` 0–2, `tint` 0–12, `mica`, `sheet` (`solid`, `veil`, `frosted`) | a translucent sheet only at a proved-legible opacity; material leaves in forced colours |
| `shape` | `corner` (`round`, `squircle`, `bevel`, `square`), `scale` 0–150, `profile` (`even`, `cut`, `clipped`), `border` (`none`, `hairline`, `rule`, `heavy`), `tone` (`neutral`, `ornament`), `frame` (`none`, `double`) | controls take the corner, never the profile; borders are one solid colour; 44px targets |
| `ornament` | `set` (`rule`, `fade`, `filigree`, `segment`), `level` 0–3, art `edge` (`fade`, `straight`, `torn`, `burned`, `arch`), `dropCap`, `custom` path data | vector paths only, single ink, never inside controls or body text runs; removed by "decoration off" |
| `type` | `display` face, `case`, `tracking`, `weight`, `names` (`ui` or `display`), `reading` suggestion, `numerals`, `paragraphs` (`spaced`, `indented`) | interface face fixed; reading face and size belong to the reader |
| `imagery` | `art`, `placement` (`continuous`, `backdrop`, `header`), `focus`, `treatments` (`veil`, `vignette`, `duotone`, `blur`, `parallax`, each 0–100; they combine), `title` (`page`, `art`, `hidden`), `titleBacking` (`shade`, `halo`, `none`) | one image per World; the only text on art is the World's own title, kept at 4.6:1; parallax stops under reduced motion; art leaves under "decoration off" and forced colours |
| `icons` | `style`, `weight` 200–600 | FILL reserved for state |
| `motion` | `character` (`standard`, `crisp`, `soft`, `eerie`) | functional timing capped; reduced motion wins |
| `density` | `comfortable`, `standard`, `compact` | minimum target and text sizes |
| `composition` | `post` (`open`, `card`, `dossier`, `log`), `portrait` (`gutter`, `inline`, `none`), `names` (`plain`, `plate`), `plate` (`ink`, `accent`) | the same information, order and actions in every layout; the account line is never removed |

**Levels.** Level 1 turns on the set's divider motif and heading flanks and draws dividers in the ornament colour. Level 2 adds the set's surface decoration: filigree corner pieces (with padding so they never touch text) or the segment mark on framed surfaces. Level 3 is reserved for the richest sets; the drop cap is its own switch. The more a World adds, the plainer the reading surface stays.

**Sets.** `rule` is a hairline (Motet's own). `fade` is a hairline that fades out at both ends. `filigree` is the one pictorial set, for ornate fantasy. `segment` is the geometric accent of angular sci-fi and terminal UIs: an offset bar and square on a hairline, and the same bar riding the top edge of framed surfaces. For hard-edged styles, the frame **profile** does the job that corner pieces do in ornate ones.

**Placement.** A World shows one image, used three ways:

- `continuous` (default): the art is fixed behind the page. At load, its `focus` point sits in the header window. Below it, the content sheet rises with the World's edge. On wide screens the art's lower edge fades into the surface; on narrow screens the art is as wide as the screen and the sheet covers everything below the window.
- `backdrop`: the art is fixed behind the page, with only a band left under the chrome. Use it for atmosphere that surrounds the content (a city at night, fog) rather than a picture worth framing.
- `header`: the art sits at the top only and scrolls away, like a map pinned above the work.

**The sheet's sides follow its top.** A `fade` edge dissolves the art into the page across the full width, with an eased gradient, so the art shows only above the page. Every other edge (`straight`, `torn`, `burned`, `arch`) makes a column with hard sides and the art shows in the margins. A soft top is never paired with hard sides, or the reverse. There is always clear space between the edge and the World's title.

**Where the title goes.** `imagery.title: 'page'` (default) sets the World's mark and title on the page below the edge. `'art'` stands the mark, title and byline on the art, just above the edge, and starts the sheet at the World tabs; the header window grows to fit them. The engine chooses dark ink over light art and light ink over dark art from the art's measured dominant colour, then backs it as `titleBacking` says: `shade` tints the art under the title just enough to keep 4.6:1 over the art's worst measured colour, and not at all where the art already contrasts; `halo` outlines the letters only; `none` holds only while the art itself gives 4.6:1, and otherwise becomes a halo with a note. `'hidden'` is for art that already carries the World's wordmark: the mark and title leave the screen (screen readers still get the title, and the byline stays). They come back on phone widths, where the art's own wordmark is too small to read, and whenever the art is not shown.

**Treatments combine.** Each is 0–100 and 0 is off: `veil` lays page colour over the art, `vignette` fades its edges into the page, `duotone` recolours it in the accent and ornament hues, `blur` softens it (up to 24px), and `parallax` lets it drift as the page scrolls. Fixed art (`continuous`, `backdrop`) rises at up to half the page's speed over the first screen; `header` art scrolls away at down to half speed. Parallax moves the art layer and never enlarges its box, so the image keeps its scale. It runs on scroll-driven animations where the browser has them and on one passive scroll listener in the engine elsewhere; readers who reduce motion never see it. A record from before treatments combined, with one `treatment` and a `strength`, reads as that single treatment. A translucent sheet (`material.sheet`: `veil` or `frosted`) is always a straight-edged pane with a hairline, like the HUD panels of Elite Dangerous and Star Citizen. It reads best with `backdrop` placement, where the art fills the whole screen behind it.

**Compositions.** `open` separates posts with dividers. `card` frames each post in the World's profile. `dossier` files each post as a document with a form header strip (Papers, Please; Orwell). `log` is ruled rows with the speaker in the accent, like a terminal transcript, and suits `portrait: none`. Spacing between posts comes from `density`, not from the composition. `names: plate` sets speaker names on a solid label, like a stencilled tag.

## Seven starting themes

| | Motet (The Salt Road) | Neon Meridian | Ashfall | The Gilded Reach | Hollowmere | Cold Front | Kepler Relay |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Genre | default | cyberpunk | post-apocalyptic | ornate fantasy | gothic horror | contemporary strategy | diegetic terminal SF |
| Palette | petrol, tonal, cool | cyan + magenta, vibrant, dark | rust + ochre, warm, dark | navy + gold, fidelity, light | oxblood, neutral, very dark | navy + signal orange | amber phosphor on brown-black |
| Material | satin, edge 1 | sheen, grain 2, frosted sheet | matte, grain 4 | satin, grain 2, deep shadow | matte, grain 3 | matte, no shadow | matte, grain 3, no shadow, veil sheet |
| Shape | squircle, even | bevel 60%, clipped, rule | square, cut, heavy | round 90%, gilt double frame | round 35% | squircle 45%, rule | square, ornament-tone rule |
| Ornament | rule, fade edge | segment 2 (marks), fade edge | none, burned edge | filigree 3, arch edge, drop cap | fade 1, torn edge | none, straight edge | segment 1, straight edge |
| Art | continuous | backdrop, duotone | continuous, vignette | continuous | backdrop, vignette | header | backdrop |
| Display | Plex Sans | Oxanium, upper, tracked | Big Shoulders Stencil, upper | Cormorant Garamond, indented prose | Grenze Gotisch | Archivo condensed, upper, tabular | Plex Mono, upper, wide tracking |
| Icons | outlined 400 | sharp 300 | sharp 500 | rounded 400 | outlined 300 | outlined 400 | sharp 300 |
| Motion | standard | crisp | crisp | soft | eerie | standard | crisp |
| Posts | open stream | cards | cards, name plates | open, chapters | open | dossiers, inline portraits | log rows, no portraits |

## Three Worlds from existing games

The sample content also includes three Worlds modelled on play-by-forum games that already exist, so the system is tried against real settings rather than invented ones. Their settings, places and forum sections come from the games; characters, posts and marks are invented. Elarid's and Oskad's art is original. Mgławica Mocy shows the forum's own header image, included only to review the sample with the art's author; replace it before anything is published.

| | Elarid | Oskad | Mgławica Mocy |
| --- | --- | --- | --- |
| Source | [Wieloświat, Elarid](https://wieloswiat.pl/category/95/elarid): classic fantasy where magic takes the place of firearms; Verden, Nirgald, Karak'Akes | [Wieloświat, Oskad](https://wieloswiat.pl/category/93/oskad): fantasy western on the plains of the New World; telegraph, wanted posters, golems | [Mgławica Mocy archive](https://archiwum.mglawicamocy.pl/): a Star Wars PBF, 2013–2021; HoloNet, the cantina, the galaxy's regions |
| Palette | emerald + bronze, expressive, dark | rust + saddle brown, warm, light | holo blue + gold, vibrant, cold, dark |
| Material | satin, grain 2 | matte, grain 3, double frame | sheen, grain 1, strong edge light |
| Shape | bevel 70%, cut profile, ornament-tone rule | square, heavy border | squircle 110%, hairline |
| Ornament | filigree 2, arch edge | rule 1 with a custom sheriff's-star motif and flank, torn edge | fade 1, fade edge |
| Art | continuous: a citadel under an aurora; vignette and parallax; title on the art | header: mesas, the rail line and the telegraph; parallax | header: the forum's own banner, wordmark included; parallax; title hidden |
| Display | Source Serif 4, upper, tracked | Archivo condensed, upper, heavy | Plex Sans condensed, upper, very wide tracking |
| Posts | cards | open, names on accent plates | open |

They are starting points, not genres a World must pick. Two horror Worlds can share a palette and still differ by type, edge, ornament and composition. **Game UI lessons** traces each back to the games it learns from.

## Authoring

Start an owner from a complete theme, then let them adjust in five groups: colour, type, surfaces, imagery, composition. The editor previews the same populated Overview, a long Scene, an unfinished reply, light and dark, a phone width and a reader with decoration off, side by side. It shows clamps in place ("ornament darkened to keep 3:1"), never a wall of tokens.

The [Motet Theme Creator](https://claude.ai/artifact/Vo1KybPbrkLnu2Pu7W1sC2) is a working version of this editor, built from these components: every record field as a control, the engine's `adjustments` beside the control that caused them, Scene, front page, component and palette previews at desktop and phone width, reader settings (high contrast, decoration off, reduced transparency, Motet's plain look), and export as the Motet record, its standard.mode companion and DTCG tokens.

**Ornaments are a library first.** Owners pick a set and a level. A specialist author may supply their own motif, corner and flank as SVG path data (`vb` + up to 16 paths, fill or stroke, optional mirror). `Motet.validateOrnament()` accepts path commands and numbers only, no markup, styles, URLs or scripts. Motet renders them as single-ink masks in `ornament`, so they recolour per mode and vanish with decoration off.

## What a World cannot do

Change the interface face; move or rename navigation, Reply, Post, Back, the Inbox or the account; restyle chrome beyond a tint of up to 8%; set text on images; lower a contrast duty; hide the writing account; make danger, focus, selection or audience look like something else; animate continuously; ship CSS, scripts or fonts outside the vetted list.
