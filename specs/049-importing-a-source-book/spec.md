# Feature Specification: Importing a Source Book

**Feature Branch**: `049-importing-a-source-book`

**Created**: 2026-09-12

**Status**: Draft

**Input**: User description: "Wrote it up in a spec honestly and the biggest thing I think we should do is before committing to the server with the data give them a confirmation window of everything it parsed and if it looks right have a submit button. But because books and text can get huge show then a progress bar of whats being sent to us and maybe we get a little fancy with it too — i'd love for them to be able to explore their content by compendium, like if I upload the Dungeon Master's Guide that gets saved as a compendium of everything we loaded from the pdf, like a bucket of content — but because its commercial the share is disabled citing its commercialized work and cannot be shared."

## What this is, and what already exists

A Game Master has a shelf of rulebooks as PDFs. This is how one of them becomes
content in their world: spells they can hand out, items they can place,
creatures they can drop on a map, and the class features and feats their
players will ask about mid-session.

Three pieces are already built and are not being rebuilt here:

- **A reader.** `crates/thunderforge-pdf` turns a PDF into positioned, styled
  text and runs in the Game Master's own browser. Measured against a 246-book
  library: **all 246 open**, and 51 are image scans with no text layer at all.
- **A creature reader.** Originally `statblock.rs` in the 5e pack, **retired**
  in this spec's phase 3 and replaced by a declaration-driven reader that names
  no game system. It worked for one reason, and the replacement works for the
  same one: armour class is an unambiguous anchor. Every 5e creature has one,
  it is always labelled, and that label appears nowhere else.

  Over the whole corpus the generic reader finds **2750 creatures and 3049
  per-attack reach values**, against the old reader's 2155 and 1880. Both
  figures are a floor on recall and say nothing about precision — see
  [measurements.md](./measurements.md), which carries that caveat in full.
- **A memory of what has been read.** Spec 047 FR-070 to FR-075 already
  require a world to record the SHA-256 of every book imported into it,
  hashed in the browser before the upload, so a re-import offers to overwrite
  rather than silently doubling everything.

What is missing is everything that is not a monster, everything that is not
D&D 5e, and the entire moment between "the reader has finished" and "the world
has changed".

## Four words this spec keeps apart

The repository already uses two of these, and getting them confused would be
expensive later.

- **Compendium** *(spec 011)* — the world's own content portal at
  `/world/:id/compendium`. It is a place, not a thing you own.
- **Collection** *(spec 026)* — user-authored content, gathered to be shared
  and adopted by other worlds. Never called a pack, never called a bundle.
- **Compendium (of a book)** — *new here, and the owner's word*: the bucket of
  everything one import produced. Read the Dungeon Master's Guide, get one
  bucket holding everything that came out of it.
- **Library** *(spec 050)* — the account's shelf of compendiums. A book is
  imported **once per account**, not once per world, and the worlds that
  account owns inherit from it.

These are compatible rather than in conflict: a compendium belongs to the
account, a world inherits it and may delta it, and the world's Compendium
portal is where you browse the result — by kind as it always has, and now by
compendium too. The portal gains a dimension, not a rival.

A book's compendium is deliberately *not* a collection, because the whole
point of the last section of this spec is that most of them can never be
shared.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - See everything that was found, then decide (Priority: P1)

A Game Master opens their world panel, picks a PDF off their own disk, and
watches it being read. When the reading finishes, nothing has been sent
anywhere. Instead they are shown what was found — spells, items, creatures,
class features, feats — grouped by kind, countable, and readable. Values the
reader was unsure of are marked as unsure rather than presented as fact. They
look it over, and only then press submit.

If it looks wrong — the wrong book, a book that came out as gibberish, a book
that yielded four spells when it should have yielded four hundred — they close
the window and nothing has happened.

**Why this priority**: This is the owner's stated requirement and it is the
one that makes the rest safe to build. A wrong import is far cheaper to refuse
than to undo. Every defect found while building the reader produced text that
looked plausible and was wrong — subsetted fonts with no usable encoding,
columns welded together, faux-bold overprinting — and the only reliable
detector of that class of failure is a person looking at it.

**Independent Test**: Read a real book in a browser, confirm the review lists
what was found with per-field certainty, confirm no network request carried
content before submit was pressed, and confirm that closing the window leaves
the world unchanged.

**Acceptance Scenarios**:

1. **Given** a Game Master has picked a book, **When** the reading finishes,
   **Then** they are shown everything found, grouped by kind and counted, and
   nothing has been sent.
2. **Given** the review is open, **When** the Game Master closes it without
   submitting, **Then** the world holds nothing from that book and no record
   of an import exists.
3. **Given** an entry whose fields were only partly read, **When** it appears
   in the review, **Then** the unread and uncertain fields are marked as such
   and no value has been invented to fill them.
4. **Given** a book the reader could not get text out of, **When** the reading
   finishes, **Then** the review says so plainly — naming how many pages were
   silent — rather than presenting an empty result as a successful import.
5. **Given** the Game Master presses submit, **When** the import completes,
   **Then** what the world holds matches what the review showed.

---

### User Story 2 - Sending a large book, visibly (Priority: P2)

The Game Master presses submit on a seventy-megabyte book that produced two
thousand entries. A progress indicator shows what is being sent and how far it
has got. They can leave the panel open and watch, and if it fails halfway the
world is not left holding half a book.

**Why this priority**: Without it the previous story ends in a frozen screen,
and a person who cannot tell a slow import from a broken one will reload the
page — which is the one action most likely to break it.

**Independent Test**: Import a book large enough to take measurable time,
confirm progress advances and reflects real work rather than an animation,
and confirm that a failure partway leaves the world exactly as it was.

**Acceptance Scenarios**:

1. **Given** a large import is in flight, **When** the Game Master watches,
   **Then** progress advances and states what is being sent.
2. **Given** an import fails partway, **When** it stops, **Then** the world
   holds nothing from that book and the failure names what went wrong.
3. **Given** an import is in flight, **When** the Game Master abandons it,
   **Then** it stops and leaves nothing behind.

---

### User Story 3 - Browse what came out of one book (Priority: P3)

Afterwards the Game Master opens their world's Compendium and can look at the
Dungeon Master's Guide as a thing: everything that came out of it, in one
bucket, browsable by kind. They can see when it was imported, which file it
came from, and how much of it there is. If they decide it was a mistake they
can remove the whole bucket in one action.

**Why this priority**: An import that vanishes into an undifferentiated pile
of world content cannot be reviewed, corrected, or undone. The bucket is what
makes the import a decision rather than an event.

**Independent Test**: Import two books into one world, confirm each appears as
its own compendium with its own counts, confirm content is attributable to the
book it came from, and confirm removing one leaves the other intact.

**Acceptance Scenarios**:

1. **Given** two books have been imported, **When** the Game Master opens the
   Compendium, **Then** each appears as its own compendium with its own name,
   import date and counts.
2. **Given** a compendium is open, **When** the Game Master browses it,
   **Then** they see only what came out of that book, grouped by kind.
3. **Given** any imported entry, **When** the Game Master looks at it,
   **Then** it names the book it came from and the page it was found on.
4. **Given** a compendium is removed, **When** the removal completes, **Then**
   what it contributed is gone and nothing from any other book is touched.

---

### User Story 4 - Uploaded content stays at your tables (Priority: P4)

The Game Master tries to share the Dungeon Master's Guide compendium, or to
publish something out of it as a collection. They are refused, and the refusal
says why: this was read out of a book they uploaded, and uploaded content does
not leave the account that uploaded it.

**Why this priority**: The owner's rule, already recorded in specs 047 and
048 — *"In real life I can show my book to the whole table, but I cannot have
my book and lend it out."* The table may read it over the owner's shoulder;
the book does not travel.

What makes it enforceable is that the test is **origin, not licence**. Nobody
has to judge whether a work is commercial, and nobody can get that judgment
wrong: a piece of content either came through the authoring tools or came out
of an upload, and the system knows which with certainty.

**Independent Test**: Import any book, attempt every route out of the account
— share, publish, adopt into a collection, export — and confirm each is
refused with a reason naming the upload. Then author content by hand and
confirm sharing it is allowed.

**Acceptance Scenarios**:

1. **Given** any uploaded compendium, **When** the Game Master attempts to
   share, publish, export, or adopt it into a collection, **Then** each is
   refused and the refusal names its origin as the reason.
2. **Given** the same compendium, **When** it is used in a world the importing
   account owns, **Then** it works exactly as any other content does.
3. **Given** a world's change to an uploaded entry, **When** it is placed into
   a collection by any route, **Then** that route is refused too — the
   restriction follows what the content is derived from.
4. **Given** a document with an open licence, **When** it is uploaded, **Then**
   what comes out of it is still unshareable, and the refusal names the two
   routes that remain: author it, or propose it as a system pack.
5. **Given** content authored in ThunderForge by hand, **When** the Game
   Master shares it, **Then** it is allowed — the rule restricts an origin,
   not a subject.

---

### User Story 5 - A second system's book (Priority: P5)

Someone imports a Pathfinder book into a Pathfinder world. It reads as well as
a 5e book does, and no shared code learned a word of Pathfinder to make that
happen — the Pathfinder pack declares what anchors a spell, an item and a feat
in Pathfinder, the same way it already declares where its darkvision lives.

**Why this priority**: It is the difference between a feature and a 5e
feature. The existing creature reader looks for "armor class", "hit points"
and "challenge"; Pathfinder says AC, hp and CR, and the third system says
something else again. Shared code must not accumulate them —
`scripts/check-system-registry.mjs` already fails the build if a system
identifier appears in shared server or web code, and this spec must not need
an exception.

**Independent Test**: Import a Pathfinder book into a Pathfinder world and a
5e book into a 5e world using the same shared code path, confirm both produce
correct content, and confirm the system registry check still passes.

**Acceptance Scenarios**:

1. **Given** a system pack that declares content patterns, **When** a book is
   imported into a world on that system, **Then** the shared reader uses the
   declaration and finds that system's content.
2. **Given** a system pack that declares none, **When** a book is imported,
   **Then** the import is refused with a reason, rather than guessing with
   another system's vocabulary.
3. **Given** the shared importer code, **When** the system registry check
   runs, **Then** it passes with no system identifier in shared code.

---

### Edge Cases

- **A book that is entirely scans.** 51 of 246 measured books have no text
  layer. The reader finds nothing, and "nothing" must be reported as "this
  book is images, we cannot read it" and not as an empty success.
- **A book that is partly scans.** Some pages yield text and some do not. The
  review must say how many pages were silent, because an import that quietly
  covers a third of a book is worse than one that says so.
- **Text that looks right and is not.** A subsetted font with no usable
  encoding produces readable-looking nonsense. Entries built from text the
  reader marked suspect must be marked suspect in the review.
- **The same entry in two books.** A spell reprinted in a second book is two
  entries from two sources, not one entry overwritten. Deduplication across
  books is a decision for the Game Master, not for the importer.
- **A re-import of the same file.** Handled by spec 047 FR-070 to FR-075: the
  hash is recognised before the upload and overwriting is offered.
- **A near-identical file.** The same work re-saved hashes differently and
  must not be claimed as a match (spec 047 FR-075).
- **A book with no system.** A world whose system pack declares no content
  patterns cannot have a book read into it, and must be told that rather than
  shown an empty review.
- **An enormous book.** A book that would produce more entries than a world
  can reasonably hold must be refused at a stated bound, before the send, with
  the bound named.
- **A browser that runs out of memory mid-read.** The read must fail with a
  reason, and must not leave a partial import behind — it never started one.
- **Content adopted into play, then its compendium removed.** Removing a
  compendium must not leave a scene referring to a creature that no longer
  exists; what breaks must be named before the removal is confirmed.

## Requirements *(mandatory)*

### Functional Requirements

**What can be read out of a book**

- **FR-001**: The importer MUST treat imported content as two kinds, and MUST
  NOT treat one as the other.
- **FR-001a**: **Anchored content** — spells, items, creatures — is a block of
  labelled fields introduced by an unambiguous anchor, the way a 5e creature
  is anchored by its armour class. Its fields MUST be read into structured
  values.
- **FR-001b**: **Prose content** — class features, feats, subclass options,
  backgrounds — has no labels and no fixed fields. It MUST be captured as a
  name, its text, and where it came from, and the importer MUST NOT invent
  mechanical fields for it.
- **FR-002**: The importer MUST NOT present a mechanical value it did not
  read. Absence MUST be reported as absence.
- **FR-003**: Every value MUST carry, per field, whether it was read, unread,
  or uncertain — the same rule spec 048 FR-003 sets and the existing creature
  reader already follows.
- **FR-004**: Text the reader marked untrustworthy MUST propagate to the entry
  built from it, so a person reviewing can see which entries came from pages
  the reader struggled with.
- **FR-005**: A book that yields no text MUST be refused with that as the
  reason. A book that yields text on some pages only MUST report how many
  pages were silent.

**What a system declares**

- **FR-010**: A system pack MUST be able to declare its **content patterns**:
  for each kind of content it supports, what anchors an entry of that kind and
  which fields to read from it.
- **FR-011**: The declaration MUST live in the pack's own manifest, beside the
  `vision` and `movement` blocks that already work this way (spec 045).
- **FR-012**: Shared importer code MUST read the declaration and MUST NOT
  contain any system's vocabulary. `scripts/check-system-registry.mjs` MUST
  continue to pass without an exemption for importer code.
- **FR-013**: D&D 5e MUST declare patterns covering spells, magic items,
  creatures and feats in this spec.
- **FR-013a**: **Class features are deferred, on evidence.** A class feature is
  introduced by a bold run-in name — `Rage. In battle, you…` — and the layout
  pass marks a line bold only when *every* run on it is bold, so such a line
  reads as ordinary prose. Declaring it as a heading instead matches every
  section of every book: measured on 2026-09-12, that produced 42,295 entries
  across 246 books. There is no marker line under a class feature to confirm
  it by either (FR-013b), so the kind cannot be declared honestly. Catching a
  run-in name needs sub-line run data, which the layout pass deliberately
  collapses; until that exists this kind ships nothing rather than garbage.
- **FR-013b**: A **prose** pattern MUST declare what confirms an entry is one
  of that kind, and a pattern that declares nothing MUST be refused at install.
  A prose kind described only as "a heading, then paragraphs" describes every
  section of every book — measured, it returned 72,974 magic items including
  `Table of Contents`, and the identical number for feats because the two
  declarations were indistinguishable.
- **FR-014**: Pathfinder 2e MUST be importable by adding a declaration to its
  pack and changing no shared code.
- **FR-015**: A world whose system declares no content patterns MUST have the
  import refused with that as the reason, before any file is read.
- **FR-016**: The existing creature reader MUST be reached through the same
  declaration rather than being a second, separate path. Whatever it does that
  the declaration cannot yet express MUST be named, not quietly preserved.

**The confirmation, before anything is sent**

- **FR-020**: Reading MUST happen on the Game Master's machine, and nothing
  read MUST leave it before the Game Master submits.
- **FR-021**: The review MUST show everything found, grouped by kind, with a
  count per kind and a total.
- **FR-022**: The review MUST let the Game Master look at any individual
  entry, including its uncertain fields and the page it came from.
- **FR-023**: The review MUST make uncertain and unread fields visibly
  different from read ones.
- **FR-024**: The review MUST require an explicit submit. No timer, no default
  action, no submit-on-close.
- **FR-025**: Closing or abandoning the review MUST leave the world unchanged
  and MUST leave no import record.
- **FR-026**: The Game Master MUST be able to exclude kinds of content, or
  individual entries, from what is submitted.
- **FR-027**: What is committed MUST be what the review showed. A value
  changed between review and commit is a defect.
- **FR-028**: Only a **Game Master** MUST be able to read a book into a world,
  and only from that world's own panel. This is the boundary spec 048 records:
  a Game Master can already put anything into their own world by hand, so
  reading on their machine hands them nothing new. A player's character-sheet
  import is a different path with different guards, and this spec does not
  widen it.

**Sending**

- **FR-030**: Submitting MUST show progress that reflects real work sent, not
  an animation.
- **FR-031**: Progress MUST say what is being sent, not only how much.
- **FR-032**: An import MUST apply completely or not at all. A failure partway
  MUST leave the world exactly as it was.
- **FR-033**: A failure MUST name what went wrong.
- **FR-034**: The Game Master MUST be able to abandon an import in flight, and
  abandoning MUST leave nothing behind.
- **FR-035**: A book producing more entries than a stated bound MUST be
  refused before sending, with the bound named.
- **FR-036**: The server MUST re-check authorization and the world's system on
  arrival. A review that happened in a browser is not a permission.

**The compendium**

- **FR-040**: Everything one import produced MUST be held as one **compendium**
  belonging to the **account that imported it**, not to a world. A world uses
  it by inheriting it. Where it lives, how a world inherits it, and how a
  world's own changes sit over it are spec 050's; this spec produces the
  compendium and stops there.
- **FR-041**: A compendium MUST record the book's name, the file's SHA-256,
  who imported it, when, the system it was read as, and a count per kind.
- **FR-042**: The world's Compendium portal MUST let a Game Master browse by
  compendium, beside the existing browse by kind.
- **FR-043**: Every imported entry MUST name the compendium it belongs to and
  the page it was found on.
- **FR-044**: A Game Master MUST be able to remove a whole compendium from
  their library, and removal MUST take only what that import contributed.
- **FR-045**: Removal MUST name what is in use before it is confirmed — a
  creature on a scene, an item in an inventory, in **any** world that
  inherited it — rather than leaving a dangling reference.
- **FR-046**: An entry a Game Master has edited by hand MUST NOT be silently
  replaced by a re-import (spec 047 FR-074) and MUST be named before removal.
  Under spec 050 such an edit is a world's delta, and a re-import replaces the
  base beneath it rather than the edit itself.
- **FR-047**: Re-importing the same file MUST follow spec 047 FR-070 to
  FR-075: recognised by hash before the upload, with overwriting offered. The
  hash MUST be checked **against the account's library**, not against one
  world — importing a book a second time for a second world is the duplication
  this is here to prevent.

**Provenance, and what may not be shared**

- **FR-050**: Whether content may be shared MUST be decided by **where it came
  from**, not by what licence it carries. There are two origins and the line
  between them is mechanical:
  - **Authored** — made in ThunderForge by a person, through the authoring
    tools. Shareable, and the raw material of a collection (spec 026).
  - **Uploaded** — read out of a document somebody supplied. **Never**
    shareable, published, exported, or adopted into a collection.
- **FR-050a**: This MUST NOT require anybody to determine whether a work is
  commercial. The system already knows with certainty which of its two paths a
  piece of content arrived by, and that is the whole test.
- **FR-050b**: Content shipped in a **system pack** is a third thing and is
  unaffected. It is distributed by the platform under the pack's own `legal`
  block (spec 016), not uploaded by a user.
- **FR-051**: Every compendium MUST record its origin at import, automatically.
  A compendium produced by this spec is **uploaded**, always, with no
  question asked and no answer to get wrong.
- **FR-052**: An uploaded compendium MUST have sharing, publishing, export and
  collection-adoption **disabled**, regardless of the licence of the document
  it came from.
- **FR-053**: Each refusal MUST say why: that this content was read out of an
  uploaded book, and uploaded content stays with the account that uploaded it.
  Where the source is known to be commercial the refusal MAY say so as well,
  but MUST NOT depend on knowing it.
- **FR-054**: The restriction MUST follow individual entries, not only the
  bucket, and MUST follow anything **derived** from them. A world's change to
  an uploaded entry is a change to uploaded content and is not shareable — a
  mutation has no meaning apart from the thing it mutates.
- **FR-054a**: A collection MUST NOT be able to contain uploaded content by
  any route. This MUST be enforced as an invariant, not as a check on the
  routes known today.
- **FR-055**: An uploaded compendium MUST be fully usable in **every world the
  importing account owns** — placed on scenes, handed to players at those
  tables, used in play. One person's copy of a book serves all of that
  person's tables; that is still reading it over their shoulder, and it is
  still not lending it out.
- **FR-055a**: It MUST NOT become inheritable by any **other** account,
  including a co-Game Master of a world it was inherited into. Being able to
  use content at a table is not being able to take it home.
- **FR-056**: An openly licensed document read in by a user is still uploaded
  content and MUST NOT become shareable on the strength of its licence. Where
  its attribution requirements are known they MUST still be carried and shown
  as spec 016 requires — a licence's obligations survive even where its
  permissions are not being exercised.
- **FR-056a**: This spec accepts a known cost: a person who uploads a document
  they wrote themselves, or an openly licensed one, cannot share what comes out
  of it. The two routes that remain open MUST be named where the refusal is
  shown — author it through the authoring tools, which produces shareable
  content, or propose it as a system pack.
- **FR-057**: Origin MUST NOT be editable. There is no path, for any role, that
  turns uploaded content into authored content. A value that can be flipped is
  a value that will be flipped.

**Proof**

- **FR-060**: A measurement run over the real 246-book library MUST report,
  per book: whether it opened, how many pages were silent, and how many
  entries of each kind were found. The numbers in Success Criteria MUST come
  from that run, generated rather than transcribed.
- **FR-061**: An end-to-end test MUST prove that nothing is sent before submit
  — by observing the network, not by reading the code.
- **FR-062**: An end-to-end test MUST prove an import applies completely or
  not at all, by failing one partway.
- **FR-063**: An end-to-end test MUST prove that sharing an uploaded
  compendium is refused, that sharing a world's change to one of its entries
  is refused, and that sharing content authored by hand is not. The licence of
  the uploaded document MUST make no difference to any of the three.
- **FR-064**: An end-to-end test MUST prove a second system imports through
  the same code path as the first.
- **FR-065**: Unit tests alone MUST NOT be accepted as proof for any of the
  above. A reader that passes on an idealised layout and fails on a real book
  has already happened here more than once.

### Key Entities

- **Compendium**: everything one import produced, belonging to the **account**
  that imported it — never to a world (FR-040, decision 1). Carries the book's
  name, the file's hash, who imported it and when, the system it was read as,
  its origin, and counts per kind.
- **Imported entry**: one spell, item, creature, feature or feat. Carries its
  kind, its values with per-field certainty, the page it was found on, and the
  compendium it belongs to.
- **Content pattern**: a system pack's declaration of what anchors a kind of
  content in that system and which fields to read from it.
- **Origin**: which of two paths a piece of content arrived by — **authored**
  or **uploaded** — and therefore whether it may be shared. Recorded
  automatically, never declared, never editable.
- **Import record**: who imported what, when, from which file, and what it
  wrote — the thing a removal or a rollback works against.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Across the 246-book measurement library, every book that yields
  text produces either content or a stated reason for producing none. No book
  reports an empty success.
- **SC-002**: For D&D 5e source books, at least 90% of spells present in a
  book are found, measured against a hand-counted sample of at least three
  books. *(A target, not yet an observation. FR-060's measurement run will
  either meet it or replace this number with the one it found — the creature
  reader's 717 creatures were arrived at the same way.)*
- **SC-003**: No entry in any measurement run carries a mechanical value that
  is not present in the book — verified by sampling, and any instance is a
  release blocker.
- **SC-004**: A Game Master can go from choosing a file to a reviewable list
  of what was found, for a 350-page book, in under 30 seconds on a typical
  laptop.
- **SC-005**: Zero bytes of book content leave the machine before submit is
  pressed, observed on the wire.
- **SC-006**: A Game Master who submits a 70 MB book sees progress advance at
  least once every two seconds throughout.
- **SC-007**: An import that fails partway leaves the world byte-identical to
  its state before the import, in 100% of induced-failure runs.
- **SC-008**: Every route out of an account — share, publish, adopt, export —
  refuses uploaded content and anything derived from it, with a reason naming
  its origin, in 100% of attempts. Authored content is refused by none of
  them.
- **SC-009**: Adding a second game system's book support requires changes to
  that system's pack only, and the system registry check passes unchanged.
- **SC-010**: A Game Master can tell which book any piece of imported content
  came from in one step from where they found it.

## Assumptions

- **Sharing is decided by origin, not licence.** Uploaded content is never
  shareable; authored content is. Nothing needs to determine whether a work is
  commercial, which is why there is no longer a question about how to. See
  decision 4.
- **Image-only books are refused, not OCR'd.** 51 of 246 measured books have
  no text layer. Optical character recognition is out of scope for this spec;
  those books are refused with that as the reason. Q2 asks whether that is
  right.
- **One file, one compendium, one account.** A multi-volume work imported as
  three PDFs is three compendiums. Merging them is the Game Master's business,
  not the importer's. A book imported by two different accounts is two
  compendiums, and deliberately so — see the decision below.
- **Deduplication is not attempted.** The same spell in two books is two
  entries. Choosing between them is a decision with no correct automatic
  answer.
- **The existing browser reader is the only reader.** The same Rust compiled
  to WebAssembly, not a second implementation of "what does this book say".
- **Spec 047's hash rules are reused unchanged.** This spec does not restate
  or modify FR-070 to FR-075; it depends on them.
- **Spec 048's per-field certainty rules are reused unchanged.** FR-003 there
  is the rule; FR-003 here points at it.
- **The world's Compendium portal (spec 011) is extended, not replaced.** It
  gains a way to browse by compendium; its existing tabs stay.
- **Removal is not versioned.** Spec 048's rollback covers a character's
  imports. A compendium is removed whole or kept whole; if per-import
  versioning is wanted, that is a separate decision — Q3.

## Decisions (owner, 2026-09-12)

1. **A compendium belongs to the account, and worlds inherit it** (decided
   after this spec was first written, before any of it was built).

   The question that prompted it was the operator's: a Game Master with eight
   worlds who imports the same book eight times costs eight times the disk for
   one book. The first answer considered was deduplicating **across** accounts
   by SHA — if two Game Masters upload the identical file, serve them both the
   one parse.

   That answer was rejected, and it is worth recording why, because it looks
   free and is not:

   - **A hash is not proof of possession.** "Send the SHA, receive the book"
     makes the SHA *be* the book, and the hashes of the popular books would
     circulate immediately. Making it safe needs a random-range possession
     challenge against the actual bytes — real work, and only ever as strong
     as the assumption that nobody runs an oracle.
   - **It walks into the constitution's DMCA guardrail.** Serving one
     canonical store of parsed commercial books to multiple accounts is at
     least arguably the "centralized public repository" category that
     checkpoint exists to catch, and it would need the takedown program
     operational and an on-record risk acceptance before any build work.
   - **It makes the deletion promise untrue.** A shared base that survives its
     last-but-one referrer cannot be described to a person as "deleted", and
     no amount of careful wording fixes that.

   Moving compendiums **out of worlds and onto the account** gets the entire
   saving that motivated the question — one book, one parse, however many
   worlds — with none of those three problems. Possession is not in question
   because it is the same person. Nothing crosses an account boundary, so the
   guardrail is not engaged. And when the account goes, its compendiums go,
   with nothing kept behind the scenes and nothing to explain.

   Cross-account deduplication is therefore **not being built**, and is not
   merely deferred: it would need its own spec, an ADR for the possession
   protocol, and the constitution's determination made by an accountable
   owner first.

2. **Worlds inherit and delta.** A world does not copy a compendium; it
   inherits it and keeps its own changes over the top. What a Game Master
   changes in one world does not change it in another, and the base underneath
   stays shared. Spec 050 owns this.

3. **Account deletion takes the compendiums.** Consistent with what already
   happens to worlds, and now simply true rather than true-with-a-caveat.

4. **Sharing is decided by origin, not by licence** (owner, closing Q1 by
   removing it).

   There are two kinds of content and the line between them is mechanical:

   - **Authored** — made in ThunderForge through the authoring tools. May be
     shared, and is what a collection is made of.
   - **Uploaded** — read out of a document somebody supplied. Never shared.

   The rule this replaces asked a Game Master to declare whether a book was
   commercial, with document detection as a possible assist. That rule needed
   a judgment, made by the person least motivated to answer strictly, against
   PDF metadata that is often absent or wrong, and it could be got wrong in
   the direction that costs the most. This rule needs no judgment from anyone:
   the system already knows which of its two paths a piece of content arrived
   by, with certainty, and cannot be told otherwise.

   It is also simpler to enforce, because it becomes an invariant rather than
   a check. A collection may not contain uploaded content **by construction**,
   rather than because every route into one remembered to ask.

   Content shipped in a **system pack** is a third thing and is untouched — it
   is distributed by the platform under the pack's own `legal` block (spec
   016), not uploaded by a user.

   **The known cost, recorded rather than discovered**: a person who uploads a
   document they wrote themselves, or an openly licensed one, cannot share
   what comes out of it. Two routes remain open and the refusal names them —
   author it through the authoring tools, or propose it as a system pack. That
   is a real loss, and it is smaller than the cost of any rule that requires
   somebody to correctly classify a PDF.

## Questions for the owner

*(Q1 — how "commercial" is determined — was withdrawn on 2026-09-12. It has no
answer because it has no question: see decision 4.)*

### Q2 — What happens to a book with no text layer?

**Context**: Edge cases, and the Assumptions entry above. 51 of 246 measured
books — 21% of a real shelf — are image scans.

| Option | Answer | Implications |
|--------|--------|--------------|
| A | Refuse with a plain reason: this book is images | Honest and cheap. A fifth of a real library cannot be imported at all. |
| B | Refuse, and say what would let it work — a text-layer PDF from the publisher | Same cost, better outcome for the person, who often does have a better file. |
| C | Optical character recognition, in the browser | Unlocks the fifth. A large piece of work, and OCR output is exactly the "plausible and wrong" text this spec is most afraid of — it would need the confidence marking to be right first. |

### Q3 — Can a compendium be rolled back, or only removed?

**Context**: FR-044 to FR-047, and the Assumptions entry. Spec 048 gives a
character's imports versioning and rollback; a compendium currently gets
removal only.

| Option | Answer | Implications |
|--------|--------|--------------|
| A | Removal only | Simplest. A Game Master who re-imports a corrected book and dislikes the result has to remove and re-import. |
| B | Each import of the same book is a version, and a Game Master can go back | Matches spec 048's shape, and matches "you already have this, overwrite?" — the previous state is what overwriting destroys. More storage, more to explain. |
| C | Removal only, but the previous file is kept so a re-import is one click | A middle that costs storage without giving the restore. |
