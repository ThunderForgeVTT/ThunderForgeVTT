# Feature Specification: A Fight in the Browser

**Feature Branch**: `079-a-fight-in-the-browser`
**Created**: 2026-10-06
**Status**: Draft
**Input**: The owner, 2026-10-06: "simulate the combat … on that ambush, do the goblins and the heroes in a combat queue so we could simulate the combat just locally in the browser without having to involve the server. I think that'd be really huge so the user can go through it." Asked whether to port the rules to TypeScript or share them: "do b move it into a shared code state like thunderforge-combat and have it be shared logic essentially via wasm."

## Why

The demo (spec 074) is the world a visitor tries before they have an
account. It runs entirely in the browser: the real web app, with a small
local stand-in answering what the server would. Today it can move tokens,
open doors and roll a check on a sheet. It cannot fight. The Grassy Path
Ambush scene sets two heroes against three goblins, a hobgoblin and a wolf,
and nothing happens when they meet: no initiative, no turn, no attack, no
damage. A virtual tabletop that cannot run a fight does not feel real.

Spec 046 built the fight, but only on the server, and its rules are
written against the database. A demo fight could be written a second time
in the browser, but then the demo would play by rules of its own and drift
from the game it advertises. The owner decided the other way: the combat
rules move into one place that both the server and the browser run, so a
fight in the demo resolves exactly as it would at a real table.

## What exists

Counted on 2026-10-06:

- `crates/thunderforge-server/src/combat/` (spec 046): turn order and
  initiative, the action budget, attacks and their preview, hit points and
  damage offers, reach, size, legendary and lair actions, and what each
  viewer may see (redaction). About 8,400 lines with tests; nearly every
  file mixes a rule with the database reads and writes around it.
- `crates/thunderforge-server/src/turn_structure.rs`: no database use.
- A system's combat rules are *data*: `packs/systems/dnd5e/system.json`
  declares defence, sizes, legendary actions and hit points under `combat`,
  read by `combat/manifest.rs`. `packs/systems/dnd5e/server/src/rules.rs`
  adds 5e arithmetic (ability modifier, proficiency bonus) behind the
  `SystemRules` trait.
- `thunderforge-dice` and `thunderforge-canvas-core` already follow the
  discipline this spec needs: no database, network or engine dependency,
  used by both the server and the browser.
- The web app's fight UI exists and is used unchanged: `PlayDock`'s
  `CombatPanel`, `AttackFlow` and `AttackLog`, over `api/combat.ts` and
  `api/attacks.ts`.
- The demo has no combat: it answers no combat operation, and
  `sceneAttacks` is stubbed to an empty list (`apps/demo/src/backend/handlers.ts`).

## User Scenarios & Testing

### User Story 1 - A visitor fights the ambush (Priority: P1)

A visitor opens the demo, goes to the Grassy Path Ambush, and starts a
fight. Initiative is rolled for the heroes and the monsters and a turn
order appears. On the fighter's turn they attack a goblin, see the roll
against its defence, and on a hit see the damage land on the goblin's hit
points. They end the turn, and the order moves on, through the monsters'
turns, into the next round.

**Why this priority**: It is the whole point: the demo can run a fight.

**Independent Test**: Run the demo with no server, play one round of the
ambush, and read the turn order, the attack log and the hit points back
from the page.

**Acceptance Scenarios**:

1. **Given** the demo on the Grassy Path Ambush as the Game Master,
   **When** they start a fight with the heroes and the monsters,
   **Then** every combatant has an initiative and the turn order shows them
   highest first, with the first combatant's turn current.
2. **Given** it is Brannoc Stoneward's turn and a goblin is within reach,
   **When** the visitor attacks it,
   **Then** the attack log shows the roll, the goblin's defence and hit or
   miss, and on a hit the goblin's hit points fall by the damage rolled.
3. **Given** a fight in progress,
   **When** the visitor advances the turn past the last combatant,
   **Then** the round number goes up and the first combatant's turn comes
   round again.
4. **Given** a goblin's hit points reach zero,
   **Then** it is shown as down, exactly as the server shows a creature at
   zero.
5. **Given** a fight is in progress and the visitor reloads the page,
   **Then** the fight is still there — the round, whose turn it is, every
   creature's hit points and the attack log — as the rest of the demo
   survives a reload (spec 074 FR-012); "Start over" is what resets it.

---

### User Story 2 - The server fights exactly as before (Priority: P1)

Nothing a real table sees changes. The server's combat now calls the
shared rules, and every fight it ran before runs the same way.

**Why this priority**: The extraction is only safe if it changes nothing
for real games; it is equal first with the demo.

**Independent Test**: The server's existing combat tests and the combat
e2e slice pass without being edited.

**Acceptance Scenarios**:

1. **Given** the server's combat tests as they stand on 2026-10-06,
   **When** they run after the extraction,
   **Then** every one passes, and none was edited to make it pass.
2. **Given** the `combat` e2e slice,
   **When** it runs after the extraction,
   **Then** it passes.

---

### User Story 3 - The same fight, the same result (Priority: P2)

Given the same combatants and the same dice, a fight resolved in the demo
and a fight resolved by the server reach the same turn order, the same
hits and misses, and the same hit points.

**Why this priority**: It is the reason for sharing the rules rather than
copying them; this story makes the claim checkable.

**Independent Test**: A test runs one scripted fight through the shared
rules as the server calls them and as the browser calls them, with fixed
dice, and compares every step.

**Acceptance Scenarios**:

1. **Given** a scripted fight with fixed dice,
   **When** it is resolved through the server's path and through the
   browser's path,
   **Then** each step (initiative order, each attack's outcome, each change
   of hit points, each round) is identical.

### Edge Cases

- An attack on a target out of reach is refused in the demo with the same
  reason the server gives.
- A monster with legendary actions (none in the ambush today) behaves in
  the demo as on the server; the shared rules carry it even if the ambush
  does not use it.
- Viewing as a player, the demo hides what the server would hide from a
  player (redaction): a monster's exact hit points, a hidden combatant.
- A rule the demo does not support yet MUST be refused plainly ("not part
  of the demo", as spec 074 already does), never silently do nothing.
- Starting a second fight while one is running behaves as the server does.

## Requirements

### Functional Requirements

- **FR-001** The combat rules MUST live in one shared library, with no
  dependency on the database, the network or the game engine, that both
  the server and the browser run. Covered: initiative and turn order, the
  action budget, attack resolution and its preview, hit points and damage
  offers, reach and size, legendary and lair actions, and redaction.
- **FR-002** A game system's own rules stay with that system under `packs/`
  (constitution, repository layout). Whatever the shared combat rules need
  from 5e (ability modifiers, proficiency) MUST be reachable from the
  browser without moving it out of the 5e pack.
- **FR-003** The server MUST keep only what touches storage: load what a
  rule needs, call the shared rule, save what it decided. Its behaviour MUST
  NOT change (User Story 2).
- **FR-004** The shared rules MUST be callable from the browser.
- **FR-005** The demo MUST answer the combat operations the fight UI uses,
  through the shared rules, with no server: the active combat; starting and
  ending a fight; adding, updating and removing a combatant, and adding a
  lair; advancing the turn; changing hit points; making, previewing and
  resolving an attack; one attack (`attack`) and the scene's attacks; the
  combat's and the world's auto-apply settings (`setCombatAutoApply`,
  `updateWorldAutoApplyNpcDamage`, which the campaign panel reaches); and
  the abilities an attack is chosen from (`worldAbilities`,
  `actorAbilities`), which the demo answers today with nothing.
- **FR-006** The fight UI (`CombatPanel`, `AttackFlow`, `AttackLog`) MUST be
  used unchanged; the demo reaches it through the same operations as the
  server does.
- **FR-007** The demo's dice MUST be real rolls during play and fixable in
  tests, so a demo fight is not scripted and a test of it is deterministic.
- **FR-008** Viewing as a player in the demo MUST show what the server would
  show a player (FR-001's redaction).
- **FR-009** The shared rules MUST be measured and reported in what a visitor
  downloads, as the engine bundle is. The owner's hard cap is 1 GB for
  everything the demo downloads (2026-10-06): the engine is already the bulk
  of it, and the combat rules are not expected to move that number much.
- **FR-010** The visitor plays the monsters while viewing as the Game Master
  (owner, 2026-10-06), through the same panel a Game Master uses at a real
  table. Viewing as a player, a monster's turn is the Game Master's to take,
  as on the server: the visitor switches to the Game Master view to play it.
  Monsters acting on their own is out of scope.

### Key Entities

- **Combat**: one fight on a scene, with its round and whose turn it is.
- **Combatant**: a token in the fight, with its initiative, its budget for
  the turn, its hit points, and for some creatures legendary and lair
  actions.
- **Attack**: one attempt by a combatant on a target: the roll, the defence
  it met, hit or miss, and the damage.
- **Damage offer**: damage waiting to be applied or declined, per spec 046.

## Success Criteria

### Measurable Outcomes

- **SC-001**: A first-time visitor can start a fight on the ambush and
  play one full round, every combatant taking a turn, with no server
  running.
- **SC-002**: 100% of the server's combat tests as of 2026-10-06 pass
  after the extraction, with none edited.
- **SC-003**: A scripted fight with fixed dice produces identical results,
  step for step, through the server's path and the browser's.
- **SC-004**: Each action in a demo fight (start, attack, advance) shows
  its result in under half a second on the machine the e2e suite runs on.

### Proof

- The server's combat tests and the `combat` e2e slice, unchanged (US2).
- A parity test of one scripted fight through both paths (US3).
- A demo e2e test that plays one round of the ambush and reads back the
  turn order, the attack log and the hit points (US1), in the demo's own
  suite (`cd apps/demo && pnpm run e2e`).

## Assumptions

- A demo fight is demo state like any other: it survives a reload (spec 074
  FR-012) and "Start over" resets it.
- The ambush's monsters keep their SRD stat blocks; the fight uses what
  they already carry (defence, hit points, attacks).
- An ADR records the shared combat library as a new subsystem, landing with
  the change (constitution, Principle IV).
- Out of scope: door icons (an amendment to spec 071); links from the
  roster to each sheet (a separate small change); new combat rules.
