# Collapsible

One region a reader shows or hides.

- `Collapsible({ label, content, open, variant: 'default' | 'spoiler', topic })`.
- A button with `aria-expanded` controls a region hidden with `hidden="until-found"`, so find-in-page still reaches the text and opens it.
- `spoiler` is for posts: the topic is named, the content stays closed until each reader opens it, and the button says Show or Hide. Authors write spoilers in the post source; readers' choices are not saved.
