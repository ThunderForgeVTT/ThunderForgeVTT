# Feature Specification: A Fight That Resolves

**Feature Branch**: `046-a-fight-that-resolves`

**Created**: 2026-09-11

**Status**: Built, 2026-09-15 — every story (US1–US6) shipped in tasks
Phases 3–9 and is proven by e2e and by `combat-5e.playtest.ts`, which has no
soft FINDING left. One target is unmet; see
[What shipped](#what-shipped-2026-09-15). The owner's three questions were
answered on 2026-09-12 and a fourth decided on 2026-09-14
(see [Decisions](#decisions-owner-2026-09-12)); clarified 2026-09-14.

**Input**: Project owner, after watching the combat playtest: "can we do combat
tracking and turn tracking and attack / damage for 5e as a playtest scenario",
"normally a medium creature has a reach of 5ft and a large has a 10ft reach
kinda deal", and "it highlights an area we might have missed like legendary
attacks, bonus actions, the works for round based combat".

## Context

### What shipped (2026-09-15)

The rest of this Context is the problem as it stood on 2026-09-11, kept as the
reason for this spec. Three of its claims were already stale when research
read the code on 2026-09-14; they are corrected where they stand, marked
*Corrected*. What was built, by tasks phase (tasks.md numbers its phases two
ahead of plan.md's):

- **Phase 3 — damage lands, bars move, zero is out** (US2, US3). The Game
  Master's Damage and Heal on each tracker row (`changeHitPoints`): temporary
  hit points spent first, bounded at 0 and the maximum, and every board
  re-reads its bars on event 26 as well as 14 and 19. A creature reaching zero
  is marked out *by hit points* and skipped; healing brings it back; a Game
  Master's Down is not undone by healing. `combat-hit-points.spec.ts`.
- **Phase 4 — turn order holds** (US1). A player's move on somebody else's
  turn is refused by the server on every path — a drag, `moveOwnToken`, and a
  queued offline move at replay — naming whose turn it is ("Unknown" when the
  name is hidden). Game Masters and creatures outside the fight are not held.
  An attack is held the same way (C1), reactions excepted.
  `combat-turn-order.spec.ts`.
- **Phase 5 — a token is its actor, or a copy of it** (US2, ADR-102). A
  linked token reads and writes its actor's sheet; an unlinked copy holds its
  own hit points. A unique NPC is placed linked and any other NPC as a copy;
  the Game Master can change either. `tokens.health` is retired.
  `token-links.spec.ts`; two hundred copies in `engine-status-limits.spec.ts`.
- **Phase 6 — an attack is aimed at something** (US1, US2, ADR-101). An
  attack from a character's sheet chooses a target and is resolved on the
  server against its armour class. Every seat's attack log shows it, with the
  attacker redacted to "Unknown" per viewer and nothing of it in that player's
  traffic. A hit is offered to the target's controller, who takes or declines
  it, and a Game Master may resolve it on their behalf; auto-apply is a world
  default the tracker can override per encounter. Offline, an attack is
  queued and judged at replay. `combat-attack.spec.ts`.
- **Phase 7 — size fills squares, and attacks have reach** (US4). A pack's
  `combat.sizes` decides the squares a creature fills on every board, for
  drawing, snapping, hit-testing and keyboard movement. Reach, range and line
  of sight are measured footprint to footprint, warned before the roll and
  flagged to the table after it, never refused. `combat-reach.spec.ts`.
- **Phase 8 — a round is an economy** (US5). Action, bonus action, reaction
  and movement shown for every combatant on every seat, spent by attacks and
  moves, an overspend shown as a debt and never refused, and refilled at the
  creature's own turn. `combat-economy.spec.ts`.
- **Phase 9 — a legendary creature acts between turns** (US6). Legendary
  actions read from the sheet, spent from the tracker between other
  creatures' turns and refilled at its own; a lair at initiative 20 that loses
  ties and acts through the Game Master. `combat-legendary.spec.ts`.
- **Phase 10 — polish.** The fight's controls driven by keyboard alone and
  held to WCAG 2.2 AA by axe, with focus returning when the attack flow closes
  and moving on when an offer is answered (`combat-accessibility.spec.ts`).

**Decided** by the owner (see [Decisions](#decisions-owner-2026-09-12)):

1. The product rolls, and the target's controller decides: a hit is an offer.
   The only automatic path is the Game Master's auto-apply to NPCs they run.
2. The economy shows and never refuses.
3. An attack needs line of sight by default, and an ability or item can say
   it does not.
4. "Unknown" follows the board (token centres), not an attack's line of sight
   (footprints).

**Proven, 2026-09-15**: the *one second* of SC-001, SC-002, FR-002 and
FR-013, asserted (owner decision of 2026-09-15: one second, one retry).
`combat-attack.spec.ts` times Aria's attack from the moment she confirms the
roll to the slowest of the three seats showing it, and the goblin's bars from
the Game Master taking the offer; `combat-hit-points.spec.ts` times the bars
from the tracker's Damage click. Every seat is watched at once, each for up to
five seconds so a failure reports the real figure, and a step fails when the
slowest seat takes over a second. The one retry is a re-measurement of that
step alone, after the table is put back (the offer declined, or the damage
healed), never a rerun of the test; both figures are annotated
(`e2e/fixtures/seatTiming.ts`). Measured on three runs with a `dev` engine:
the attack reached every seat in 298, 340 and 361 ms; bars moved after an
offer was taken in 269, 227 and 276 ms; bars moved from the tracker in 307,
290 and 220 ms. None needed the retry.

**Unmet**: research R17's load target. Two hundred unlinked copies add about
750 ms to a scene's load-to-drawn time (+686 ms measured in Phase 5, +757 ms
in Phase 7) against a target of 500 ms. The copies cost nothing over the same
two hundred tokens linked (+46 ms, gated at 500 ms), and the frame rate holds
at 60 fps: the time is the engine bringing two hundred tokens and their bars
onto a board, which predates this spec.

**Accepted** (owner, 2026-09-15): about 750 ms for two hundred tokens is the
accepted figure for now. It is still measured on every run of
`engine-status-limits.spec.ts`; making token and bar spawning cheaper is left to
a future engine-loading task, not this spec.

### The problem, as it stood on 2026-09-11

The combat tracker works. A Game Master starts an encounter, files everyone
in, and initiative sorts server-side; every player's panel follows the round
and the active combatant live; a player is refused the turn both in the
screens and by the server; the Game Master marks a combatant down and the turn
skips it; ending returns every panel to rest. `combat-5e.playtest.ts` checks
all of that hard, and it passes.

> *Corrected (research, 2026-09-14):* "refused the turn by the server" meant
> only that `advanceTurn` is Game-Master-only. No server path checked whose
> turn it was before a move, a roll or anything else; tasks Phase 4 added
> that check.

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

> *Corrected (research, 2026-09-14):* `useUpdateActorData` **is** used, by
> Genie's sheet through `@thunderforge/host`; only the 5e pack has no sheet
> writer, so for 5e the claim stood in effect. And the write **does** emit a
> world event — `EVENT_CODE_ACTOR_SHEET_CHANGED` (26), since spec 045 — which
> the bars ignored because `tokenStatus.ts` listened only for 14 and 19. The
> fix was a listener (tasks T016), not a new event.

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

## Clarifications

### Session 2026-09-14

- Q: When a player tries something the rules don't allow — attacking beyond an attack's reach or range, or acting when it isn't their turn — does the product refuse it or allow it and tell the table? → A: Turn order is refused for players (reactions excepted); reach and range are flagged to the table, never refused.
- Q: When a hit's damage is offered to a player whose character was struck and they are offline or haven't answered, what happens to the offer? → A: It waits, survives reconnects, and the Game Master may take or decline it on the controller's behalf at any time.
- Q: When a creature players can't see, or whose name the Game Master has hidden, makes an attack, what do players see of that roll? → A: Every seat sees the roll, but the server redacts the attacker to "Unknown" for any viewer who cannot see the token or from whom its name is hidden.
- Q: Which hit-point record is the real one — the actor's game-system data or the token's own health? → A: A linked token reads and writes its actor's system data; a token placed as an unlinked copy holds its own hit points. The owner added: a Game Master may drop two hundred goblins that need no actors of their own, while a named NPC such as "Boblin the goblin" is placed linked to a real actor.
- Q: Where does the Game Master switch on auto-apply for their NPCs — the whole world, per encounter, or per NPC? → A: A world default, off, which the Game Master can override for a single encounter in the combat tracker.

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
4. **Given** an attack that misses, **Then** nothing is offered to the target.
5. **Given** an attack that hits, **Then** its damage is rolled in the same
   click and offered to whoever controls the target, who takes it or does not
   (decision 1).

---

### User Story 2 - Damage lands, and every board shows it (Priority: P1)

A hit's damage is offered to whoever controls the creature; once it is taken,
every screen's bars shorten within a second, without a reload.

**Why this priority**: It is the point of a fight, and today it is a private
blob-write nobody sees.

**Independent Test**: The goblin has 7 hit points. A hit for 5 is offered to
the Game Master, who takes it; the goblin is left on 2 on the Game Master's
board, on both players' boards and on the server, within a second and with
nobody reloading.

**Acceptance Scenarios**:

1. **Given** a hit, **When** the target's controller takes the offered damage,
   **Then** the target's current hit points fall by that amount, bounded at
   zero. **When** they decline it, **Then** nothing changes.
2. **Given** that change, **Then** every client showing the scene reflects it
   within one second, with no reload, subject to what each viewer may see.
3. **Given** temporary hit points, **Then** they absorb damage before the
   creature's own.
4. **Given** healing, **Then** it raises current hit points, never above the
   maximum.
5. **Given** a Game Master, **Then** they may apply damage or healing to any
   creature by hand, including one nobody rolled against.
6. **Given** a Game Master who has switched on auto-apply for the NPCs they
   run, **When** an attack that properly selected one of those NPCs hits,
   **Then** its damage is applied without an offer. A roll made with no target
   applies to nothing.

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
square away may swing at it; a hero four squares away may still swing, and the
table is told the target was out of reach.

**Acceptance Scenarios**:

1. **Given** a creature with a size, **Then** it fills that size's squares on
   every client, and the grid treats it as filling them — snapping,
   hit-testing and movement alike, not only the drawing.
2. **Given** an attack with a reach, **When** its target is beyond that reach,
   **Then** the attacker is warned before rolling, the attack is not refused,
   and if it is made the table is told it was out of reach.
3. **Given** a ranged attack with a normal and a long range, **When** the
   target is beyond normal range, **Then** the table is told the shot is a long
   one; beyond long range, the attacker is warned and the table is told the
   shot was beyond range, but it is not refused.
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
   another, **Then** it is not refused, and the table is shown the overspend
   (decision 2).
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
- **Cover and walls.** An attack needs line of sight to its target unless the
  ability or item says otherwise (decision 3). Cover is not modelled: blocked
  or not blocked.
- **A target that moves before the roll resolves.** The roll is judged against
  where the target was when it was made.
- **Two attacks at once.** Two offers against the same creature, taken close
  together, are applied in the order the server accepts them; neither is lost,
  and each is bounded by the hit points left when it lands.
- **Several copies of one NPC.** Five goblins placed from one NPC are five
  unlinked copies; a hit on one changes only that one (FR-015).
- **Relinking a copy.** Changing an unlinked copy to linked replaces its own hit
  points with the actor's; the table is not asked to merge them.
- **Damage beyond zero.** Current hit points stop at zero; the excess is not
  carried anywhere unless a later spec models dying.
- **A creature removed mid-fight.** Removing a combatant whose turn it is
  passes the turn on.
- **An unconscious creature's reaction.** A creature out of the fight spends
  nothing and takes no reaction.
- **The controller is away.** An offer to an offline or silent player waits
  for them, and the Game Master may resolve it for them (FR-008, FR-009).
- **Offline.** An attack made while a client is offline is resolved when it
  reaches the server, against the state the server then holds. If it is
  refused there because the turn has passed (FR-060), nothing is spent.

## Requirements *(mandatory)*

### Functional Requirements

**A roll with a target**

- **FR-001**: An attack MUST be made against a chosen target on the scene, and
  its record MUST carry attacker, target, the formula and the result.
- **FR-002**: Every member of the world MUST be shown the result of an attack
  made in a scene they are in, within one second, without a reload.
- **FR-002a**: The server MUST redact an attack's attacker, per viewer, to
  "Unknown" wherever that viewer cannot see the attacker's token (spec 045) or
  the Game Master has hidden its name (the token-name rule of 2026-09-10). The
  attacker's identity MUST NOT reach that viewer's client in any field. The
  target's controller receives the roll and the offer under the same rule.
  The same applies to the target: a viewer who cannot see it is not told what
  was attacked.
- **FR-003**: A creature MUST carry a defence its game system declares (for
  D&D 5e, an armour class), and the table MUST be told whether an attack beat
  it.
- **FR-004**: A miss MUST apply nothing.
- **FR-005**: A hit MUST roll its damage in the same action, and MUST offer it
  to whoever controls the target, who takes or declines it. The product MUST
  NOT apply damage or healing to a creature without its controller's
  acceptance, except under FR-006 (decision 1).
- **FR-008**: An offer MUST persist until it is taken or declined: it MUST
  survive the controller going offline, reloading or reconnecting, and MUST be
  shown to them when they return. It MUST NOT expire and MUST NOT resolve
  itself.
- **FR-009**: A Game Master MUST be able to take or decline any pending offer
  on its controller's behalf, at any time, and the table MUST be told who
  resolved it.
- **FR-006**: A Game Master MUST be able to switch on auto-apply for the NPCs
  they run. Auto-apply MUST act only on an attack made against a selected
  target; a roll with no target applies to nothing. It MUST be a world setting,
  off by default, which the Game Master MAY override for a single encounter
  from the combat tracker; the override MUST end with that encounter. Damage
  to a player character MUST always be an offer, whatever the setting.
- **FR-007**: An ability or item MUST carry whether it needs line of sight,
  defaulting to required, and an import MAY set it. Line of sight MUST be
  judged with the same visibility function spec 045 uses for movement, and
  auto-apply MUST consult the flag (decision 3).

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
- **FR-015**: Every token MUST be either **linked** to its actor or an
  **unlinked copy** of it. A linked token's hit points MUST be its actor's
  system resource: damage and healing write there, and its bars read from
  there. An unlinked copy MUST hold its own hit points, starting from the
  actor's values when placed, and damage to it MUST NOT change the actor or
  any other copy. No creature may have two records of its hit points that can
  disagree.
- **FR-016**: A token placed for a player character MUST default to linked. A
  token placed for an NPC MUST default to an unlinked copy, unless the NPC is
  marked **unique** (a named individual, such as "Boblin the goblin"), in which
  case it MUST default to linked. A Game Master MUST be able to mark an NPC
  unique or not, to choose otherwise when placing a token, and to change a
  token's link later.
- **FR-017**: Unlinked copies MUST NOT require an actor each. Placing many
  copies of one NPC (for example two hundred goblins) MUST create no new
  actors, and each copy MUST still take damage, drop at zero and appear in the
  turn order on its own.

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
- **FR-033**: An attack whose target is beyond its reach or long range MUST NOT
  be refused. The attacker MUST be warned before rolling, and the attack's
  record, as shown to the table, MUST flag it as out of reach or beyond range.
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
- **FR-045**: The product MUST show an over-spend and MUST NOT refuse it
  (decision 2).

**Legendary and lair**

- **FR-050**: A creature MAY declare legendary actions and how many it has each
  round; the tracker MUST show how many remain.
- **FR-051**: A legendary action MUST be spendable at the end of another
  creature's turn, and MUST resolve like any other attack.
- **FR-052**: Legendary actions MUST refill at the start of their owner's turn.
- **FR-053**: A lair MUST take its place at initiative count 20 in the order.

**Turn order**

- **FR-060**: An action, an attack or a move by a player MUST be refused when
  it is not that player's creature's turn, with the reason given naming whose
  turn it is — except what a reaction allows (FR-041). The refusal MUST be
  enforced by the server, not only the screens. This is the one refusal in
  this spec; everything else a player does is shown, not blocked (FR-033,
  decision 2).
- **FR-061**: A Game Master MUST NOT be held to the turn order (consistent with
  spec 045's decision 1).

**Proof**

- **FR-070**: `combat-5e.playtest.ts` MUST exercise every story above in a D&D
  5e world, and its checks currently marked FINDING MUST pass.
- **FR-071**: The e2e suite MUST cover: an attack flagged for reach; a player
  refused for acting out of turn; an unseen attacker shown as "Unknown", with
  its identity absent from the player's network traffic; damage
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
- **Token link**: whether a token is its actor (linked, sharing the actor's hit
  points) or a copy of it (unlinked, with hit points of its own).
- **Offer**: damage or healing from one roll, addressed to the target's
  controller. Pending until taken or declined, by the controller or a Game
  Master; records who resolved it and how.

## Success Criteria *(mandatory)*

- **SC-001**: In the playtest, an attack rolled by a player is shown to every
  other seat within one second.
- **SC-002**: A hit for 5 on a 7-hit-point goblin leaves every board showing 2,
  within one second, with nobody reloading.
- **SC-003**: A creature reduced to zero is skipped by the turn order in 100%
  of runs, with no Game Master intervention.
- **SC-004**: An ogre placed on a five-foot grid fills four squares on every
  board, and a hero four squares away who swings a sword is warned first and
  the table sees the swing flagged as out of reach.
- **SC-005**: Across a played round, every seat can say what each creature has
  left to spend.
- **SC-006**: A legendary creature spends three legendary actions across other
  creatures' turns and has three again at the start of its own.
- **SC-007**: A player acting out of turn is refused, and the refusal names
  whose turn it is.
- **SC-008**: A scene holding two hundred unlinked goblin copies creates no new
  actors, and a hit on one goblin changes that goblin's bars alone.

## Assumptions

- **The rules come from the game system, not the platform.** Sizes, reaches,
  defences and what a turn affords are declared by a pack; the platform holds
  the concepts and enforces what the pack declares. Genie's `sizeCategories`
  is the existing shape for this.
- **Distances are in the system's units**, converted through the scene's grid:
  one square is one of the system's cells (five feet for D&D 5e).
- **The Game Master can always overrule.** The one refusal in this spec, turn
  order (FR-060), is a refusal to a player; a Game Master may act anyway.
- **A creature's own visibility rules still apply** (spec 045): being told
  about an attack does not reveal a token a player cannot see, or a name the
  Game Master has hidden (FR-002a).
- **An ADR records where resolution happens.** Attacks resolving server-side
  is a decision about an ownership boundary and is recorded at planning time.

## Out of Scope

- **A party-wide debt tracker — #todo.** Raised by the owner alongside
  decision 2: a Game Master taking one player's negative balance and making it
  the party's, so a debt becomes something the table carries together. A good
  idea and a different feature — it is about a shared resource rather than
  about a fight resolving, and it needs its own thinking about who may change
  it and what happens when somebody leaves the party.


- Conditions beyond out-of-the-fight (frightened, prone, grappled, and the
  rest), and concentration.
- Death saves and dying.
- Cover, advantage and disadvantage, critical hits and fumbles, resistances
  and immunities. Decision 1 keeps the target's controller as the judge, so
  none of these is needed for a fight to resolve.
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

## Decisions (owner, 2026-09-12)

1. **The product rolls, and whoever controls the target decides** (Q1: C, with
   the choice moved). The option offered was that the *world* chooses between
   "report the result" and "apply it automatically". The owner's answer is
   that neither is a world setting: the product offers helpers, and the person
   on the receiving end applies them.

   So an attack rolls its damage in one click, and the result arrives at the
   target's controller as an offer — take the hit, or do not. A heal is the
   same mechanism with the other sign, and is offered the same way. Nobody is
   ever told by the software that they have been hit.

   **One automatic path, and it is the Game Master's to switch on**:
   auto-applying to NPCs they run. It is gated on the attacker having properly
   selected what they were attacking — a roll made into the air applies to
   nothing, because "a roll in the dark is up to the table". The product's job
   is to make the common case one click, not to adjudicate the uncommon one.

2. **The economy shows and never refuses** (Q2: A). If a table lets somebody
   overspend and the Game Master decides they are in debt for it, that is the
   game being played, and a tracker that refused it would be wrong about who
   is in charge.

   A feature the owner raised alongside it, parked rather than specified: a
   Game Master taking a player's negative balance and turning it into a
   party-wide tracker. Recorded as a #todo below.

3. **Line of sight by default, and a per-ability flag that can say otherwise**
   (Q3: B, extended). Most attacks need to see their target, and spec 045
   already shipped the answer: `is_visible`, in the crate the engine and the
   server share, already called by both for movement. The same function, a
   different question.

   But a rule that stopped at "line of sight or nothing" would be wrong about
   half of what a ruleset contains. Teleport goes through a wall; a sword does
   not. So **line of sight is a property of the ability or item**, defaulting
   to required, and an import may say otherwise. The flag is what a Game
   Master's auto-apply consults, so an ability that ignores walls keeps
   ignoring them when the product is doing the arithmetic.

   Cover is deliberately not in this: blocked or not blocked, and the table
   handles the rest.

4. **"Unknown" follows the board, not the attack** (owner, 2026-09-14). Whether
   a player's attack log names an attacker is judged from token centres, as the
   engine draws the board, while an attack's line of sight (FR-007) is judged
   from the squares each creature fills. The two can disagree about a Large
   creature half round a corner: it may attack a player whose board does not
   draw it, and that player reads "Unknown". Naming it instead would reveal a
   creature the board hides. Moving both to footprints would reopen spec 045's
   vision rules, and is not wanted. See research R11 and contract §3.

5. **Hiding a creature hides its tokens' names** (owner, 2026-09-15). One
   switch, not two. A player may read a token's name when the token's own
   `name_visible_to_players` says so **and** the creature it stands for is one
   every player may see — a character, or an NPC its Game Master has made
   `visible_to_players`. Before this, a hidden NPC's token still drew its
   creature's name on every player's board, which is the whole point of hiding
   it undone.

   **The actor wins.** A Game Master who has explicitly named one token of a
   hidden NPC still shows players nothing: hiding the creature is the broader
   statement, and the token's switch is kept, waiting, to take effect the
   moment the NPC is shown. The other precedence would let a single token leak
   a creature a player is not supposed to know exists — the leak this closes.

   The rule is stated once, in
   `auth::npc_visibility::player_may_read_token_name`, and asked by every path
   that serves a name: the board's token list, a mutation's answer, the combat
   tracker, the out-of-turn refusal, and the attack log's redaction (contract
   §3). Showing the NPC nudges token-changed per scene and combat-changed for
   the running fight, so every board and tracker reads the name at once,
   without a reload. A token with no actor is its own switch alone.

## Questions for the owner

1. **Q1 — How far does the product resolve an attack?** *(answered: C, with
   the choice belonging to the target's controller — see decision 1)*

   | Option | Answer | Implications |
   |--------|--------|--------------|
   | A | It rolls and says whether the number beat the defence; the Game Master applies the damage | Smallest step, keeps every judgement at the table. The damage is still a button somebody presses. |
   | B | A hit applies its damage automatically; a miss applies nothing | The fight runs itself. Needs critical hits, resistances and the rest sooner rather than later. |
   | C | The world chooses between A and B | Tables differ; costs a setting and both paths tested. |

2. **Q2 — Does the economy refuse, or only show?** *(answered: A)*

   | Option | Answer | Implications |
   |--------|--------|--------------|
   | A | Show what is spent; refuse nothing | A tracker a table reads. Nobody is ever blocked by a rule the fiction overrode. |
   | B | Refuse an over-spend, with a Game Master override | The rules hold; the Game Master lifts them when the table decides. |
   | C | Show only this spec; refuse in a later one | Ships the visible half now. |

3. **Q3 — Does a wall stop an attack?** *(answered: B, with a per-ability
   flag — see decision 3)*

   | Option | Answer | Implications |
   |--------|--------|--------------|
   | A | No: reach and range only | Simplest. A hero may shoot through a closed door. |
   | B | An attack needs line of sight, reusing spec 045's vision | Consistent with what a player can see. No cover rules, just blocked or not. |
   | C | Line of sight, plus cover as a modifier | Truest to the rules, and the largest of the three. |
