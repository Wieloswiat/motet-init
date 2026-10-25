# Reading and writing

## A Scene page

From top: the chrome islands floating over the World's art; the header window, where the art's focus point shows in full; the content sheet rising over the art with the World's edge, carrying the World mark (overlapping the edge), the title, who runs it and the World tabs; the scene heading in the display face, led by the World's flank, with its Location and Arc as fields and the lane filter; then the stream and, on wide screens, a sticky rail with the cast and scene facts. The composer ends the stream. On wide screens the art stays fixed behind the page and shows in the margins, where the sheet dissolves into it. There is no framed panel and no second image: the header and the background are the same art. The tall window belongs to the World's Overview; Scenes use the slim one.

**The lane filter** decides which posts the stream shows: in character, out of character, or both (the default). It is a split button of two toggles, one of which is always on: clicking the only lane switches to the other, clicking the other adds it, and with both on a click hides the lane clicked. That last rule is one switch in code (`Motet.lanePolicy.fromBoth`: `'unselect'` or `'solo'`). On wide screens the filter sits at the top of the sticky rail, under the chrome with the cast; on narrow screens it ends the scene header. Hidden posts take with them the dividers that only separated them.

## Posts

- Portrait column (56px) on wide containers, sticky so it stays beside a long post; inline 28px portrait on narrow containers or in `inline` Worlds; none in `log` Worlds that choose `portrait: none`. On narrow containers the account line runs under the time so handles keep their width.
- Line 1: character name (World display face if `type.names` is `display`, on a solid plate if `composition.names` is `plate`), then the epithet in `ink-muted`.
- Line 2: the writing account in platform style, an edit icon, display name and handle, and a role badge (GM, Moderator, Automation) drawn by Motet, never by the World.
- Top right: "New" when unread, an edited marker with the date in its tooltip, the date, and the actions menu.
- A post that answers a roll or a tool's result starts with a link back to it ("Follows Insight, 21"). The roll itself is never inside the post.
- Prose in the reading face. Actions under it as text buttons: Reply, Quote; content notes as read-only chips.
- **OOC posts** use the interface face on `surface-quiet`, account first, smaller avatar. They never take a World's card frame.

## Between posts

Rolls, roll requests, tool results and changes such as a new round are **Scene events** (see **Scene events and tools**): their own blocks between the posts, in time order, with a marker in the portrait column and a quiet card no wider than the prose. A roll shows the exact dice (DiceFont faces where the die has one), the total and the outcome, and publishes the moment it is made, so the prose that follows can't change it. A request is raised and accent-edged, with its action only for the person asked. A change of state is a single line.

## The composer

The editor dominates. One row above it states **who speaks** (character, changeable), **where** (Scene and lane) and **who can read** (audience button), with Expand at its far end. One row below it holds the formatting tools (Roll among them), then the draft state and the one filled button. Rolling from the composer publishes the roll as an event at once and attaches "Follows" to the draft. The draft state is a single icon (saved on this device, saving, saved to your account, offline, not saved) whose words are its tooltip and accessible name; offline and failure states also change its colour. The publishing account is not repeated here: the speaker menu shows which account each character belongs to. On narrow containers the lane chip goes first, then the scene name truncates; speaker, audience and Post never drop. Expanding the editor is a view transition into a fixed sheet; Back returns to the same anchor.

## Signals that must not merge

| Signal | Shape | Word |
| --- | --- | --- |
| Unread posts | accent bar on the row edge, bolder title, a count | the count and "unread" in its name |
| Response requested | accent-soft flag with a raised hand | "Your turn", "Response requested" |
| Draft | outlined neutral flag | "Draft" |
| Done | positive-ink check, no fill | "Done" |

Reading never resolves a request. Nothing pulses. A count is a number, never a dot.

## Scene lists

Columns are fixed, so rows line up whatever each scene carries: title and fields, then the signals (your turn, draft, unread count), then the cast, then the age. The cast is the least important column and the first to go: it hides below 640px, and below 460px the signals move under the title. Those widths are the column's, not the window's: World pages and Motet's pages make each layout column a container, so a list in a narrow column adapts the same way it does on a phone.
