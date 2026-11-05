# 0020. AppView-backed web screens

Status: Accepted. Supersedes the demo-route and demo-data decisions in
[0012](0012-design-system-and-demo-screens.md); its design-system and Theme
Creator decisions still stand.

## Context

The AppView now provides authenticated browser sessions, public World and
scene projections, and procedures for World creation, scenes and posts. The
browser holds a World's owner rotation key. Demo screens cannot honestly
represent this workflow or its authorization and admission outcomes.

## Decision

Sign-in, World creation, World overview, scene lists and scenes use the
AppView. Real routes never fall back to sample World or post data. The
remaining design-export studies retain their demo content and are labelled
as such on `/design`.

Browser mutations use the shared HTTPS origin and its session cookie. Server
loads query the configured `MOTET_APPVIEW_PUBLIC_ORIGIN`, not the request's
Host header, before forwarding cookies. The owner key and backup passphrase
remain in the browser. Setup finishes only after the downloaded encrypted
backup has been re-imported and verified.

The AppView does not yet project World theme records. Real screens use
Motet's default appearance rather than a demo World's theme. Compile a real
World's theme for the server-rendered first paint when theme records become
available.

## Consequences

The AppView and its projections determine what the reader sees, so a newly
created World, scene or admitted post can take time to appear. The gallery
covers only the screens that still show samples; browser journey tests cover
the real routes.
