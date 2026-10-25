# Post

A post: the character speaks, the writing account is always on the second line.

- Props: `character` (`name`, `epithet`, `portrait`), `account` (`handle`, `display`), `role` (`gm`, `mod`, `bot`), `lane` (`ic`, `ooc`), `time`, `edited`, `unread`, `follows` (`label`, `href`, `icon`: the Scene event this post answers), `notes`, `lead`, `body` (sanitised HTML rendered from Typst).
- Layout follows the World: `open` with dividers, `card` frames in the World's profile, `dossier` documents with a form header, or `log` rows; portraits in a sticky gutter, inline, or none. Names may sit on plates (`composition.names: plate`).
- On narrow streams the account line runs under the time so handles keep their width.
- Rolls and tool results are never inside a post: they are **SceneEvent** blocks in the stream. `follows` links back to one.
- OOC posts always use the interface face on `surface-quiet` and never take a card frame.
- Accessibility: an `article` named by speaker; the account handle carries the DID in its title; the edited marker has a text label.
