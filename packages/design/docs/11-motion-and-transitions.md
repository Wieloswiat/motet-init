# Motion and view transitions

Motion in Motet explains a change: where something went, what replaced it, what it became. It never decorates a resting screen. Every change that moves things goes through one call, so timing, reduced motion and stacking are decided in one place.

## One entry point

`Motet.transition(update, { types })` runs `update` inside `document.startViewTransition` when the browser has it and the reader allows motion, and runs it straight away otherwise. `update` may return a promise; the new state is captured once it settles. In a component framework, `update` applies the state synchronously (React's `flushSync`, Svelte's `tick`). The call returns the transition, or a stand-in with the same promises.

Reduced motion (the system setting, or Motet's own Reduce motion setting, which writes `data-reduce-motion`) skips the transition: the change happens in place.

## Types

| Type | Used for | What moves |
| --- | --- | --- |
| `forward`, `back` | a step in a flow, a section in a side menu, a drill-in on phones, tabs in reading order | the region named `m-pane` slides 24px and fades the way the reader is going; anything leaving goes the same way |
| `filter` | a filter changes what a list shows | named items move to their new places, leaving ones settle out, new ones rise in; the page under them swaps at once, because a shorter list can scroll the page and two scroll positions must not cross-fade |
| `expand`, `collapse` | the composer becoming a full-screen sheet | `m-composer` |
| `post` | publishing a post | the text leaves the editor (`m-draft`) and settles into the new post while the composer passes over it |
| `roll` | rolling dice, answering a roll request | the event card changes shape in place and the die tumbles in once (`m-die`) |
| `create` | a new World appearing | the preview card grows into the World's header |
| none | anything else: a decision in a queue, a dialog closing, a section opening | the page cross-fades and named items move |

Types are passed to the transition and also written to `<html data-vt>`, so browsers without transition types style them the same way.

## Names

- **Names exist only while a transition runs** (`html[data-vt]`, or `:active-view-transition` for page loads). A named element is a stacking context and a backdrop root: a permanent name would cut frosted menus off from what is behind them and lift posts over each other's menus.
- **`.m-vt` with `--vt`** names one item: a post (`m-s-<id>`), a scene row, a character card, a member, a report. Names are unique on the page.
- **`.m-vt-box`** is for a card or row whose shape changes (a card becoming a list row, a request becoming its result). Its content keeps its size and is uncovered or cut by the moving box instead of being stretched.
- **Shared parts** inside a box carry their own `.m-vt`: in Characters, the portrait and the name move from the card to the list row on their own.
- **`.m-vt-pane`** is the one region a directional change swaps. Named things inside it stay part of it while it slides.
- Motet names its own parts: `m-chrome-start` and `m-chrome-end` (always on top), `m-world-mark`, `m-world-title`, `m-composer`, `m-draft`, `m-die`, `m-pane`.

## Timing

Groups move on `ease-spatial-default`; images cross-fade on the effects spring. Things that appear rise 6px and fade in; things that leave fade out on `dur-effects-fast` and settle to 98%. Panes travel 24px. A die tumbles on `ease-spatial-slow`. All of these are the World's springs, so a crisp World moves crisply and an eerie one slowly, inside the functional caps.

## Page loads

`raw.css` opts full page loads in (`@view-transition { navigation: auto; }`), and the same names carry across: the chrome stays put, a World's mark and name hold their place between its pages. A page that draws itself after it loads is not revealed half-drawn: Motet skips the transition when the page isn't there yet (`pagereveal`). Inside the app, a client-side router wraps each navigation in `Motet.transition` (SvelteKit's `onNavigate`), so route changes are same-document transitions with the same types.

In the design canvas each screen is a separate artboard that draws itself after it loads, so changes within a screen animate there and links between screens do not.

## Never

- No transition for typing, hovering, scrolling, or anything that happens many times a second.
- Nothing loops, pulses or moves on its own.
- No transition runs longer than the slow spatial spring, so the page stops taking input for at most that long.
- No transition carries meaning alone: the new state is complete without it.
