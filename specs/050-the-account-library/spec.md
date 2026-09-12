# Feature Specification: The Account's Library

**Feature Branch**: `050-the-account-library`

**Created**: 2026-09-12

**Status**: Draft

**Input**: User description: "Maybe we do per account dedupe but more explicitly move compendiums outside worlds and have worlds be able to inherit content and delta it, like change per world. That covers our bacon, and when the account's gone the compendiums are gone." Following from: "how can we reduce the size on disk too for us as the operator — if a GM has 8 worlds and they upload the book 8 times… think like a graph db but for tabletop PDFs?" And: "can a user who GMs a world have worlds and compendiums where they select a game system and can upload PDFs, allowing them to preseed their worlds with content?"

## The problem, in one sentence

A Game Master with eight worlds and one Monster Manual should own one Monster
Manual.

Today content lives inside a world and nowhere else. An actor cannot exist
outside one, and under spec 049 as first written a compendium could not
either — so the eighth world means the eighth import of the same book, the
eighth parse, and the eighth copy on disk of text that is identical in all
eight.

This spec moves the shelf out from under the world. A book is read once, into
the **account's library**. Worlds **inherit** from it. A world that changes
something keeps that change as its own **delta**, over a base that stays
shared with the other seven.

## Why not deduplicate across accounts

It was considered first, because it is the bigger number: everybody's shelf
has the same three books on it. Spec 049's decision 1 records the whole
argument; the short version is that it needs a proof-of-possession protocol to
stop a hash from *being* the book, it engages the constitution's DMCA
guardrail because it means operating one canonical store of parsed commercial
books served to many accounts, and it makes "delete my data" a sentence with
an asterisk on it.

Per-account inheritance gets the entire saving that motivated the question —
one book, one parse, however many worlds — and none of the three problems.
Possession is not in question, because it is the same person. Nothing crosses
an account boundary. And when the account goes, its library goes with it,
with nothing kept behind the scenes.

## Where this sits

- **Spec 049** reads a book and produces a compendium. It stops there.
- **This spec** owns where that compendium lives, how a world uses it, and
  what happens when either changes.
- **Spec 011** owns the world's Compendium portal, which is where the
  inherited result is browsed.
- **Spec 026** owns collections — user-authored content, made to be shared.
  A library compendium is not one, and mostly cannot become one.

It also answers a direction recorded on 2026-09-10 but never specified: that a
user should be able to hold content outside a world, transportable into future
worlds, with game-system compatibility attached. This is that, for imported
content.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Read a book once, use it everywhere (Priority: P1)

A Game Master reads the Monster Manual into their library. It appears on their
shelf with its name, its system, when it was read and how much is in it. They
open a world and inherit it; they open a second world and inherit it there
too. Both worlds now have the whole Monster Manual, and the book was read
once and is stored once.

When they try to read the same file in again, they are told they already have
it — because the check is now against their shelf rather than against one
world, which is the duplication this exists to stop.

**Why this priority**: It is the entire premise. Every other story is about
what happens after a world has inherited something.

**Independent Test**: Read one book into a library, inherit it into two
worlds, confirm both worlds can use all of it, and confirm the stored size is
that of one import rather than two.

**Acceptance Scenarios**:

1. **Given** a Game Master has read a book, **When** they open their library,
   **Then** it is listed with its name, system, import date and counts.
2. **Given** a compendium on the shelf, **When** they inherit it into a world,
   **Then** everything in it is usable in that world.
3. **Given** the same compendium inherited into a second world, **When**
   stored size is measured, **Then** it has not grown by a second copy.
4. **Given** a book already on the shelf, **When** the same file is read in
   again, **Then** they are told they already have it and offered the
   overwrite path, per spec 047 FR-070 to FR-075.
5. **Given** a world that has inherited a compendium, **When** a Game Master
   browses the world's Compendium portal, **Then** inherited content appears
   there beside anything authored in the world itself.

---

### User Story 2 - Each world changes its own copy (Priority: P2)

A Game Master decides the goblins in their gritty campaign hit harder, and
edits the goblin in that world. The goblin in their other six worlds is
untouched. They add a homebrew monster to one world; it does not appear in
the others. They hide an entry they dislike in one world; it is still there in
the rest.

**Why this priority**: Without it, inheriting is worse than copying — one
edit would reach across every table the Game Master runs, which is a
regression against what they have today.

**Independent Test**: Inherit one compendium into two worlds, change an entry
in one, and confirm the other is unchanged and that the base is unchanged.

**Acceptance Scenarios**:

1. **Given** a compendium inherited into two worlds, **When** an entry is
   edited in one, **Then** the other world's copy is unchanged.
2. **Given** an edited entry, **When** the world reads it, **Then** it sees
   the edit, not the original.
3. **Given** a world-only addition, **When** another world reads the same
   compendium, **Then** the addition is absent there.
4. **Given** an entry hidden in one world, **When** it is read there, **Then**
   it is absent; **When** read in another world, **Then** it is present.
5. **Given** any of the above, **When** the account's stored base is examined,
   **Then** it is byte-identical to what the import produced.
6. **Given** an edited entry, **When** the Game Master asks, **Then** they can
   see what it was before they changed it, and put it back.

---

### User Story 3 - Start a world from your own shelf (Priority: P3)

A Game Master creates a new world, chooses a game system, and is offered the
compendiums on their shelf that match it. They tick three, and the world
begins with content already in it — monsters to drop on a map, spells to hand
out, items to place — rather than empty.

**Why this priority**: It is the payoff the Game Master actually feels. The
first two stories save the operator disk and save the Game Master a re-import;
this one is the reason they would look at the feature at all.

**Independent Test**: Create a world on a system, confirm only compendiums
matching that system are offered, seed from two of them, and confirm the new
world opens with their content present.

**Acceptance Scenarios**:

1. **Given** a Game Master is creating a world, **When** they choose a system,
   **Then** only compendiums read as that system are offered.
2. **Given** they select some, **When** the world is created, **Then** their
   content is present without a further step.
3. **Given** they select none, **When** the world is created, **Then** it is
   empty, and inheriting later is still available.
4. **Given** an existing world, **When** the Game Master inherits a compendium
   into it, **Then** it behaves exactly as it would have at creation.

---

### User Story 4 - The shelf is yours, and it goes when you go (Priority: P4)

A Game Master looks at what their library is costing, removes a book they no
longer want, and is told first what in which worlds depends on it. When they
delete their account, the library goes with it — entirely, with nothing
retained.

**Why this priority**: It is the promise the whole architecture was chosen to
be able to make honestly. It is also the one that is expensive to retrofit,
because it constrains where things may be stored from the first commit.

**Independent Test**: Remove a compendium that is in use and confirm what
depends on it is named first; delete an account and confirm nothing of its
library remains.

**Acceptance Scenarios**:

1. **Given** a compendium in use in two worlds, **When** removal is requested,
   **Then** what is in use is named, per world, before it is confirmed.
2. **Given** removal is confirmed, **When** it completes, **Then** the
   compendium and every world's delta over it are gone, and no other
   compendium is touched.
3. **Given** an account is deleted, **When** deletion completes, **Then** no
   part of its library remains, including bases no longer referenced by any
   world.
4. **Given** a world shared with players is deleted along with its account,
   **When** deletion runs, **Then** the existing rule that copies players'
   actors to their own accounts still runs first, and inherited content is not
   copied with them.

---

### User Story 5 - A corrected book, without losing your changes (Priority: P5)

The Game Master re-reads a book — a better scan, a corrected file, or the same
file after the reader itself improved. The base underneath is replaced, and
the changes their worlds made over it survive. Where a change can no longer be
applied, because the entry it belonged to is no longer there, they are told
which ones and why.

**Why this priority**: It is the thing that makes the model trustworthy over
time. Spec 047 FR-074 already promises that re-importing will not trample
hand edits; under this architecture that promise is structural rather than
best-effort, and it needs proving.

**Independent Test**: Inherit a compendium, edit entries in two worlds,
re-import a changed version of the book, and confirm the edits survive and
that any that cannot be re-applied are reported rather than dropped.

**Acceptance Scenarios**:

1. **Given** worlds with deltas over a compendium, **When** the book is
   re-imported, **Then** the base is replaced and the deltas still apply.
2. **Given** a delta whose base entry no longer exists, **When** the
   re-import completes, **Then** it is reported by name and is not silently
   discarded.
3. **Given** a re-import, **When** it fails partway, **Then** the previous
   base is still in place and no delta has been touched.

---

### Edge Cases

- **A world stops inheriting a compendium whose entries it had edited.** The
  deltas belong to a base that is no longer there. They must be reported
  before the inheritance is dropped, not orphaned in silence.
- **A co-Game Master in someone else's world.** They can use inherited
  content at that table and cannot inherit it into a world of their own. Being
  able to use a book is not being able to take it home.
- **Two compendiums that both contain Fireball.** Inheriting both means two
  entries of the same name from two sources. They are not merged; each names
  the book it came from, and choosing between them is the Game Master's.
- **A world whose system does not match a compendium's.** It cannot be
  inherited. A world that changes system afterwards must report what is now
  mismatched rather than quietly continuing to serve it.
- **An entry a player's character depends on.** Removing the inheritance must
  name the character, the same way removing a compendium names what is in use.
- **A re-import that renames an entry.** Entry identity is by kind and name
  within the compendium, so a rename reads as a removal plus an addition, and
  the delta on the old name is reported as unattachable. This is the known
  weak point of the identity rule and is stated rather than hidden.
- **A compendium inherited by no world at all.** It stays on the shelf. A
  library is a shelf, not a cache.
- **A delta that grows larger than the base.** Legitimate — a world can
  rewrite everything it inherited. Nothing should constrain it below the
  bounds spec 049 already sets.
- **The same book read in by two accounts.** Two compendiums, two copies,
  deliberately. See spec 049 decision 1.

## Requirements *(mandatory)*

### Functional Requirements

**The library**

- **FR-001**: Every compendium MUST belong to exactly one **account** and MUST
  NOT belong to a world.
- **FR-002**: An account MUST have a library listing its compendiums with, for
  each, the book's name, the system it was read as, when it was read, counts
  per kind, its provenance, and which of that account's worlds inherit it.
- **FR-003**: The library MUST report what it is costing in storage, per
  compendium and in total.
- **FR-004**: The duplicate check on import (spec 047 FR-070 to FR-075) MUST
  be against **the account's library**, not against a world.
- **FR-005**: A compendium MUST remain in the library when no world inherits
  it. The shelf is not a cache.
- **FR-006**: A base, once written, MUST be immutable for as long as it is the
  current base. Changing what a book says MUST be a new version of the base,
  never an edit of the existing one.

**Inheriting into a world**

- **FR-010**: A Game Master MUST be able to inherit a compendium from their
  library into any world they own.
- **FR-011**: Inheriting MUST NOT copy the content. A second world inheriting
  the same compendium MUST NOT increase stored content.
- **FR-012**: Inherited content MUST be usable in the world exactly as
  world-authored content is — placed on scenes, handed to players, searched,
  referenced.
- **FR-013**: A Game Master MUST be able to stop inheriting, and stopping MUST
  name what is in use and what deltas will be lost before it is confirmed.
- **FR-014**: A compendium MUST NOT be inheritable by an account that does not
  own it, by any route, including through a world that account co-runs.
- **FR-015**: Inheritance MUST be recorded — which world, which compendium,
  which base version, who, when.

**The delta**

- **FR-020**: A world MUST be able to change what it inherited without
  changing the base or any other world.
- **FR-021**: A world's changes MUST be held as a **delta** over the base, in
  three forms: an entry **changed**, an entry **hidden**, and an entry
  **added**.
- **FR-022**: What a world reads MUST be the base with its delta applied.
- **FR-023**: A delta MUST record only what differs, not a whole copy of the
  entry.
- **FR-024**: A Game Master MUST be able to see what an entry was before their
  world changed it, and restore it.
- **FR-025**: An entry's identity within a compendium MUST be its kind and its
  name. A delta attaches to that identity. *(The consequence — a renamed entry
  reads as a removal and an addition — is stated in Edge Cases and is Q2 for
  the owner.)*
- **FR-026**: Deltas MUST survive a re-import that replaces the base beneath
  them.
- **FR-027**: A delta that can no longer attach to anything MUST be reported
  by name and MUST NOT be silently discarded.
- **FR-028**: Removing a world MUST remove its deltas and MUST NOT affect the
  base or any other world.

**Seeding a new world**

- **FR-030**: Creating a world MUST offer the compendiums on the creator's
  shelf that match the world's chosen system.
- **FR-031**: Compendiums selected at creation MUST be inherited as part of
  creating the world, with no further step.
- **FR-032**: Creating a world with none selected MUST be allowed and MUST
  leave inheriting available afterwards.
- **FR-033**: Seeding at creation and inheriting later MUST produce the same
  state. Two routes to two different outcomes is a defect.

**System compatibility**

- **FR-040**: A compendium MUST record the system it was read as.
- **FR-041**: A compendium MUST NOT be inheritable into a world on a different
  system.
- **FR-042**: A world that changes system MUST report which inherited
  compendiums no longer match, and MUST NOT continue serving them as though
  they did.

**What may not leave**

- **FR-050**: Spec 049's provenance rules MUST hold at every point here. A
  commercial compendium MUST NOT be shareable, publishable, or adoptable into
  a collection, from the library or from any world that inherited it.
- **FR-051**: Inheriting into the owner's own world MUST NOT count as sharing.
  One person's copy serves all of that person's tables.
- **FR-052**: A refusal MUST say why, naming the commercial source, as spec
  049 FR-053 requires.
- **FR-053**: The content agreement MUST be shown on every import and MUST
  state what is true under this architecture: that the content is for that
  person and their games, that it is not shared between users at any point,
  and that deleting it deletes it.

**Deletion**

- **FR-060**: Removing a compendium MUST name what depends on it, per world,
  before it is confirmed.
- **FR-061**: Removing a compendium MUST remove its base and every delta over
  it, and nothing else.
- **FR-062**: Deleting an account MUST delete its entire library, including
  bases no world referenced.
- **FR-063**: Account deletion MUST continue to copy players' actors to their
  own accounts first, and MUST NOT copy inherited content with them — that
  would move a book between accounts.
- **FR-064**: Nothing from a deleted account's library MUST be retained for
  any purpose, including deduplication. There is no other account's copy to
  keep it for.

**Storage, which is the operator's half of this**

- **FR-070**: Stored size MUST scale with the number of distinct books an
  account has read, not with the number of worlds that use them.
- **FR-071**: The saving MUST be **measured** against a realistic library and
  reported, generated rather than transcribed, the way engine and cache
  performance numbers already are.
- **FR-072**: Resolving base plus delta MUST NOT make reading content
  noticeably slower than reading world-owned content is today, and "noticeably"
  MUST be given a number by measurement before this ships.

**Proof**

- **FR-080**: An end-to-end test MUST prove one book inherited into two worlds
  is stored once.
- **FR-081**: An end-to-end test MUST prove an edit in one world does not
  reach another, and does not reach the base.
- **FR-082**: An end-to-end test MUST prove a re-import replaces a base with
  deltas over it and the deltas survive.
- **FR-083**: An end-to-end test MUST prove account deletion leaves nothing of
  the library.
- **FR-084**: An end-to-end test MUST prove a co-Game Master cannot inherit
  another account's compendium into a world of their own.
- **FR-085**: Unit tests alone MUST NOT be accepted as proof for any of the
  above.

### Key Entities

- **Library**: an account's shelf. Owns compendiums, belongs to exactly one
  account, dies with it.
- **Compendium**: everything one book produced (spec 049). Belongs to a
  library. Carries its book's name, hash, system, provenance and counts.
- **Base**: the immutable content of a compendium at one version. Replaced
  wholesale by a re-import, never edited.
- **Inheritance**: a link from a world to a compendium in its owner's library,
  naming the base version in force.
- **Delta**: one world's changes over one inherited compendium — entries
  changed, hidden and added. Belongs to the world.
- **Resolved entry**: what a world actually sees: the base entry with its
  delta applied, or a world-only addition.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A Game Master with eight worlds and one book stores one copy of
  that book's content, measured on a real instance.
- **SC-002**: Stored content for an account grows with distinct books read and
  does not grow when a compendium is inherited into an additional world.
- **SC-003**: Creating a world seeded from three compendiums takes no longer
  than creating an empty one plus the time to show the choice.
- **SC-004**: Reading an inherited entry is no slower than reading a
  world-owned one by a margin fixed by measurement before release.
- **SC-005**: An edit in one world reaches no other world and no base, in 100%
  of runs.
- **SC-006**: After a re-import, 100% of deltas whose entry still exists still
  apply, and 100% of those whose entry does not are reported by name.
- **SC-007**: After account deletion, zero bytes of that account's library
  remain, verified by inspection rather than by assertion.
- **SC-008**: No account can reach another account's compendium by any route
  exercised in testing.
- **SC-009**: A Game Master can tell, for any piece of content in a world,
  which book it came from and whether this world has changed it.

## Assumptions

- **Compendiums do not cross accounts, at all.** Not by sharing, not by
  export, not by deduplication. This is the decision that makes the rest of
  the spec simple, and it is spec 049 decision 1.
- **Only imported content moves to the library in this spec.** World-authored
  actors, items and lore stay where they are. The 2026-09-10 direction of
  profile-level content organised by collections is broader than this and
  stays a separate piece of work; this spec is the imported half of it and
  should not foreclose the rest.
- **A world inherits from its owner's library only.** Co-Game Masters use, do
  not inherit.
- **Entry identity is kind plus name.** The simplest rule that works across a
  re-parse. Its weakness is renames, which is Q2.
- **Deltas are not versioned.** A Game Master can restore an entry to its base
  (FR-024), which covers the common mistake. A full history of world edits is
  a different feature.
- **Spec 049 ships first, or at least its reader does.** This spec has nothing
  to hold until a book can be read into a compendium.
- **The world's Compendium portal is extended, not replaced** (spec 011), the
  same assumption spec 049 makes.

## Questions for the owner

### Q1 — What happens to a world's changes when it stops inheriting?

**Context**: FR-013, FR-027, and the first edge case. A Game Master inherits
the Monster Manual, spends a month tuning forty monsters for their campaign,
then removes the inheritance. The base goes. Their month does not obviously go
with it.

| Option | Answer | Implications |
|--------|--------|--------------|
| A | The deltas go too, named first so the Game Master can change their mind | Simplest and most predictable — a delta is meaningless without its base. Loses real work to one confirmed click. |
| B | Changed and added entries are kept as world-owned content, hidden ones are forgotten | Keeps the month. The kept entries are now derived from a commercial book and sitting in a world as ordinary content, which is exactly the provenance leak FR-050 is guarding. |
| C | B, but the kept entries keep their provenance and stay unshareable | Keeps the month and keeps the guard. Most to build, and a Game Master ends up with unshareable content whose source they can no longer see on the shelf. |

### Q2 — How should an entry be identified across a re-import?

**Context**: FR-025, FR-026, and the rename edge case. A delta has to find the
entry it belongs to in a base that was parsed again, possibly by a better
parser that draws the boundaries differently.

| Option | Answer | Implications |
|--------|--------|--------------|
| A | Kind plus name | Works for the overwhelming majority, is explicable to a person in one sentence, and breaks on renames and on two entries sharing a name. The spec's current default. |
| B | Kind, name, and the page it was found on | Survives a duplicate name. Breaks on any re-flow, which a corrected file usually is — likely worse in practice than A. |
| C | Fuzzy match on name and content, reporting anything below a confidence bar | Survives renames and re-flows. Introduces exactly the class of quiet, plausible wrongness this project has been bitten by repeatedly, into the one place where being wrong silently rewrites a Game Master's work. |

### Q3 — Does the library eventually hold more than imported books?

**Context**: The Assumptions entry, and the 2026-09-10 direction about
profile-level content by collection. This spec deliberately scopes to imported
compendiums.

| Option | Answer | Implications |
|--------|--------|--------------|
| A | Imported books only, forever; collections stay world-bound | Smallest scope. Leaves the original problem — a player's character with nowhere to live when a world goes — unsolved. |
| B | Imported books now, built so authored collections can join the same shelf later | What this spec assumes. Costs a little care in naming and structure now, no extra build. |
| C | One library holding both from the start | The whole 2026-09-10 direction at once. Much larger, and it drags collection sharing and adoption rules in with it. |
