# Feature Specification: The Bestiary

**Feature Branch**: `047-the-bestiary`

**Created**: 2026-09-11

**Status**: Draft. Three owner questions are open (see
[Questions for the owner](#questions-for-the-owner)).

**Input**: Project owner: "can we beef up our heroes package to support
generating some mob tokens and stuff like goblins and orcs and trolls and
dragon the works in the same art style".

## Context

### The factory draws people, and only people

`packages/heroes` is a factory with no dependencies and no framework: a hero
spec in, two self-contained SVGs out — a 256×256 portrait card and a round
token. Every choice is data (`HERO_PARTS`, `HERO_COLORS`, `HERO_FLAGS`,
`SKIN_TONES`), every spec goes through `validateHero`, and the twelve presets
include an orc (green skin, tusks, horned helm) and a tiefling (horns as
headgear). Species today is skin colour, ear shape, tusks and headgear.

What holds the style together is a **fixed skeleton**, stated in the parts
file: the head is a circle centred at (128, 112) with radius 58, the neck and
shoulders fill the bottom from y=186, the eyes sit at x=106 and x=150, and
props occupy the right-hand strip. Every filled shape carries the same
outline — `#2b2233` at 3px with round joins — and colour is flat, with one
helper (`shade`) for interior shadow. Parts are drawn back to front so those
outlines overlap correctly.

That skeleton is why a goblin and an orc are easy and a dragon is not. A
goblin is a small green person; a troll is a hunched one; **a dragon is not a
person at all**, and a circle head with two eyes at fixed coordinates cannot
draw it.

### There are no monsters anywhere

The repo contains no monster art, no monster data and no bestiary. The seeded
NPCs are the preset heroes; Genie's demo NPCs exist to show size categories.
A Game Master who wants a goblin today draws one themselves or does without.

### The path from a drawing to the map already works

`createHero(spec).token()` produces an SVG; `uploadActorImage(actorId, role,
file)` stores it; the server rasterises SVG at 1024px on its longest edge and
keeps WebP renditions; a token with no art of its own resolves to its actor's
`token` image, which the engine loads as a sprite sized to its grid footprint.
`scripts/seed-demo-art.mjs` already drives that path for the demo world, and
an e2e proves the bytes arrive as WebP.

### Size reaches the drawing, not the board

A monster's size matters twice: it should look big, and it should *be* big. The
first works — `tokens.scale = 2` draws a sprite twice as large. The second does
not: the engine's `Footprint`, which drives snapping, hit-testing, movement and
the placement of bars and nameplates, is never persisted and is set by nothing
in the product; its only caller in the repo is the engine sandbox, whose
buttons are labelled Tiny through Gargantuan. Genie's manifest declares
`sizeCategories` with a scale each, resolved from an NPC's own data into a
default token scale — the pattern exists, in one pack, and reaches only the
drawing.

Spec 046 requires a creature's size to be what the grid uses. This spec
declares a monster's size; 046 makes the board obey it.

### What may be drawn and named

The hero art is ours: every part is drawn in `parts.ts`, nothing is traced
from a publisher. A generated goblin is therefore ours to ship and share.

Names and statistics are a different matter, and the project already has an
answer: each system pack carries a `legal` block — for D&D 5e, CC-BY-4.0 with
the SRD 5.2.1 attribution, a non-affiliation disclaimer, and trademark
restrictions that explicitly forbid iconic creatures the SRD excludes. Spec 016
requires that block and where it must be shown. A bestiary lives inside those
rules: SRD creatures only, attributed as the pack declares.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - A Game Master fills a dungeon in a minute (Priority: P1)

A Game Master needs goblins. They pick one from the bestiary, and it arrives as
an NPC with a face, a token and a size, ready to place.

**Why this priority**: It is the whole point — the difference between running
a dungeon tonight and drawing one first.

**Independent Test**: From an empty world, a Game Master adds a goblin from the
bestiary and places it on a scene. It is drawn with its own art, at its own
size, within a minute of starting.

**Acceptance Scenarios**:

1. **Given** a Game Master in a world, **When** they choose a creature from the
   bestiary, **Then** an NPC is created with that creature's art in both roles
   and its size recorded.
2. **Given** that NPC, **When** it is placed on a scene, **Then** its token is
   its own art, at its own size, on every client.
3. **Given** several of the same creature, **Then** each is its own NPC and
   they need not look identical (see Story 3).

---

### User Story 2 - Monsters in the same hand as the heroes (Priority: P1)

A goblin, an orc and a troll look like they belong beside Grom and Nettle —
same outlines, same flat colour, same chibi proportions.

**Why this priority**: A table that mixes generated heroes with bought
monsters looks like two games. The style is the product.

**Independent Test**: A goblin, an orc, a troll and a hero are drawn side by
side. Every one has the same 3px ink outline, the same flat fills, and reads as
one set.

**Acceptance Scenarios**:

1. **Given** any creature the bestiary can draw, **Then** it uses the same
   outline weight, colour discipline and layering as a hero.
2. **Given** a creature's token, **Then** it is a 256×256 self-contained SVG
   with a unique id prefix, like a hero's.
3. **Given** a creature whose shape a hero's skeleton cannot hold — see Q1 —
   **Then** it is drawn by the answer to that question, not by stretching a
   person.

---

### User Story 3 - No two goblins are the same goblin (Priority: P2)

A pack of six goblins is six goblins, not one goblin six times.

**Why this priority**: It is what makes a generated bestiary better than a
stock image, and the factory is already built for it.

**Independent Test**: Six goblins generated from one seed differ visibly from
each other, and the same seed gives the same six again.

**Acceptance Scenarios**:

1. **Given** a creature and a seed, **When** several are generated, **Then**
   they vary within what that creature allows, and the same seed reproduces
   them exactly.
2. **Given** variation, **Then** every result still reads as that creature.
3. **Given** a Game Master who wants one particular goblin, **Then** they can
   fix its choices and keep it.

---

### User Story 4 - A bestiary travels (Priority: P3)

A Game Master shares their monsters with another table, and they arrive whole.

**Why this priority**: Collections already carry actors and their art, so this
is mostly assembly — but it is what turns a bestiary into something people
pass around.

**Independent Test**: A collection of six monsters is shared by link and copied
into another world. All six arrive with their art and their sizes.

**Acceptance Scenarios**:

1. **Given** monsters in a world, **Then** they may be gathered into a
   collection and shared like any other content.
2. **Given** a copied collection, **Then** each monster arrives with both image
   roles and its size.
3. **Given** the recipient, **Then** the copies are theirs to edit and
   re-share.

---

### Edge Cases

- **A creature with no humanoid parts.** See Q1; until it is answered, the
  bestiary draws what the skeleton can hold.
- **Huge and Gargantuan on a small map.** A 20-foot creature on a scene whose
  grid is finer than five feet still fills its own footage, not four squares by
  habit.
- **A creature on a gridless scene.** Size still scales the drawing; footprint
  behaviour follows the scene's grid, or its absence.
- **A hex grid.** Multi-cell footprints on hexes are unmodelled in the canvas
  core; a large monster on hexes stays centre-snapped until that is solved.
- **A monster with art of its own.** A Game Master who uploads their own image
  keeps it; the bestiary never overwrites art it did not put there.
- **An SRD creature the pack's trademark restrictions exclude.** It is not in
  the bestiary, and the refusal says why.
- **A world whose system declares no sizes.** The monster still arrives; its
  size is Medium until a system says otherwise.

## Requirements *(mandatory)*

### Functional Requirements

**Drawing monsters**

- **FR-001**: `packages/heroes` MUST be able to draw creatures that are not
  heroes, from a spec, with no new runtime dependency and no framework.
- **FR-002**: Every creature MUST draw with the same outline, colour
  discipline, canvas and layering rules as a hero.
- **FR-003**: A creature's choices MUST be data, exported from the package the
  way `HERO_PARTS` is, so a builder can render controls from them rather than
  keeping its own list.
- **FR-004**: Every spec MUST pass validation that rejects unknown fields and
  unknown choices, as `validateHero` does.
- **FR-005**: Drawing MUST be deterministic: the same spec gives the same SVG,
  byte for byte.
- **FR-006**: Generating a creature with variation MUST take a seed, and the
  same seed MUST give the same creature.
- **FR-007**: No choice may draw the same as another, as the heroes package
  already asserts for every part.

**Which creatures**

- **FR-010**: The first bestiary MUST cover the creatures a low-level dungeon
  needs; which ones is settled by Q2.
- **FR-011**: Every creature MUST declare its size.
- **FR-012**: A creature MUST NOT be included when the world's system pack's
  trademark restrictions exclude it.

**Size on the board**

- **FR-020**: A creature's size MUST reach the token: a Large monster is drawn
  large *and* fills its squares, per spec 046 FR-030/031.
- **FR-021**: A system pack MUST be able to declare its own size categories, as
  Genie does; a creature's size MUST be expressed in those terms.
- **FR-022**: Size MUST travel with a monster when it is copied or shared.

**Into a world**

- **FR-030**: Creating a monster MUST produce an NPC with art in both roles,
  through the upload path any token art uses.
- **FR-031**: Art MUST be uploaded as SVG and rasterised by the server, never
  rasterised in the browser.
- **FR-032**: A Game Master MUST be able to make several of one creature at
  once, each its own NPC.
- **FR-033**: The bestiary MUST NOT replace art a Game Master supplied.

**Sharing**

- **FR-040**: Monsters MUST be gatherable into a collection and shareable by
  link, like any other content.
- **FR-041**: A copied monster MUST arrive with both image roles and its size.

**Licence**

- **FR-050**: Generated art MUST be the project's own, drawn by the package,
  never traced from a publisher's artwork.
- **FR-051**: Where a creature's name or statistics come from a system's
  source, the pack's `legal` block MUST govern it, and the attribution it
  declares MUST be shown where spec 016 requires.

**Proof**

- **FR-060**: The heroes package's own tests MUST cover every creature the
  same way they cover heroes: every choice draws differently, every preset
  validates and draws, determinism holds.
- **FR-061**: A playtest MUST place a bestiary monster of each size on a scene
  and show it at its size on every client.

### Key Entities

- **Creature spec**: what a monster is made of — its kind, its choices, its
  colours. The monster equivalent of a hero spec.
- **Creature kind**: goblin, orc, troll, dragon… what the thing is, which
  decides which parts it is drawn from.
- **Size**: Tiny to Gargantuan, in the world's system's terms.
- **Bestiary entry**: a creature kind plus the defaults that make it that
  creature, the way a preset hero is a named set of choices.

## Success Criteria *(mandatory)*

- **SC-001**: A Game Master goes from an empty world to a placed goblin, with
  its own art and size, in under a minute.
- **SC-002**: Six goblins from one seed are six visibly different goblins, and
  the same seed reproduces them exactly.
- **SC-003**: A hero and every bestiary creature, drawn side by side, share one
  outline weight and one colour discipline.
- **SC-004**: A Large monster fills two squares by two on every client, and the
  grid agrees.
- **SC-005**: A collection of monsters copies into another world with every
  monster's art and size intact.

## Assumptions

- **The art stays ours.** Every part is drawn in the package; nothing is
  traced, and no image is generated by a model (ADR-051).
- **A monster is an NPC.** It is a `world_actors` row with images, not a new
  kind of thing, so everything that already carries actors carries monsters.
- **Statistics are not art.** What a goblin *does* in a fight belongs to spec
  046 and the system pack; this spec makes it exist and look right. Q3 settles
  whether a bestiary entry may also carry them.
- **The builder of spec 044 is where a person edits one.** This spec adds
  creatures to the factory; the builder renders whatever the factory exports.

## Out of Scope

- Statblocks, abilities and any rules behaviour — spec 046 and the packs.
- AI-generated art, and user-supplied parts (both out of scope in spec 044
  too).
- Animated or layered tokens.
- Maps, props and scenery.
- A marketplace or registry of bestiaries; sharing is by collection and link.

## Dependencies

- **Spec 044**: the hero builder — this spec must not contradict its
  requirements, and its phase (d) `hero_spec` column is what would let a
  monster's recipe travel rather than only its pixels.
- **Spec 046**: a fight that resolves, which makes size mean something on the
  grid.
- **Spec 026**: content collections, the way a bestiary is shared.
- **Spec 032 and spec 016**: the pack architecture and its legal block.
- **ADR-051**: no generated art.

## Questions for the owner

1. **Q1 — How far from a person may a creature be drawn?**

   | Option | Answer | Implications |
   |--------|--------|--------------|
   | A | Surface features only: skin, ears, tusks, horns, eyes, teeth on the hero skeleton | Cheapest, and goblins, orcs, ogres and trolls all work. A dragon or a wolf does not. |
   | B | Per-creature head and body parts on the same canvas and outline rules | Draws a dragon's snout and a troll's hunch. Every new creature is drawing work, and the style must be held by hand. |
   | C | A second skeleton for beasts, beside the humanoid one | The widest bestiary. Two skeletons to keep consistent, and the largest first step. |

2. **Q2 — What is in the first bestiary?**

   | Option | Answer | Implications |
   |--------|--------|--------------|
   | A | The humanoid dungeon: goblin, orc, hobgoblin, bugbear, ogre, troll | Fits option A of Q1 exactly. No beasts. |
   | B | That, plus the undead: skeleton, zombie, ghoul | Still humanoid-shaped; palettes and a few parts do most of the work. |
   | C | A full low-level menagerie including beasts and a dragon | Needs Q1 = B or C, and is a much longer first cut. |

3. **Q3 — Does a bestiary entry carry statistics?**

   | Option | Answer | Implications |
   |--------|--------|--------------|
   | A | Art and size only | Cleanest split: this spec draws, spec 046 and the packs rule. A Game Master fills in the rest. |
   | B | Art, size and the system's own statblock where its licence allows | A goblin arrives ready to fight. Ties the bestiary to each system pack and to its `legal` block. |
   | C | Art and size now, statistics when spec 046 lands | Sequenced; two passes over the same entries. |
