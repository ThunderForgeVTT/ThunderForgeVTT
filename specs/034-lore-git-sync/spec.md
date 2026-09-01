# Feature Specification: Optional Lore Synchronisation to an External Repository

**Feature Branch**: `034-lore-git-sync`

**Created**: 2026-09-01

**Status**: Draft

**Input**: User description (playtest, 2026-09-01): "a potential github or gitlab integration where we can offload the docs to a github repo as an optional integration — we should support our path out of the gate but allowing a synced repo is a good user thing. This would involve a job that can translate our [format]" (sentence incomplete as recorded; the direction of translation is resolved in Assumptions).

## Context

Lore in this platform is already the shape a repository wants. Entries are
markdown. Every save writes a revision that records the text, who wrote it, and
when. Restores are recorded as restores. A repository of markdown files with a
commit history and an author per commit is not a different data model — it is
the same data model with a different storage surface.

This matters for how the feature should be scoped. The instinct on hearing
"sync to GitHub" is to imagine a format translator. There is very little format
to translate. What this feature actually has to decide is:

- **where each entry lives** in a directory tree, and whether that location is
  stable across a rename;
- **how a run of revisions becomes a run of commits** that a human can read;
- **what happens to everything markdown does not carry** — per-entry
  permissions, images, and cross-links to actors, items and abilities;
- **which side wins** when both sides have changed.

The first of those is blocked on lore gaining a tree and tags, which is
specified elsewhere (`031-playability` FR-038) and is a hard dependency of this
feature, not part of it.

The framing that governs every decision below: **our path stays first-class.**
The repository is additive and optional. A world that never connects one loses
nothing. A world that connects one and then loses the connection — revoked
token, deleted repository, host outage — loses nothing either. In-app lore is
authoritative at all times, and no failure on the far side of the network may
alter, block, or degrade it.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - A Game Master mirrors a world's lore into a repository they own (Priority: P1)

A Game Master who already keeps notes in version control, or who simply wants
their world's writing to exist somewhere they control, connects one of their
own repositories to a world. From that point on, every lore entry in that world
appears in the repository as a markdown file in a directory that matches the
lore tree they see in the app, and every subsequent edit arrives as a commit
attributed to the person who made it. They can clone the repository, read the
world offline in any editor, and see the history of a place or a faction as a
sequence of readable commits.

**Why this priority**: This is the whole of the user's stated request and the
only story that delivers value alone. Export is also the only direction that
cannot damage in-app lore, so it is the correct thing to ship first and the
correct thing to ship even if nothing else in this spec is ever built.

**Independent Test**: Connect an empty repository to a world containing lore,
run the first synchronisation, clone the repository, and confirm the file tree
and file contents match what the app shows. Then edit an entry in the app and
confirm a new commit appears carrying that change and naming that author.

**Acceptance Scenarios**:

1. **Given** a world with lore entries organised in a tree, and a connected
   empty repository, **When** the first synchronisation runs, **Then** the
   repository contains one markdown file per entry, arranged in directories
   mirroring the tree, with each file's body being the entry's current markdown.
2. **Given** a connected and previously synchronised world, **When** a Game
   Master edits an entry and saves it, **Then** a commit appears in the
   repository containing only that file's change, attributed to the account
   that authored the revision, and carrying the entry's title in its message.
3. **Given** a connected world, **When** an entry is renamed or moved to a
   different position in the lore tree, **Then** the repository records a move
   of the existing file rather than a deletion and an unrelated creation, so
   the file's history survives the rename.
4. **Given** a connected world whose entries carry tags, **When**
   synchronisation runs, **Then** each file carries its entry's title, tags,
   durable identifier, and last-updated time in a machine-readable header,
   and the body below that header is the entry's markdown exactly as authored.
5. **Given** an entry whose markdown cross-links to another lore entry, **When**
   synchronisation runs, **Then** the link in the exported file resolves to the
   target entry's file within the repository.
6. **Given** an entry that has been disabled by a moderation action, **When**
   synchronisation runs, **Then** that entry is not written to the repository
   and its absence does not stop the rest of the world from synchronising.

---

### User Story 2 - The connection fails and the world is unharmed (Priority: P2)

The repository host is down, or the Game Master revokes the access they granted,
or someone force-pushes the branch, or the repository is deleted outright. In
every case the Game Master keeps playing. Lore reads and writes in the app are
unaffected. The world's settings show plainly that synchronisation is not
currently working and why, and offer the one action that fixes it. When the
cause is resolved, a fresh synchronisation restores the repository to match the
app without the Game Master having to reconstruct anything by hand.

**Why this priority**: A connection to a third party will fail routinely. If
failure is anything other than cosmetic, the feature is a liability rather than
a convenience, and the "our path stays first-class" framing is not real. This
is second only because Story 1 must exist before it can fail.

**Independent Test**: With a world synchronising normally, revoke the granted
access at the host. Confirm lore editing in the app continues to work
unchanged, that the world's settings report the connection as broken with a
cause and a remedy, and that re-granting access followed by a resynchronisation
returns the repository to a faithful mirror.

**Acceptance Scenarios**:

1. **Given** a connected world, **When** the remote host is unreachable, **Then**
   lore reading, editing, and revision history in the app behave exactly as they
   do for an unconnected world, and no editing action reports an error.
2. **Given** a connected world, **When** granted access is revoked at the host,
   **Then** the world's settings show the connection as needing attention, name
   the cause in plain language, and offer reconnection; no lore content is
   changed or hidden.
3. **Given** a world whose synchronisation has been failing, **When** the cause
   is resolved and synchronisation is retried, **Then** all changes made while
   the connection was broken appear in the repository, in order, without
   duplicate or lost entries.
4. **Given** a connected repository whose branch has been force-pushed or whose
   history no longer contains the platform's last known commit, **When**
   synchronisation next runs, **Then** the system does not overwrite the
   divergent history silently; it reports the divergence and requires an
   explicit choice from the Game Master before writing again.
5. **Given** a connected repository that has been deleted at the host, **When**
   synchronisation next runs, **Then** the connection is marked broken, the
   world is unaffected, and the Game Master may connect a different repository
   without any loss of in-app lore.
6. **Given** repeated synchronisation failures, **When** they continue, **Then**
   retries back off rather than repeating at full rate, and the Game Master is
   notified once rather than repeatedly.

---

### User Story 3 - Writing in the repository and bringing it back (Priority: P3)

A Game Master who prefers a text editor writes a chunk of a world's lore
directly in the repository and pushes it. Rather than that change silently
becoming the truth, the app tells them there are changes in the repository that
the world does not have, shows them what those changes are, and lets them accept
them into the world — as ordinary revisions, authored by them, appearing in the
entry's history like any other edit. Where the same entry changed on both sides,
they are shown both and choose, per entry, which text to keep; nothing merges
prose automatically.

**Why this priority**: This is the story most users asking for a repository
eventually want, and the only one that makes the repository feel like a real
workspace rather than a backup. It is last because it is where all the risk
lives: it is the only story that can put text into a world that the world's
members did not write in the app, and it is worthless until export is trusted.

**Independent Test**: With a world synchronising normally, edit a file in the
repository and push it. Confirm the app reports pending incoming changes, shows
the difference, and — on acceptance — updates the entry and adds a revision
attributed to the accepting user. Then change the same entry on both sides and
confirm the app presents both versions and applies only the one chosen.

**Acceptance Scenarios**:

1. **Given** a connected world with incoming acceptance enabled, **When** a
   change is pushed to a synchronised file, **Then** the app reports pending
   incoming changes for that world and shows what would change, and the world's
   lore is not altered until a user with authority accepts.
2. **Given** pending incoming changes, **When** a user with authority over the
   world accepts them, **Then** each affected entry gains a new revision
   carrying the incoming text, attributed to the accepting user with the
   repository named as the origin of the change.
3. **Given** an entry changed in the app and in the repository since the last
   synchronisation, **When** incoming changes are reviewed, **Then** both
   versions are presented side by side and the entry is left unchanged until
   one is chosen; the system never merges the two texts on its own.
4. **Given** a file in the repository that carries no durable entry identifier,
   **When** incoming changes are reviewed, **Then** it is offered as a new
   entry to create rather than matched to an existing entry by filename alone.
5. **Given** a file deleted in the repository, **When** incoming changes are
   reviewed, **Then** the corresponding entry is not deleted automatically;
   deletion is presented as an explicit choice, and declining it restores the
   file on the next synchronisation.
6. **Given** incoming acceptance has never been enabled for a world, **When**
   anything at all changes in the repository, **Then** the world's lore is
   never modified by it.

---

### Edge Cases

- **A repository that already has files.** A first synchronisation must never
  delete or overwrite content it did not write. Files outside the directory the
  world synchronises into are left untouched forever; a collision inside that
  directory stops the first synchronisation with an explanation rather than
  resolving itself.
- **Two worlds, one repository.** Two worlds must not be able to write to the
  same directory of the same repository. Either the second connection is
  refused or it is given its own distinct directory.
- **The same repository connected twice to the same world.** Refused; a world
  has at most one connection.
- **An entry whose title produces an unusable or colliding filename** — empty
  after normalisation, non-Latin script, differing only by case or accent, or
  colliding with a sibling. The mapping must produce exactly one stable file per
  entry regardless.
- **An entry moved to a tree position that a repository cannot express** —
  excessive nesting depth or path length. The system must remain able to place
  every entry somewhere deterministic.
- **Per-entry visibility.** Lore entries carry per-member permissions; a
  repository has one access list. An entry visible to the Game Master alone
  becomes visible to everyone with repository access once mirrored.
- **A world member who is not the connection's owner authors a revision.** Their
  authorship must be represented in the repository without exposing a private
  email address they did not consent to publish.
- **Moderation after mirroring.** An entry disabled by a takedown has already
  left the platform's control if it was mirrored. Future synchronisation must
  stop carrying it, and the Game Master must be told that removing it from the
  repository is an action only they can take.
- **World deletion, connection removal, or member removal** while a
  synchronisation is in flight.
- **A very large world** — thousands of entries, or a single entry with a very
  large body or many images — on a first synchronisation.
- **Rapid successive edits** to one entry, which must not produce one commit per
  keystroke nor lose the intermediate revisions the app recorded.
- **A repository host that reports success but silently rejects the write** —
  the platform's record of what the repository contains must be verifiable
  rather than assumed.

## Requirements *(mandatory)*

### Functional Requirements

**Connection and scope**

- **FR-001**: A synchronisation connection MUST be established per world, not
  per account, and a world MUST have at most one connection at a time.
- **FR-002**: Only a user holding owner-level authority over a world MUST be
  able to create, reconfigure, or remove that world's connection.
- **FR-003**: Credentials granted for a connection MAY be reused across the
  connections of worlds the same user owns, but the authority to synchronise a
  given world MUST derive from that user's authority over that world at the
  time of each synchronisation, re-checked rather than captured at connection
  time.
- **FR-004**: The system MUST support connecting to more than one repository
  host, and MUST NOT expose host-specific concepts in the user-facing
  connection flow beyond what the user must supply to grant access.
- **FR-005**: Removing a connection MUST leave the world's lore entirely intact
  and MUST leave the repository's existing contents untouched.
- **FR-006**: A world with no connection MUST behave exactly as it does today
  in every lore surface.

**Export fidelity**

- **FR-007**: Every lore entry in a connected world that is not
  moderation-disabled MUST be represented by exactly one markdown file in the
  repository.
- **FR-008**: A file's location MUST mirror the entry's position in the lore
  tree, so that the repository's directory structure is legible to a reader who
  has never used the app.
- **FR-009**: Each exported file MUST carry a machine-readable header holding at
  minimum the entry's durable identifier, title, tags, and the time of the
  revision it represents; the identifier MUST be what the system uses to match
  a file to an entry, never the filename.
- **FR-010**: A rename or a move of an entry MUST be represented in the
  repository as a move of the existing file, preserving that file's history.
- **FR-011**: The body of an exported file MUST be the entry's markdown as
  authored, with no reformatting beyond the rewriting of links required by
  FR-012 and FR-014.
- **FR-012**: A cross-link whose target is another lore entry in the same world
  MUST be rewritten to resolve to that entry's file within the repository.
- **FR-013**: A cross-link whose target is an actor, item, or ability MUST be
  preserved as readable text naming the target and recorded in the file's
  header as an unresolvable reference; this is a declared and documented loss of
  fidelity, not an error, and MUST survive a round trip without being silently
  dropped or converted into a broken lore link.
- **FR-014**: Images embedded in a lore entry MUST be mirrored into the
  repository as files and referenced by relative path, so a clone renders
  without reaching back to the platform. Derived renditions MUST NOT be
  mirrored; one file per uploaded image is sufficient.
- **FR-015**: An entry that has been disabled by a moderation action MUST be
  excluded from synchronisation, and its exclusion MUST NOT block the
  synchronisation of other entries.

**History and attribution**

- **FR-016**: A revision recorded in the app MUST produce a corresponding commit
  in the repository; a run of revisions MUST produce a run of commits in the
  same order.
- **FR-017**: A commit MUST be attributed to the account that authored the
  revision, using an identity that does not disclose a personal email address
  the user has not chosen to publish.
- **FR-018**: A commit message MUST name the entry and the nature of the change
  in language a reader can understand without the app open.
- **FR-019**: A revision recorded as a restore of an earlier revision MUST be
  identifiable as such in the repository history.
- **FR-020**: Successive revisions to a single entry MAY be batched into one
  commit within a bounded window, provided no revision recorded in the app is
  omitted from the repository's eventual content.

**Direction and authority**

- **FR-021**: In-app lore MUST be the source of truth. Where the two sides
  disagree and no user has chosen otherwise, the app's content MUST prevail and
  the repository MUST be brought to match it.
- **FR-022**: Export MUST be available without import; a world MUST be able to
  synchronise outward with acceptance of incoming changes never enabled.
- **FR-023**: Incoming changes MUST NOT alter a world's lore without an explicit
  acceptance by a user holding authority over that world.
- **FR-024**: Where an entry has changed on both sides since the last
  synchronisation, the system MUST present both versions for a per-entry choice
  and MUST NOT merge prose automatically.
- **FR-025**: An accepted incoming change MUST be recorded as an ordinary
  revision, attributed to the accepting user and marked as originating from the
  repository, so the entry's in-app history remains complete.
- **FR-026**: A deletion in the repository MUST NOT delete a lore entry without
  an explicit confirmation; a declined deletion MUST be reversed on the next
  synchronisation.
- **FR-027**: A file with no recognised durable identifier MUST be treated as a
  proposed new entry, never matched to an existing entry by path or title.

**Failure and resilience**

- **FR-028**: No failure of the remote host, the credential, or the repository
  MUST affect the availability, correctness, or latency of in-app lore reading
  or writing.
- **FR-029**: The system MUST surface a connection's current state — working,
  needing attention with a stated cause, or never configured — in the world's
  settings, in language that names the remedy.
- **FR-030**: Failed synchronisations MUST be retried with progressively longer
  intervals and MUST converge to the correct repository contents once the cause
  is resolved, without user reconstruction.
- **FR-031**: If the repository's history no longer contains the last state the
  platform wrote, the system MUST stop, report divergence, and require an
  explicit choice before writing again.
- **FR-032**: A first synchronisation into a repository containing existing
  files MUST NOT delete or modify any file the system did not write. A collision
  within the world's own directory MUST stop the first synchronisation with an
  explanation.
- **FR-033**: Two worlds MUST NOT synchronise into the same directory of the
  same repository.
- **FR-034**: The system MUST be able to verify that the repository's contents
  match what it believes it wrote, rather than assuming a reported success.

**Credentials and privacy**

- **FR-035**: Credentials granting access to a third-party repository MUST be
  stored encrypted at rest, MUST never be returned to any client, MUST never
  appear in logs or error messages, and MUST be revocable by the granting user
  from within the app with immediate effect.
- **FR-036**: The system MUST request the narrowest access the feature needs —
  write access to the connected repository and nothing else — and MUST show the
  user what access is being granted before they grant it.
- **FR-037**: Before a first synchronisation, the system MUST tell the Game
  Master, in plain language, that per-entry lore permissions do not survive the
  mirror: everything exported is visible to everyone with access to the
  repository, including entries restricted to a subset of world members, and
  that the repository's access control is theirs to manage, not the platform's.
- **FR-038**: Synchronisation MUST NOT begin until the Game Master has
  acknowledged FR-037's notice for that world.

**Content policy and moderation**

- **FR-039**: The system MUST NOT aggregate, index, list, search, or otherwise
  make discoverable the repositories connected across worlds; there MUST be no
  query that enumerates connections beyond a user's own.
- **FR-040**: When content in a connected world is disabled by a moderation
  action, the system MUST stop exporting it, MUST notify the world owner that
  the content may already exist in a repository outside the platform's control,
  and MUST state that removing it there is the owner's responsibility.
- **FR-041**: The user-facing terms for this feature MUST state that content
  exported to a repository the user controls leaves the platform's hosting, that
  the platform cannot retract it once exported, and that responsibility for what
  is published there rests with the user who chose to export it.
- **FR-042**: An explicit determination under the constitution's DMCA guardrail
  — whether a user-initiated mirror to a repository the user owns constitutes a
  centralized public repository — MUST be recorded and accepted by an
  accountable owner before implementation of this feature begins.

### Key Entities

- **Lore Repository Connection**: the association between one world and one
  external repository. Holds which repository, which branch, which directory
  within it, whether incoming acceptance is enabled, who established it, when it
  last synchronised successfully, and its current state. At most one per world.
- **Synchronisation Run**: one attempt to bring a repository into agreement with
  a world. Records what it was working from, what it wrote, whether it
  succeeded, and if not, why — in terms a Game Master can act on.
- **Exported Entry Mapping**: the durable association between a lore entry and
  the file that represents it, independent of that file's current path, so that
  renames are recognised as renames and a file can be matched back to its entry.
- **Pending Incoming Change**: a change observed in the repository that the world
  does not have, awaiting a decision. Carries what changed, which entry it
  concerns (or that it proposes a new one), and whether the same entry also
  changed in the app.
- **Repository Credential**: the granted authority to write to a third-party
  host on a user's behalf. Belongs to the user who granted it, is reusable
  across that user's connections, is revocable at any time, and is never
  disclosed to any client.
- **Export Fidelity Note**: a recorded instance of something that could not be
  represented in the repository — a cross-link to a non-lore target, a
  permission that could not be carried — surfaced to the user rather than
  silently dropped.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A Game Master can connect a repository to a world and see the
  world's lore in that repository in under 5 minutes from starting the
  connection flow, for a world of up to 200 entries, without reading
  documentation.
- **SC-002**: For a world of any size, the exported file tree and file bodies
  match what the app displays for 100% of non-moderation-disabled entries, with
  a difference of at most one entry's most recent edit.
- **SC-003**: An edit saved in the app appears in the repository within 60
  seconds under normal operation.
- **SC-004**: Renaming or moving 100% of a world's entries preserves the history
  of every corresponding file; no file's history is truncated by a rename.
- **SC-005**: With the remote host unavailable for 24 hours, in-app lore
  read and write success rates and response times are indistinguishable from a
  world with no connection, and 100% of edits made during the outage appear in
  the repository after recovery, in order.
- **SC-006**: Zero instances, across all failure modes exercised in testing
  (unreachable host, revoked credential, force-pushed branch, deleted
  repository, divergent history), of in-app lore being altered, hidden, or lost.
- **SC-007**: A first synchronisation into a repository with pre-existing files
  modifies zero files the system did not write.
- **SC-008**: A round trip — export an entry, change nothing, accept it back —
  produces byte-identical markdown, and every fidelity loss (non-lore
  cross-links, per-entry permissions) is enumerated in a fidelity note rather
  than discovered by the user.
- **SC-009**: 100% of moderation-disabled entries are absent from the repository
  after the next synchronisation following the disabling action.
- **SC-010**: No credential value appears in any log, error message, API
  response, or client-visible surface, verified by inspection.
- **SC-011**: A clone of the repository renders every entry, including its
  images and its links to other entries, in a standard markdown viewer with no
  network access to the platform.
- **SC-012**: 90% of Game Masters shown the pre-synchronisation notice can
  correctly state, when asked, that entries restricted to some world members
  will be visible to everyone with repository access.

## Assumptions

- **The translation direction is export-first, and the user's cut-off sentence
  is resolved that way.** The recorded request ends mid-sentence at "a job that
  can translate our [format]". This spec reads it as translating *our* content
  *into* a repository — outward — and treats incoming changes as a separate,
  later, explicitly-accepted path (User Story 3). If the intent was inbound
  authoring as the primary mode, Stories 1 and 3 swap priority and this
  assumption must be revisited before planning. **This needs the user's
  confirmation.**
- **This is not a format-translation problem.** Lore is already markdown with a
  revision history carrying author, timestamp, and restore lineage. The work is
  path mapping, commit synthesis, and reconciliation. Any plan that budgets
  significant effort for converting content between formats has misread the
  data model.
- **Lore tree and tags (`031-playability` FR-038) are a hard dependency.**
  Stable, human-legible repository paths require the hierarchy and tags
  specified there. Without them, every entry lands in one flat directory and
  FR-008 cannot be satisfied. This feature MUST NOT begin implementation before
  that lands.
- **The slug-versus-UUID tension is resolved by separating URL identity from
  repository path.** There is a standing intent to move in-app lore URLs from
  title slugs to opaque identifiers, because guessable slugs let an outsider
  enumerate a world's lore from its URLs. A repository, meanwhile, is only
  useful if its paths are human-meaningful. These do not actually conflict,
  because they answer different threats: the enumeration concern is about
  *unauthenticated reachability of a platform URL*, and a connected repository
  is neither unauthenticated nor the platform's. The position taken here:
  in-app URLs may become opaque identifiers freely, with no consequence for this
  feature; repository paths are human-meaningful, derived from the tree position
  and title; and identity in both directions is carried by the entry's durable
  identifier in the file header (FR-009), never by the path. A path is a label,
  not a key. **This is the single most consequential decision in this spec and
  needs the user's explicit confirmation**, because accepting it means a private
  repository may carry readable paths the platform's own URLs deliberately will
  not.
- **Two-way sync is out of scope for a first delivery.** Story 3 is a reviewed
  import, not continuous bidirectional synchronisation. Prose does not merge;
  no automatic merge is offered anywhere in this spec, at any priority.
- **The connection is per world, and credentials are per account.** A user with
  five worlds grants access once and connects five worlds, each to its own
  repository or its own directory. A per-account single mirror of everything a
  user owns is explicitly rejected: worlds have different members, different
  permissions, and different lifetimes.
- **Images are mirrored, not referenced.** Storing the uploaded original in the
  repository is what makes a clone self-contained (SC-011) and is what a user
  asking for "offload the docs" means. Derived renditions stay on the platform;
  the repository is not a rendition store. If repository size proves a problem
  for image-heavy worlds, referencing is the fallback, and that is a plan-time
  decision, not a spec-time one.
- **Repository access control is the user's to manage.** The platform grants no
  access, revokes none, and audits none. Per-entry lore permissions are
  therefore flattened by the mirror, which is why FR-037's notice is mandatory
  rather than advisory.
- **The moderation posture is that mirroring is user-initiated distribution.**
  The user chooses to export, to a repository they own, using their own
  credential; the platform is not the publisher there. Under ADR-049's test, the
  feature adds no aggregation, no discovery surface, and no enumeration
  (FR-039), so it does not read as a centralized public repository. That
  reasoning is stated here as the input to a determination, not as the
  determination itself — FR-042 requires the determination on record before
  implementation begins, per the constitution's guardrail.
- **A takedown cannot reach mirrored content.** Once content has been written to
  a repository the platform does not control, the platform can stop exporting it
  and can tell the owner, and that is the entire extent of its reach. The spec
  requires both (FR-040) and claims nothing further. This is a genuine reduction
  in takedown effectiveness for connected worlds and should be stated as such in
  the FR-042 determination rather than glossed.
- **Scope boundary.** This feature covers lore only. Actors, items, abilities,
  scenes, and packs are not synchronised, and cross-links to them are declared
  lossy (FR-013). Extending the mirror to other content types is a separate
  spec and would re-open the FR-042 determination.
