# Composer

The writing surface: speaker, destination, audience and Expand above; the editor; tools, the draft state icon and Post below.

- Props: `speaker`, `portrait`, `speakerHue`, `scene`, `lane`, `audience`, `audienceIcon`, `sync` (`saved`, `syncing`, `synced`, `local`, `failed`), `text`, `placeholder`, `submit`.
- The draft state is one icon. Its words ("Draft saved on this device", "Offline. Draft saved on this device", "Draft not saved") are its tooltip and accessible name, in a `role="status"` element; `local` and `failed` also change its colour.
- The editor gets the space. On narrow containers the lane chip hides first, then the scene name truncates; secondary tools fold away below 520px. Speaker, audience, Expand and Post never drop.
- Expand is a view transition (`expand`, `collapse`) to a fixed sheet; the composer is named `m-composer` while things move, and its place in the stream keeps its height so the page doesn't jump. Switching speaker, lane or World never drops the text.
- Posting is a `post` transition: the text leaves the editor (`m-draft`) and settles into the new post's body while the composer moves down past it.
