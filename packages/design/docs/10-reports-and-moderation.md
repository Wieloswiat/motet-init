# Reports and moderation

Reports follow `com.atproto.moderation.createReport`, the call every AT moderation service accepts. A report has one `subject` (an account as a `repoRef`, or one record as a `strongRef`, URI and CID, so the service sees the exact version that was reported), a `reasonType`, and an optional `reason` text of at most 2000 graphemes. The reporter's server sends it to one service. Choosing three recipients sends three reports, which is why the dialog says "Each one gets its own copy."

## Reasons

The dialog asks "What's wrong?" in plain words. Each choice is one reason type; where a service accepts Ozone's finer types, Motet sends those instead.

| Choice | `com.atproto.moderation.defs` | Finer type (`tools.ozone.report.defs`) |
| --- | --- | --- |
| Breaks *World*'s rules or content lines | `#reasonViolation` | `#reasonRuleOther` |
| Harassment, hate or trolling | `#reasonRude` | `#reasonHarassmentTroll`, `#reasonHarassmentTargeted`, `#reasonHarassmentHateSpeech` |
| Unwanted sexual content | `#reasonSexual` | `#reasonSexualUnlabeled` |
| Spam or a scam | `#reasonSpam` | `#reasonMisleadingSpam`, `#reasonMisleadingScam` |
| Pretending to be someone else | `#reasonMisleading` | `#reasonMisleadingImpersonation` |
| Threats, or someone may be in danger | `#reasonViolation` | `#reasonViolenceThreats`, `#reasonSelfHarmContent` |
| Something else | `#reasonOther` | |

The World's rules come first and only for posts in a World. "Someone may be in danger" shows where to get help before anything is sent. Appeals (`#reasonAppeal`) start from a moderation notice on your own post or account, not from this dialog.

## Recipients

- **The World itself**, for posts in a World. Every World is a moderation service (below), so this is an ordinary report to the World's account; it lands in the World's queue in Manage. A report about a GM is shown only to the other GMs, and the dialog says so by name.
- **Motet moderation**, always on in Motet's apps and checked by default for anything beyond a World's rules.
- **Services the reader added** in Settings, Moderation and safety. A service that doesn't take a reason or a kind of subject (its labeler record can list `reasonTypes`, `subjectTypes` and `subjectCollections`) is shown unavailable, with why.

Defaults follow the reason: World rules to the World; harassment, sexual content and danger to the World and Motet; spam and impersonation to Motet. The reader can change every one before sending.

## Worlds are moderation services

A World is an account with its own DID and handle (`@gilded-reach.motet.at`). Its World record lives in that account's repository, and the same account publishes a labeler service record (`app.bsky.labeler.service`) when the World is created. That makes the World a moderation service like any other:

- **Reports** to the World are `createReport` calls addressed to the World's DID. Its service record lists the reasons and subjects it takes (`reasonTypes`, `subjectTypes`, `subjectCollections`), normally posts and accounts in the World.
- **GMs act for it.** Hiding a post, warning an author or marking a spoiler is a label the World's account applies, through the queue in Manage. Each label records which GM applied it, and every decision can be undone.
- **Its labels** are the World's own values, defined in the service record: `hidden` (Hidden in this World), `needs-cn` (Needs a content note) and `spoiler` (Spoiler for an arc), plus any the GMs add. Motet applies a World's labels to everyone reading inside that World, the way it always applies Motet moderation. Other apps apply them for readers who subscribe to the World.
- **Escalation** goes out from the World's account: "Sent as The Gilded Reach". The reporter's identity stays with the World unless they agreed to pass it on.
- **Handing over or archiving** a World hands over its moderation service with it. An archived World's labels stay in force.

The World's GMs see their service in Manage, under Moderation, and readers see "Your Worlds" in their list of services, marked as applying inside each World.

## What a report carries

- One subject. "Link the two posts before it" adds their links to the reason text; the service reads them itself.
- The note is the `reason` text, with a counter as it nears 2000.
- The reported person is never told who reported them; the service sees the reporter's account.
- After sending, the dialog shows which recipients received it and offers Mute and Block. The reader's reports, with outcomes when a service shares them, are listed in Settings.

## The GM queue

Reports never mix with posts. **Needs you** shows one row while reports are open ("Moderation: 2 open reports", the oldest named) and links to **Moderation** in Manage, which lists reports as Open, Escalated and Closed. Each report shows the reported version of the post with its time, Go to post, the author's profile, the reporter and their note, earlier reports about the same account, and a history of every action.

GM actions: **Hide post** (in this World), **Warn** the author, **Dismiss**, or **Escalate**. Every decision can be undone from the history, and a note for other GMs travels with the report.

**Escalate** sends a new report from the World's account to Motet moderation, a service the World uses, or any service by handle. It keeps the same subject (the reported version) and starts from the original reason. The reason text is the GM's note, plus the reporter's note if included, plus who reported it only when the reporter agreed. The service's outcome, when it shares one, joins the history.
