# Feature Specification: Roll for Shoes Extras

**Feature Branch**: `062-roll-for-shoes-extras`

**Created**: 2026-09-22

**Status**: Draft

**Input**: Spec 061 shipped the six core rules of Roll for Shoes and named the
site's optional "Extras" as later work. This is that work: five independent,
per-world, opt-in settings — difficulty, the tie rule, statuses, skill slots and
customised starting skills.

## The problem

### The pack plays the whole core game, and only the core game

`packs/systems/roll_for_shoes/` exists and works. A character starts with one
skill, rolls a pool sized by its level, fails, earns XP, spends XP to force an
advancement, and names what they just learnt. That is the six-rule game, proven
end to end in `apps/web/e2e/system-roll-for-shoes.spec.ts`.

The source publishes more than six rules. Alongside the core it offers a set of
optional additions, each presented as something a table may take or leave. Spec
061 excluded every one of them on purpose and listed them in its Out of Scope
section so they could be picked up rather than rediscovered. Nothing about that
exclusion was a judgement that they are unwanted; it was a judgement that the
core had to be provably right first.

### Optional rules are a settings problem, not a rules problem

The trap here is treating these as five more mechanics to implement. They are
not. Each one changes an answer the core already gives:

- Difficulty changes where the opposition number comes from.
- The tie rule changes whether a tie is a failure — and therefore whether it
  pays XP, which is the whole of progression.
- Statuses change a roll's total after the dice are summed.
- Skill slots change whether an earned advancement can be taken.
- Customised starting skills change what a new character is handed.

A table that wants none of them must get exactly the game spec 061 built, with
no new field to ignore and no new question to answer. A table that wants one
must not be given the other four. That makes this a feature about isolation:
five switches that do not touch each other, all off until somebody turns one
on.

### The tie rule contradicts the core, on purpose

Spec 061 settled that a tie is a failure, because the core resolution rule is
strictly "higher than", and recorded that the Extras tie rule is a different
reading that must arrive as a toggle rather than a change. It also settled, in
advance, that when the tie succeeds it awards no XP — a tie cannot be both a
success and a failure that pays.

That decision is honoured here unchanged. This spec adds the switch; it does not
reopen the question.

### Statuses are the game's only damage rule, and the source is ambiguous

The source presents statuses as named conditions carrying a signed number, and
uses them for harm as well as for weather and circumstance. Spec 061 flagged
that the source's own worked example leaves the rating of harm ambiguous, and
required that the ambiguity be settled and documented as *our* decision rather
than silently absorbed.

This spec settles it the narrow way: a status is a label and a number that the
table writes. The system ships no list of statuses, no rating scale for harm, no
duration, and no rule that any status is ever applied automatically. What a
broken arm is worth is the table's call, exactly as what counts as a
sufficiently specific skill is the table's call.

## What exists, and what this spec adds

| Surface | Today | After this spec |
| --- | --- | --- |
| Opposition | A free number typed by the Game Master | Unchanged by default; optionally a rolled GM pool or a static target chosen from four named bands |
| Tie | Always a failure, always 1 XP | Unchanged by default; optionally a success that pays no XP |
| Roll total | The sum of the dice | Unchanged by default; optionally the sum plus the character's statuses |
| Advancement | Always offered on all sixes | Unchanged by default; optionally blocked when the destination level is full, and said so out loud |
| XP | Buys a die into a six, nothing else | Unchanged by default; optionally also buys a skill slot at twice the level |
| New character | Exactly one skill, `Do Anything 1` | Unchanged by default; optionally the world's own starting skills |

Every row's default column is the existing behaviour, unchanged and unmoved.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - A world that wants none of this never sees it (Priority: P1)

A Game Master creates a world with the Roll for Shoes system and plays. They
never open a settings screen. The game they get is exactly the one spec 061
built: they type an opposition number, a tie fails and pays, no character has a
status, every advancement is available, and every character starts with a single
`Do Anything 1`.

**Why this priority**: This is the only story whose failure breaks something
that already works. If the Extras leak into a world that did not ask for them,
this feature is a regression however good the other five stories are.

**Independent Test**: Run the existing `system-roll-for-shoes` end-to-end
specification unmodified against the new build. It must pass with no edit.

**Acceptance Scenarios**:

1. **Given** a newly created Roll for Shoes world, **When** the Game Master
   inspects the system settings, **Then** every Extra is off.
2. **Given** a world with every Extra off, **When** a character rolls a pool
   whose sum exactly equals the opposition, **Then** the roll fails and the
   character gains 1 XP.
3. **Given** a world with every Extra off, **When** a character is created,
   **Then** it holds exactly one skill named `Do Anything` at level 1.
4. **Given** a world with every Extra off, **When** a character's sheet is
   opened, **Then** it shows no statuses area, no slot counts and no difficulty
   band picker.

---

### User Story 2 - The Game Master picks a difficulty instead of a number (Priority: P1)

A Game Master turns on difficulty bands. When a player rolls, the Game Master
names the difficulty as Easy, Moderate, Hard or Very Hard rather than inventing
a number. Depending on how the world is configured, that band either rolls a
pool of Game Master dice (one to four d6, summed) or stands for a fixed target
(3, 6, 9 or 12).

**Why this priority**: It is the Extra that most changes what play feels like,
and it is the one the source presents first. It also touches the roll path,
which every other story shares.

**Independent Test**: In a world with rolled difficulty, roll against Very Hard
and confirm four Game Master dice are shown, their sum is the opposition, and
the verdict is "strictly greater than" that sum. Repeat in a static-target
world and confirm the opposition reads 12 with no Game Master dice shown.

**Acceptance Scenarios**:

1. **Given** a world set to rolled difficulty, **When** a roll is made against
   Moderate, **Then** two Game Master dice are rolled, each between 1 and 6, and
   the opposition is their sum.
2. **Given** a world set to static targets, **When** a roll is made against
   Hard, **Then** the opposition is 9 and no Game Master dice are rolled.
3. **Given** a world with difficulty on in either mode, **When** the Game Master
   prefers to name a number, **Then** they still can, and it is judged as it is
   today.
4. **Given** a rolled difficulty whose sum equals the character's total,
   **When** the result is judged, **Then** the tie is resolved by whatever the
   world's tie setting says — the two Extras compose and neither overrides the
   other.
5. **Given** a world with difficulty off, **When** a roll is made, **Then** no
   band can be chosen and the opposition is a free number.

---

### User Story 3 - A tie stops being a loss (Priority: P1)

A Game Master turns on the tie rule because their table dislikes a tie costing
a player the outcome. From then on, a total equal to the opposition counts as a
success — and, because it succeeded, awards no XP.

**Why this priority**: It is a single switch over a decision spec 061 already
made and documented, so it is cheap; and it is the one Extra that changes
progression, so getting it wrong is expensive.

**Independent Test**: In a world with the tie rule on, force a tie and confirm
the verdict reads success and the XP total is unchanged. Turn it off in a second
world and confirm the same tie fails and pays 1.

**Acceptance Scenarios**:

1. **Given** a world with the tie rule on, **When** a roll's total equals the
   opposition, **Then** the verdict is a success.
2. **Given** that same tie, **When** the result is recorded, **Then** the
   character's XP is unchanged.
3. **Given** a world with the tie rule on, **When** a roll's total is below the
   opposition, **Then** it still fails and still pays 1 XP.
4. **Given** a world with the tie rule on, **When** a tied roll shows every die
   as a six, **Then** the advancement is still offered — the advancement check
   reads the dice, not the verdict.

---

### User Story 4 - Circumstance written on the character (Priority: P2)

A player is fighting in the rain with a hurt arm. The Game Master, or the player
with the Game Master's say-so, writes two statuses on the character — a name and
a signed number each. Every roll that character makes adds those numbers to its
total until somebody removes them.

**Why this priority**: It is the Extra that carries the most of the game's
texture, and it is the source's only damage rule. But nothing else depends on
it, so it follows the two that touch resolution directly.

**Independent Test**: Add a status worth −4 to a character, roll, and confirm
the shown total is the dice sum minus 4 while the dice themselves are unchanged.
Add a second worth +1 and confirm the total moves by 1.

**Acceptance Scenarios**:

1. **Given** a world with statuses on, **When** a status named `Raining` worth
   −4 is added to a character, **Then** every subsequent roll by that character
   shows a total 4 below the sum of its dice.
2. **Given** a character holding several statuses, **When** it rolls, **Then**
   the modifiers sum and the roll shows both the raw dice sum and the modified
   total, with the statuses that changed it named.
3. **Given** a character whose statuses reduce a roll below zero, **When** the
   roll is judged, **Then** the total is used as it stands and the roll fails
   like any other failure, paying its XP.
4. **Given** a roll showing every die as a six on a character carrying a
   negative status, **When** the advancement is checked, **Then** it is offered
   — a status can never create or destroy an advancement.
5. **Given** a status is no longer true, **When** it is removed, **Then** later
   rolls are unaffected by it and earlier recorded rolls are unchanged.

---

### User Story 5 - A level fills up, and the character is told (Priority: P2)

A world runs with skill slots on. A character has accumulated four level-2
skills. They roll all sixes on a level-1 skill, which would grant a fifth. The
game tells them level 2 is full, and offers to buy another slot for 4 XP if they
have it.

**Why this priority**: It is the most intricate of the five — it is the only one
that spends XP on something other than dice, so it puts the XP ledger at risk —
and it is worthless without a character who has played long enough to fill a
level.

**Independent Test**: Give a character four level-2 skills, roll an advancement
into level 2, and confirm the grant is refused with a message naming the full
level rather than silently dropped. Then buy a slot for 4 XP and confirm the
grant proceeds and the XP is deducted once.

**Acceptance Scenarios**:

1. **Given** a world with slots on, **When** a character holds fewer skills at
   a level than that level's cap, **Then** an advancement into it proceeds as it
   does today.
2. **Given** a character at the cap for level 2, **When** an advancement into
   level 2 is earned, **Then** the character is told the level is full, no skill
   is created, and the advancement offer remains available.
3. **Given** that same character with at least 4 XP, **When** they buy a
   level-2 slot, **Then** 4 XP is deducted, the cap for that character at that
   level rises by one, and the pending advancement can be taken.
4. **Given** a character with fewer than twice the level in XP, **When** they
   try to buy a slot, **Then** it is refused and no XP is deducted.
5. **Given** a world with slots on, **When** an advancement into level 1 or into
   level 5 or above is earned, **Then** it is never blocked — those levels are
   uncapped.
6. **Given** a character who spends XP both to force sixes and to buy a slot,
   **When** their XP total is read afterwards, **Then** it equals what they
   earned minus everything they spent, counted once each.

---

### User Story 6 - A world where everybody starts as something (Priority: P3)

A Game Master is running a setting where every character is a small monster with
a nature, not an ordinary person who can try anything. They define the world's
starting skills once. Every character created in that world afterwards begins
with those skills instead of `Do Anything 1`.

**Why this priority**: It changes only character creation, affects no roll, and
the core default is a perfectly good game without it. It is also the Extra the
source illustrates with a single scenario rather than a rule.

**Independent Test**: Define two starting skills in a world, create a character,
and confirm it holds exactly those two at the levels defined and no
`Do Anything`. Confirm a character created before the change still holds what it
was created with.

**Acceptance Scenarios**:

1. **Given** a world defining starting skills, **When** a character is created,
   **Then** it holds exactly those skills at exactly those levels.
2. **Given** a world whose starting skills are later changed, **When** an
   existing character is opened, **Then** its skills are untouched.
3. **Given** a world with the setting on but no starting skills defined,
   **When** a character is created, **Then** it falls back to `Do Anything 1`
   rather than being created with no skill at all.
4. **Given** a world-defined starting skill, **When** it is rolled and shows all
   sixes, **Then** the advancement it grants sits one level above it, exactly as
   for `Do Anything`.

---

### Edge Cases

- **Two Extras that touch the same roll.** Difficulty sets the opposition,
  statuses change the total, the tie rule judges the comparison. All three can be
  on at once; the order is fixed — dice are rolled, statuses adjust the total,
  the opposition is established, then the comparison is made under the world's
  tie rule.
- **A status turns a win into a tie.** It is then judged as a tie, by whatever
  the world's tie setting says. Nothing special happens because a status caused
  it.
- **A setting is turned off mid-campaign.** Characters keep whatever they were
  given. Statuses already written stay written but stop being applied; bought
  slots stay bought; skills already granted are never removed.
- **Slots are turned on in a world whose characters already exceed the caps.**
  Existing skills are never taken away. The character is simply over the cap and
  cannot gain more at that level until they buy room.
- **Rolled difficulty produces an opposition of 4 from a single die.** Nothing
  in the game requires the Game Master's pool to be beatable. The roll fails if
  it fails.
- **A status with a modifier of zero.** Accepted and shown; a table may want a
  label with no mechanical weight.
- **A starting skill defined at a level above 1.** Accepted — the setting names
  a level, and the game has no level cap.
- **XP spent to buy a slot during a pending advancement, by two people at once.**
  The ledger must not permit the same XP to be spent twice.
- **The player has no edit rights on the character.** Statuses, slot purchases
  and advancements are all writes; a player who has claimed a character but not
  been granted Editor cannot make them. This is a host limitation recorded
  separately, not something this feature works around.

## Requirements *(mandatory)*

### Functional Requirements

#### Scope and isolation

- **FR-001**: Each of the five Extras MUST be an independent per-world setting.
  Enabling one MUST NOT change the behaviour governed by any other.
- **FR-002**: Every Extra MUST default to off for every world, including worlds
  created before this feature exists.
- **FR-003**: With every Extra off, the system's behaviour MUST be identical to
  the behaviour spec 061 defined, with no additional character field, sheet
  region or roll prompt shown.
- **FR-004**: The existing `system-roll-for-shoes` end-to-end specification MUST
  pass unmodified.
- **FR-005**: The settings MUST be visible and changeable by whoever runs the
  world, and MUST NOT be changeable by a player who merely holds a character.
- **FR-006**: No file under `src/server/src`, `src/app/src` or `apps/web/src`
  may name this system or any of its settings; the registry check MUST pass with
  no new exemption.
- **FR-007**: Turning a setting off MUST NOT delete data the setting created.
  Statuses, bought slots and granted skills persist; they simply stop being
  applied.

#### Difficulty

- **FR-008**: A world MUST be able to select one of three opposition modes: free
  number only (the default), named bands rolled as Game Master dice, or named
  bands as static targets.
- **FR-009**: The four bands MUST be Easy, Moderate, Hard and Very Hard,
  corresponding to 1, 2, 3 and 4 Game Master dice in rolled mode and to targets
  of 3, 6, 9 and 12 in static mode.
- **FR-010**: The band chosen for a given roll MUST be a per-roll choice, never
  a stored world setting.
- **FR-011**: In rolled mode, the Game Master's dice MUST be six-sided, rolled
  and recorded on the server like a character's dice, and their individual faces
  MUST be shown alongside their sum.
- **FR-012**: An opposition established by either mode MUST be compared using
  the same rule the core uses — the character's total must exceed it, subject to
  the world's tie rule.
- **FR-013**: The free-number opposition MUST remain available in every mode.
- **FR-014**: Game Master dice MUST NOT count toward the character's
  advancement check under any circumstance.

#### The tie rule

- **FR-015**: A world MUST be able to declare that a total equal to the
  opposition succeeds.
- **FR-016**: When that setting is on, a tie MUST be recorded as a success and
  MUST award no XP.
- **FR-017**: When that setting is off, a tie MUST fail and award 1 XP, as
  today.
- **FR-018**: The tie setting MUST NOT affect the advancement check, which reads
  the dice as rolled plus any bought sixes and nothing else.

#### Statuses

- **FR-019**: A character MUST be able to hold any number of statuses, each with
  a free-text name and a signed integer modifier.
- **FR-020**: The system MUST NOT ship a fixed list of statuses, and MUST NOT
  judge whether a name or a number is appropriate.
- **FR-021**: A character's status modifiers MUST sum and MUST be applied once,
  flat, to a roll's total after the dice are summed.
- **FR-022**: Statuses MUST NOT change how many dice are rolled.
- **FR-023**: Statuses MUST NOT be read by the advancement check, so a status
  can neither create nor destroy an advancement.
- **FR-024**: A roll affected by statuses MUST show the raw dice sum, the
  modified total, and the name and value of each status that contributed.
- **FR-025**: A status MUST be removable, and removal MUST NOT alter any roll
  already recorded.
- **FR-026**: A modifier of zero MUST be accepted.
- **FR-027**: A total reduced below zero MUST be used as it stands rather than
  clamped.

#### Skill slots

- **FR-028**: A world MUST be able to declare that skills are capped per level.
- **FR-029**: The default caps MUST be 4 skills at level 2, 3 at level 3 and 2
  at level 4. Level 1 and levels 5 and above MUST be uncapped.
- **FR-030**: An advancement into a full level MUST be refused with a message
  naming the level and stating that it is full. It MUST NOT be silently dropped,
  and the offer MUST remain available until taken or declined.
- **FR-031**: A character MUST be able to buy one additional slot at a level for
  a price of twice that level in XP.
- **FR-032**: A purchase MUST be refused, with no XP deducted, when the
  character's balance is below the price.
- **FR-033**: A bought slot MUST apply to that character only, at that level
  only, and MUST persist.
- **FR-034**: XP spent on slots and XP spent on converting dice to sixes MUST
  draw on the same balance, and each unit of XP MUST be spendable exactly once.
- **FR-035**: Caps MUST constrain how many skills sit at a level, never how high
  a level may go. Skill levels remain unbounded.
- **FR-036**: Enabling the setting in a world whose characters already exceed a
  cap MUST NOT remove any skill.

#### Customised starting skills

- **FR-037**: A world MUST be able to define an ordered list of starting skills,
  each with a name and an integer level of at least 1.
- **FR-038**: A character created in such a world MUST begin with exactly those
  skills, and MUST NOT additionally receive `Do Anything 1`.
- **FR-039**: Changing or removing the world's starting skills MUST NOT alter
  any existing character.
- **FR-040**: A world with the setting on but an empty list MUST fall back to
  `Do Anything 1`.
- **FR-041**: A world-defined starting skill MUST behave in every other respect
  as `Do Anything 1` does — rollable, advanceable, and the parent of what it
  grants.

#### Proof

- **FR-042**: At least one end-to-end specification MUST exercise each Extra
  independently, in a world where only that Extra is on.
- **FR-043**: At least one end-to-end specification MUST exercise difficulty,
  statuses and the tie rule together on a single roll, proving the fixed order of
  application.
- **FR-044**: The proof MUST live in the `game-systems` slice and MUST run under
  `pnpm e2e:game-systems`.
- **FR-045**: Where a specification drives a character from the play dock, it
  MUST grant the claiming player Editor on that character explicitly, because
  claiming grants no write access today.

### Key Entities

- **World Extras settings**: five independent switches held against a world —
  opposition mode (free / rolled bands / static bands), tie-succeeds (on/off),
  statuses (on/off), skill slots (on/off) and starting skills (a list, possibly
  empty). All default to off or empty.
- **Status**: a name (free text) and a signed integer modifier, held against a
  character. Several may be held at once. Not a fixed vocabulary.
- **Slot grant**: a record that a particular character has bought one additional
  skill slot at a particular level, and the XP it cost.
- **Difficulty band**: one of four named steps chosen per roll, resolving to a
  Game Master dice count or a static target according to the world's mode.
- **Starting skill definition**: a name and a level, held against a world, used
  only at character creation.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A world that enables no Extra behaves identically to the system as
  shipped by spec 061 — demonstrated by the existing end-to-end specification
  passing with zero edits.
- **SC-002**: Each Extra can be enabled alone, and doing so changes only the
  behaviour that Extra governs, in 100% of the five cases.
- **SC-003**: Across every combination of difficulty mode and tie setting, a
  roll's verdict matches the stated comparison rule in 100% of cases, including
  every tie.
- **SC-004**: A status never changes the number of dice rolled and never changes
  whether an advancement is offered, across every case tested.
- **SC-005**: A character's XP total always equals what it earned minus what it
  spent, counted once per unit, across a session that spends on both sixes and
  slots.
- **SC-006**: An advancement into a full level is always reported to the player
  with the level named, and never silently lost.
- **SC-007**: A character created before a world's starting skills were defined
  or changed is never altered by that definition.
- **SC-008**: The pack can still be removed by deleting its directory and its
  build linkage, with no edit to any shared file that names it.
- **SC-009**: `pnpm e2e:game-systems` is green and the slice stays within its
  measured budget.

## Assumptions

- **A tie that succeeds pays no XP.** Settled in spec 061 and carried forward
  unchanged: XP is the price of failure, and a tie cannot be both.
- **Advancement reads the raw dice.** Spec 061 stated this while it was trivially
  true. Statuses are the reason it was stated, and it holds here without
  exception.
- **Statuses are the table's vocabulary.** The source uses them for harm as well
  as circumstance but gives no rating scale. Rather than invent one, this spec
  ships the mechanism and leaves the numbers to the table — the same call spec
  061 made about skill specificity.
- **Statuses apply to the character who rolls.** The source writes them on
  characters, not on scenes or on the world.
- **The bands map to counts and targets as the source states them** — one to four
  Game Master dice, or 3 / 6 / 9 / 12. No fifth band is invented.
- **A bought slot is permanent and personal.** Nothing suggests slots expire or
  are shared, and a purchase that could evaporate would make the price
  unjustifiable.
- **Caps are per level, not per character.** The source states them as a table of
  levels.
- **Starting skills are not retroactive.** Changing the world's premise should
  not rewrite a character somebody already played.
- **Settings belong to the world, not the campaign or the scene.** The product's
  existing unit of shared configuration is the world, and none of these rules
  reads as something to change between scenes.

## Out of Scope

- **The "New Shoes!" variant.** Unchanged from spec 061: a separate replacement
  ruleset, not an Extra. It needs a first-class scene boundary the core game has
  no use for, and mid-campaign conversion is undefined. Its own feature.
- **The site's published scenarios** and its D66 NPC generator tables. Content,
  not ruleset; their home is a collection, not this pack.
- **Automatic statuses.** Nothing here applies, escalates or expires a status on
  its own. A status exists because somebody wrote it and ends because somebody
  removed it.
- **A damage or health model.** Statuses are how the source expresses harm, and
  this spec ships statuses. It does not add a track, a threshold or a death rule.
- **Replacing a skill when a level is full.** The source mentions replacement as
  an option; this spec ships refusal plus purchase, which is unambiguous. Whether
  to also offer replacement is a later question.
- **Per-character or per-scene overrides of a world setting.** The settings are
  the world's.
- **Claiming a character granting write access to it.** Decided separately and
  needing its own host spec; this feature works within today's behaviour and its
  proof grants Editor explicitly.

## Dependencies

- The `roll_for_shoes` pack as spec 061 left it — its sheet, its roll path, its
  XP ledger and its advancement check are what these settings modify.
- The pack contract at `packs/systems/README.md` and the host surface a bundled
  pack may import (ADR-029).
- Whatever the product uses to hold per-world, per-system configuration; if no
  such surface exists, establishing one is part of this feature's planning.
- The server-resolved roll path, which must now also carry Game Master dice and
  a status-modified total.
- The `game-systems` e2e slice (spec 060, constitution Principle VI).

## Decisions (2026-09-22)

1. **Every Extra is off by default and independent of the others.** A table gets
   what it asked for and nothing else.
2. **The fixed order of a roll is: roll dice → sum → apply statuses →
   establish opposition → compare under the world's tie rule.** Stating the order
   once removes every question about how the Extras interact.
3. **Statuses never touch the dice.** They move the total only, so the
   advancement check is untouched by definition rather than by care.
4. **A full level refuses the advancement out loud and keeps the offer open.**
   Losing an earned advancement silently is the same failure mode as the lost XP
   spec 061's play-dock proof caught.
5. **Slots are bought, not freed by replacement.** Refusal plus purchase is
   unambiguous; replacement can be added later if a table asks.
6. **Statuses carry no rating scale for harm.** The source is ambiguous; we ship
   the mechanism and leave the number to the table rather than invent a scale and
   attribute it to the game.
7. **Starting skills are not retroactive**, and an empty list falls back to
   `Do Anything 1` rather than creating a character with no skill.
8. **Skill levels remain uncapped.** Slots cap breadth at a level, never height.
