# Scene events and tools

A post is writing: its author owns it, edits it and may delete it. A roll is a fact: made once, never edited, part of the Scene's record. The two never share a container. Rolls, requests, tool results and changes to the Scene are **Scene events**, each its own block in the stream, between the posts, in time order. The post that answers an event links back to it ("Follows Insight, 21") and the event stays where it happened.

## One container

Every event uses the same shell (`SceneEvent`, recipe `m-event`), so a tool added later looks native without new design work:

- **Marker** in the portrait column, so events line up with portraits and prose. It goes, with the column, on narrow streams and in Worlds without portraits.
- **Card** on `surface-quiet`, no wider than the prose. It never takes the World's post frame: events are Motet's, posts are the World's.
- **Header**: what happened, the exact expression in the mono face, the time, and who acted, as the character and the writing account. An automation shows as its account with the Automation badge, plus the GM who added it.
- **Body**: the result. Dice today; a table, a short form or a frame tomorrow.
- **Footer**: the source (which tool made it, as text with an icon, always present for tool output) and actions, right-aligned.

## Kinds

| Kind | Use | Shape |
| --- | --- | --- |
| `roll` | A dice roll, made by Motet dice | quiet card, DiceRoll in the body |
| `request` | Someone is asked to act: a roll, a choice, an order before a deadline | raised card with the accent edge, like the composer; the action only for the person asked |
| `tool` | Output of any other tool: initiative, a turn's resolution, a map update | quiet card, any body |
| `system` | One line of state: a round starts, orders close, the cast changes | no card, small marker |

A request appears in the asked person's inbox as "Response requested" and marks the Scene "Your turn". Reading never resolves it; answering does, and the result replaces the request in place, keeping who asked and when.

## Rules

- Events are never edited. A wrong roll is answered by another event ("Rerolled by the GM") or hidden by a GM with a visible note, never changed.
- Rolls publish the moment they are made, before any prose. Writing afterwards can't change what was rolled, and the order of the stream shows it.
- Events belong to a lane: in character by default, out of character when the tool says so. The lane filter hides them with their lane's posts.
- Posts cite events with `follows`; quoting a post never copies an event into it.
- Notifications, inbox excerpts and exports use the event's text: title, expression, total and outcome. Nothing an event says exists only as pixels.

## First-party tools

Dice (with roll requests) is the first tool, with full support: DiceFont faces, dropped dice, outcomes with tone, rerolls as new events. Turn order, deadlines and timers, and strategy-game turn resolution are next and use `tool` and `system` events. A World enables tools in Manage; automations run under their own accounts with the `bot` role, so a reader always sees that a machine, and which one, posted.

## Web Tiles, later

[Web Tiles](https://webtil.es/) (part of [DASL](https://dasl.ing/)) package a small web app as a MASL manifest (`name`, `description`, `icons`, `screenshots`, `sizing`, `resources`, `background_color`, `theme_color`, `prev`) and publish it on AT as an `ing.dasl.masl` record with its files as blobs, addressed by CID. A host renders a tile in a sandboxed frame on its own origin, with a strict CSP and no network beyond the tile's own resources; anything else goes through the host. That fits the `tool` event, with the tile in the `m-event__frame` slot. Nothing here is built yet; these are the constraints any implementation must keep:

- **Motet's chrome always wraps the tile.** Header with the tile's name and who added it; footer with the publisher's handle, a short CID and View record. A tile draws only inside its frame, so it can't imitate a post, a badge, a Motet button or another player.
- **Bounded size.** `sizing` sets the frame's height between 120 and 480px (`--tile-h`). Anything larger opens full screen through Open, the same transition as the composer's Expand.
- **Run on consent.** Before a tile runs, the event shows its screenshot and description with a Run button. A reader setting decides whether tiles run automatically, in Worlds they play in or never; GMs allow tiles per World, off by default, and may limit them to chosen publishers.
- **Pinned by CID.** An event names the exact tile version it ran. A newer version (`prev`) never changes events already in a Scene.
- **Mediated writes.** A tile can't post. It can ask Motet to publish an event or request a roll from Motet dice, and Motet shows what will be published, under which account and in which lane, before it does. The tile's text result is what the event stores.
- **Theme through tokens.** The host passes the World's resolved roles (surfaces, inks, accent, focus), the reader's mode and contrast and the type scale. `background_color` and `theme_color` cover only the loading state.
- **Accessible by contract.** `description` is required; the event's accessible name is its title; the stored text result is what screen readers, the inbox, notifications and exports read. Under forced colours the frame keeps its border. Off-screen frames are paused and unloaded.
- **Moderated like posts.** Any event is reportable (a `strongRef` to the event record); the tile record can be reported on its own, and a World's GMs can hide either.

What tiles will never do: sit inside a post, appear in the chrome, or run without a source line.
