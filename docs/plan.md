# Motet backend: research and plan

Oct 16, 2026 · @opliko

## Summary

Keep the backend in Rust, and split it the way Tangled splits its forge: an AppView that Motet runs, and tenors that anyone can run, the way anyone can run a knot. A tenor is what atproto now calls a group host: it hosts each World as a group account, doing for the World what a PDS does for a person, with its identity, its own records and its Spaces. People's posts and characters stay in their own repos. Moving a World off Motet's servers is an ordinary account migration to another tenor; no author's records have to move.

Five ideas carry the design:

1. **Repositories hold the published truth; projections are rebuildable.** People's repos hold what they write, and the World's account holds what the World decides. Motet and tenors also keep history, evidence and their own service state, each with a defined recovery source, and every query table can be rebuilt from those. "Rebuild gives the same projections" is a test from the first week.
2. **A post counts once the World admits it.** Anyone can put any record in their own repo; a post becomes part of a World when its tenor admits it, in a record written as the World that pins the exact version and gives it its place in the scene. Every consumer follows the same admissions, so they agree on what counts and in what order. Motet sends a post to its author's PDS and asks for admission at the same moment; the tenor checks permissions while the PDS writes, and reads the record back before admitting it. While a tenor is down, posts still publish and wait as pending. Whether a post belongs to the World's history, whether its author may post now, and where it appears are three separate questions.
3. **A World is a group account on its tenor.** It has its own DID, independent of any person, so its Spaces and labels survive a change of owner or tenor. GMs act for it through the tenor with their own accounts, and every such action is recorded with the GM who took it. The owner holds its rotation key; the tenor only signs. Where the opensocial.group proposal fits, Motet uses its records, so a World can be a group in other apps too.
4. **Spaces are part of the design, alpha or not.** GM material, applications and private scenes are built on Spaces in the first milestone, using atproto-crates' Rust implementation. The people building Spaces don't expect major changes, and Motet accepts the remaining churn. Real private data is limited to a closed alpha and an invite-only beta whose players know the risks, on Motet's own backed-up Spaces PDS; the open beta waits for Spaces to be finished upstream.
5. **Buckets for what must last, PostgreSQL for what must be queried.** Tenors keep their state the vlpds way: a group-committed log and SlateDB in an object-storage bucket, one fenced writer per shard, acknowledgements only after the log is durable, nodes that can be replaced. The AppView archives every verified record to a bucket too, so PostgreSQL's projections (with ParadeDB for search) can be rebuilt without re-crawling the network.

On language: Rust for the AppView and the tenor, TypeScript in the SvelteKit web app. The backend starts from scratch on atproto-crates. The September code is thrown away, though a few of its ideas are worth keeping (see Language).

**First milestone:** a World as a group account on its tenor; one public and one private scene; posts the tenor admits, and durable signed dice. It passes the rebuild test, and the World is recovered onto a fresh tenor from the owner's export with the old tenor stopped, including a private post by someone who has since left.

## What the backend has to do

Drawn from the screens on the canvas and the decisions already made (September 6 register, the architecture notes in the uploads).

| Area | What the backend does |
| --- | --- |
| Sign-in and accounts | OAuth with any PDS; several accounts per browser, each with its own private state; sessions held server-side |
| Worlds | Create a World (the five steps in Start a World), create its group account on a tenor, with its own DID and a handle such as `@gilded-reach.motet.at`, store theme, artwork, rules, content lines, recruiting and the retention policy people agree to before joining; hand over or archive |
| Membership | Applications with a character, GM review, roles (GM, player, visitor), per-member settings in the World |
| Characters | Characters and sheets owned by players, portraits, GM approval where a World asks for it |
| Play | Locations, arcs and scenes with a cast; posts in Typst, in or out of character, with replies, edits and the exact version others replied to; scene events between posts, with dice first-class; drafts; read position |
| Lore | A tree of pages with audiences that never widen below a parent |
| Home and Inbox | "Needs you" queue, notifications and unread counts across Worlds |
| Search | Across everything the reader may see, filtered before anything is counted or ranked |
| Moderation | Reports through `createReport` to the World, Motet or another service; the GM queue; World labels (`hidden`, `needs-cn`, `spoiler`); escalation sent as the World |
| Languages | Polish and English UI, a language on every post |
| Outages | People keep publishing to their own PDS through independent clients while Motet is down, and Motet catches up; while a World's tenor is down, posts still publish and wait as pending |
| Extension | GMs configure, bots automate: bots are ordinary accounts writing records, no plugin runtime |

**Leaving Motet.** A World moves to a tenor Motet doesn't run by ordinary account migration, and everything it consists of can be read with ordinary atproto tools. The AppView doesn't need to be self-hostable; nothing about the data depends on it.

Things the backend does not have to do: chat, voice, a battle map, a global social feed, arbitrary plugins or scripts, payments (non-goals in the product brief).

## Reference systems

### vlpds

A PDS in Rust whose only durable store is an object-storage bucket; every node holds caches only. The demo instance runs on a single 4-vCPU VPS with data on Cloudflare R2, and the design note reports about 60k commits a second on one 16-core node against a store with 25 ms of injected latency. It is new and says so.

What carries over to Motet, even though Motet is mostly an AppView:

- **The log is the only write-ahead log; everything else is materialized and rebuildable.** For Motet the log is the network itself: repositories plus the sync stream.
- **Records are the source of truth; derive what you can.** vlpds even rebuilds MST leaves from records instead of storing them.
- **Read-after-write is planned, not hoped for.** Indexers lag, so the author's own recent writes are served from a cheap per-repo log. Motet needs the same so a post appears the moment it's sent.
- **Retries are safe by construction.** It separates "not applied" from "maybe applied" and never resends an ambiguous write. Motet's write path needs the same rule.
- **Bound everything**: connections, in-flight requests, caches, backfill, slow subscribers.
- **Test with faults**: takeovers under load and an independent verifier of the output found bugs unit tests missed.

Motet can reuse more than these principles. The storage design fits tenors almost as it stands (see Worlds and tenors), and its log-as-archive idea fits the AppView. As a PDS, vlpds is a candidate for opt-in hosted accounts once it supports Spaces; its README doesn't mention them yet.

### Tangled core

A git forge on atproto. The monorepo is now about 44% Rust and 42% Go. The current AppView is Go with SQLite and reads from Jetstream; knots (git hosts) and spindles (CI) are separate servers people can self-host; Bobbin is a newer API-only AppView with no disk at all, rebuilt from an upstream indexer (Hydrant) on every restart.

What carries over:

- **A repo is a record, a DID and a knot.** The `sh.tangled.repo` record lives in the owner's PDS and names its knot and, if assigned, the repo's own DID (`repoDid`). The knot holds each repo's signing key, sealed under its master key. Knot 2, the Rust rewrite on gitoxide, keeps members and collaborators in git rather than a database. A Motet World is the same shape.
- **Authority at ingest.** A knot member record only takes effect if its author may invite to that knot (a Casbin check with domains per knot and roles such as `server:owner`, `repo:push`). Issue and pull state records count only if the author wrote the subject or holds `repo:push`. Motet's tenor checks posts the same way before admitting them.
- **Parking.** A state record whose subject isn't indexed yet is parked, retried when the subject arrives, swept hourly, and dropped after seven days. Motet needs this for posts that arrive before their scene, decisions before applications, and anything during backfill.
- **Self-hostable services, a central AppView.** Knots and spindles run anywhere; tangled.org runs managed knots for free and the one AppView. That's the split Motet wants: tenors anywhere, managed ones by Motet, one AppView.
- **Bobbin's critique of their own AppView**: no API because it doubles as the server-rendered site, hard to backfill, treats itself as the source of truth. Motet should be the inverse from the start: XRPC first, SvelteKit as one client of it.

What to avoid: their AppView admits a state record without a permission check when the knot is unreachable. Motet keeps such a post pending instead of failing open.

### Barazo and atBB

Barazo is the closest working example of a forum AppView: TypeScript on Fastify, PostgreSQL with pgvector through Drizzle, Valkey as cache, Tap filtered to its `forum.barazo.*` collections, OAuth through `@atproto/oauth-client-node`, Zod validation on every endpoint, AGPL, alpha. It runs either as one forum or as a global aggregator, keeps blocks and mutes as records on the user's PDS, and has a useful moderation checklist (first-post queue, word lists, link-spam checks, lock/pin, bans, a moderation log, maturity tiers with age gating).

atBB (Malpercio's version) puts categories, roles and moderation actions on a dedicated Forum Service Account and posts on each user's PDS. A Motet World is the same shape, done the group-host way: the World's account is hosted by a tenor anyone can run, and GMs act for it with their own accounts, each action recorded with their name.

### NodeBB

From the retrospective in the uploads: keep the single data-access boundary, the room model for live updates, and capability-style privileges. Avoid what caused its worst problems: an RPC bus where every method is remotely callable and auth is checked per handler (four of ten core CVEs), a key-value layer emulated over SQL, hand-written non-transactional upgrade scripts, and in-process write-path plugins. For Motet that means typed XRPC methods that declare their auth, PostgreSQL used natively, reindexing instead of migrations for derived data, and no plugin runtime.

### Group hosts and opensocial.group

Motet's tenors arrived independently at the role atproto now calls a **group host**. Bryan Newbold's "What is a Group Host?" (September 22) describes it as an account host that only hosts group accounts: it manages the group's identity and handle (including PLC recovery keys), hosts its public repo, acts as its Space host, and lets members act for the group through OAuth with their own accounts, within their roles. Apps get an OAuth session for the group through the same flow. Bluesky hasn't committed to a reference group host yet, and Tranquil is experimenting with one inside a PDS.

The Spaces team's "Developer Experience Goals for Atproto Spaces" (October 14) puts it the same way: "A group host does for groups what a PDS does for users", with the authority being a group DID so a group can change hands without members migrating, and each member writing to their own space repo.

The **opensocial.group proposal** (a draft by Daniel Holmgren, on Tangled) is the closest thing to a shared standard for such groups. Its model matches most of Motet's:

- each group is a DID with two well-known Spaces, `meta` (profile, rules, join policy) and `members` (roles, permissions, membership, the index of the group's other Spaces), and separate Spaces per app ("modality");
- flat roles that grant actions (`invite`, `admit`, `eject`, `role.assign`, `label`, `takedown`, `mod.read`, `mod.resolve`, `space.create` and so on), and per-Space access by role;
- membership written by the group and accepted by the member, invitations written into a Space the invitee hosts, join requests;
- the group as a moderation service over its own content: reports through `createReport` with open, escalated and resolved states, and labels written as records into the subject's Space;
- governance deliberately left to each group.

Motet should follow it where it fits, so a World is also a group other apps understand, and keep its own records (scenes, characters, rolls, applications with characters) under `at.motet`.

### Smaller references

- **Tap** (Bluesky): verified sync with backfill, per-repo ordering and acknowledgements; Go binary, SQLite or Postgres, beta. Jetstream is simpler but unverified and has no backfill.
- **Hydrant**: a Rust atproto indexer on fjall with filtering, XRPC queries and a cursor-backed stream; runs as a service or as a library inside your binary.
- **microcosm**: Slingshot (record and identity cache) and Constellation (backlinks), which Bobbin leans on while it warms up.
- **Ozone**: Bluesky's moderation tool; one instance per labeler.
- **contrail** (TypeScript, "atproto backend in a bottle"): has its own Spaces, but stores space records in the operator's database, unlike the upstream proposal where they stay on each author's PDS. Useful to read, not to copy.

## Architecture

&#91;embedded content: Motet's parts and where data flows · the AppView Motet runs, tenors anyone can run, records in people's PDSes\]

Writes go to the author's own PDS, and the World's tenor admits each post. Everything Motet shows comes back in through Tap or Spaces sync, and counts once a World record says it does.

### The parts

- **API and sign-in.** An OAuth confidential client: tokens stay on the server, encrypted, and each refresh is serialized across instances so none reuses a stale refresh token. It serves Motet's XRPC methods to the SvelteKit app and to anyone else, each method's access rule kept in a typed, fail-closed policy registry next to the lexicons (lexicons don't carry authorization), and one server-sent event stream per reader for live updates (NodeBB's rooms: per account, per scene) that resumes from a cursor after a reconnect.
- **Indexer.** Reads its own Tap's stream (Tap shares events out between connected consumers, so the AppView and each tenor run separate ingestion) and the Spaces it can read, validates each record against its lexicon, appends it to the archive in the bucket, matches each member record with the World record that makes it count (an admission, an approval, a roll result), writes projections, and parks what it can't place yet.
- **Workers.** Typst rendering, an image proxy, email and push, exports. Typst and images run in separate processes with time and memory limits because their input is untrusted.
- **Tenor.** A separate binary: Motet's group host, described in Worlds and tenors. Motet runs managed tenors, as Tangled runs managed knots; anyone can run their own, and the AppView treats both the same.

### Write path

1. The composer sends the post to Motet's API.
2. The API records an outbox entry (author, public or Space destination, the record, an operation ID, the expected previous state), then sends two requests at once. One writes to the author's PDS with a create-only or compare-and-swap condition (`swapRecord`, `swapCommit`). The other asks the World's tenor to admit the record, with its CID, the operation ID and, for an edit, the admission it replaces. That request is authenticated as the author, through the author's PDS or with a short-lived, method-bound service token Motet fetched ahead of time.
3. An ambiguous outcome on either is reconciled by reading back before any retry. A PDS retry keeps its preconditions, and an unexpected current version becomes a conflict to resolve, never something to overwrite. An admission retry with the same operation ID returns the same outcome, and an operation ID is never reused for a different request.
4. When the PDS answers, the author sees the post, marked pending. The tenor, which has checked the request meanwhile, reads the record from the PDS and commits the admission, checking permission again at that moment; then the post appears to everyone in its place. If the tenor can't be reached, the outbox keeps asking. If it refuses (the author was just removed, or the scene closed), the post stays in the author's repo outside the World, and Motet says why and offers to delete it. The index tracks each record's repo revision, so a late or older stream event can't roll the display back.
5. Independent clients write to their PDS and nothing else, though they may call the admission method too. The relay carries the commit and Tap delivers it to the tenor and to Motet; a private post reaches the tenor through the Spaces write notification, or through reconciliation if that is lost. The tenor admits either like any other.

### Read path

Pages are built from PostgreSQL projections in admission order, filtered by what the reader may see before anything is counted or ranked. Typst is compiled once per record version into a typst.ts vector artifact that the browser draws at the reader's width and theme (see Services and infrastructure). It's cached under a key that also names the compiler and renderer versions, packages, fonts and layout variants; private renders and images never go through the public CDN.

### Spaces path

Spaces have no relay. The indexer asks each World's tenor for a credential to read that World's Spaces (as an app the World allows, with a member's delegation token), registers for write notifications, pulls each member's space repo from their own PDS, verifies it, and indexes it with the Space's audience attached, beside the World's admissions in that Space. A credential reads a whole Space, so every audience is its own Space: the GMs, each private scene's cast, each application under review.

### What Motet runs, and what it must back up

Motet runs the AppView and managed tenors. It can also host PDS accounts for people who want one, which matters while most PDSes can't do Spaces yet. It indexes, renders, caches and retains writing, but it is never the canonical host or the only keeper of anyone's.

PostgreSQL's projection tables rebuild from the bucket archive. The outbox, sessions, read positions and notification-delivery state don't; they're backed up like any database, and replaying the archive never resends an email or issues a roll. A tenor rebuilds from its own bucket, which needs backups of its own (see What a tenor keeps).

## Worlds and tenors

A **tenor** is Motet's group host. The name comes from the medieval motet, where the tenor is the voice that holds the piece up while the other voices add their words; a tenor does that for a World.

### A World is a group account

| Part of a World | Where it lives |
| --- | --- |
| Identity | Its own DID (`did:plc`), independent of any person, and a handle such as `@gilded-reach.motet.at` |
| Profile and rules | The World's `meta` Space (opensocial), public for a public World |
| Look, artwork, recruiting, retention, game settings | A Motet Space of the World, pointing at the profile; artwork as blobs on the World's account, never the founder's |
| Roles, permissions, membership, the index of its Spaces | The World's `members` Space (opensocial), readable by members; a public World can publish a roster in its profile instead of opening it |
| Admissions, decision effects, scenes, lore, dice results, labels | Records written as the World, in its public repo or the Space whose audience needs them; reasons stay in the GM Space |
| Posts, characters, applications, roll requests | Their authors' own repos and Space repos |

Earlier drafts of this plan put the World record in the founding GM's PDS and gave the World a DID with no repository behind it. The group-host model is simpler: the World's own records sit in its own account, and moving the World is a normal account migration. People's writing stays in their own repos either way, and the World's records are no use without the World anyway. It also protects the World when its founder hands it over and later deletes their account: their posts go with them, but the World stays with its new owner.

### Acting as the World

GMs never get a password for the World. Each GM signs in to the tenor with their own account and gets their own session to act as the World, so on every change the tenor knows which GM is acting and checks that GM's current roles; an actor named by Motet is informational only. Motet never routes all GMs through one shared World session: shared sessions are only for named automation, and are audited as such. Every action taken as the World is recorded with the GM who took it, as an audit record in the GM Space (or, for a moderation case, in the tenor's case store), which keeps the September decision that GM decisions stay attributable, and the trail moves with the account. That record is the tenor's statement that a GM acted; guarding against a tenor that fabricates decisions would need GM-signed intents, which Motet doesn't plan for now.

The World's own records have one writer, its tenor, so they are ordered among themselves. Two GMs changing the same thing at once is caught by compare-and-swap and shown to the second GM: the explicit resolution chosen in September.

### Who controls the identity

- **The owner holds the rotation key; the tenor holds signing keys.** Records keep flowing without the owner, but any identity change (handle, tenor, keys) needs the owner's signature. This is the stronger guarantee, and the one credible exit needs: PLC lets any rotation key change the document, including removing the owner's, and a higher-priority key can only undo that within 72 hours.
- **A tenor-held rotation key is an opt-in convenience** for owners who won't manage keys, the way a PDS usually holds one for its accounts. With it, recovery depends on noticing within those 72 hours, so the owner's client, not the tenor, watches the PLC log and alerts on changes it didn't expect.
- **A lost owner key freezes the identity**: the World keeps running on its tenor, but can no longer move or change handle. So the key backup is part of Start a World from the first milestone: generated on the owner's device, kept separate from the key that decrypts the World's backups, exported as an encrypted file with the World's DID and recovery instructions, and checked by re-importing it before setup counts as done.
- **Transfer is one recoverable workflow**: owner role, rotation key, backup recipients and delegated sessions change together. The DID, and everything that refers to it, stays.
- **Other DID methods.** Nothing in the records assumes PLC. `did:web` on a domain the owner controls long-term is possible, with two costs: no recovery if the domain is lost, and no public history of past keys. A `did:web` under `motet.at` would tie the World to Motet's domain forever, so Motet won't offer it.

### What a tenor does

- **Hosts the World's account**: identity and handle, public repo and blobs, Space repos, and per-GM sessions for acting as the World.
- **Admits posts.** A post naming one of the World's scenes is checked when it reaches the tenor: its author is in the scene's cast and isn't sanctioned, and the scene is open. If so, the tenor writes an admission as the World, in the scene's repo or Space, pinning the post's version and giving it the scene's next place; if not, it tells the author why. Requests come from Motet's API as people post, from Spaces write notifications, and from sync for everything else.
- **Space host and authority** for the World's Spaces: credentials, writer lists, write notifications, revocation, and access per Space by role. As the Spaces proposal specifies, the space host stops tracking writes from repos no longer allowed to write, so a removed player's private posts never reach admission.
- **Group management**: roles, membership, invitations and join requests as opensocial.group defines them. Applying with a character opens a review Space for that one application: the applicant can write and read their own submission there, reviewers' discussion stays in the GM Space, and the Space closes if the application is withdrawn or expires.
- **Moderation service**: takes reports through `createReport`, issues signed labels for public content, writes label records into Spaces for private content, and records World-scoped sanctions (a posting suspension, a removal) as decisions it enforces, since a label only changes what is shown.
- **Dice**: for an action that allows one roll, a GM action or a versioned game rule tied to that action first opens a roll opportunity (World, action, actor, slot) binding the expression, rules version and character-sheet version; issuing a result fills that slot, so ten requests can't fish for a good roll. Free-form rolls stay available but never fill a restricted slot. Each request gets exactly one result, committed to the World's repo before it's returned; a timeout means pending, never a new roll, and a request that can't complete (an expression over the limits) ends in a recorded failure rather than another attempt. A roll request never opens an opportunity by itself. In a public scene the World's signed commit authenticates the result. Space commits are deliberately deniable, so a private roll that needs portable proof carries its own signature over the result.
- **An API** for GMs and apps, authenticated with standard service auth, so a self-hosted tenor trusts nothing about Motet. Every mutation declares its actor, World, subject, required capability, expected version and idempotency, and generic record writes go through the same checks. Admission requests are part of it, so independent clients can ask directly. A tenor can add its own governance on top, such as notice periods or N-of-M approval.

A tenor learns about members' records the way the AppView does, through Tap for public records and Spaces sync for the rest, so it admits posts from any client; Motet's requests only make that faster for its own users.

### What a tenor keeps

A tenor is an account host, so most of what it keeps is the World's repos. Beside them are its own outputs: reports received, credentials issued, label state. It keeps all of it the vlpds way, as an operational design (durable state in object storage, replaceable compute) more than a throughput one:

- Worlds are grouped into storage shards, each with exactly one fenced writer. Even a single-node tenor refuses a second process writing the same state.
- A node group-commits entries from its shards into one log in the bucket; SlateDB holds current state and replay checkpoints. What gets reused is the whole persistence subsystem with its fault tests: durable log prefixes, ambiguous-write handling, fencing, checkpoints, replay, garbage collection.
- **Acknowledged means durable on the tenor.** Process loss can't undo an acknowledged write.
- **Recoverable by the owner is a separate promise.** The owner's export runs on its own cycle (initial targets: increments every five minutes, a full verification daily, both to be measured). Anything after the last export is inside a declared loss window, and a restore never turns "possibly already rolled" into "safe to roll again".
- A single-node tenor starts with one shard; multi-node takeover is built and tested before it's turned on.
- **The primary bucket is not a backup**: protected snapshots and a tested restore, including the master key that unseals the Worlds' keys.

vlpds now documents Spaces support (a Spaces section and a page on where it follows and diverges from the reference PDS), so before assembling vlpds's storage with atproto-crates' Spaces code, compare that against vlpds as it stands. SlateDB fits the single-writer model; its pluggable write-ahead-log API is still an RFC, so the log-as-WAL arrangement follows what vlpds does today rather than a SlateDB feature.

### Moving or recovering a World

Moving a World is an account migration, the same as moving a person's account to another PDS: its repo, its blobs (transferred separately, as in any migration), its Space repos, a PLC operation, an import. Members' own repos never move; a new tenor reads their Space content with credentials under the same World identity, without them signing in again. What migration doesn't carry is the tenor's own state: public labels (signed objects outside the repo), open reports, issued credentials. The move rotates the old tenor's signing key out in its PLC operation, per-GM sessions and the AppView's Space credentials are set up again at the new tenor, and the new tenor takes no writes until it holds the World's whole authority state and inventory.

The owner's export covers that gap, and the case where the old tenor won't cooperate. It is encrypted before it leaves the tenor, with keys the tenor doesn't hold, split into chunks under a signed manifest (format and rules versions, checkpoint, hashes, what is covered and what isn't), and stored as blobs referenced from a record in the owner's own PDS, plus a copy the owner can download. It holds the World account and its Spaces; a privileged inventory of every Space and every writer it has had (the members-visible index would give away secret scenes); critical outcomes such as decision effects, roll results and pending publications; and labels with their negations. Open reports go in only with their own permission and with case restrictions intact, so a report about the owner never reaches the owner through an export.

The tenor writes the increments itself, through a narrowly scoped OAuth grant from the owner's PDS (blob uploads and the backup manifest's collection only), encrypting to the owner's public key, because a browser can't keep a five-minute schedule. Older increments are folded into periodic full snapshots so the owner's PDS doesn't fill up.

| Failure | What recovery needs |
| --- | --- |
| The old tenor is gone, members' PDSes are fine | The World account, its authority configuration, the Space and writer inventory |
| The old tenor and a member's PDS are gone | The above, plus any copy of that member's records the World was allowed to keep |
| An author deleted content the World may not keep | A tombstone |
| Motet itself disappears | Material outside Motet's control: the owner's downloaded copy, a PDS Motet doesn't run |

Restarting, moving and recovering promise different things:

| Situation | What survives |
| --- | --- |
| Restart | Everything acknowledged: the tenor replays its log |
| Move | Everything: the old tenor hands over a consistent state and the current repo heads before the new one takes writes |
| Recovery from an older export | Everything up to the export. The new tenor writes a recovery declaration (the export's checkpoint, a new epoch, what's covered and what's unresolved) and reconciles the rest from surviving copies: Motet's archive of the World's records, members' repos, other consumers. Admissions it can confirm are issued again in the new epoch; requests it can't reconcile stay pending, and roll opportunities that may have been used stay undecided until a GM rules, never re-rolled |

Places carry their epoch, so read positions and links from before a recovery are translated or marked stale, never read as places in the new sequence. The World's repo continues at a higher revision, announced with a sync event as atproto requires; that explains the repo, not the lost game outcomes, which is what the declaration is for. The current erasure ledger is applied before anything restored is served, and content whose erasure state is unknown stays quarantined until reconciled.

The first-milestone test: recover a World onto a fresh tenor from the owner's export with the old tenor stopped, including a private post by someone who has since left. It doesn't prove surviving the loss of Motet while the owner's PDS and backups are hosted by Motet; that comes later.

### What stays with Motet

The AppView: indexing, search, Inbox, the web app. It doesn't need to be self-hostable. Any authorized AppView reading the same records, admissions included, derives an equivalent World; its access, platform moderation and presentation may differ.

## Rules and guarantees

The architecture holds only if a few behaviours are pinned down before the lexicons stabilise. Each one below ends in something testable.

### Trust boundaries

| Who | Trusted with |
| --- | --- |
| A player or an independent client | Their own repo, nothing more: what they write counts only through a World record |
| A GM | The World actions their roles allow, checked on each action; not every private Space or moderation case |
| The World's owner | The World's identity and its recovery; not the content members wrote |
| The tenor | Enforcing the World's rules, ordering its decisions, issuing dice and running its Spaces. It can refuse or delay and it reads what it hosts, but it can't change anyone's signed records, and the owner can move the World away from it |
| A person's PDS | Hosting their repo and Space repos, and saying what they currently hold |
| Motet's AppView | The private content it indexes, served only to readers allowed to see it; no part in governing a World |
| A visitor | Public content only |

Private Spaces keep data from people and apps without access. They don't hide it from Motet, the PDSes involved or the tenor, which process it, as the Spaces proposal intends: Motet promises access control and safe rendering, not end-to-end secrecy. Space commits are deniable, so private history recovered from a tenor's copy proves less about authorship than a public repo does. Motet states that rather than adding signatures to every private post.

### Authority: the World admits posts

This is atproto's ordinary model applied to Worlds. Records are canonical in their authors' repos and nobody else can change them; apps compute views over them, and moderation adds labels on top. A GM's decision is a record too, written as the World: it changes how the World's view is computed, never anyone's post. Bluesky's reply controls work the same way: a reply that breaks a thread's rules stays in the replier's repo, and the AppView leaves it out of the thread.

What needs pinning down is that every consumer computes the same view. Relays and Tap order events within a repo, not across repos, so the AppView may see Alice's post before her removal from the scene while another consumer sees them the other way round. The World's own records have one writer, its tenor, so they are ordered among themselves. Admission builds on that: the tenor decides, in one order, which posts count, and writes it down as the World.

This reverses the September 6 baseline, which chose records plus explicit decisions over an ordered acceptance ledger because the host was "just an AppView". A tenor is the World's account host, and private play, dice and moderation already need it up. What September asked for still holds: while the tenor is down, people keep publishing to their own PDS, and their posts wait as pending. The earlier architecture notes had the same shape (AR-033: acceptance records in the World's own repo).

- **Admission decides what counts.** A post is part of a World's history once an admission, written as the World, pins its exact version (URI and CID) and gives it its place in the scene. The tenor writes one only after reading the record from the author's PDS, so an admission never points at a record that doesn't exist, and it checks the author's permission at the moment it commits (see Implementation contracts). The admission names the authority revision it was checked against, which moves with every cast, sanction or scene change in that audience, not just the rules version. The order consumers received things in, and an author's timestamp, decide nothing.
- **Every member record counts through a World record.** Posts through admissions, character sheets through approvals, rolls through results, membership through the World's membership record. Consumers check that each pair matches rather than re-running the rules. Motet still runs the same rules code to warn an author before sending a post the tenor would refuse.
- **Three separate questions.** Whether a post is part of the World's history (its admission), whether its author may post now (current roles and sanctions), and where it appears (its place). Removing a player doesn't touch their admitted posts; banning them and hiding their posts is done with labels, as anywhere in atproto.
- **A removal governs what reaches the tenor after it.** The tenor orders removals and admissions in one sequence, so a post committed after the removal isn't admitted, whatever it cites and however early its admission was requested, and one admitted before it stays. If the GMs want an admitted post gone, they label it like any other. A refused post stays in its author's repo, outside the World.
- **Edits and deletions.** An edit is a new version, and its admission keeps the post's place. Its request names the admission it replaces, so an edit arriving after a newer one is a conflict, not an overwrite. A former cast member's edits aren't admitted, so their posts stay as admitted. A deletion is always honoured without the tenor: the post leaves a tombstone in its place, or the World's archived copy where its policy keeps one. When the tenor sees the deletion, it closes the admission with a record of its own, so a post recreated later, even byte for byte, needs a new admission.
- **Decisions point at what they change.** Decisions are immutable: a revocation, correction or supersession is a new record pointing at the exact earlier version. World records are only written through the tenor, which enforces this for generic record writes too. A decision that vanishes or changes anyway is a consistency problem to reconcile, never a revocation timed by whoever noticed it first.
- **Conflicts are scoped** to the subject or capability. Two GMs changing the same thing at once is caught by compare-and-swap at the tenor and doesn't freeze anything else; a disputed GM-role change is resolved by the owner or GMs who aren't party to it.
- **Parking bounds work, not truth.** A deadline stops automatic retries and marks a record unresolved; a late dependency can still make it valid. An admission can reach a consumer before its post does; it waits for the post and is never shown alone.
- **Display order is the admission order.** Places are numbered within a recovery epoch (see Moving or recovering a World), so a place from before a recovery never takes on a new meaning. Replies still pin the version they answer, and a post's `createdAt` is shown as when it was written, but neither orders the scene.

### What admission costs

Posting gains a step. Motet sends the post to the author's PDS and the admission request to the tenor at the same moment, so the tenor checks permissions while the PDS writes. It then reads the record back from the PDS before committing. Otherwise a player could collect admissions for unpublished posts while in the cast and publish them after being removed. For a private post, the PDS's write notification says when to read; for a public one, Motet passes on the PDS's answer as a hint.

|  | With admission |
| --- | --- |
| Author sees it | When the PDS write returns (roughly 50–300 ms depending on the PDS, an estimate to measure), as their own post marked pending |
| It joins the scene | After the PDS write, one read of the record by the tenor and the tenor's durable commit, for which vlpds targets about 40 ms at p50 and 150 ms at p99 on S3-class storage. A distant self-hosted tenor adds its network round trips |
| Others see it | On Motet once the admission is in; elsewhere after sync, the admission travelling through the relay alongside the post |
| Independent clients | Admitted when sync or a write notification brings the post to the tenor, or sooner if the client calls the admission method itself |
| Throughput | Not a concern: a busy World stays well under a post a second, and numbering a scene on a single writer is cheap |
| Storage | One small record per admitted post version, and one more when an admitted post is deleted |

Admission also gives every scene a stable sequence. Read positions and pages use it directly, and the World's repo becomes an index of its own history, which helps backfill and recovery.

| When | What happens |
| --- | --- |
| The tenor is down | Posts still publish to their authors' PDSes and nothing is lost. They wait as pending, shown to their author (and to the GMs where the World allows it), and are admitted when the tenor returns, in the order it receives them; Motet's outbox resends in posting order. Public play pauses the way private play, dice and moderation already do |
| Motet can't reach the tenor, or loses its answer | The post stays pending a little longer; the tenor admits it from sync anyway, and a retry with the same operation ID gets the same outcome |
| The PDS write fails | The tenor never finds the record, so nothing is written to the World's repo, and the operation ends as failed |
| A player is removed | Permission is checked when the tenor commits, so anything committed after the removal is refused, however early it was requested, and anything admitted before it stays |
| A write notification is lost | Reconciliation of the World's Space repos finds the post anyway (see Implementation contracts); the reference PDS stops retrying a notification after 24 hours |
| The tenor is restored from an older export | It declares a recovery and starts a new epoch (see Moving or recovering a World). Requests it can't reconcile stay pending instead of being judged against old membership |
| The tenor misbehaves | It can delay, refuse or reorder posts, but not forge or alter them, since each admission pins a CID the author's repo signed. That's no more than it can already do through moderation, and the remedy is moving the World |

### What's kept, and what it comes back from

| Kind of state | Examples | Recovered from |
| --- | --- | --- |
| Current published content | Posts, characters, the World's records as they are now | Authors' repositories and the World's account |
| Retained history and evidence | Earlier versions, past decisions, observed deletions | The AppView's archive, or the World's account export |
| Service-originated state | Reports, issued credentials, pending publication | The tenor's log or the AppView's database, with backups |
| Derived projections | Membership views, search documents, scene indexes | Replaying the three above |

- **Old versions are kept as observed.** Motet preserves the versions it saw, as the World's retention allows. A version replaced or deleted before anything archived it may be gone; a reply or report that pins it then shows "this version isn't available", never the current text in its place. A post created and deleted entirely during an outage may never have been seen at all.
- **The archive is trusted, not proof.** Tap delivers verified records as JSON, not signed commits with proofs, so the archive rebuilds a deployment that trusts its own ingestion. The indexer archives or durably queues an event before acknowledging it to Tap, which delivers at least once.
- **The archive holds more than records**: account and identity lifecycle changes, label issues, negations and expiries, stable event identities, rules and archive-format versions, and the retention and erasure ledger. Former authors stay enrolled while their contributions can still change.
- **Events keep their identity.** Operations (create, update, delete) are stored separately from content; content can be deduplicated by CID, operations can't, or a delete followed by a re-post would vanish.
- **Compaction keeps the evidence, not the prose.** Dropping deleted content never drops what explains surviving posts: admissions, decision effects, deletion and account-state markers, active moderation, idempotency outcomes. Erasable content is stored apart from that evidence.
- **Rebuilds have no side effects.** Replaying never resends an email, a push or a credential, and never issues a roll.
- **"Rebuild gives the same projections"** means: from the same archive cut, rules version and retention and erasure state. A restored older backup applies the current erasure ledger before anything is served.
- Reindexing replaces projection migrations, not schema evolution, archive-format compatibility or rules versioning.

**Deleted posts.** By default nothing is archived: a deleted post disappears and a tombstone marks where it was. A World can instead choose an archive policy, shown before anyone joins. Its tenor then writes the last version it observed into the World's own repo, in the same Space, as a record naming the author and the original, so the archive belongs to the World and moves with it. Erasure requests the law requires still apply to archived copies.

### Permission facts versus deliberation

Admissions settle which posts count, but readers still need the facts behind them: who is in a cast, which scene is closed, who is suspended. An AppView that reads a World through an ordinary player's credentials can't see the GM Space, so those facts can't live only there.

opensocial.group already draws the line Motet needs: roles and membership live in the World's `members` Space, which every member can read, and profile and rules in `meta`. The same goes for Motet's own decisions: whatever an admission depends on (a cast, a sheet approval, a closed scene, a sanction) is published as its effect to the audience that reads the posts, while applications, reports, GM notes and reasons stay in GM-only Spaces or the tenor's case store. So any reader with access can check an admission against the facts it relied on, and private reasons are never exposed just to make authority checkable. A public World's `members` Space stays members-only: anonymous readers rely on the World's admissions, and the World can publish a roster in its profile. Anonymous access to Spaces is itself still a proposal (public spaces for 0016, open since October 13), so public discovery shouldn't depend on it.

### Audience lifecycle

- **New members see the whole history of the Spaces they're admitted to.** There's no play-by-forum reason to hide earlier posts; joining a World doesn't open its private scenes.
- **Former members follow Spaces' own semantics**: they lose access to the World's Spaces and keep their own posts, which never left their repos. Nothing is retained for them; keeping their access would mean sending them a copy anyway.
- **Each private scene is its own Space**, with a cast role of its own, because a Space can't hide records from some of its readers and opensocial grants access by role. Role assignments are visible to all members in the `members` Space, so an ordinary private scene shows who is in it. A truly secret scene or cast needs a Motet access extension that the tenor enforces and only the cast can see; Motet shouldn't promise secrecy until that exists.
- **Roles add up, they never subtract.** Removing one role may leave another route to the same Space, so bans and suspensions are sanction decisions the tenor enforces, not a "banned" role.
- **Lore audiences nest at all times**: a child page's readers are always a subset of its parent's. The check runs on every role, membership, access and parent change, not only when a page moves. The child points at its parent, so a public page never lists private children.
- Widening an audience is its own explicit operation, never the side effect of moving a page or editing a cast.
- "Stop serving", "keep a cached copy during an outage" and "delete retained copies" are three different actions.
- Renewing a Space credential needs a member's delegation; when the AppView loses its last usable session for a Space, it stops serving that Space's content and says so.

### Private data in the alpha and beta

Building on the Spaces alpha is a deliberate choice, and the design doesn't hedge against it. Real private data follows the release stages instead:

- **Closed alpha and invite-only beta**: real private play, applications and GM notes are allowed, because every participant is told the risks when invited. They play on separate accounts hosted on Motet's own Spaces PDS, leaving their main accounts where they are (the reference PDS's Spaces build), since their usual PDS can't do Spaces yet and the Bluesky-hosted sandbox is for experiments and will be deleted.
- **What Motet adds to make that reasonable**: protected snapshots of its PDS and tenor buckets with a tested restore; the owner's account export; and a way to carry Space data across a breaking alpha change (upstream says PDS migrations will likely change before the production release).
- **Open beta**: only after Spaces has its upstream security review and production release, so anyone can play from their own PDS.

The Spaces alpha still hasn't had a careful security review, and Motet's backups don't change that; the invitation is where people learn it.

### Implementation contracts

- **Endpoint authorization**: one typed, fail-closed registry of who may call what, enforced centrally, not remembered per handler. Independent clients reach Motet's methods the standard way, through their PDS with the atproto-proxy header, which mints service auth naming the caller, the service and the method (lxm). The tenor checks the authenticated caller, the audience (the World's DID with the tenor's service fragment), the method and the expiry; a DID inside a request is never the caller. Motet publishes a permission set for its own collections and methods; a set can't include another namespace, so opensocial and com.atproto scopes, and blob, account and identity permissions, are requested alongside it.
- **Serialized World commands**: each World has one command stream in its storage shard. A mutation (an admission, a removal, a roll, a GM action) checks permissions against the state at that point in the stream and persists its outcome atomically. The records it produces in the World's repo and Spaces are published afterwards, idempotently, since atproto has no transactions across repos, so a scene closure and its audit record can appear at slightly different times, and consumers park what refers to something not yet seen.
- **Operations**: every authoritative mutation carries an operation ID, never reused with a different body, and an expected version, and returns pending or a final outcome. Independent clients can query an operation's state, so a lost response never needs a second attempt.
- **Conditional writes**: the Spaces `putRecord` in the reference implementation takes no swap conditions yet (checked against the October 15 snapshot), and an upstream discussion reports concurrent admins overwriting each other. Motet's own Spaces PDS must enforce them before the alpha, tested through the deployed endpoint; the tenor enforces them for the World's records. Other PDSes must advertise the capability before Motet relies on it.
- **Per-reader authorization**: a Space credential lets Motet sync a Space, not serve it to everyone. Motet checks separately whether each reader may see it, with a freshness bound for private content (60 seconds as a starting target) and immediate effect when a revocation is known; if the check can't be renewed within the bound, that audience stops being served until it can. That covers cached pages, media, downloads, search, SSE replay and queued notifications, and switching accounts in the browser switches the reader everywhere. References are Space-aware: the same author, collection and key can exist in two Spaces, and a CID says nothing about who may read it.
- **Verified ingestion**: the AppView and tenors evaluate the same verified inputs. Jetstream may be used for discovery, followed by verification, never as an equal source. A new repository enrolls by writing a Motet profile record, and that is part of the published client contract, not an undocumented prerequisite.
- **Space reconciliation**: write notifications are for speed, not completeness. The reference PDS gives up on a notification after 24 hours of retries, and the space host stops tracking writers who lost access, so a first private post during a long tenor outage, or a former member's deletion, would otherwise be missed. The tenor and the AppView reconcile Space repos on startup, after an outage or a notification gap, when membership changes, and on a schedule, choosing whom to check from membership and audiences, former writers included, not only from the writer list. An unreachable PDS is not a deletion: the last verified state stays, marked stale.
- **Outbound HTTP**: one hardened fetcher for identity, OAuth, PDS and media requests (the OAuth spec names SSRF and malicious responses). Every private request, cache entry and live subscription carries an explicit account.
- **Live updates**: `NOTIFY` only wakes listeners; SSE clients resume from a cursor or refetch, and notifications are never the only record of a change.
- **Rendering**: Typst runs in a worker with no network, sessions, credentials or unrelated files; packages are fetched outside it, with archive paths, sizes and hashes checked. Typst can load plugins from raw bytes, so the worker's limits, not the package denylist, are the execution boundary. Cache keys include the compiler, renderer and security-policy versions and the layout variants; environmental inputs such as the date are fixed. The browser draws vector artifacts only from Motet's render cache, never from a record or another service, since typst.ts doesn't describe the format as safe to load from untrusted sources, and artifact size and drawing work are budgeted. A restrictive Content Security Policy is the script boundary; the renderer's DOM insertion paths, its HTML layer included, are audited for the pinned version, and a link becomes clickable only once its scheme passes an allow-list. The drawing goes into a shadow root, which keeps styles apart but isn't a security boundary. A generic sanitizer over the whole output could strip what the renderer needs, such as SVG glyph references, and no separate origin is needed. typst.ts's docs say nothing about screen readers, so Groundwork tests one; if the renderer falls short, text extracted from the compiled document goes alongside the drawing, since escaped source is no reading of a post. A failed render leaves the post with an actionable error, never an older render shown as if current, and deterministic failures are cached. Verified dice, moderation controls and status are drawn by Motet outside the rendered content, since an author can draw a convincing fake roll in Typst.
- **Search authorization**: readers get only the results and metadata they may see. Authorization is part of the query's candidate selection, before any top-K or truncation, so a page never comes back short or with wrong counts, and counts, facets, snippets, autocomplete and pagination stay inside it. Check the query plans, since whether ParadeDB applies a filter inside its search depends on the query's shape. Ranking may use corpus-wide BM25 statistics: Motet doesn't promise that a hidden document leaves everyone else's ranking unchanged. Moderation cases stay out of general search.

### Acceptance tests

| Test | What it shows |
| --- | --- |
| Feed two consumers the same records in different cross-repo orders | They show the same posts in the same order |
| Remove a player while their post is in flight, in a public and a private scene | One outcome, admitted before the removal or refused after it, and every consumer agrees |
| Stop the tenor while players post, then restart it | Posts publish and show as pending, then are admitted from the outbox and sync, none lost or doubled |
| Request admission for a post whose PDS write fails | No admission is ever written, and the operation ends as failed |
| Edit a post after its author has left the cast | The edit isn't admitted; readers keep the admitted version or see that it isn't available |
| Delete an admitted post and recreate it byte for byte | The old admission doesn't apply; the recreation needs its own |
| Two edits of one post arrive out of order | The stale one becomes a conflict, not an overwrite |
| A first private post during a notification outage longer than 24 hours | Reconciliation finds it |
| A former member deletes a private post | The deletion is found without a notification |
| Typst emits hostile markup or malformed vector data | No script runs, links stay inside the allow-list, and drawing work stays bounded |
| Restore from an export after readers saw admissions now lost | A recovery is declared, old read positions can't alias new places, and unreconciled requests stay pending |
| Remove a member, then rebuild | Historical contributions and current permissions stay separate |
| Two clients edit a private record against the actual hosted PDS | The stale conditional write fails |
| Time out a publication while another client edits the same record | Reconciliation never overwrites independent work |
| Reuse a GM's session after their role is removed | The tenor rejects the action |
| Many requests for one restricted roll slot, with retries | One outcome survives |
| Return a roll, lose the requesting client, then migrate | Issuance survives without the result being published |
| Restore an export older than a possibly completed roll | No silent second roll |
| Recover a private scene containing former members' posts | Recovery isn't limited to current members |
| Recover with both the old tenor and a former author's PDS gone | Retained content comes back; missing coverage is stated |
| Rebuild projections with no network access, after compaction and an erasure request | The archive is enough, nothing erased reappears, no side effects repeat |
| Stop the API and the indexer, not just the web app | The outage test exercises real ingestion recovery |
| Revoke Alice while Bob keeps Motet's Space credential alive | Alice loses every serving path within the declared bound |
| Change a cast role after a lore subtree exists | Audiences still nest |
| The same CID in two Spaces | Caches and media stay within the right audience |
| Report a GM who can read the GM Space, then check their audit and export access | Nothing about the case reaches them |
| Search with private documents that match | Only authorized hits, counts, facets, snippets and suggestions come back, including when a page comes back short; rankings may vary |
| Start a second writer against the same bucket | Fencing stops conflicting acknowledgements |

## Data model and lexicons

Record families: opensocial.group's where a World is simply a group, and Motet's own under the settled pilot namespace `at.motet.pilot.*` (shortened to `…` below). Exact names and fields are still an open gate; what matters now is who writes each one, where it lives, and when it counts.

| Record | Lives in | Written by | Counts when |
| --- | --- | --- | --- |
| `group.opensocial.profile`, `.rule` | The World's `meta` Space | As the World, by a GM | Always |
| `….world.config` (look, artwork, recruiting, retention, game settings), pointing at the profile | A Motet Space of the World | As the World, by a GM | Always |
| `group.opensocial.role`, `.permissions`, `.membership`, `.space` | The World's `members` Space | As the World, by a GM whose role allows it | Always; the tenor enforces it |
| `group.opensocial.access` | The Space it controls | As the World, by a GM whose role allows it | Always |
| `group.opensocial.acceptance` | The member's repo in the `members` Space | The member | Shows them on the roster |
| `….application` (with a character) | That application's review Space | Applicant | Until admitted, declined or expired |
| `….character`, `….character.sheet` | Player's repo | Player | Where a World requires approval, once the approval's effect names that version |
| `….decision` (effects: sheet approved, scene closed, sanction) | Wherever the audience that must evaluate it can read; reasons in the GM Space | As the World, by a GM | Immutable; superseded only by a newer decision |
| `….audit` (which GM or automation acted as the World, on what) | The World's GM Space; moderation cases stay in the tenor's case store | The tenor | Always |
| `….location`, `….arc`, `….scene` | The World's public repo, or a Space for its audience | As the World, by a GM or a player the World lets open scenes | Always |
| `….post` (Typst source, scene, character, reply, language, self-labels) | Author's repo, or the author's repo in the scene's Space | Player or GM | An admission pins this version |
| `….admission` (post version, scene, epoch and place, authority revision, the admission it replaces) | The World's public repo, or its repo in the scene's Space | The tenor, as the World | Always; one per admitted post version, and one closing it when the post is deleted |
| `….recovery` (checkpoint, new epoch, coverage, unresolved outcomes) | The World's public repo; sensitive details in the GM Space | The tenor, as the World | Always; written once per recovery from an older export |
| `….roll.opportunity` | The scene's audience | As the World, by a GM | Restricts one action to one roll |
| `….roll.request` | Author's repo, or the scene's Space | Player or GM | The author is in the cast |
| `….roll.result` | The World's repo, or its repo in the scene's Space | The tenor, as the World | Always; one per request, one per opportunity slot |
| `….page` (lore, Typst, pointing at its parent) | The World's public repo, or a Space for its audience | As the World, by a GM | Its audience nests inside its parent's |
| `group.opensocial.label` | The World's repo in the subject's Space | As the World, by a GM | Moderation of Space content |
| `….draft` (destination, base version) | The author's personal Space | Author | Never indexed for anyone else |
| `….backup.manifest` | The owner's own PDS | The owner's client | Describes the encrypted export |

Reports are not records: they go through `createReport` to the World's DID or another service, and their case material stays in the tenor's case store with access per case. Labels for public content are ordinary signed atproto labels from the World's label key. Content notes and spoilers an author marks themselves are self-labels on the post, which atproto already defines; a World's `needs-cn` label is for when the author didn't add one.

### Modelling rules

- **Refer to a World by its DID**, never by a record's URI: the DID is what survives a change of owner or tenor.
- **Extend standard records with sidecars.** Where a standard record is close to what Motet needs (opensocial.group's, or another community standard), Motet adds a sidecar record in its own namespace, as atproto currently recommends, instead of adding fields to someone else's schema. It shares the target's record key where that helps discovery, but always carries an explicit reference to its target, pinned to a version when an approval depends on it, since sidecars can live in a different Space or repo. Unknown values in a sidecar never grant anything. A World's configuration is a sidecar to its opensocial profile; play-by-forum details about a member are a sidecar to their membership. Expect this pattern often.
- **Pin versions with strong references.** Replies, admissions, decisions, roll results and reports carry URI plus CID, so they name the exact version they answered. A plain URI is for "whatever is current", like a post's character.
- **An edit is a new CID under the same key.** The index keeps the versions it observes, as the World's retention allows, which is what lets a report show the reported version and a reply show what it replied to (see What's kept).
- **Typst source goes in the record; rendered output never does.**
- **Records describe one thing each.** No World mega-record and no command log; composable records were already the proposed direction (AR-003).
- **Unknown fields survive.** Records written by newer clients pass through untouched.

### What the index stores

- Every record seen: URI, CID, author, collection, JSON, first-seen time, deleted or not. The same stream is appended to the bucket archive first, and PostgreSQL is rebuilt from that archive rather than from the network.
- Projections per entity (World, member, character, scene, post, roll, page) with the authorization data they need.
- A capability table: who may do what in which World, granted by which decision, since when, computed by the rules code the tenors also run.
- Parked records with the reason; a deadline ends automatic retries and marks them unresolved, never invalid.
- Labels from each World's tenor, render cache keyed by CID, search documents, inbox fan-out, sync cursors, sessions and keys.

Post order within a scene is the admission sequence, numbered within a recovery epoch (see Rules and guarantees); a post's `createdAt` is shown as when it was written, never used to order. Read positions are places in that sequence. An unread count is the number of visible posts after a reader's place, an ordinary indexed query, because deletions, hidden posts and scene events that don't count leave gaps.

Drafts live in the author's personal Space, behind a browser working copy with queued autosave, which needs the conditional writes described under Implementation contracts. Read positions and notification delivery start in PostgreSQL, kept apart and referring to scenes and post versions rather than database rows.

## Language: Rust or TypeScript

|  | Rust | TypeScript |
| --- | --- | --- |
| atproto libraries | atproto-crates (identity, MST/CAR, lexicon validation, OAuth, XRPC, service auth, firehose, Jetstream, Tap, Spaces, attestations); Jacquard 0.12 (with lexicon codegen and Axum extractors); rsky; Atrium | The reference implementation (`@atproto/*`), atcute, worked forum examples in Barazo and atBB |
| Spaces | atproto-crates (MIT): atproto-space and atproto-space-client cover credentials signed with RFC 9421 HTTP Message Signatures, set-hash commits, CAR sync, writer sets and revocation; its PDS includes a space host. The Spaces crates don't appear on crates.io yet, so they'd be git dependencies | `@atproto/space` alpha snapshots from Bluesky |
| Sync | Tap beside the binary (atproto-crates has a client), or Hydrant as a library inside it | `@atproto/tap` client for Tap |
| Typst | The compiler is a Rust library: compile in-process with a custom file and package resolver; typst.ts adds a vector format and a browser renderer | Typst through WebAssembly, or a Rust sidecar anyway |
| Moderation tooling | Labeler endpoints are small; nothing off the shelf | Ozone, and community labeler kits |
| Safety | No prototype pollution and far fewer memory-safety bugs; authorization, injection and SSRF still need the same controls (NodeBB's worst bugs were authorization in its RPC layer) | Needs strict runtime validation everywhere (Zod or similar), and still exposed to prototype pollution |
| Shared types with the web app | Generate TypeScript from the same lexicons | Same language end to end |
| Footprint | One small binary, steady memory | Larger, but fine at Motet's scale |
| Speed of change | Slower to write, slower to compile | Faster to write and to hire for |

**Go** has indigo, Tap and Jetstream, and was Tangled's first language, but Tangled's repository is now slightly more Rust than Go, and a third language buys Motet nothing it can't get by running Tap as a binary.

### Recommendation: Rust, with eyes open about Spaces

Typst decides more than it seems: the canonical content format is a Rust library, and rendering untrusted Typst safely means controlling its file and package access, which is easiest in-process. Add the fact that the community's newer atproto infrastructure (vlpds, Hydrant, microcosm, a growing share of Tangled) is Rust, and the safety record against NodeBB's failure modes.

Spaces is a smaller price than I first wrote. atproto-crates already implements the 0016 draft's client side in Rust, and its PDS has a space host, the piece a tenor needs most. Two practical cautions: the Spaces crates aren't released yet, and they track the draft pinned to a commit of the proposals repo. Alpha churn itself is an accepted risk, since the people working on Spaces don't expect major changes. Code existing isn't proof it interoperates, so the Groundwork spike runs a pinned end-to-end combination: a separate authority host, credential renewal, private writes, and replacing the host. Motet keeps its own seam (`motet-spaces`) over the crates.

Starting fresh, the base is atproto-crates. It covers Spaces, Tap, firehose and Jetstream consumers, service auth, OAuth for confidential clients and record attestations (useful for signed dice) with one set of core types. What it doesn't offer is Rust types generated from lexicons, so Motet writes its own generator. Jacquard has one, but its next versions are expected to break compatibility again and development is slow, so it isn't something to build on. The same generator produces TypeScript types for the web app from Motet's, opensocial.group's and com.atproto lexicons.

From the September code, keep the ideas, not the code: OAuth tokens and DPoP keys sealed at rest; one refresh at a time per session across instances; an outbox with leases; a write whose outcome is unknown is reconciled by reading it back before any retry; a failed Spaces write never falls back to a public one.

TypeScript stays where it already is: the SvelteKit web app, and types generated from the lexicons for it. Upstream components Motet doesn't need to change (Tap, the reference PDS for test accounts) run as they ship rather than being rewritten.

**Revisit if** the Rust Spaces stack falls behind the alpha for long enough to block work. The fallback is not splitting authority between two runtimes (rejected in AR-022); it would be a whole-backend decision, made once.

## Services and infrastructure

| Need | Start with | Why, and what comes later |
| --- | --- | --- |
| AppView database | PostgreSQL, used natively (relational tables, JSONB for raw records, real transactions) | One database, no key-value layer on top (NodeBB). |
| AppView archive | Verified records appended to group-committed segments in a bucket before PostgreSQL applies them | vlpds's "the log is the only write-ahead log": PostgreSQL's projections rebuild from the bucket, and the rebuild test runs without touching the network. Deleted records are dropped when segments are compacted, or kept where a World's retention says so |
| Tenor storage | A bucket only: a group-committed log plus SlateDB, one fenced writer per shard, one shard to start | Disposable nodes; a self-hoster needs one binary and R2, S3 or MinIO. vlpds reports a one-shard server idling inside R2's free request tier |
| Search | ParadeDB (`pg_search`) inside the same PostgreSQL | BM25 on Tantivy, a Polish Snowball stemmer, vector and hybrid search in the same index. BM25 ranks all results, with authorization inside the query before any truncation (see Search authorization). The free edition is single-node, with replication in the paid edition, which suits an index that can be rebuilt. Community edition is AGPL-3.0; check it against Motet's licence (Barazo is AGPL too). The alternative is pg\_textsearch (Timescale, PostgreSQL licence, PostgreSQL 17 and 18), which ranks with BM25 over PostgreSQL's own text search configurations; PostgreSQL ships none for Polish as far as I know, so it would need a Polish dictionary. Both register an access method named `bm25`, so it's one or the other per database |
| Public sync | Tap, tracking every repo that writes a Motet profile record (its collection signal), with backfill | Verified and ordered per repo, unlike Jetstream. Hydrant as a library is the later option if one fewer process matters. Tenors can use the same setup or a narrower one: only their World's members |
| Spaces | `motet-spaces` over atproto-crates, in both binaries | Client side in the AppView, space-host side in tenors |
| Jobs and outbox | A PostgreSQL table with leases and `SKIP LOCKED` | Leases make a crashed worker's jobs safe to retake. A broker only after measurements (AR-008) |
| Live updates | Server-sent events per reader; PostgreSQL `LISTEN/NOTIFY` between instances, as a wake-up only; clients resume from a cursor | NATS or similar only if one instance stops being enough |
| Images and artwork | Stay on the author's PDS (World artwork on the World's account); a Motet image proxy fetches, re-encodes and caches them in the bucket behind a CDN | Like Tangled's `camo`. Re-encoding strips anything hostile in the file |
| Typst | typst.ts's Rust library (`reflexo-typst`, over the `typst` crate) in a worker process: no file or network access, packages only from Motet's mirror, time and memory limits. Each post version is compiled once into typst.ts's vector format, with the layouts a reader needs (light and dark, narrow and wide) in one artifact; the browser draws it with typst.ts's renderer, WebAssembly without the compiler | typst.ts's "Form 2": compile on the server, draw in the browser, so a post fits the reader's width and theme instead of being a fixed image. The vector format only promises compatibility between matching compiler and renderer versions, and typst.ts doesn't mean it for other languages, so artifacts are a cache keyed by those versions and rebuilt from source after an upgrade. typst.ts builds on a patched Typst: its stable 0.7 is on Typst 0.14.2 and its 0.8 release candidates on 0.15, while Typst itself is at 0.15.1, so the worker's Typst follows typst.ts, and cmarker and packages must fit it. Independent clients and readers without JavaScript get a static SVG and the escaped source. HTML export (still behind a feature flag in Typst 0.15) comes later under an allow-list of elements, attributes, CSS and URLs, since Typst can build arbitrary HTML. Starting limits to tune, for all of a post's layouts together: 64 KiB of source, 512 MiB of memory, 2 CPU seconds, 5 seconds of wall clock, 8 MiB of output. The composer previews with typst.ts's in-browser compiler at the worker's exact version. Markdown goes through cmarker inside Typst. Any @preview package may be imported: Typst requires an exact version in every import, so each post already pins its packages, and the mirror fetches each name and version once and keeps it, so old posts render without Typst's servers. A denylist handles packages that misbehave; packages' WebAssembly plugins run inside Typst's own sandbox |
| Dice | The tenor rolls with the OS's secure random source and commits one result per request to the World's repo before returning it, naming the algorithm | Standard dice notation, with the dice-roller library's notation as the reference. A subset first: NdM, d% and fudge dice, parentheses and integer +, -, \*, keep and drop highest or lowest, exploding on the maximum face, successes against explicit comparisons. Integers only (atproto records have no floats), a cap on the values one request can generate, and no explosion that can't end, such as on a one-sided die. Rerolls, unique, compounding, penetrating and roll groups later; the grammar version goes in every result. Verifiable randomness (drand, from the original notes) can come later without changing old rolls |
| Keys | AppView: sessions sealed with a key kept outside the database. Tenors: World keys sealed under the tenor's master key; the owner holds the rotation key (a tenor-held one is opt-in); snapshots and a restore test cover the master key | A cloud KMS later |
| Handles and endpoints | Wildcard DNS and TLS for `*.motet.at`: World handles and managed tenors' endpoints | Self-hosted Worlds can use their own domain |
| Opt-in PDS accounts | The reference PDS's Spaces build, run and backed up by Motet, for alpha and beta players | atproto-crates' PDS or vlpds once they're ready, for people whose PDS can't do Spaces |
| Email | Any SMTP provider | Push notifications later |
| Observability | `tracing` with OpenTelemetry, Prometheus metrics | Alerts on sync lag, parked records, failed publications, tenor lease churn |
| Deployment | Containers: the AppView, PostgreSQL with ParadeDB, Tap, a tenor, a reverse proxy; one bucket (MinIO in development) | More AppView instances behind the proxy when needed; tenors scale by adding nodes on the same bucket |

## Moderation

### Every World is a labeler; its tenor does the work

The World's DID document names a label key and a labeler endpoint, both on its tenor. From the outside each World is an ordinary moderation service:

- **Labels** are signed with the World's key and served through `queryLabels` and `subscribeLabels`.
- **Reports** arrive as `createReport` through the reporter's PDS, with a service-auth token addressed to the World's DID, and land in that World's queue in Manage. Reports about a GM reach the other GMs only. Case material (the report, the actions taken on it and the audit entries naming it) lives in the tenor's case store with access per case, never in a Space the reported GM can read, and exports follow the same rule. Reports about the owner, the only GM, or the operator of a self-hosted tenor go from the client straight to Motet's own moderation, bypassing the tenor.
- **Escalation** "sent as The Gilded Reach" is an ordinary report from the World's account: the tenor hosts it, so it can send `createReport` as the World the way any PDS does for its accounts.

Ozone runs one instance per labeler, which doesn't fit Worlds; a tenor's labeler is small (sign, serve, take reports) and serves every World on that tenor. Motet's own platform moderation can still use Ozone.

The World's public repo also carries the `app.bsky.labeler.service` declaration with its label values and accepted reasons, so general atproto apps can list a World as a moderation service.

### Acting as the World, with names attached

A GM's moderation action is taken as the World and recorded in the audit trail with the GM's name and a note. Labels change only what is shown: undoing one is a negation (`neg`), a temporary label carries an expiry (`exp`), hiding uses the system value `!hide`, and the World declares its own values (`needs-cn`, `spoiler`). Bans and suspensions are different: they're World-scoped sanction decisions the tenor enforces, because a label grants and revokes nothing. World labels go on content in the World, not on accounts, since an account-level label carries no World scope and other apps subscribing to the World would read it as applying everywhere.

### Spaces change the rules

The Spaces proposal says a moderation service should not publish public labels about records in a Space, because the label alone leaks that the record exists. For private scenes the World writes a label record into the subject's own Space instead, as opensocial.group proposes, so only that audience sees it. Any outside service needs its own Space credential to moderate there.

### What Motet applies

Inside a World, Motet applies the World's labels and decisions to everyone, as it always applies Motet's own moderation. As anywhere in atproto, moderation never changes content. Banning a player and removing their posts means labels that hide them, applied by Motet and by any app that subscribes; the posts stay in the player's repo. Content only disappears without its owner's consent when their PDS removes it, for example for unlawful content, and Motet's PDS does that only for the accounts it hosts.

## Phased plan

&#91;embedded content: Phased plan · five phases, four gates, no dates\]

Each phase ends at a gate rather than a date. The phases are engineering; the release stages ride on them: a tiny closed alpha once First World works, an invite-only beta (mainly people from Wieloświat) once the alpha's worst problems are fixed, and an open beta once Spaces is finished upstream.

| Release stage | Who | Private data |
| --- | --- | --- |
| Closed alpha | A handful of testers who know the risks | Yes, on Motet's own Spaces PDS, backed up by Motet |
| Invite-only beta | Invited groups, mainly from Wieloświat | Yes, same setup, risks explained in the invitation |
| Open beta | Anyone | Yes, from any Spaces-capable PDS, after the upstream security review and production release |

| Phase | Done when |
| --- | --- |
| **Groundwork** | Decided: the admission rules and state machine (what the tenor checks and when, operation identity, edits, places and recovery epochs, conflict scope); identity control; which opensocial.group records Motet adopts and where it needs its own; what the owner's export contains; where each permission fact lives. A pinned Spaces spike runs end to end: separate authority host, credential renewal, private writes, host replacement, and conditional writes enforced by Motet's own Spaces PDS (patched if needed). vlpds's Spaces support is compared against assembling atproto-crates' Spaces code. A light legal check covers the closed alpha's data handling. `docker compose up` brings up the AppView, PostgreSQL with ParadeDB, Tap, a tenor, a Spaces-alpha PDS and MinIO. Lexicon drafts are reviewed against the screens, and the first version of Motet's own lexicon generator runs. A typst.ts spike draws a post's vector artifact in the browser at two widths and in both themes, and is tried with a screen reader |
| **First World** | Start a World creates the World's group account on a tenor, with the owner's recovery key and a basic key backup; GMs act as the World with their own accounts, and the audit trail names them. A player applies and joins, posts in Typst in a public and a private scene, admitted by the tenor (posts written while it's stopped wait as pending, then are admitted), and rolls dice with a durable signed result. Publication uses conditional writes. An independent client, following the published enrollment contract, posts while Motet's API and indexer are stopped. Projections rebuild with no network access. The World is recovered onto a fresh tenor from the owner's export with the old tenor stopped, including a former member's private post; a recovery from an export older than the last admissions declares itself, and private posts missed by notifications are found by reconciliation. Motet runs its own backed-up Spaces PDS with hosted accounts, and the closed alpha starts |
| **Running Worlds** | Applications with review, roles and settings; Manage with reports, labels, undo and escalation; Inbox, unread and notifications; search with ParadeDB; Polish and English. End-to-end authorization tests across search, media, Inbox, cached pages and live streams. Carrying Space data across a breaking alpha change has been rehearsed. A scoped legal review is done: the lawful basis and retention for each kind of personal data, erasure and export procedures, a notice channel that doesn't need an atproto account, a licence covering hosting, rendering, migration and archives, and how the 18+ rule is recorded and enforced. A one-off importer, kept outside the platform, brings beta groups' history over where they want it (the alpha starts fresh). The invite-only beta starts |
| **Self-hosting** | The recovery path First World tested, packaged and documented for people running their own tenor; full log export and import; multi-node tenors once their fencing and takeover tests pass |
| **Opening up** | Spaces has its upstream security review and production release, and Motet's own privacy and security tests pass: the open beta starts, from any Spaces-capable PDS. The independent-client contracts have been public since First World, so this phase only widens them |

## Risks and open questions

### Risks

| Risk | What to do about it |
| --- | --- |
| Spaces `putRecord` has no conditional writes in the reference implementation, so concurrent edits overwrite each other | A Groundwork gate: Motet's own Spaces PDS enforces them, tested through the deployed endpoint; other PDSes must advertise the capability |
| Most people's PDSes can't do Spaces yet, and even a GM needs one from the first World | Alpha and beta players get accounts on Motet's own Spaces PDS; for the open beta, show clearly in Start a World and Join which parts need a Spaces-capable PDS |
| Group hosts are new: no reference implementation, and the alpha PDSes are untested against a Space host that isn't the authority's own PDS | The tenor implements the role itself, with atproto-crates' PDS and space host and vlpds's Spaces work as references; the Groundwork spike tests a separate host, credential renewal and host replacement |
| opensocial.group is a draft and will change | Adopt its records where they fit, keep Motet's own under `at.motet`, and pin a revision, with a short compatibility profile of the records, group methods, OAuth scopes and Space host behaviour Motet supports |
| Spaces changes under us (accepted: the people building it don't expect major changes), and its Rust crates aren't released yet | One seam (`motet-spaces`); pinned versions and git revisions; backups and a rehearsed way to carry Space data across a breaking change |
| A tenor outage now pauses public play too | Posts still publish and wait as pending, then are admitted when it returns; managed tenors are monitored like any account host, and self-hosters are told an outage pauses their Worlds |
| An owner loses the only rotation key | The World keeps running but its identity is frozen. A key backup with a re-import check in Start a World; the opt-in tenor key trades this for a 72-hour watch |
| A restore from an older export repeats a roll or reuses a scene's places | A recovery declaration and a new epoch; possibly used opportunities and unreconciled requests stay undecided until a GM rules, never re-rolled or re-judged |
| Two correct consumers disagree about what counts | They follow the World's admissions instead of re-running its rules; the different-arrival-order test |
| A tenor delays, refuses or reorders posts | It can't forge or alter them; each admission names the authority revision it was checked against and each refusal gives a reason, so GMs and players can check them; the owner can move the World to another tenor |
| Space write notifications are lost (the reference PDS gives up after 24 hours) | Periodic reconciliation of Space repos, former writers included |
| A report reaches the GM it's about | Case material in the tenor's case store with access per case; reports about the owner or a self-hosted tenor's operator go straight to Motet |
| A shared Space credential keeps serving a removed reader | Per-reader authorization with a freshness bound; the revocation test |
| A recovery misses Spaces or former writers | The privileged inventory in the owner's export; the former-member recovery test |
| A tenor's bucket is deleted or corrupted | Protected snapshots and a restore test that includes the master key |
| A delayed write overwrites someone's edit | Conditional writes; conflicts instead of overwrites; the timeout test |
| Object-storage costs per request | Group commit and large segments, as vlpds does; watch PUT and GET counts as a first-class cost |
| Typst as an attack surface: endless loops, huge output, raw-byte plugins, unsafe output | The worker's limits as the execution boundary, packages only from the mirror, vector artifacts only from Motet's worker, drawn under a restrictive CSP with checked links and the renderer's DOM paths audited, a strict policy before any HTML |
| typst.ts trails Typst and patches it; cmarker's current release needs Typst 0.15, which typst.ts has only in release candidates | Pin typst.ts, Typst, the renderer, cmarker and the package set as one set and key the render cache by it; the plain typst crate with a static SVG is the fallback |
| Deleted posts that later play depends on | No archive by default, just a tombstone; a World's archive policy is shown before joining. Agreeing to it doesn't remove anyone's data-protection rights, which the legal review covers |
| Dice trust: the tenor sees results first | One committed result per request and per opportunity slot; the signature proves the issuer, not fairness; verifiable randomness can come later with a provider field already in the schema |
| Tap is beta | Replaceable behind the indexer's input; Hydrant is the alternative |
| ParadeDB: AGPL licence, and single-node in the free edition | Check the licence against Motet's early; the search index is rebuildable, so single-node is acceptable at first; pg\_textsearch with a Polish dictionary is the alternative |

### Still open

- Exact NSIDs, fields and which Space holds what (open since September).
- Which play-by-forum extensions opensocial.group's records need as sidecars; its membership, roles, invitations, removals and join policy look like they cover the core.
- Whether opensocial access can name a private scene's cast directly, or each cast needs a role of its own, and what a secret-cast extension looks like.
- What a public World's roster shows; the `members` Space itself stays members-only.
- Admission: the exact state machine and operation fields; whether short-lived, method-bound service tokens fetched ahead of time work with the PDSes Motet meets, or requests go through the author's PDS; whether GMs see pending posts. The record keeps its name, and its methods and permissions get names that can't be read as opensocial's admit, such as `….play.submitAdmission`.
- The default identity arrangement: owner-only rotation (recommended) or a tenor-held key with a 72-hour watch.
- The owner's export: encoding (CAR and Space snapshots plus event segments to start), chunk size against PDS blob limits, how increments fold into full snapshots, whether open reports go in it, and what a recovery declaration records.
- The XRPC surface and permission set for independent clients, and the tenor API, published from First World.
- Typst: the denylist policy, final limits, how a render failure shows, which layouts each artifact carries, and whether typst.ts's renderer works with screen readers.
- Dice: timeouts, and which game rules open roll opportunities.
- The 18+ floor and the rest of the legal policy, through the scoped review before the beta.

## Sources

Read for this plan (October 16, 2026):

- atproto fundamentals: [Understanding atproto](https://atproto.com/guides/understanding-atproto), the [protocol overview](https://atproto.com/specs/atp), [atproto for distributed systems engineers](https://atproto.com/articles/atproto-for-distsys-engineers), [The atproto ethos](https://atproto.com/articles/atproto-ethos), the [labels spec](https://atproto.com/specs/label) and the [permissions spec](https://atproto.com/specs/permission)
- [Developer Experience Goals for Atproto Spaces](https://atproto.com/blog/devex-goals-for-spaces) (October 14), [What is a Group Host?](https://bnewbold.leaflet.pub/3mw5al7ocuk2t) by Bryan Newbold (September 22) and the [opensocial.group proposal](https://tangled.org/opensocial.group/proposal/)
- [The Atproto Spaces Alpha is Live](https://atproto.com/blog/atproto-spaces-alpha), [Spaces alpha updates](https://discourse.atmosphere.community/t/atproto-spaces-alpha-updates/1129) (through October 15) and [Proposal 0016: Permissioned Data](https://raw.githubusercontent.com/bluesky-social/proposals/main/0016-permissioned-data/README.md)
- [vlpds.jazco.dev](https://vlpds.jazco.dev/), [jazware/vlpds](https://github.com/jazware/vlpds) and its [DESIGN.md](https://raw.githubusercontent.com/jazware/vlpds/main/DESIGN.md)
- [Tangled core](https://tangled.org/tangled.org/core): the knot server's `knotserver/ingester.go`, `rbac/rbac.go`, the AppView's `appview/ingester.go` and the `sh.tangled.repo` lexicon
- [Tangled docs](https://docs.tangled.org/single-page) (knot 2 migration, repo signing keys) and [Migrating to the new Tangled knot2](https://jola.dev/posts/migrating-tangled-knot2)
- [Introducing Bobbin](https://blog.tangled.org/bobbin) and the [Tangled blog index](https://blog.tangled.org/)
- [atproto-crates](https://tangled.org/ngerakines.me/atproto-crates), [atproto-identity on docs.rs](https://docs.rs/atproto-identity) and the earlier [Rust PDS with Spaces design brief](https://gist.github.com/ngerakines/efd4c8fd0d9e75f8796f40edc6748a0c) (May 2026)
- [Barazo API](https://github.com/singi-labs/barazo-api) and [atBB (Malpercio)](https://tangled.org/malpercio.dev/atbb/blob/main/README.md)
- [Introducing Tap](https://atproto.com/blog/introducing-tap) and the [Tap README](https://raw.githubusercontent.com/bluesky-social/indigo/main/cmd/tap/README.md) (sharding across consumers, per-repo ordering, event fields)
- [did:plc specification](https://web.plc.directory/spec/v0.1/did-plc) (rotation keys and the 72-hour recovery window)
- [Jacquard 0.11 docs](https://docs.rs/jacquard/0.11.0) and its [crates.io entry](https://crates.io/crates/jacquard) (0.12.1, June 26, 2026)
- [Hydrant](https://tangled.org/ptr.pet/hydrant) and [contrail: Spaces](https://next.tangled.org/flo-bit.dev/contrail/blob/pr42/docs/06-spaces.md)
- [ParadeDB stemming](https://www.paradedb.com/docs/reference/token-filters/stemming.md), [ParadeDB repository](https://github.com/paradedb/paradedb) and [pg\_textsearch](https://doc.pigsty.io/ext/e/pg_textsearch)
- [Typst 0.15 release notes](https://github.com/typst/typst/releases/tag/v0.15.0) and [Typst releases](https://github.com/typst/typst/releases); [typst.ts](https://github.com/Myriad-Dreamin/typst.ts), its docs on [the precompiler](https://myriad-dreamin.github.io/typst.ts/cookery/guide/compiler/ts-cli.html), [compiler services](https://myriad-dreamin.github.io/typst.ts/cookery/guide/compiler/service.html), [the all-in-one library](https://myriad-dreamin.github.io/typst.ts/cookery/guide/all-in-one.html) and [responsive rendering](https://myriad-dreamin.github.io/typst.ts/cookery/direction/responsive.html), the [vector format proposal](https://github.com/Myriad-Dreamin/typst.ts/blob/main/docs/proposals/8-vector-representation-for-rendering.typ), its [releases](https://github.com/Myriad-Dreamin/typst.ts/releases) and its [workspace manifest](https://raw.githubusercontent.com/Myriad-Dreamin/typst.ts/main/Cargo.toml)

Checked for the second review: the [Spaces `putRecord` lexicon at the October 15 snapshot](https://raw.githubusercontent.com/bluesky-social/atproto/619c6e8556db7dd37bb7df1d92a5eae83da8d8b0/lexicons/com/atproto/space/putRecord.json) (no swap conditions), the Spaces proposal on deniable commits and write-notification enforcement, and that [vlpds documents Spaces](https://vlpds.jazco.dev/docs/spaces) (the page bodies couldn't be read here, so the details of its divergences and backup gaps come from the review). The upstream report of concurrent admins overwriting each other, ParadeDB's edition split and Typst's raw-byte plugins are as the review cites them, not independently checked.

Checked for the third review: the reference PDS's [space notification worker](https://raw.githubusercontent.com/bluesky-social/atproto/619c6e8556db7dd37bb7df1d92a5eae83da8d8b0/packages/pds/src/space-notifications.ts) at the October 15 snapshot (retries back off up to an hour, a newer revision restarts them, and a notification is dropped after 24 hours) and [proposal PR #120](https://github.com/bluesky-social/proposals/pull/120) (public Spaces without a delegation token, open since October 13).

Three reviews from other models (October 16) prompted the Rules and guarantees section and several corrections. Their claims about PLC rotation keys, Tap's consumer sharding and Spaces notifications were checked against the sources above. On search, the plan follows the third review: ranking may use shared statistics, which the second review had argued against.

From the uploaded project files: the NodeBB retrospective, the architecture decision register and blueprint, the September 6 decisions in the planning excerpts, the reference library and decision register.

Not reached: Tangled's docs don't say who holds a repo DID's rotation keys or how a repo moves between knots, and the Chatto repository page doesn't describe its log-oriented architecture, so neither is relied on here.
