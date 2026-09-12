# Feature Specification: The Account's Library

**Feature Branch**: `050-the-account-library`

**Created**: 2026-09-12

**Status**: Draft

**Input**: User description: "Maybe we do per account dedupe but more explicitly move compendiums outside worlds and have worlds be able to inherit content and delta it, like change per world. That covers our bacon, and when the account's gone the compendiums are gone." Following from: "how can we reduce the size on disk too for us as the operator — if a GM has 8 worlds and they upload the book 8 times… think like a graph db but for tabletop PDFs?" And: "can a user who GMs a world have worlds and compendiums where they select a game system and can upload PDFs, allowing them to preseed their worlds with content?"

> **Planned with spec 049, not separately.** The owner chose on 2026-09-12 to
> build the import and the library as one arc, so this spec's plan, research,
> data model, contracts and tasks live in
> [`specs/049-importing-a-source-book/`](../049-importing-a-source-book/plan.md).
> There is deliberately no `plan.md` here.

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
  Collections sit on the same shelf as imported compendiums and behave the
  same way in a world; what separates them is origin, and everything that
  follows from it.

It also answers a direction recorded on 2026-09-10 but never specified: that a
user should be able to hold content outside a world, transportable into future
worlds, with game-system compatibility attached. This is that.

## Two kinds of thing on one shelf

| | **Compendium** | **Collection** |
|---|---|---|
| Where it came from | Read out of a document | Authored in ThunderForge |
| Switched on in a world | Yes | Yes |
| Deltaed by a world | Yes, to any extent | Yes |
| Changes sync back to it | **No, ever** | Yes |
| Shared, published, adopted | **No** | Yes |
| Downloaded by its owner | **No** | Yes, as JSON |
| Dies with the account | Yes | Yes |

The owner's word for both is "compendium". This spec uses two words because
the repository already reserves "collection" for user-authored shareable
content, and because the difference between these two columns **is** the legal
boundary — it is the one place in the product worth spending a second noun on.

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

### User Story 3 - A book list on the world, like a mod list (Priority: P3)

A world shows its books down the left: which compendiums are switched on for
this table. The Game Master picks from the ones on their shelf that match the
world's system, ticks three, and the world's content is simply *there* —
fetched from the library and shown, not copied into the world.

Players see the same list, read only. They cannot change it, and they do not
need to: it tells them what this table is running, the way a mod list does.

Turning one off takes its content out of the world again. Nothing was
duplicated going in and nothing is left behind coming out.

**Why this priority**: It is the payoff the Game Master actually feels, and it
is also what makes the first two stories coherent. There is no "seeding" step
at world creation and no separate "inherit later" flow — there is one list,
and it can be ticked at creation or at any point after. Two routes to the same
state is two things to keep agreeing with each other.

**Independent Test**: Create a world, confirm only compendiums matching its
system are offered, switch two on, confirm their content is present and that
stored content did not grow, switch one off and confirm its content is gone.
Then open the world as a player and confirm the list is visible and read-only.

**Acceptance Scenarios**:

1. **Given** a Game Master is in a world, **When** they open the book list,
   **Then** it shows which of their compendiums are on for this world and
   offers those on their shelf that match its system.
2. **Given** they switch a compendium on, **When** the world reads content,
   **Then** that compendium's entries are there, fetched rather than copied.
3. **Given** a compendium is switched on in a second world, **When** stored
   content is measured, **Then** it has not grown.
4. **Given** a player opens the world, **When** they view the book list,
   **Then** they see which books are on and can change nothing.
5. **Given** a compendium is switched off, **When** the world reads content,
   **Then** its entries are gone and no copy of them remains in the world.
6. **Given** a world is being created, **When** the Game Master ticks
   compendiums during creation, **Then** the result is identical to creating
   it empty and ticking the same ones afterwards.

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

### User Story 6 - Push an improvement back to the shelf (Priority: P6)

A Game Master keeps a collection of their own homebrew on their shelf. In one
world they improve a spell — better wording, a fixed number. They sync that
change back to the collection, and every world they start afterwards gets the
better version. The world stops holding it as a change, because it is not a
change any more.

They try the same thing with a monster they retuned out of the Monster Manual,
and there is no such option. The compendium underneath is a record of what the
book says, and it stays that however much they have changed it in their world.

**Why this priority**: It is what stops the delta model becoming a trap. Work
done in a world is otherwise stuck in that world forever, which quietly
punishes the Game Master for using the feature.

**Independent Test**: Change an entry in a world over an authored collection,
sync it back, confirm the shelf has it and a new world gets it; attempt the
same over an imported compendium and confirm there is no route.

**Acceptance Scenarios**:

1. **Given** a world's change over an authored collection, **When** the Game
   Master syncs it back, **Then** the collection holds it and a world started
   afterwards gets it.
2. **Given** a sync back, **When** it is requested, **Then** what will change
   on the shelf is shown and an explicit confirmation is required.
3. **Given** a completed sync back, **When** the world is read, **Then** it no
   longer holds that change as a delta.
4. **Given** other worlds with their own deltas over the same collection,
   **When** a sync back lands, **Then** their deltas behave exactly as they do
   after a re-import.
5. **Given** a world's change over an **imported** compendium, **When** the
   Game Master looks for a sync back, **Then** there is none, and the reason
   is stated where the absence would be noticed.
6. **Given** a completed sync back, **When** the Game Master regrets it,
   **Then** the collection's previous version is still recoverable.

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
- **A book switched off mid-session.** Because content is fetched rather than
  copied, switching a book off takes its content out from under a live table —
  a creature standing on a scene, an item in a player's hands. This is the
  price of not copying, and it MUST be paid visibly: what is in use is named
  before the switch takes effect, never discovered afterwards by a player
  whose sword vanished.
- **A player's client holding fetched content when it is switched off.** What
  a client was delivered is not a second copy that outlives the decision. The
  existing rule stands: content the world does not serve is not delivered, and
  an attempt to use what was withdrawn is refused by the server.
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

**Collections on the same shelf**

- **FR-007**: A library MUST hold **collections** — authored content — beside
  imported compendiums, and MUST present them as one shelf.
- **FR-008**: A collection MUST behave identically to a compendium everywhere
  origin does not decide the answer: on the shelf, in the book list, in a
  world, and under a delta.
- **FR-009**: A collection MUST carry a system, the same way a compendium
  does, and MUST be offered to worlds on that system only.
- **FR-009a**: A Game Master MUST be able to download a collection they own,
  **as JSON**. It is theirs and they made it.
- **FR-009b**: An imported compendium MUST NOT be downloadable, by that route
  or any other. This is spec 049 FR-052, not a new rule.
- **FR-009c**: A download MUST contain only authored content. Where a
  collection could contain anything derived from an import, that content MUST
  be excluded and the exclusion MUST be reported — a silently thinner file is
  worse than a refused one.

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
  name, unless FR-029's measurement says otherwise. A delta attaches to that
  identity.
- **FR-026**: Deltas MUST survive a re-import that replaces the base beneath
  them.
- **FR-027**: A delta that can no longer attach to anything MUST be reported
  by name and MUST NOT be silently discarded.
- **FR-028**: Removing a world MUST remove its deltas and MUST NOT affect the
  base or any other world.
- **FR-029**: The identity rule MUST be **settled by measurement before the
  delta model ships**, not by argument. The experiment: re-parse books from
  the existing corpus that exist in more than one file — a re-save, a later
  printing, or the same file under an improved parser — and report how many
  entries keep a stable kind-and-name identity, how many are renamed, and how
  many collide. FR-025 stands if the evidence supports it and is replaced by
  what the evidence supports if not.
- **FR-029a**: The measurement MUST be reported as generated output, not
  transcribed into prose, consistent with how engine and parser numbers are
  already produced.

**What a world can push back up**

- **FR-100**: A Game Master MUST be able to **sync a world's changes back** to
  the **collection** they came from, so an improvement made in one world
  reaches the shelf and the worlds that follow.
- **FR-101**: Syncing back MUST be available for **authored collections only**.
  An imported compendium MUST have no path by which a world's changes reach
  its base, at any volume of change.
- **FR-102**: The reason MUST be stated where the absence would be noticed: an
  imported base records what a document says, and it stays that.
- **FR-103**: Syncing back MUST show what will change on the shelf before it
  happens, and MUST require an explicit confirmation — it writes to something
  every other world is reading.
- **FR-104**: Syncing back MUST create a new version of the collection's base,
  leaving the previous one recoverable. Other worlds' deltas over it MUST
  behave exactly as they do after a re-import (FR-026, FR-027).
- **FR-105**: A world that has synced back MUST no longer hold the synced
  changes as a delta — they are in the base now, and holding both would make
  the world's copy silently diverge on the next change.

**The book list**

- **FR-030**: A world MUST show the compendiums switched on for it, and MUST
  offer the Game Master those on their shelf that match its system.
- **FR-031**: Switching a compendium on MUST make its content available in the
  world by **fetching it**, and MUST NOT copy it into the world. There is no
  seeding step and no import-into-world step; the list is the mechanism.
- **FR-032**: Switching one off MUST remove its content from the world and
  MUST leave no copy behind.
- **FR-033**: Ticking compendiums while creating a world and switching them on
  afterwards MUST produce the same state. There MUST be one mechanism, not a
  creation path and a separate later path.
- **FR-034**: A world created with none switched on MUST be allowed.
- **FR-035**: **Players MUST be able to see the list** — which books this table
  is running — and MUST NOT be able to change it.
- **FR-036**: The list MUST show a book's name and nothing that amounts to its
  content. Naming a book is not reproducing it.

**System compatibility**

- **FR-040**: A compendium MUST record the system it was read as.
- **FR-041**: A compendium MUST NOT be inheritable into a world on a different
  system.
- **FR-042**: A world that changes system MUST report which inherited
  compendiums no longer match, and MUST NOT continue serving them as though
  they did.

**What may not leave**

- **FR-050**: Spec 049's origin rule MUST hold at every point here. Everything
  in a library is **uploaded** content, and MUST NOT be shareable,
  publishable, exportable, or adoptable into a collection — from the library
  or from any world that switched it on.
- **FR-051**: Switching a compendium on in the owner's own world MUST NOT
  count as sharing. One person's copy serves all of that person's tables.
- **FR-052**: A world's **delta** over uploaded content MUST inherit its
  origin. A changed sword is a change to an uploaded sword; a mutation has no
  meaning apart from the thing it mutates, and MUST NOT become shareable by
  being edited.
- **FR-052a**: A world-only **addition** — content authored by hand in the
  world, sitting alongside what it inherited rather than modifying it — is
  authored content and MUST remain shareable. The delta's three forms do not
  all carry the same origin, and the difference MUST be tracked per entry.
- **FR-053**: A refusal MUST say why, naming the origin, as spec 049 FR-053
  requires.
- **FR-054**: The content agreement MUST be shown on every import and MUST
  state what is true under this architecture: that the content is for that
  person and their games, that it is never shared between users at any point,
  and that deleting it deletes it — with no shared copy retained behind the
  scenes, because there is none.

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
- **FR-085**: An end-to-end test MUST prove a player sees the book list and
  cannot change it.
- **FR-086**: An end-to-end test MUST prove switching a book off removes its
  content from the world with no copy left behind — the claim that nothing was
  copied is only worth as much as the evidence that nothing was.
- **FR-087**: An end-to-end test MUST prove the delta's origin split: a change
  to an uploaded entry cannot be shared, and a world-only addition beside it
  can.
- **FR-089**: An end-to-end test MUST prove a change synced back to a
  collection reaches a world started afterwards, and that the world that
  synced it no longer holds it as a delta.
- **FR-089a**: An end-to-end test MUST prove there is **no route** by which a
  world's change reaches an imported compendium's base — attempted through
  every surface that offers the sync for collections.
- **FR-089b**: An end-to-end test MUST prove a collection downloads as JSON
  and an imported compendium does not.
- **FR-089c**: The identity measurement (FR-029) MUST be run and reported
  before the delta model ships. It is a gate, not a report — if the evidence
  contradicts FR-025, the rule changes before the work continues.
- **FR-090**: Unit tests alone MUST NOT be accepted as proof for any of the
  above.

### Key Entities

- **Library**: an account's shelf. Holds compendiums and collections, belongs
  to exactly one account, dies with it.
- **Compendium**: everything one book produced (spec 049). Imported, so never
  shared, never downloaded, never written back to. Carries its book's name,
  hash, system, origin and counts.
- **Collection**: authored content on the same shelf (spec 026). Shareable,
  downloadable as JSON, and the only kind a world can sync changes back to.
- **Base**: the immutable content of a compendium at one version. Replaced
  wholesale by a re-import, never edited.
- **Book list**: which compendiums are switched on for a world. Visible to
  players, changeable only by the Game Master, and the single mechanism by
  which a world gets content from a library. Each entry on it names the base
  version in force.
- **Delta**: one world's changes over one compendium it has switched on —
  entries changed, hidden and added. Belongs to the world. Each entry carries
  its own origin: a change to an uploaded entry is uploaded, an addition
  beside it is authored.
- **Resolved entry**: what a world actually sees: the base entry with its
  delta applied, or a world-only addition.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A Game Master with eight worlds and one book stores one copy of
  that book's content, measured on a real instance.
- **SC-002**: Stored content for an account grows with distinct books read and
  does not grow when a compendium is inherited into an additional world.
- **SC-003**: Switching three compendiums on takes no longer than switching
  none on, because nothing is copied — creating a world with books ticked is
  not measurably slower than creating an empty one.
- **SC-003a**: A player can see which books their table is running, and cannot
  change them, in 100% of attempts.
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
  which book or collection it came from and whether this world has changed it.
- **SC-010**: A change made in one world and synced back reaches every world
  started afterwards, with no further action, in 100% of runs.
- **SC-011**: No world's change reaches an imported compendium's base, by any
  route exercised in testing, at any volume of change.
- **SC-012**: A Game Master can download a collection they authored and open
  it in something that is not ThunderForge.

## Assumptions

- **Compendiums do not cross accounts, at all.** Not by sharing, not by
  export, not by deduplication. This is the decision that makes the rest of
  the spec simple, and it is spec 049 decision 1.
- **Nothing is ever copied into a world.** A world references and fetches;
  the library holds. This is what makes the storage saving real rather than
  nominal, and it is why switching a book off has consequences a copy would
  not have had.
- **The shelf holds imported compendiums and authored collections, both.**
  Decision 4. What is *not* in scope is moving every world-authored actor,
  item and lore entry onto the profile — that is the wider 2026-09-10
  direction, and this spec should leave room for it rather than attempt it.
- **A world's changes flow back only to authored content.** Decision 5. An
  imported base is a record of what a document says and is never written to.
- **A world inherits from its owner's library only.** Co-Game Masters use, do
  not inherit.
- **Entry identity is kind plus name, provisionally.** The simplest rule that
  works across a re-parse, and its weakness is renames. It is not being
  settled by argument: FR-029's measurement against the real corpus decides
  it before the delta model ships. Decision 6.
- **Deltas are not versioned.** A Game Master can restore an entry to its base
  (FR-024), which covers the common mistake. A full history of world edits is
  a different feature.
- **Spec 049 ships first, or at least its reader does.** This spec has nothing
  to hold until a book can be read into a compendium.
- **The world's Compendium portal is extended, not replaced** (spec 011), the
  same assumption spec 049 makes.

## Decisions (owner, 2026-09-12)

1. **No seeding at world creation. A book list instead.** Content is not
   copied into a world at any point — the world shows which compendiums are
   switched on, fetches their content, and displays it. Creating a world with
   books ticked and ticking them afterwards are the same action through the
   same mechanism, which removes an entire class of "the two paths disagree"
   defect before it can exist.

   The owner's framing: *a mod list, but for the Game Master* — and players
   see it read-only, so the table knows what it is running.

2. **Changes are mutations over an immutable original.** A Game Master who
   retunes a sword's damage has that change saved in the world; the
   compendium underneath is untouched and stays shared with every other world
   that has the book switched on.

3. **Sharing is decided by origin, not licence** (spec 049 decision 4). Two
   kinds of content, mechanically distinguishable:

   - **Authored** — made through the authoring tools. Shareable.
   - **Uploaded** — read out of a document. Never shareable.

   One consequence needs stating because it is not obvious: the three forms a
   delta takes do **not** all carry the same origin. Changing or hiding an
   uploaded entry is uploaded content and cannot be shared (FR-052). Adding a
   world-only entry beside it is authored content and can be (FR-052a). Origin
   is tracked per entry, not per compendium.

4. **The shelf holds both kinds, and the authored kind is a collection**
   (answering Q3). A library is not only imported books. It holds:

   - **Compendiums** — imported from a document. Never shared, never
     exported, never downloaded.
   - **Collections** — authored through the authoring tools. Shared,
     adopted, downloadable.

   The owner's word for both is "compendium", and the reason this spec uses
   two is that the repository already reserves "collection" for user-authored
   shareable content (spec 026). Having one word for the shareable thing and
   another for the unshareable thing is worth more here than anywhere else in
   the product, because the difference between them **is** the legal boundary.

   Both behave identically where it does not matter — both sit on the shelf,
   both are switched on through the book list, both carry a system, both take
   deltas. They differ only in origin, and everything that follows from it.

5. **A world can push its changes back up — if the content is authored**
   (answering Q1). A Game Master who improves something in a world can **sync
   the change back** to the collection it came from, so the next world gets
   the improvement and the work has somewhere to live other than one world.

   For an **imported compendium there is no sync back, at any volume**. The
   owner: *"if uploaded pdf it doesn't matter how much delta they do."* The
   base is a record of what a book says, and writing a Game Master's changes
   into it would destroy the one property that makes it worth keeping — that
   it is still what the book says. They may delta as heavily as they like; it
   simply never flows upstream.

   This answers what happens when a book is switched off, and the answer is
   different for each of the delta's three forms, for reasons that now follow
   from the model rather than being chosen:

   - **Changed** and **hidden** entries over an imported base go with it,
     named first. They are meaningless without the entry they modified.
   - **Added** entries stay. They are authored content that was sitting
     beside the book rather than derived from it, and they never needed it.
   - Over an **authored collection**, the Game Master is offered the sync
     first, so nothing has to be lost at all.

6. **Entry identity across a re-import is decided by measurement, not now**
   (answering Q2). The owner: *"we will have to test this one out until we
   have an answer, i feel its too nuanced."* That is the right instinct and it
   is this project's established way of settling a question of this kind.

   Kind-plus-name stands as the default. Before the delta model ships, the
   experiment in FR-029 runs against the real corpus and either confirms it or
   replaces it with what the evidence supports. A rule invented at a desk for
   how to re-attach somebody's month of work to a re-parsed book is exactly
   the kind of guess this project has been burned by.

7. **An authored collection can be downloaded as JSON** (answering Q3's second
   half). It is theirs, they made it, and they can take it with them. Imported
   compendiums cannot be downloaded, which is the same rule as everything else
   in decision 3 rather than a new one. Richer formats are explicitly not the
   point yet.
