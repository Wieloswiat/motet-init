# SceneEvent

Something that happened between posts: a roll, a request for a roll, a tool's result, a change in the Scene. Events are not prose, so they never sit inside a post. Each is its own block in the stream, in time order, and stays where it happened when posts around it are edited or deleted.

- Props: `kind` (`roll`, `request`, `tool`, `system`), `title`, `expr`, `icon`, `time`, `character` and `account` (who acted), `detail` (replaces the generated who-line), `roll` (DiceRoll props), `body` (any nodes), `source` (`label`, `icon`, `href`: the tool that made it), `actions`, `lane` (`ic` by default, or `ooc`), `label` (accessible name).
- Anatomy: a marker in the portrait column, so events line up with portraits and prose; a quiet card no wider than the prose; a header with what happened, the exact expression, the time and who did it; the body; a footer naming the tool and holding actions. On narrow streams, and in Worlds without a portrait column, the marker goes and the card takes the full width.
- `roll`: the DiceRoll result fills the body without its own caption, since the header says what was rolled. A roll publishes the moment it is made. The post that answers it links back with `follows` on **Post**.
- When a request is answered, the card changes shape in place (`.m-vt-box`) and the die tumbles in once (`m-die`), as a `roll` transition.
- `request`: someone is asked to act, usually a GM asking for a roll. Raised card with the accent edge, like the composer and the "Your turn" flag. The action is shown only to the person asked; everyone else sees whom it waits for. Once answered, the result replaces the request in place and keeps who asked.
- `tool`: any other tool's output, from a first-party tool (initiative, a strategy turn's resolution) or, later, a sandboxed tile in `m-event__frame`. The card chrome stays Motet's whatever the body holds; see **Scene events and tools**.
- `system`: one line without a card: a round starts, orders close, someone joins the cast.
- The lane filter hides events with their lane's posts: in-character events with IC posts, `data-lane="ooc"` events with OOC posts.
- Accessibility: a `section` named by its title (and total for rolls); the marker is decorative; expressions and outcomes are text; the source line is always present for tool output.
