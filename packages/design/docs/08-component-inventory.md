# Component inventory

What Motet ships, what it is called in the libraries people already know, and what is left out on purpose. Names follow Ark UI where the three disagree, since Ark's state machines (Zag) are the likeliest engine for a Svelte layer.

## Controls and forms

| Motet | Ark UI | Chakra UI | Radix | Built on |
| --- | --- | --- | --- | --- |
| Button, IconButton | — | Button, Icon Button | — | `button` |
| Chip (filter) | Toggle | Tag, Toggle | Toggle | `button aria-pressed` |
| Segmented | Segment Group | Segmented Control | Toggle Group (single) | radiogroup |
| LaneFilter | Toggle Group (multiple, never empty) | — | Toggle Group (multiple) | two `aria-pressed` buttons in a group |
| Switch | Switch | Switch | Switch | `button role=switch` |
| Checkbox, CheckboxGroup | Checkbox | Checkbox | Checkbox | native checkbox |
| RadioGroup (list, cards) | Radio Group | Radio, Radio Card | Radio Group | native radios in a fieldset |
| Field, TextInput | Field | Field, Input | Label, Form | native input |
| Fieldset | Fieldset | Fieldset | — | `fieldset` |
| Textarea | — | Textarea | — | native, `field-sizing` |
| PasswordInput | Password Input | Password Input | Password Toggle Field | native input + toggle |
| NumberInput | Number Input | Number Input | — | `role=spinbutton` |
| Select | Select | Select | Select | select-only combobox + listbox |
| Combobox | Combobox | Combobox | — | ARIA 1.2 combobox |
| TagsInput | Tags Input | Tags Input | — | input + listbox |
| InlineSelect | — | — | — | button + listbox popup, inside a sentence |
| DatePicker, Calendar | Date Picker, Date Input | Date Picker, Calendar | — | text input + grid popup |
| TimeField | — | — | — | native time input + zone |
| Slider | Slider | Slider | Slider | native range |
| FileUpload | File Upload | File Upload | — | native file input in a label |
| ColorPicker | Color Picker | Color Picker | — | OKLCH plane (one `role=slider`), native hue range, typed L C H and hex |

## Disclosure and navigation

| Motet | Ark UI | Chakra UI | Radix | Built on |
| --- | --- | --- | --- | --- |
| Tabs | Tabs | Tabs | Tabs | links with `aria-current` (World navigation) |
| Accordion | Accordion | Accordion | Accordion | `details` / `summary` |
| Collapsible (default, spoiler) | Collapsible | Collapsible | Collapsible | button + `hidden=until-found` region |
| TreeView | Tree View | Tree View | — | ARIA tree, roving focus |
| Pagination | Pagination | Pagination | — | `nav` + links or buttons |
| Breadcrumb | — | Breadcrumb | — | `nav` + `ol` |
| Steps | Steps | Steps | — | `ol` with `aria-current=step` |
| Page | — | — | — | layout: solid `surface`, centred `main`, quiet `panel`, sign-in `card` |

## Overlays and feedback

| Motet | Ark UI | Chakra UI | Radix | Built on |
| --- | --- | --- | --- | --- |
| Menu | Menu | Menu | Dropdown Menu | top-layer popup |
| Dialog | Dialog | Dialog | Dialog, Alert Dialog | `dialog.showModal()` |
| Drawer | Drawer | Drawer | — | `dialog.showModal()` |
| Popover | Popover | Popover | Popover | top-layer `role=dialog` |
| HoverCard | Hover Card | Hover Card | Hover Card | top-layer preview, no controls |
| Tooltip | Tooltip | Tooltip, Toggle Tip | Tooltip | `data-tip`, shown on hover and focus |
| toast() | Toast | Toast | Toast | a chrome island, polite or alert |
| Alert | — | Alert | — | soft fill, icon and words |
| Progress, ProgressRing, Spinner | Progress | Progress, Progress Circle, Spinner | Progress | `role=progressbar` |
| Skeleton | — | Skeleton | — | static, no shimmer |
| DataList | — | Data List | — | `dl` |
| Avatar | Avatar | Avatar | Avatar | account round, character in World shape |

## Choices that differ from the libraries

- **Native first.** Checkbox, radio, range, time and file inputs keep the platform's semantics, keyboard handling and form participation; Motet only styles them. Custom widgets exist where a native popup can't wear a World (select, combobox, calendar) or doesn't exist (tags, tree).
- **Every popup is the same material and the same positioning code.** Listboxes, the calendar, popovers and hover cards use the frosted surface in the top layer, flip above their anchor when there is no room below, and stay inside the viewport. `Motet.popup` exposes it for anything new.
- **Words over colour.** Calendar today has a ring, selected days a fill; combobox matches are underlined and bold; step states and tag changes are announced.
- **Sentences where a label can't explain.** InlineSelect sets a value inside a sentence ("While I'm away, GMs may move [Lady Maren and Anwen] along") and summarises several values in words. It is for a few settings that read as one thought; ordinary fields stay the default.
- **Polish and English.** Matching ignores diacritics (ł included), date formats follow the locale (DD.MM.RRRR), and the week starts where the locale starts it.
- **Colour is picked against its duties.** ColorPicker greys out every colour that would miss a contrast duty on the grounds it is given, and hatches the colours past sRGB, so an author sees the safe region before choosing.
- **Time zones are shown.** A play-by-forum lives on deadlines across zones, so time fields print the reader's zone.
- **No motion for its own sake.** Skeletons don't shimmer, toasts don't bounce, indeterminate bars stop under reduced motion.
- **One call for motion.** Filtering, stepping, opening and posting all go through `Motet.transition()` as view transitions, named only while they run; no component animates layout on its own.

## Not yet, and why

| Component | Status |
| --- | --- |
| Web Tile | A sandboxed frame inside a Scene event, pinned by CID and run on consent; see **Scene events and tools**. |
| Table | Next, for member lists and character stat blocks; DataList covers simple sheets. |
| Editable | Inline editing invites accidental edits of published records; editing goes through the composer. |
| Context Menu, Menubar, Navigation Menu | No desktop-app menus; the chrome islands and Tabs cover navigation. |
| Rating Group, Pin Input, Signature Pad, QR Code, Marquee, Carousel, Tour, Splitter, Angle Slider, Timer | No current need in Motet. Carousel is out on principle: it hides content behind motion. |
