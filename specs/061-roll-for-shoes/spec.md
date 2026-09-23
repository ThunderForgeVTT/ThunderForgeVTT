# Feature Specification: Roll for Shoes

**Feature Branch**: `061-roll-for-shoes`

**Created**: 2026-09-22

**Status**: Draft

**Input**: Project owner, 2026-09-22: "Spider this site and create a module for
it — https://rollforshoes.com/ — Its cc0 so I think we can make a system for
it."

## The problem

### Every bundled ruleset asks for a character before it asks for a game

Eight system packs ship today: `basic-game-system` (the template, hidden from
the picker), `blades_in_the_dark`, `cypher_system`, `dnd5e`, `fate_core`,
`genie`, `pathfinder2e` and `year_zero_engine`. Every one of them expects a
built character before the first roll — an attribute array, action ratings, a
class and level, a set of trained skills. That is what those games are, and the
packs are right to reflect them.

The cost is that a group with an hour and no prepared characters has nowhere to
start. The shortest path to a first roll in this product is still measured in
sheet fields, and a Game Master who wants to teach somebody what a virtual
tabletop *is* has to teach them a ruleset first.

### Roll for Shoes is a complete game in six sentences

Roll for Shoes is a tabletop RPG micro-system created by Ben Wray and published
at <https://rollforshoes.com/>. The whole core game, as the homepage states it:

1. Say what you do, then roll a number of d6 equal to the level of the relevant
   skill you have.
2. If the sum of your roll is **higher than** an opposing roll, the thing you
   wanted to happen, happens.
3. At start, you have only one skill: **Do Anything 1**.
4. If you roll all 6s, you get a new skill specific to the action, one level
   higher than the one you used.
5. For every roll you fail, you get 1 XP.
6. XP can be used to change a die into a 6 **for advancement purposes only**.

There is no attribute array. No hit points, no damage, no death. No initiative,
no action economy, no inventory, no currency, no character classes. Character
creation is writing a name. The second rule is the entire resolution system and
the fourth is the entire progression system.

That makes it the one ruleset in this product a group can start playing in the
time it takes to make a world — and the one that demonstrates, in a single
sitting, that the pack contract can carry a game whose shape is nothing like
D&D's.

### The licence is unusually clean, and the attribution must not overreach

Every page of rollforshoes.com carries the same footer, site-wide, with no
per-page variation and no additional terms:

> "This work is marked with CC0 1.0"

…linking `http://creativecommons.org/publicdomain/zero/1.0?ref=chooser-v1`. A
CC0 1.0 public-domain dedication waives the author's rights worldwide, so the
rules text and the mechanics may be implemented here freely, and no attribution
is legally owed.

The site's homepage credits **Ben Wray** as the creator, hyperlinked to an
archived story-games.com thread. It names nobody else. It gives **no year**, no
forum handle, and no author for the website itself. Crediting Ben Wray and
rollforshoes.com is the courteous and expected practice; asserting anything
beyond that would be this product inventing provenance, which is exactly the
failure the legal metadata exists to prevent.

The pack contract makes attribution mandatory regardless of licence:
`legal.licenseName` and `legal.attributionText` must both be non-empty, and
`GET /api/systems/<id>/manifest.json` answers **422** rather than serving a
manifest that fails that check. A pack does not half-load; it does not load.

### The pack contract carries most of this, and stops exactly where the game starts

`packs/systems/README.md` is the author-facing contract, and it carries the
static half of Roll for Shoes without extension. A manifest declares its own
`data_types` slots, its own `sheet` entries, and — because every `combat` key is
optional — declares no hit points, no defence, no sizes and no legendary
actions, which is the honest description of this game. `turnStructure.rounds`
is already a per-ruleset answer and here it is `false`.

The dice are expressible too. Blades in the Dark already sizes a pool from a
bound value with `"(rating)d6kh1"`, so Roll for Shoes' roll is `(level)d6` and
needs no new dice syntax.

Three things the contract does not carry, and they are the game:

**A skill the player invented five minutes ago cannot be a declared check.**
The `checks[]` mechanism binds placeholders to values the manifest declares by
fixed id, and the contract is explicit that "a binding looks a number up; it
does not compute one" — the sheet sends only a check id. A Roll for Shoes pool
size comes from *Climbing Walls in the Rain 3*, a skill named at the table
during play. There is no id to declare. So the roll has to be issued by the
pack's own sheet, with the level it holds, rather than by a manifest `checks[]`
entry.

**There is no shared advancement model.** Genie's levelling is genie's own — a
field in its `trait_data` plus a ladder its own rules crate reads. Nothing
shared knows what XP is. Roll for Shoes' XP, and the all-6s rule that spends it,
live in the pack or they live nowhere.

**A sheet entry is a shape of a value, never a rule about one.** The contract
says so directly: "Every kind here is a shape of value, never a rule about one.
There is no conditional, no expression and no formula." Deciding that a roll
failed, awarding the XP for it, and noticing that every die came up 6 are rules.
They belong in the pack's own web and server code, which ADR-029 permits a
bundled pack to contribute.

### What there is not

There is no scene boundary concept in the core game, and nothing in this spec
needs one. There is no health track, and this spec adds none. There is no
opposed-roll arbitration surface today beyond `rollDice`, and this spec does not
build one — the Game Master rolls the opposition the same way the player rolls,
or names a number.

## What exists, and what this spec adds

| Capability | Today | After this spec |
|---|---|---|
| A ruleset playable within minutes of making a world | None | Roll for Shoes, character creation is a name |
| Bundled system packs | 8 (one a hidden template) | 9 |
| A pack that declares no combat block at all | None | Roll for Shoes declares none, deliberately |
| Skills created during play, named by the player | None | A skill lineage rooted at Do Anything 1 |
| XP earned by failing | None | 1 XP per failed roll, recorded on the actor |
| A roll that can *grant* something on a result | None | All 6s grants a new skill one level higher |
| A pack whose sheet issues its own roll | genie only | Roll for Shoes' sheet rolls `(level)d6` |
| CC0 provenance recorded in a manifest | None | `legal` block crediting Ben Wray, nothing more |

## User Scenarios & Testing *(mandatory)*

### User Story 1 - A game that starts with a name (Priority: P1)

A Game Master creates a world, picks Roll for Shoes from the system list, and
invites three players who have never used a virtual tabletop. Each player makes
a character by typing a name and a sentence of description. Every one of them
has exactly one skill, **Do Anything 1**, and zero XP. They begin playing.

**Why this priority**: It is the entire premise. If a character is not playable
the moment it is named, this pack has no reason to exist over the eight that
ship today.

**Independent test**: Create a world on Roll for Shoes, create an actor, open
its sheet. The sheet shows the name, the description, `XP 0`, and a single skill
row reading `Do Anything 1`. No other field is required, and no other field is
present.

**Acceptance Scenarios**:

1. **Given** a world whose system is Roll for Shoes, **When** a new actor is
   created, **Then** its sheet shows exactly one skill, named `Do Anything` at
   level 1, and an XP total of 0.
2. **Given** that actor, **When** the sheet is read for hit points, armour
   class, ability scores or initiative, **Then** none of them exist — the sheet
   offers no such field and the manifest declares no combat block.
3. **Given** the system picker, **When** a Game Master browses the available
   systems, **Then** Roll for Shoes appears with its title and description, and
   its CC0 licence and credit to Ben Wray are shown before the choice is made.

---

### User Story 2 - Roll the skill, beat the opposition (Priority: P1)

A player says their character kicks the door down. They roll their **Do Anything
1** — one die — and get a 4. The Game Master has set the opposition at 3. Four
beats three, so the door goes down.

Later the same player tries to pick a lock against an opposition of 5, rolls a
2, and fails.

**Why this priority**: Rule 2 is the resolution system. Nothing else in the game
can be proven until a roll can be made and judged.

**Independent test**: From an actor's sheet, roll a named skill against a target
number. The dice shown are the dice the server rolled, the pool size equals the
skill's level, and the verdict follows strictly-greater-than.

**Acceptance Scenarios**:

1. **Given** an actor with `Climbing 2`, **When** the player rolls that skill,
   **Then** exactly two d6 are rolled, and the roll is resolved and recorded on
   the server rather than decided in the browser.
2. **Given** a roll totalling 7 against an opposition of 6, **When** the result
   is shown, **Then** it reads as a success.
3. **Given** a roll totalling 6 against an opposition of 6, **When** the result
   is shown, **Then** it reads as a **failure** — rule 2 requires *higher than*.
4. **Given** a roll totalling 4 against an opposition of 9, **When** the result
   is shown, **Then** it reads as a failure and the opposition it was judged
   against is shown alongside it.

---

### User Story 3 - Failure is the only thing that pays (Priority: P1)

The player who failed the lock now has 1 XP. They fail twice more over the
evening and have 3.

**Why this priority**: Rule 5 is half of the progression system and the only
source of XP in the game. Without it rule 6 has nothing to spend.

**Independent test**: Fail a roll; the actor's XP increases by exactly 1 and the
new total persists across a reload.

**Acceptance Scenarios**:

1. **Given** an actor with 0 XP, **When** a roll of theirs fails, **Then** their
   XP becomes 1.
2. **Given** an actor with 0 XP, **When** a roll of theirs succeeds, **Then**
   their XP stays 0.
3. **Given** a roll that ties the opposition, **When** it is resolved, **Then**
   it counts as a failure and awards 1 XP.
4. **Given** an actor who has earned XP, **When** the page is reloaded or
   another player opens that actor, **Then** the same XP total is shown.

---

### User Story 4 - All sixes, and a skill that did not exist before (Priority: P1)

A player with **Do Anything 1** rolls a 6. The sheet tells them they have earned
a new skill, one level higher, specific to what they just did. They were
climbing a drainpipe, so they name it **Climbing 2**. It appears on their sheet
beside Do Anything 1, which they keep.

Three sessions later, rolling **Climbing 2** against a wet wall, they roll 6 and
6, and name **Climbing Walls in the Rain 3**.

**Why this priority**: Rule 4 is the other half of progression and the reason
the game is played. It is also the rule that cannot be expressed in the existing
declarative contract, so it is the one that justifies the pack owning code.

**Independent test**: Force an all-6s result; the sheet prompts for a name,
accepts it, and the new skill is stored at the rolled skill's level plus one
with the rolled skill as its parent.

**Acceptance Scenarios**:

1. **Given** an actor rolling a level-1 skill, **When** the single die shows 6,
   **Then** they are prompted to name a new skill, and it is created at level 2.
2. **Given** an actor rolling a level-3 skill, **When** all three dice show 6,
   **Then** the new skill is created at level 4.
3. **Given** an actor rolling a level-3 skill, **When** two dice show 6 and one
   shows 5, **Then** no skill is granted.
4. **Given** the prompt to name a new skill, **When** the player dismisses it
   without naming one, **Then** no skill is created, and the roll's success or
   failure and any XP it earned are unaffected.
5. **Given** a new skill has been named, **When** the sheet is read, **Then**
   the skill the player rolled is still present — advancement adds, it never
   replaces.
6. **Given** a roll that both fails and shows all 6s, **When** it resolves,
   **Then** the actor gains 1 XP *and* is offered the new skill.

---

### User Story 5 - Spending XP buys the skill, never the outcome (Priority: P2)

A player with **Climbing 2** and 2 XP rolls 6 and 3 against an opposition of 11.
They have failed — nothing will change that. But they spend 1 XP to read the 3
as a 6, which makes it all 6s, and they name **Climbing Fast 3**.

They still did not get up the wall.

**Why this priority**: Rule 6, and the rule most often misread. It is also the
one place where getting the behaviour wrong would silently turn Roll for Shoes
into a different, easier game.

**Independent test**: On a failed roll, spend XP to convert the non-6 dice; the
advancement is offered, the failure verdict is unchanged, and the XP is
deducted.

**Acceptance Scenarios**:

1. **Given** a failed roll of 6 and 3 with 2 XP available, **When** the player
   spends 1 XP on the 3, **Then** the advancement prompt appears and the roll is
   still shown as failed.
2. **Given** that spend, **When** the sheet is read afterwards, **Then** the XP
   total has decreased by exactly 1 per die converted.
3. **Given** a roll of 2, 2 and 2 with 2 XP available, **When** the player tries
   to convert all three dice, **Then** they are stopped at two conversions and
   told they are short — no advancement is granted from a partial conversion.
4. **Given** a roll that just failed and earned 1 XP, **When** the player spends
   that XP on the same roll, **Then** it is allowed — the XP from a failure is
   spendable immediately on the roll that produced it.
5. **Given** a roll that already succeeded, **When** the player spends XP to
   reach all 6s, **Then** the advancement is granted and the success is
   unchanged.

---

### User Story 6 - The table decides what "more specific" means (Priority: P2)

A player advancing from **Do Anything 1** proposes **Do Anything Better 2**. The
Game Master says no, and the player names **Kicking Doors 2** instead.

**Why this priority**: The source rule is "new skills should be more specific
than the skill rolled, and relevant to the action taken", and it gives no formal
test. A product that invented one would be arbitrating a judgement the game
hands to the table.

**Independent test**: The naming prompt shows the parent skill and the rule, and
accepts any non-empty name. Nothing rejects a name on grounds of specificity.

**Acceptance Scenarios**:

1. **Given** the naming prompt, **When** it is shown, **Then** it names the
   skill being advanced from and states that the new skill should be more
   specific than it and relevant to the action taken.
2. **Given** any non-empty name, **When** it is submitted, **Then** it is
   accepted — the system records the table's decision, it does not judge it.
3. **Given** an empty name, **When** it is submitted, **Then** it is refused and
   no skill is created.

---

### User Story 7 - A sheet that shows where a skill came from (Priority: P3)

A player four sessions in has nine skills. The sheet shows them as a lineage —
`Do Anything 1` at the root, `Climbing 2` and `Kicking Doors 2` beneath it,
`Climbing Walls in the Rain 3` beneath `Climbing 2` — so the character's history
is legible as a shape rather than a list.

**Why this priority**: It makes the game's one piece of long-term structure
visible. Valuable, and not required for a playable first session.

**Independent test**: An actor with a three-deep lineage renders each skill
under the skill it was advanced from.

**Acceptance Scenarios**:

1. **Given** an actor whose skills form a lineage, **When** the sheet is read,
   **Then** each skill appears beneath the skill it advanced from.
2. **Given** a skill at level N, **When** it is shown, **Then** its level is
   shown as the number of dice it rolls.

---

### Edge Cases

- **A level-0 or negative skill level.** Cannot arise from play — the starting
  skill is level 1 and advancement only adds one. Stored data claiming otherwise
  must be refused by the pack's validator rather than rolled as an empty pool.
- **A skill with no parent that is not Do Anything.** Possible only in stored
  data. It renders at the root of the lineage rather than disappearing.
- **Two skills with the same name.** The game forbids nothing here, and two
  different level-3 skills can plausibly be named the same thing by different
  players. Names are not unique and must not be treated as identifiers.
- **A player deletes Do Anything 1.** The game never contemplates losing it.
  Skill removal is out of scope for this spec; nothing in it removes a skill.
- **Negative XP.** Cannot arise — spending is refused when the balance is short.
  Stored data claiming a negative total must be refused by the validator.
- **Spending XP with no dice left to convert.** Every die already shows 6:
  there is nothing to buy, and the option is not offered.
- **A roll with no opposition set.** The core rules assume an opposing roll
  always exists. Where none is given, the roll is still made and recorded, and
  it is shown as unjudged rather than as a success — this product does not
  invent a verdict the table did not ask for. Awarding XP requires a verdict, so
  an unjudged roll awards none.
- **An enormous skill level.** No cap exists in the source and none is added.
  The pool is sized by the level; a level large enough to be absurd is a table
  problem, not a product one, but the roll must not be permitted to become a
  denial-of-service against the dice engine.
- **A world paused mid-roll.** Play/pause gating is the application's, and the
  pack's writes obey it like any other.

## Requirements *(mandatory)*

### Functional Requirements

#### The pack and its identity

- **FR-001**: A system pack MUST exist at `packs/systems/roll_for_shoes/` whose
  manifest `id` equals its directory name, and it MUST appear in the system
  picker for a new world.
- **FR-002**: The manifest MUST NOT set `template`, so the pack is offered
  rather than hidden.
- **FR-003**: No file under `src/server/src`, `src/app/src` or `apps/web/src`
  may be named for this system or contain its id, except the linkage and test
  modules the registry check already exempts.
- **FR-004**: The pack's contribution MUST reach the registry under its own id,
  proven by a test in the pack's own crate, and the pack's `SYSTEM_ID` constant
  MUST equal the `id` in its manifest, proven by a second test that reads the
  manifest.

#### Licence and attribution

- **FR-005**: `legal.licenseName` MUST name the CC0 1.0 Universal public-domain
  dedication, and `legal.attributionText` MUST credit Ben Wray as the creator of
  Roll for Shoes and name rollforshoes.com as the source.
- **FR-006**: The attribution MUST NOT assert a publication year, a forum
  handle, an author of the website, or any publisher or organisation — the
  source states none of these.
- **FR-007**: The attribution MUST record that the work is dedicated to the
  public domain under CC0 1.0 and that no attribution is legally required, so a
  reader can tell courtesy from obligation.
- **FR-008**: `legal.sourceUrl` MUST point at the CC0 1.0 deed, and the
  manifest's `url` MUST point at rollforshoes.com.
- **FR-009**: The system's licence and credit MUST be visible where a Game
  Master picks the system, through the existing legal-notice surface, with no
  new placement mechanism.
- **FR-010**: A provenance digest MUST be recorded under `research/` naming
  every page of rollforshoes.com the implementation was derived from, and the
  manifest `description` MUST reference it, as every other derived pack does.

#### The character

- **FR-011**: A character MUST have a name, a free-text description, an XP total
  and a list of skills, and MUST have nothing else — no ability scores, no hit
  points, no defence, no inventory, no currency, no class and no level.
- **FR-012**: A newly created character MUST begin with exactly one skill, named
  `Do Anything`, at level 1, and an XP total of 0.
- **FR-013**: The manifest MUST declare no `combat` block, so the application
  offers no hit points, defence, sizes or legendary actions for this system.
- **FR-014**: The manifest MUST declare `turnStructure.rounds` as false — the
  game counts no rounds.
- **FR-015**: A skill MUST carry a name, an integer level of at least 1, and a
  reference to the skill it was advanced from, which is absent only for the
  starting skill.
- **FR-016**: A skill's level MUST equal its parent's level plus one wherever a
  parent exists.
- **FR-017**: Skill names MUST NOT be treated as unique, and MUST NOT be used as
  identifiers.
- **FR-018**: The pack's validator MUST refuse stored character data with a
  skill level below 1 or an XP total below 0, rather than rendering or rolling
  it.

#### Rolling

- **FR-019**: Rolling a skill MUST roll a number of six-sided dice equal to that
  skill's level.
- **FR-020**: Every roll MUST be resolved and recorded on the server, and the
  dice shown to the player MUST be the dice the server rolled.
- **FR-021**: A roll MUST be judged against either a target number or another
  roll, supplied by the Game Master, and the value it was judged against MUST be
  shown with the result.
- **FR-022**: A roll succeeds if and only if the sum of its dice is **strictly
  greater** than the opposition. An equal sum is a failure.
- **FR-023**: Where no opposition has been given, the roll MUST be made, shown
  and recorded as unjudged, and MUST NOT be reported as a success or a failure.
- **FR-024**: Every skill a character holds MUST be rollable from its sheet.

#### XP

- **FR-025**: A judged roll that fails MUST increase the character's XP by
  exactly 1.
- **FR-026**: A judged roll that succeeds, and an unjudged roll, MUST NOT change
  the character's XP.
- **FR-027**: Spending 1 XP MUST convert exactly one of that roll's dice to a 6
  for the purposes of the advancement check.
- **FR-028**: Spending XP MUST NOT change whether the roll succeeded, what its
  dice totalled, or the XP it awarded.
- **FR-029**: XP awarded by a failure MUST be spendable on the roll that awarded
  it.
- **FR-030**: A spend MUST be refused when the character's XP balance is lower
  than the number of dice being converted, and the refusal MUST say so.
- **FR-031**: XP MUST NOT be spendable on anything other than converting dice to
  6s.
- **FR-032**: An XP total MUST persist on the character and be visible to anyone
  who can see the sheet.

#### Advancement

- **FR-033**: When every die of a roll shows 6 — whether rolled or bought with
  XP — the character MUST be offered a new skill at the rolled skill's level
  plus one.
- **FR-034**: The all-6s check MUST read the dice as rolled plus any bought
  conversions, and MUST NOT be affected by any modifier applied to a total.
- **FR-035**: The player MUST name the new skill, and the prompt MUST state that
  it should be more specific than the skill rolled and relevant to the action
  taken.
- **FR-036**: Any non-empty name MUST be accepted. The system MUST NOT judge
  specificity or relevance.
- **FR-037**: An empty name MUST be refused, and no skill created.
- **FR-038**: Declining the prompt MUST create no skill and MUST leave the
  roll's verdict, its dice and any XP it awarded unchanged.
- **FR-039**: A granted skill MUST record the skill it advanced from as its
  parent, and MUST NOT remove or replace that skill.
- **FR-040**: Skill levels MUST have no upper bound.
- **FR-041**: A roll that both fails and shows all 6s MUST award the XP and
  offer the advancement.

#### The sheet

- **FR-042**: The character sheet MUST show the name, the description, the XP
  total and every skill with its level.
- **FR-043**: The sheet MUST present skills as a lineage, each beneath the skill
  it advanced from.
- **FR-044**: The sheet MUST be contributed by the pack and discovered by the
  application, with no shared code naming this system.
- **FR-045**: The sheet MUST use the host's presentational primitives so it
  looks like the rest of the product.

#### Proof

- **FR-046**: The pack's directory MUST be added to the `game-systems` slice's
  paths in `scripts/e2e/slices.json`, and `pnpm e2e:game-systems` MUST be the
  named proof of this feature.
- **FR-047**: At least one end-to-end specification MUST play the loop the game
  is made of: create a character, roll its starting skill, fail and gain XP, and
  reach an advancement and name the skill it grants.

### Key Entities

- **Roll for Shoes character**: a name, a description, an XP total (integer, at
  least 0, starting at 0) and a list of skills. Nothing else. Stored as system
  data on an existing actor; this spec introduces no new actor concept.
- **Skill**: a name (free text, not unique, not an identifier), a level
  (integer, at least 1, no maximum, equal to the d6 pool it rolls) and a parent
  skill. The starting skill is `Do Anything` at level 1 with no parent. Every
  other skill has a parent and a level one higher than it.
- **Roll**: a skill, the dice the server rolled, the opposition it was judged
  against (a number, another roll, or nothing), the resulting verdict (success,
  failure, or unjudged), the XP it awarded and the dice bought with XP.
- **Advancement**: the grant produced by an all-6s roll — a new skill, its name
  supplied by the player, its level one above the skill rolled, its parent that
  skill.
- **Provenance digest**: the record under `research/` of which pages of
  rollforshoes.com the pack was derived from, and what the site's licence
  footer said when it was read.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A Game Master who has never used this system can go from creating
  a world to a player's first resolved roll in under five minutes, without
  filling in any character field except a name.
- **SC-002**: A new character has exactly one skill and zero XP, in 100% of
  creations, with no setup step in between.
- **SC-003**: Across the full range of pool sizes, a roll's verdict matches
  "sum strictly greater than the opposition" in 100% of cases, including the
  tie, which is always a failure.
- **SC-004**: Every failed judged roll awards exactly 1 XP, and no other event
  in the system awards any.
- **SC-005**: Every all-6s result offers an advancement, and no result that is
  not all 6s offers one.
- **SC-006**: Spending XP changes the advancement offer and never changes the
  verdict — verified by a case that buys an advancement off a roll that failed
  and stays failed.
- **SC-007**: The system's licence and credit to Ben Wray are visible before a
  Game Master commits to the system, and the attribution asserts no fact the
  source does not state.
- **SC-008**: The pack can be removed by deleting its directory and its build
  linkage, with no edit to any shared file that names it — the registry check
  passes with no new exemption.
- **SC-009**: `pnpm e2e:game-systems` is green, and the slice stays within its
  measured budget.
- **SC-010**: A character sheet renders from the pack alone, with no shared web
  code naming this system.

## Assumptions

- **CC0 covers what we are using.** The footer marking appears on every page of
  the site, with no additional terms anywhere, and applies to the rules text we
  read. The mechanics themselves are not copyrightable in any case; the CC0
  marking removes the question for the expression as well.
- **A tie is a failure.** Core rule 2 says "higher than", and rule 5 gives XP for
  every failed roll, so a tie awards 1 XP. The site's optional Extras page adds a
  tie rule that contradicts this; it is out of scope here, and when it arrives it
  is an opt-in world setting that suppresses the XP.
- **Advancement reads the raw dice.** The source says so where statuses exist;
  since this spec adds no modifiers at all, the rule is trivially satisfied, and
  stating it now keeps it true when statuses arrive.
- **XP may be spent after seeing the dice.** The source has a player spend XP
  earned from the failure they just suffered, which is only possible after the
  dice are read.
- **XP may be spent on a successful roll.** Rule 6 restricts what XP buys, not
  which rolls it may be bought on, and nothing in the source forbids it.
- **There is no level cap.** None is stated, and the XP cost of forcing an
  advancement from level N is N minus the natural 6s, so growth self-limits
  without a rule.
- **A character's data lives on an existing actor.** This is a ruleset, not a
  new kind of entity; it uses the actor system data every other pack uses.
- **The Game Master supplies the opposition.** The core rules assume an opposing
  roll exists and say nothing about where it comes from. The GM rolls it or names
  a number; this spec builds no difficulty table, because that is an Extras rule.
- **Skill removal is not part of the game.** Nothing in the six rules removes a
  skill, so nothing here does.

## Out of Scope

Each of the following is deliberately excluded, and each is named so it can be
picked up later rather than rediscovered.

**The site's optional Extras rules**, all of which the source presents as
supplementary to the six:

- The difficulty table (Easy / Moderate / Hard / Very Hard as 1–4 GM dice, or
  static targets 3 / 6 / 9 / 12).
- The tie rule — a tie as partial success and partial failure awarding no XP.
  It contradicts the core reading this spec adopts, so it must arrive as a
  toggle, not a change.
- Statuses: signed flat modifiers written rating-first (`-4 Raining`), applied
  to a roll's total after summing, never to the dice the advancement check
  reads. This is also the game's only damage rule, and the source's own worked
  example is ambiguous about how harm is rated — that ambiguity must be settled
  and documented as *our* decision when it is implemented, not silently.
- Skill Slots: capping skills per level at four, three and two for levels 2, 3
  and 4, with replacement when a level is full.
- Buying skill slots with XP at twice the level.
- Customised starting skills, of which the site's *Minimon* scenario is the only
  concrete example.

**The "New Shoes!" variant.** It is a separate nine-rule replacement ruleset,
not an add-on: no XP at all, marks instead; three single-word starting skills;
advancement capped once per character per scene; failed "dangerous" actions
disable a skill until the scene ends. It requires a first-class *scene* boundary
that the core game has no use for, and switching between the two mid-campaign
has no defined conversion. It is its own feature.

**The site's published content.** Six scenarios (The Heist, Kitchen Khaos,
Minimon, Outbreak Hotel, Redshirt Recon, The Stranger), each with introduction
prompts, three gameplay segments and random tables; and the two D66 NPC
generator tables and body-parts list under the site's Tools. Content, not
ruleset — and the natural home for it is a collection, not this pack.

**A dice-roller or opposed-roll surface of its own.** This pack uses the roll
path the product already has.

**Any health, damage or death model.** The game has none. Shipping one would be
inventing a game the source did not write.

## Dependencies

- The pack contract at `packs/systems/README.md`, and the manifest schema it
  documents, including the legal metadata enforced fail-closed at
  `/api/systems/<id>/manifest.json`.
- The host surface a bundled pack may import (ADR-029), which is what lets the
  pack ship its own data-connected sheet.
- The existing server-resolved roll path, which returns each die's value and so
  makes the all-6s check readable from real dice rather than from a client
  guess.
- The `game-systems` e2e slice (spec 060, constitution Principle VI), which is
  this feature's proof.
- The registry, ability-vocabulary and file-length checks in `pnpm verify`,
  which constrain what the pack may name and how large its files may be.

## Decisions (2026-09-22)

The source leaves these open; this spec closes them, and records that they are
ours rather than the game's.

1. **A tie is a failure and earns 1 XP.** Core rule 2 is "higher than". The
   Extras tie rule is a different reading and becomes an opt-in setting later.
2. **No health, damage or death.** The core game has none; the manifest declares
   no combat block at all.
3. **No level cap.** None is stated and the economics self-limit.
4. **Specificity is the table's call.** Any non-empty name is accepted; the
   prompt states the rule and records the answer.
5. **XP may be spent after the dice are seen, on the roll that earned it, and on
   a successful roll.** All three follow the source where it speaks and nothing
   forbids the third.
6. **A roll with no opposition is unjudged, not successful**, and awards no XP.
7. **Skill names are not identifiers.** Duplicates are allowed because the game
   allows them.
8. **Attribution credits Ben Wray and rollforshoes.com, and stops.** No year, no
   forum handle, no website author — the site states none of them.
