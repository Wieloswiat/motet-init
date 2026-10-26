# 0015. Adopt opensocial.group at a pinned revision

Status: Provisional (the proposal is a draft and will change)

## Context

The backend plan asks for a World to be a group other apps understand, using
opensocial.group's records where they fit and Motet's own under `at.motet`
otherwise. The proposal (https://tangled.org/opensocial.group/proposal) defines
two well-known Spaces per group (`group.opensocial.meta`,
`group.opensocial.members`), the records in them (profile, rules, labeler,
roles, permissions, membership, acceptance, access, the space index), a
per-user `invites` Space, permission sets, and the group host's methods.
Access is granted by OAuth Space scopes per role, not by named verbs, and the
group host is the OAuth authorization server for the group's DID.

## Decision

- Vendor the proposal's lexicons byte for byte at
  `2b41a9bf9171c03a1aa5ee26c6e39823ba27c79d` (2026-10-19) into
  `lexicons/group/opensocial/`, with `com.atproto.label.label`, the
  proposal's strawman label record, and the `com.atproto` definitions they
  reference.
- The proposal has no licence file yet. Its authors intend it to be public
  (https://atproto.com/blog/atmospheric-groups-intro). Asking upstream for a
  licence is left to the maintainer; `lexicons/README.md` records the gap.
- World creation follows the proposal: `meta` and `members` Spaces, each
  with an `access` record; a `profile` and a `permissions` record; the
  owner's `membership`, with a role that grants every scope and can assign
  every role; and a `declaration` in the public repo when `meta` is public.
- Motet's own Spaces (GMs, private scenes, applications) are modality Spaces
  under `at.motet.pilot.space.*` and are listed in the space index unless
  they must stay secret.
- Where Motet needs fields opensocial's records don't have, it adds sidecar
  records under `at.motet` (the plan's modelling rules), never fields in
  opensocial's schemas.

## Consequences

- `motet-lexgen` handles Space, permission-set, query, procedure, token and
  union definitions.
- A new revision of the proposal is an explicit bump of the vendored files.
  The code that writes these records changes with it.

## Revisit when

The proposal gets a licence, tags a release, or changes its Space or record
model.
