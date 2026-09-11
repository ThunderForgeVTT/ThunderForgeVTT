# Feature Specification: A Fight That Resolves

**Feature Branch**: `046-a-fight-that-resolves`

**Created**: 2026-09-11

**Status**: Draft. Three owner questions are open (see
[Questions for the owner](#questions-for-the-owner)).

**Input**: Project owner, after watching the combat playtest: "can we do combat
tracking and turn tracking and attack / damage for 5e as a playtest scenario",
"normally a medium creature has a reach of 5ft and a large has a 10ft reach
kinda deal", and "it highlights an area we might have missed like legendary
attacks, bonus actions, the works for round based combat".

## Context

The combat tracker works. A Game Master starts an encounter, files everyone
in, and initiative sorts server-side; every player's panel follows the round
and the active combatant live; a player is refused the turn both in the
screens and by the server; the Game Master marks a combatant down and the turn
skips it; ending returns every panel to rest. `combat-5e.playtest.ts` checks
all of that hard, and it passes.

Everything the tracker points *at* is missing. The playtest of 2026-09-11
records seven findings, and they are one story: **a turn is a pointer, and a
roll is a number, and nothing joins them to the board.**

### A roll has nobody to hit

`rollDice` resolves a formula and returns a number to the caller. It takes no
target. A 5e actor has no armour class the product reads — `armor_class`
exists only in a pack web package the app does not render — so nothing
compares a result to a defence. An ability may be typed `ATTACK_ROLL` or
`DAMAGE`, and `types_abilities.rs` says plainly that the trigger is
"scaffolded, never evaluated in this pass": pressing it rolls the formula and
applies nothing.

The roll is also private. There is no world event for a roll, and
`worldRollRecords` is Game-Master-only, so a player's attack reaches no other
seat. In the playtest, Aria rolled 22 to hit and Brom's screen showed nothing.

### Damage is not an operation

Nothing subtracts anything. The only way a creature's hit points change is a
whole-blob write of `resource_data` through `updateActorSystemData`, by
someone with Editor on the actor — and there is no screen that does it: the
pack sheet is read-only, and `useUpdateActorData` is imported by no component.
That write emits no world event, so no client re-reads the status: the bars do
not move, on any board, until a token event or a reload. Spec 029 US1's own
independent test says the opposite ("Reduce the character's hit points from
another session; the bar shortens without a reload"), so the product
contradicts a shipped spec.

At zero, nothing happens. The 5e validator allows `current_hp: 0` and says
"Zero HP is valid (unconscious)", and that is the whole of it: no dying, no
unconscious, no defeated. `DerivedStats.is_dead` is computed in the engine and
read by nothing. The tracker's own "Down" is a button the Game Master presses,
connected to no hit points.

### Size is a drawing instruction, and reach does not exist

A creature's size and an attack's reach are two different things, and the
product has neither.

- **Size** is the space a creature fills. In the rules a Medium creature fills
  a five-foot square, a Large one ten feet — two squares by two — and a
  Gargantuan one twenty feet, four by four.
- **Reach** belongs to the attack. An ogre is Large and its greatclub still
  reaches five feet, the same as a longsword; a tarrasque's four attacks reach
  ten, fifteen, ten and twenty feet. Deriving reach from size gets both
  creatures wrong.

Today "large" means `tokens.scale = 2`, which multiplies the sprite's
transform and nothing else. The grid-correct concept exists and is unreachable:
`Footprint` in `thunderforge-canvas-core` drives snapping, hit-testing,
movement commits, nameplates and status bars, but it is never persisted, has no
GraphQL field, and its only caller in the repo is the engine sandbox — whose
buttons are labelled Tiny, Medium, Large, Huge and Gargantuan. So a Large ogre
draws twice as big while the grid treats it as one square, and a hero can swing
at it from across the room unremarked.

`packs/systems/dnd5e` declares no size at all. Genie's manifest already
declares `sizeCategories` with a scale each, resolved from an NPC's
`trait_data.size_category` into a default token scale — the pattern to follow
exists, in the other pack.

### A round is not an economy

At a table, a turn affords movement, an action and often a bonus action; a
reaction happens between turns; a monster may have multiattack; a legendary
creature spends legendary actions at the end of *other* creatures' turns, and
lair actions on initiative count 20. Concentration ends when its holder is
struck.

The tracker counts rounds and points at a combatant. It records nothing of
what anyone has spent, and there is nowhere for a legendary action to live.
Nothing checks whose turn it is before a move, a roll or an ability, either:
in the playtest it was the ogre's turn and Aria moved anyway.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - An attack is aimed at something (Priority: P1)

A player attacks a creature: they choose the target, the roll happens against
that target, and the table is told what happened.

**Why this priority**: Everything else in this spec needs a roll that knows
who it is aimed at. Without a target there is no defence to beat, no distance
to measure and nothing to damage.

**Independent Test**: A player rolls their longsword against the goblin. The
result names the attacker, the target and the number, and the Game Master's
screen shows it without being asked.

**Acceptance Scenarios**:

1. **Given** a player on their turn, **When** they use an attack of their own
   against a creature on the scene, **Then** the roll is made against that
   creature and its result is recorded with attacker, target and total.
2. **Given** that roll, **Then** every seat at the table is shown it — the
   Game Master, the other players, and the roller.
3. **Given** a creature with a defence, **When** an attack is rolled against
   it, **Then** the table is told whether the attack beat that defence.
4. **Given** an attack that misses, **Then** nothing is applied to the target.

---

### User Story 2 - Damage lands, and every board shows it (Priority: P1)

A hit's damage is applied to the creature, and every screen's bars shorten
within a second, without a reload.

**Why this priority**: It is the point of a fight, and today it is a private
blob-write nobody sees.

**Independent Test**: The goblin has 7 hit points. A hit for 5 leaves it on 2
on the Game Master's board, on both players' boards and on the server, within a
second and with nobody reloading.

**Acceptance Scenarios**:

1. **Given** a hit, **When** its damage is applied, **Then** the target's
   current hit points fall by that amount, bounded at zero.
2. **Given** that change, **Then** every client showing the scene reflects it
   within one second, with no reload, subject to what each viewer may see.
3. **Given** temporary hit points, **Then** they absorb damage before the
   creature's own.
4. **Given** healing, **Then** it raises current hit points, never above the
   maximum.
5. **Given** a Game Master, **Then** they may apply damage or healing to any
   creature by hand, including one nobody rolled against.

---

### User Story 3 - A creature at zero is out of the fight (Priority: P1)

A creature reduced to zero hit points stops taking turns, and the table can
see that it has.

**Why this priority**: Without it, a fight has no end state and the Game
Master keeps the whole thing in their head.

**Independent Test**: The goblin is reduced to zero. Its row is marked, the
turn order skips it, and both players' boards show it as out of the fight.

**Acceptance Scenarios**:

1. **Given** a creature reduced to zero hit points, **Then** it is marked as
   out of the fight and the turn order passes over it.
2. **Given** such a creature healed above zero, **Then** it takes turns again.
3. **Given** a Game Master, **Then** they may mark any combatant out, or back
   in, by hand — as today.

---

### User Story 4 - Size fills squares, and attacks have reach (Priority: P1)

A large creature occupies the squares it should, and every attack knows how
far it reaches.

**Why this priority**: It is what makes a board a board rather than a picture,
and it is the owner's own example: a Large ogre whose greatclub still reaches
only five feet.

**Independent Test**: An ogre is placed on a five-foot grid. It fills two
squares by two, on every board and for the grid's own purposes. A hero one
square away may swing at it; a hero four squares away is told the target is out
of reach.

**Acceptance Scenarios**:

1. **Given** a creature with a size, **Then** it fills that size's squares on
   every client, and the grid treats it as filling them — snapping,
   hit-testing and movement alike, not only the drawing.
2. **Given** an attack with a reach, **When** its target is beyond that reach,
   **Then** the table is told so before anything is rolled.
3. **Given** a ranged attack with a normal and a long range, **When** the
   target is beyond normal range, **Then** the table is told the shot is a long
   one; beyond long range, that it cannot be made.
4. **Given** a Large creature, **Then** its reach is whatever its attacks say
   — not ten feet by virtue of being Large.
5. **Given** a game system that declares size categories, **Then** a creature's
   size comes from its own data, as Genie's NPCs already do.

---

### User Story 5 - A round is an economy (Priority: P2)

On their turn a creature moves, takes an action and perhaps a bonus action;
between turns it may take a reaction. The tracker knows what has been spent.

**Why this priority**: It is the shape of every round-based fight, and the
tracker has none of it. It sits after the resolution stories because spending
an action is only meaningful once an action does something.

**Independent Test**: On Aria's turn, her action, bonus action, reaction and
movement are shown as unspent. She attacks; her action is spent, and the table
can see it. The turn passes and her budget is fresh, with the reaction spent
until her next turn begins.

**Acceptance Scenarios**:

1. **Given** a creature's turn beginning, **Then** its action, bonus action and
   movement are unspent, and every seat can see what remains.
2. **Given** an attack taken as an action, **Then** the action is spent.
3. **Given** a creature that has spent its action, **When** it attempts
   another, **Then** the table is told (see Q2 on whether it is refused).
4. **Given** a reaction, **Then** it is spendable between turns and returns at
   the start of the creature's own turn.
5. **Given** movement, **Then** what a creature has moved this turn is counted
   against its speed, and the table can see how much is left.
6. **Given** an attack with multiattack, **Then** one action makes every
   attack it names.

---

### User Story 6 - A legendary creature acts between turns (Priority: P3)

A tarrasque spends legendary actions at the end of other creatures' turns, and
its lair acts on initiative count 20.

**Why this priority**: It is the top of this ladder: it needs targets, reach,
damage and an economy before it means anything. It is also what a table
remembers about a big fight.

**Independent Test**: A creature with three legendary actions per round spends
one at the end of a player's turn; the tracker shows two left, and refills them
at the start of its own turn.

**Acceptance Scenarios**:

1. **Given** a legendary creature, **Then** its legendary actions are shown
   with how many remain this round.
2. **Given** the end of another creature's turn, **Then** the Game Master may
   spend a legendary action, and what it does resolves like any other attack.
3. **Given** the start of the legendary creature's turn, **Then** its
   legendary actions refill.
4. **Given** a lair, **Then** the tracker shows initiative count 20 in the
   order, and the Game Master may act there.

---

### Edge Cases

- **Reach measured from a big body.** A Large creature's reach is measured
  from the squares it fills, not from its centre, so an ogre adjacent to a hero
  is adjacent from any of its four squares.
- **A big creature on hexes.** A Large creature on a hex grid is a seven-hex
  flower, which the canvas core records as unmodelled. Hex footprints stay as
  they are until that is solved.
- **Cover and walls.** An attack across a wall that blocks vision — see Q3.
- **A target that moves before the roll resolves.** The roll is judged against
  where the target was when it was made.
- **Two attacks at once.** Two players resolving attacks on the same creature
  are applied in the order the server accepts them; neither is lost.
- **Damage beyond zero.** Current hit points stop at zero; the excess is not
  carried anywhere unless a later spec models dying.
- **A creature removed mid-fight.** Removing a combatant whose turn it is
  passes the turn on.
- **An unconscious creature's reaction.** A creature out of the fight spends
  nothing and takes no reaction.
- **Offline.** An attack made while a client is offline is resolved when it
  reaches the server, against the state the server then holds; a refusal
  returns the spender's budget.

## Requirements *(mandatory)*

### Functional Requirements

**A roll with a target**

- **FR-001**: An attack MUST be made against a chosen target on the scene, and
  its record MUST carry attacker, target, the formula and the result.
- **FR-002**: Every member of the world MUST be shown the result of an attack
  made in a scene they are in, within one second, without a reload.
- **FR-003**: A creature MUST carry a defence its game system declares (for
  D&D 5e, an armour class), and the table MUST be told whether an attack beat
  it.
- **FR-004**: A miss MUST apply nothing.
- **FR-005**: How far the product goes in resolving an attack is settled by Q1.

**Damage and hit points**

- **FR-010**: Damage MUST be an operation the product performs on a creature,
  not a rewrite of its whole data by hand.
- **FR-011**: Applying damage MUST lower current hit points, bounded at zero,
  and MUST spend temporary hit points first.
- **FR-012**: Healing MUST raise current hit points, bounded by the maximum.
- **FR-013**: A change to a creature's hit points MUST reach every client
  showing the scene within one second, without a reload, and MUST respect what
  each viewer is allowed to see.
- **FR-014**: A Game Master MUST be able to apply damage or healing to any
  creature directly.
- **FR-015**: The two hit-point pools MUST be reconciled: `tokens.health` and
  the system's own resource MUST NOT disagree about how hurt a creature is.

**Out of the fight**

- **FR-020**: A creature at zero hit points MUST be marked out of the fight,
  and the turn order MUST pass over it.
- **FR-021**: Healing above zero MUST return it to the order.
- **FR-022**: A Game Master MUST keep the manual marking they have today.

**Size and reach**

- **FR-030**: A creature MUST carry a size, declared by its game system, and
  that size MUST decide how many squares it fills.
- **FR-031**: A creature's size MUST be what the grid uses — snapping,
  hit-testing and movement — and not only how large it is drawn. The engine's
  footprint MUST follow it.
- **FR-032**: An attack MUST carry its own reach, or its normal and long
  range. Reach MUST NOT be derived from size.
- **FR-033**: An attack whose target is beyond its reach or long range MUST be
  refused, and the reason given, before anything is rolled.
- **FR-034**: A ranged attack beyond its normal range MUST be marked a long
  shot.
- **FR-035**: Distance MUST be measured from the squares a creature fills.

**The economy of a round**

- **FR-040**: A turn MUST afford an action, a bonus action and movement, and
  what remains of each MUST be visible to every seat.
- **FR-041**: A reaction MUST be spendable between turns and MUST return at the
  start of its owner's turn.
- **FR-042**: Spending MUST be recorded against the creature whose turn it is,
  and MUST reset when its next turn begins.
- **FR-043**: Movement spent MUST be counted against the creature's speed.
- **FR-044**: An attack that names several attacks (multiattack) MUST make them
  all for one action.
- **FR-045**: Whether the product refuses an over-spend or merely shows it is
  settled by Q2.

**Legendary and lair**

- **FR-050**: A creature MAY declare legendary actions and how many it has each
  round; the tracker MUST show how many remain.
- **FR-051**: A legendary action MUST be spendable at the end of another
  creature's turn, and MUST resolve like any other attack.
- **FR-052**: Legendary actions MUST refill at the start of their owner's turn.
- **FR-053**: A lair MUST take its place at initiative count 20 in the order.

**Turn order**

- **FR-060**: An action, an attack or a move by a player MUST be refused when
  it is not that player's creature's turn, with the reason given — except what
  a reaction allows (FR-041).
- **FR-061**: A Game Master MUST NOT be held to the turn order (consistent with
  spec 045's decision 1).

**Proof**

- **FR-070**: `combat-5e.playtest.ts` MUST exercise every story above in a D&D
  5e world, and its checks currently marked FINDING MUST pass.
- **FR-071**: The e2e suite MUST cover: an attack refused for reach; damage
  reaching another client's bars within a second; a creature dropping at zero;
  an action spent and refilled; and a legendary action spent between turns.

### Key Entities

- **Attack**: what a creature does to another. Has a to-hit roll, damage, and
  either a reach or a normal and long range.
- **Defence**: what an attack is measured against — for D&D 5e, armour class.
- **Size**: how much space a creature fills, declared by its game system:
  Tiny, Small, Medium, Large, Huge, Gargantuan.
- **Turn budget**: what a creature has left this turn — action, bonus action,
  reaction, movement.
- **Legendary actions**: a pool spent between other creatures' turns, refilled
  each round.
- **Roll record**: attacker, target, formula, result, and whether it beat the
  defence.

## Success Criteria *(mandatory)*

- **SC-001**: In the playtest, an attack rolled by a player is shown to every
  other seat within one second.
- **SC-002**: A hit for 5 on a 7-hit-point goblin leaves every board showing 2,
  within one second, with nobody reloading.
- **SC-003**: A creature reduced to zero is skipped by the turn order in 100%
  of runs, with no Game Master intervention.
- **SC-004**: An ogre placed on a five-foot grid fills four squares on every
  board, and a hero four squares away is refused a sword swing with a reason
  naming reach.
- **SC-005**: Across a played round, every seat can say what each creature has
  left to spend.
- **SC-006**: A legendary creature spends three legendary actions across other
  creatures' turns and has three again at the start of its own.
- **SC-007**: A player acting out of turn is refused, and the refusal names
  whose turn it is.

## Assumptions

- **The rules come from the game system, not the platform.** Sizes, reaches,
  defences and what a turn affords are declared by a pack; the platform holds
  the concepts and enforces what the pack declares. Genie's `sizeCategories`
  is the existing shape for this.
- **Distances are in the system's units**, converted through the scene's grid:
  one square is one of the system's cells (five feet for D&D 5e).
- **The Game Master can always overrule.** Every refusal in this spec is a
  refusal to a player; a Game Master may do it anyway.
- **A creature's own visibility rules still apply** (spec 045): being told
  about an attack does not reveal a token a player cannot see.
- **An ADR records where resolution happens.** Attacks resolving server-side
  is a decision about an ownership boundary and is recorded at planning time.

## Out of Scope

- Conditions beyond out-of-the-fight (frightened, prone, grappled, and the
  rest), and concentration.
- Death saves and dying.
- Cover, advantage and disadvantage, critical hits and fumbles, resistances
  and immunities — unless Q1's answer draws them in.
- Spell slots, and any resource other than hit points.
- Automating a whole monster's turn.

## Dependencies

- **Spec 029**: in-engine status displays, whose bars this finally moves.
- **Spec 030**: interactive elements, the existing seam for a thing that acts.
- **Spec 031**: the initiative tracker this builds on.
- **Spec 032**: the pack architecture, through which a system declares sizes,
  defences, attacks and what a turn affords.
- **Spec 033**: the abilities vocabulary, which is where attacks live today.
- **Spec 045**: token movement and vision — its decisions on Game Master
  freedom, and its assumption that a move is judged by a token's centre, which
  FR-031 revisits.

## Questions for the owner

1. **Q1 — How far does the product resolve an attack?**

   | Option | Answer | Implications |
   |--------|--------|--------------|
   | A | It rolls and says whether the number beat the defence; the Game Master applies the damage | Smallest step, keeps every judgement at the table. The damage is still a button somebody presses. |
   | B | A hit applies its damage automatically; a miss applies nothing | The fight runs itself. Needs critical hits, resistances and the rest sooner rather than later. |
   | C | The world chooses between A and B | Tables differ; costs a setting and both paths tested. |

2. **Q2 — Does the economy refuse, or only show?**

   | Option | Answer | Implications |
   |--------|--------|--------------|
   | A | Show what is spent; refuse nothing | A tracker a table reads. Nobody is ever blocked by a rule the fiction overrode. |
   | B | Refuse an over-spend, with a Game Master override | The rules hold; the Game Master lifts them when the table decides. |
   | C | Show only this spec; refuse in a later one | Ships the visible half now. |

3. **Q3 — Does a wall stop an attack?**

   | Option | Answer | Implications |
   |--------|--------|--------------|
   | A | No: reach and range only | Simplest. A hero may shoot through a closed door. |
   | B | An attack needs line of sight, reusing spec 045's vision | Consistent with what a player can see. No cover rules, just blocked or not. |
   | C | Line of sight, plus cover as a modifier | Truest to the rules, and the largest of the three. |
