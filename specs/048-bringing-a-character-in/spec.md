# Feature Specification: Bringing a Character In

**Feature Branch**: `048-bringing-a-character-in`

**Created**: 2026-09-11

**Status**: Draft. Three owner questions are open (see
[Questions for the owner](#questions-for-the-owner)).

**Input**: Project owner: "i'd really like to build a 5e specific character
sheet importer that works via the actor screen for a player, a subsystem
that's a shared subsystem but has mappings for other systems — eventually we
will support others but 5e is the most popular"; "if they import something and
the feat or skill or etc isn't in the world it imports it and the DM maybe has
a flag if it was character added and can choose to adopt it for everyone to see
with a simple accept button"; "we want to build the PDF parser in rust"; "can
our pdf parser be a crate and our 5e parser be based on that because I don't
want to just parse character sheets — I eventually want to be able to parse
source books for easy import into worlds".

## Context

### A player arrives with a character, and the door is shut

There is no import of any kind. A character is built by hand in ThunderForge or
it does not exist here. A player who has played the same hero for two years,
whose sheet lives in a tool they already use, starts again from an empty form —
and the first session is spent typing.

The owner's own seven sheets are the case in point: a Cleric 1; an Artificer 4
/ Barbarian 1; a Sorcerer 7 / Warlock 7 with spells at four levels. They are
D&D Beyond exports, five pages each, and between them they carry ability
scores and saving throws, skills with proficiency, passives and senses, armour
class, initiative, hit points and hit dice, death saves, defences (resistances,
immunities), proficiency bonus, speed, armour, weapon, tool and language
proficiencies, an attacks table with to-hit and damage and properties, features
and traits with their sources and their recharges, species traits, feats,
spells by level with slots and preparation, coins, equipment with weights, and
attunement.

### What the product can hold today

The 5e pack declares five data types — ability scores, a hit-points resource,
proficiencies, traits and spells — and the app's sheet renders them read-only.
Most of what is on a real sheet has nowhere to live yet: armour class, speed,
senses, attacks, features, equipment. Spec 046 gives some of them a reason to
exist (a defence to beat, an attack with reach). The rest is Q1.

### The shape this should take, and the precedent for it

Map import is the pattern: a file is uploaded, the **server** parses it in
Rust (`src/server/src/map_import/` — `parse.rs`, `types.rs`, `geometry.rs`,
`warnings.rs`), and what it understood becomes scene content, with warnings for
what it did not. Character import wants the same shape and the same honesty,
which is also the owner's instruction: the parser is Rust, server-side.

### Two layers, because a sheet is not the last document

A character sheet is one kind of PDF, and the owner wants source books next.
Those are the same problem underneath — getting words out of a PDF with enough
of their position and styling to tell a heading from a table cell — and
entirely different problems on top, where one reader looks for an ability
score and another for a monster's statblock.

So: **a crate that reads PDFs**, knowing nothing about games, and **a 5e sheet
reader built on it**, knowing nothing about PDF internals. The crate is where
the awkward parts live — text runs, positions, fonts, columns, reading order —
and it can be tested against any document. Everything above it becomes a
matter of asking what sits at a given anchor.

Source-book import is **#todo**: named so the seam is built for it, and
deliberately not specified here, because its questions — what may be taken
from a book somebody bought, under whose licence, and what a world may do with
it afterwards — are not this feature's to answer.

### What a flattened PDF will and will not give

These exports carry real text — not scans — so the words are all there. But
they are **flattened**: `Form: none`, no fields to read. Values sit beside
their labels by position, and the proficiency marks come through inconsistently
as bullets or letters. A parser therefore reads by anchors and geometry, and it
will sometimes be unsure.

That is a design constraint, not a defect to hide: an import that guesses
silently is worse than one that says it could not tell. Every scenario below
turns on the player seeing what was read before anything is written.

### Content a world has never heard of

A sheet names things: a feat, a spell, a background, a species trait. The
world may have none of them. The owner's instruction settles what happens:
the import brings them in **as the character's own**, marked as
character-added, and the Game Master may adopt any of them for the whole table
with one action.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - A player brings their own character (Priority: P1)

A player opens their character in ThunderForge, hands over the sheet they
already have, sees what was understood, and accepts it.

**Why this priority**: It is the feature. Everything else supports it.

**Independent Test**: A player with an actor of their own uploads a five-page
D&D Beyond export from the actor screen. Within a minute they are looking at
what was read, and one action later their character has its ability scores,
hit points, proficiencies, spells and the rest of what the world's system can
hold.

**Acceptance Scenarios**:

1. **Given** a player viewing an actor they hold, **Then** the actor screen
   offers to bring a character in.
2. **Given** a sheet they upload, **When** it is read, **Then** they are shown
   what was understood, field by field, before anything is saved.
3. **Given** that review, **When** they accept, **Then** the actor carries
   those values and the change is theirs, recorded as an import.
4. **Given** that review, **When** they decline, **Then** nothing is written.
5. **Given** a Game Master, **Then** they may do the same for any actor in
   their world.
6. **Given** a person with no claim on the actor, **Then** they may not import
   onto it.

---

### User Story 2 - Nothing is guessed in silence (Priority: P1)

The import says what it read, what it could not read, and what it was unsure
of — and the unsure parts are the player's to correct before anything is
saved.

**Why this priority**: A flattened sheet is read by position and anchors. An
importer that quietly invents a value is worse than one that admits a blank,
because the mistake reaches the table as the character's own.

**Independent Test**: A sheet whose armour class sits where the parser cannot
read it imports everything else, lists armour class as unread, and the player
fills it in during review.

**Acceptance Scenarios**:

1. **Given** a value the parser read confidently, **Then** it is shown as read,
   with what it came from.
2. **Given** a value it could not find, **Then** it is listed as unread, and
   the actor keeps whatever it had.
3. **Given** a value it is unsure of, **Then** it is shown as uncertain and the
   player may correct it in the review.
4. **Given** a sheet it cannot make sense of at all, **Then** it says so
   plainly, and writes nothing.
5. **Given** any import, **Then** what was written is recorded, so a wrong
   import can be understood afterwards.

---

### User Story 3 - The world learns from a character (Priority: P1)

A character arrives with a feat the world has never heard of. It comes in as
the character's own, marked as such, and the Game Master can adopt it for
everyone with one action.

**Why this priority**: It is the owner's own design, and it is what makes
import a gift to the table rather than a private pile of text.

**Independent Test**: A player imports a character with a feat, a species trait
and two spells the world does not have. All four arrive attached to that
character and marked character-added. The Game Master adopts the feat; it
becomes the world's, available to everyone, and stops being marked.

**Acceptance Scenarios**:

1. **Given** an import naming content the world does not have, **Then** that
   content is created for the character and marked as character-added, with the
   character named as its source.
2. **Given** such content, **Then** the Game Master sees it gathered somewhere
   they can act on, not scattered through the world.
3. **Given** the Game Master, **When** they accept a piece of it, **Then** it
   becomes the world's own, visible to everyone the world's rules allow, and no
   longer marked.
4. **Given** the Game Master, **When** they decline a piece, **Then** it stays
   the character's, or is removed — see Q3.
5. **Given** content the world already has, **Then** the import uses the
   world's own rather than making a second copy.
6. **Given** two characters importing the same unknown feat, **Then** the Game
   Master is asked once, not twice.

---

### User Story 4 - One importer, many systems (Priority: P2)

D&D 5e first, because it is what most tables play. The next system is a
mapping, not a second importer.

**Why this priority**: The owner asked for a shared subsystem. Building 5e
alone in a way that cannot be extended would be the expensive mistake.

**Independent Test**: A second game system declares a mapping, and a sheet of
that system imports through the same pipeline with no change to the parser or
the review screen.

**Acceptance Scenarios**:

1. **Given** the importer, **Then** what it produces is a system-neutral
   character, and what a system stores is decided by that system's own mapping.
2. **Given** a system with no mapping, **Then** an import for it is refused
   with a reason, not half-applied.
3. **Given** a mapping, **Then** it is declared by the system pack, the way its
   data types and checks already are.

---

### User Story 5 - A character comes back changed (Priority: P3)

The character levels up elsewhere and is imported again; play is not thrown
away.

**Why this priority**: Sheets change every session. An importer that can only
be used once is used once.

**Independent Test**: A character imported at level 4 is imported again at
level 5. The review shows what differs; accepting it raises what should rise
and leaves current hit points and anything the table has changed in play alone.

**Acceptance Scenarios**:

1. **Given** a re-import, **Then** the review shows the differences, not the
   whole sheet as new.
2. **Given** values the table has changed in play (current hit points, marked
   conditions), **Then** they are not silently overwritten.
3. **Given** content adopted by the world since the first import, **Then** the
   re-import uses the world's own.

---

### Edge Cases

- **Multiclass.** "Sorcerer 7 / Warlock 7" is one character with two classes
  and one proficiency bonus; the import must not read it as level 14.
- **A sheet with no spells.** A barbarian imports with an empty spell section
  and no warnings about it.
- **A species with its own traits.** Warforged resistances and immunities are
  the character's, and arrive with it.
- **Two characters, one world, same spell.** The second import finds the first
  one's spell if it has been adopted, and its own if not.
- **A sheet from another game entirely.** Refused with a reason.
- **A password-protected or corrupt PDF.** Refused with a reason; nothing
  written.
- **A very large file, or a hundred-page document.** Bounded and refused above
  the bound.
- **A sheet whose owner is not the player.** The import is onto an actor they
  hold; who the sheet says they are does not grant them anything.
- **An import interrupted halfway.** Either it all lands or none of it does.

## Requirements *(mandatory)*

### Functional Requirements

**Reading a sheet**

- **FR-001**: The parser MUST run on the server, written in Rust, alongside the
  existing map import rather than as a second kind of thing.
- **FR-001a**: Reading a PDF MUST be a crate of its own, knowing nothing about
  any game: it takes a document and yields text with the position and styling
  needed to tell a heading from a value from a table cell. It MUST be testable
  against documents that are not character sheets.
- **FR-001b**: The 5e sheet reader MUST be built on that crate and MUST NOT
  know how a PDF is structured. A second system's reader, and one day a source
  book's, MUST be able to sit beside it on the same crate.
- **FR-002**: It MUST accept a D&D Beyond PDF export, flattened, with no form
  fields.
- **FR-003**: It MUST report, per field, whether a value was read, unread, or
  uncertain — and MUST NOT invent a value it did not find.
- **FR-004**: It MUST refuse a file it cannot parse, a file that is not a
  character sheet, and a file beyond its size and page bounds, each with a
  reason.
- **FR-005**: It MUST produce a system-neutral character: identity, level and
  classes, abilities, proficiencies, defences, senses, attacks, features,
  spells, equipment and money, as far as the sheet carries them.

**Mapping into a world**

- **FR-010**: What a system stores MUST be decided by that system's own
  declared mapping, not by the importer.
- **FR-011**: D&D 5e MUST be mapped in this spec; other systems MUST be
  possible without changing the parser or the review.
- **FR-012**: An import into a system with no mapping MUST be refused, whole.
- **FR-013**: Values a system cannot hold MUST be reported as unmapped, and
  MUST NOT be silently dropped — Q1 decides where they go.

**Review before anything is written**

- **FR-020**: Nothing MUST be written until the person importing accepts what
  they were shown.
- **FR-021**: The review MUST show what will change, including what will be
  overwritten.
- **FR-022**: The person MUST be able to correct an uncertain value in the
  review.
- **FR-023**: An accepted import MUST apply completely or not at all.
- **FR-024**: Every applied import MUST be recorded — who, when, from what, and
  what it wrote.

**Content the world does not have**

- **FR-030**: Content named by a sheet that the world does not have MUST be
  created attached to that character and marked as character-added, naming the
  character it came from.
- **FR-031**: Content the world already has MUST be used as it is, and MUST NOT
  be duplicated.
- **FR-032**: A Game Master MUST be able to see all character-added content of
  their world in one place.
- **FR-033**: A Game Master MUST be able to adopt a piece of it in one action,
  after which it is the world's own and no longer marked.
- **FR-034**: Adopting MUST NOT duplicate: the character keeps using the same
  thing, now the world's.
- **FR-035**: The same unknown thing arriving from two characters MUST be one
  decision for the Game Master.
- **FR-036**: A piece of character-added content that the world has **not**
  adopted MUST NOT be usable in play (decision 3). It is visible to its own
  character, marked as awaiting the Game Master, and every play-field action
  that would use it — rolling it, attacking with it, spending it — MUST be
  refused.
- **FR-036a**: Refusing MUST say why: that this came in with the character and
  the Game Master has not adopted it. A player who cannot use their own sword
  and is not told why will conclude the product is broken.
- **FR-037**: Unadopted content MUST NOT be **delivered** to the play field at
  all. Not greyed out, not present and refused on use — absent. The play field
  is the server's to compose, and a client cannot swing a sword it was never
  sent. This is the same rule spec 045 FR-033 already applies to a hidden
  token's name: hidden means the server never sends it.
- **FR-037a**: An attempt to use content the viewer was not sent MUST be
  refused by the server, not only by the client. A modified client is the case
  this exists for; a client-side check would be the only check.
- **FR-038**: Such an attempt MUST be **reported to the Game Master**: what was
  attempted, by which character, and when. A player who is editing requests by
  hand or by script is something the table should know about, and the server is
  the only party that can see it.
- **FR-038a**: The report MUST state what happened and MUST NOT state why.
  "Aria's client tried to use a sword this world has not adopted" is a fact;
  "Aria is cheating" is a verdict, and the product is not in a position to
  reach it.
- **FR-038b**: An honest client that is merely **stale** MUST NOT be reported.
  A player whose Game Master declined an item mid-session will have a client
  that has not caught up, and accusing them is far worse than missing a
  genuine attempt. The report MUST therefore be suppressed when the viewer's
  last delivery predates the decision that withdrew the content.
- **FR-036b**: A Game Master MUST be able to revisit an undecided or declined
  piece later and adopt it. Declining MUST NOT be final, and MUST NOT remove
  anything from the character.
- **FR-036c**: Content MUST therefore carry three states, not two: **adopted**
  (the world's, playable), **pending** (the character's, visible, refused in
  play), and **declined** (the character's, visible, refused in play, and
  marked as already considered so it does not return to the queue).

**Who may import**

- **FR-040**: A player MUST be able to import onto an actor they hold.
- **FR-041**: A Game Master MUST be able to import onto any actor in their
  world.
- **FR-042**: Nobody else MUST be able to import onto an actor.
- **FR-043**: The uploaded file MUST be kept with the import record until the
  character is deleted (decision 2), and MUST be deleted with it.
- **FR-043a**: Every import onto an actor MUST be **a version**, not a
  replacement: the record MUST show what was imported and when, so a character
  imported in March and again in June has two entries a person can read and
  compare.
- **FR-043b**: A kept file is somebody's personal document at rest. It MUST be
  reachable only by that character's owner and the world's Game Master, and
  MUST be covered by the account-deletion and data-export paths that already
  exist.
- **FR-044**: A Game Master MUST be able to **roll an actor back** to any
  earlier import, or to how it stood before the first one. A version is
  therefore something that can be restored, not only read.
- **FR-044a**: A rollback MUST restore the sheet — scores, traits, resources —
  and MUST leave alone what the table has changed in play since, in the same
  way FR-051 governs a re-import.
- **FR-044b**: A rollback MUST be the Game Master's alone. A player rolling
  their own character back would defeat the point of it.
- **FR-044c**: A rollback MUST itself be recorded, so the history reads as
  what happened rather than as a version that quietly vanished.

**Coming back**

- **FR-050**: A second import onto the same actor MUST show differences rather
  than treat everything as new.
- **FR-051**: It MUST NOT overwrite what the table has changed in play without
  showing it in the review.

**Proof**

- **FR-060**: The parser MUST be tested against a corpus of real sheets
  covering, at least: a single-class character, a multiclass character, a
  spellcaster with spells at several levels, and a character with none.
- **FR-061**: A playtest MUST bring a character in from the actor screen and
  then use it: the imported hit points on the board, the imported attacks in a
  fight (spec 046).
- **FR-062**: The corpus MUST NOT contain anybody's personal sheets. The
  owner's own exports are the working reference, kept outside the repository; a
  small redacted fixture is what is committed.

### Key Entities

- **Imported character**: what the parser produced — system-neutral, with a
  confidence per field.
- **Mapping**: a system's declaration of how an imported character becomes its
  own data.
- **Character-added content**: a feat, spell, trait or item that arrived with a
  character and that the world has not adopted, carrying the character it came
  from.
- **Import record**: who imported what, when, from what file, and what it
  wrote.

## Success Criteria *(mandatory)*

- **SC-001**: A player brings a character in from their own sheet in under two
  minutes, without typing a value by hand.
- **SC-002**: Across the reference corpus, the importer reads correctly every
  ability score, the class or classes and their levels, proficiency bonus,
  maximum hit points, armour class, speed and the spell list where there is
  one — and lists every field it did not read.
- **SC-003**: No import writes a value the person importing was not shown.
- **SC-004**: A multiclass character imports as its classes and their levels,
  never as their sum.
- **SC-005**: A Game Master adopts an imported feat for the world in one
  action, and no duplicate of it exists afterwards.
- **SC-006**: A second system's sheet imports through the same pipeline with no
  change outside that system's mapping.

## Assumptions

- **The sheet is evidence, not authority.** What a sheet says a character can
  do becomes the character's data; it does not change the world's rules.
- **The system pack declares the mapping**, as it already declares data types,
  checks and legal notices.
- **One character per import.** A file with several sheets is out of scope.
- **Imported text is untrusted** — names, descriptions and notes are escaped
  and bounded like any other user content.
- **An ADR records where parsing happens** and what it may reach: a PDF parser
  is an attack surface, and it runs server-side on uploaded files.

## Out of Scope

- **Source books — #todo.** Importing a rulebook or adventure to stock a world
  is the reason the PDF reading is a crate of its own (FR-001a), and it is not
  specified here. It needs its own spec, and its own answers about what may be
  extracted from a book somebody bought, under whose licence, and what a world
  is allowed to do with it afterwards.
- Exporting a character out of ThunderForge.
- Importing anything but a character: monsters, items, adventures, maps.
- Reading a character straight from another service's API or account.
- Rules automation for what is imported — spec 046 owns what an attack does.
- Keeping a character in step with another tool continuously; import is an
  act, not a subscription.

## Dependencies

- **Spec 032**: the pack architecture, which is where a system's mapping is
  declared.
- **Spec 046**: a fight that resolves — the reason an imported armour class,
  attack or reach is worth holding.
- **Spec 025 / 033**: the abilities compendium and its vocabulary, which is
  what an imported feat or spell becomes.
- **Spec 026**: collections, should adopted content travel between worlds.
- **Map import** (`src/server/src/map_import/`): the precedent for a
  server-side Rust parser with warnings.

## Decisions (owner, 2026-09-12)

1. **The whole sheet** (Q1: C). The 5e pack learns to hold all of it —
   features, equipment and money included — rather than only what something
   already reads. The alternative was to grow the pack to exactly what spec
   046 needs, which would mean revisiting the same sheets a third time; a
   character brought in whole is a character, and what nothing reads yet is
   simply waiting for the thing that will.

2. **The file is kept, imports are versioned, and a Game Master can roll one
   back** (Q2: B). Three reasons, and the third is the strongest.

   The history is legible: imported on one date, imported again on another.
   The case that raised it is a player working off another service and
   re-exporting — the two versions are the thing worth comparing, and a record
   that overwrote itself could not show them.

   But a version that can only be read is half the value. A Game Master must
   be able to **restore** one, because the failure that matters is not a
   confusing import, it is a dishonest one: a player who re-imports with every
   ability score at 20 and a sword that does 99,999,999 damage. FR-044.

   **This is the half that decision 3 does not cover, and the two are needed
   together.** Adoption gates *new content* — a sword, a feat, a spell the
   world has never seen — and refuses it in play until the Game Master says
   otherwise. It does nothing about a player inflating the scores their
   character already has, because those are not new content; they are the
   sheet. Rollback is what answers that. One decision guards what arrives, the
   other guards what changes.

3. **Unadopted content is visible and unplayable** (Q3: C, and further). The
   option offered was that content stays and a Game Master may hide it from
   the table. The owner's answer goes past hiding to **refusing**: until the
   Game Master adopts it, the player cannot use it either.

   The reasoning is the sword that does 99,999,999 damage. Hiding it from the
   table does not stop the player swinging it; only refusing it does. So an
   unadopted piece is flagged, stays with its character, and every play-field
   action that would use it is refused — and the refusal says why, because a
   player who cannot use their own sword and is not told will conclude the
   product is broken.

   Declining is not final. A Game Master may come back to it. FR-036 through
   FR-036c.

   **And it is withheld rather than refused.** The play field is the server's
   to compose, so unadopted content is simply not delivered — not greyed out,
   not present-and-blocked, absent. A client cannot swing a sword it was never
   sent, which is the same rule spec 045 already applies to a hidden token's
   name. A player may still craft the request by hand or by script; the server
   refuses it, and tells the Game Master that somebody tried. FR-037, FR-038.

   The report says what happened and not why. "Aria's client tried to use a
   sword this world has not adopted" is a fact the table can act on; "Aria is
   cheating" is a verdict this product is not in a position to reach — and a
   stale client belonging to an honest player looks identical from the server.
   Accusing somebody who did nothing is a worse failure than missing somebody
   who did, so the report is suppressed when the viewer's last delivery
   predates the decision that withdrew the thing. FR-038a, FR-038b.

## Parsing in the browser — built

Reading happens on the machine that holds the file. `crates/thunderforge-pdf`
compiles to `wasm32-unknown-unknown`, ships as `@thunderforge/pdf`, and reads
the Monster Manual in a browser: 354 pages counted in 63 ms, 588 KB of wasm.

### The trust question, and why it is smaller than it looks

The first draft of this section worried that "a parse on the player's machine
produces values the player's machine chose". The owner's correction is that
the two importers have different owners, and the worry only ever applied to
one of them:

- **A source book is read into a world by its Game Master, from their own
  world panel.** A Game Master can already put anything they like into their
  own world through the authoring tools. Parsing on their machine hands them
  nothing they did not already have, so there is no new trust to extend.
- **A player importing their own character sheet** is the case the guards are
  for, and those guards never trusted the parse. Unadopted content is withheld
  from the play field (FR-037), a Game Master can roll an import back (FR-044),
  and an attempt to use what was never delivered is refused server-side and
  reported (FR-037a, FR-038). Moving the parse does not weaken any of them,
  because none of them ever depended on where it happened.

### What remains true

- **The file is still uploaded.** Decision 2 keeps it for versioning and
  rollback, so parsing locally saves the server the *parse*, not the transfer.
- **One parser, not two.** The browser façade is a thin wrapper over the same
  code the server runs.

## Questions for the owner

1. **Q1 — How much of a sheet does the 5e pack learn to hold?** *(answered: C)*

   | Option | Answer | Implications |
   |--------|--------|--------------|
   | A | What it holds today; the rest is kept as notes on the actor | Smallest step. Armour class and attacks arrive as text nobody can roll. |
   | B | Grow the pack for what spec 046 needs: armour class, speed, senses, attacks | The import feeds the fight. Pack work, and 046 must land alongside. |
   | C | The whole sheet, including features, equipment and money | A complete character. The largest, and much of it nothing yet reads. |

2. **Q2 — What happens to the uploaded file?** *(answered: B, versioned)*

   | Option | Answer | Implications |
   |--------|--------|--------------|
   | A | Discarded once the import is applied | Least to hold of somebody else's data. A wrong import cannot be re-read. |
   | B | Kept with the import record until the character is deleted | Re-importable and auditable; it is a personal document at rest. |
   | C | The player chooses when they import | Honest, and one more decision at the door. |

3. **Q3 — What does declining character-added content mean?** *(answered:
   C, and stronger — see decision 3)*

   | Option | Answer | Implications |
   |--------|--------|--------------|
   | A | It stays the character's, unadopted, for good | Nothing is lost; the world quietly accumulates private content. |
   | B | It is removed from the character too | The Game Master's word is final, and a player can lose what they imported. |
   | C | It stays, and the Game Master can hide it from the table | A middle road, and a third state to explain. |
