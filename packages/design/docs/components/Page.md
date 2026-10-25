# Page

Motet's own pages: Home, Inbox, Search, Settings and profiles. No World theme is active there, so the page is quiet on purpose and the World cards and rows on it carry the colour.

- `m-page__root` is one solid `surface` under the chrome islands: no frame, no rounded sheet, no art.
- `m-page__main` centres the content at up to 1200px (`--page-max`) and reserves `chrome-offset` at the top.
- `m-page__panel` is the quiet box for side navigation and filters (`surface-quiet`, a `line` border, `radius-lg`). Wide screens put it beside the content; narrow screens stack it above.
- Side menu and content (Settings, Manage) go in an `m-drill` row with `m-drill__nav` and `m-drill__content`. Below 900px it becomes a drill-in: the menu, then the chosen section with an `m-drill__back` button; `data-pane="nav"` or `"content"` says which is showing, and moving between them is a `forward` or `back` transition. Filters that are not navigation (Inbox) become one scrolling row instead.
- A row of filter chips uses `m-scroll-row`: one line that scrolls sideways on narrow screens, its edges fading only where there is more to see.
- `m-page__navItem` is a row in that panel: icon, label, an optional `m-count` at the end; the current one takes `aria-current` and the tonal fill.
- A single-task page such as sign-in uses `m-page__root--ground_canvas` with one `m-page__card` in the middle. Nothing else sits on `canvas`.
