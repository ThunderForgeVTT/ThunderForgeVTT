# Feature Specification: 5e Roll Facets

**Feature Branch**: `084-5e-roll-facets`
**Created**: 2026-10-07
**Status**: Draft
**Input**: The owner, 2026-10-07: "in 5e it lets you sometimes reroll with inspiration or like the lucky feat lets you reroll 1s and 2s things like that we syhould add facets for". Then, asked how: "passive ones apply automatically; spendable ones are offered."

## Why

A 5e roll is often more than `1d20 + modifier`. A halfling rerolls a
natural 1. A fighter with Great Weapon Fighting never takes a 1 or a 2 on a
greatsword's damage die. A rogue sneaking past a sleeping guard rolls with
advantage. A player holding Heroic Inspiration spends it when the check
fails. Tables do these things every session, and today ThunderForge leaves
them to arithmetic in someone's head. The server rolls `1d20 + 5`, and the
player then says "wait, I'm a halfling" or "I'll use inspiration", and the
GM takes their word for the second number.

That breaks two things the project relies on. The server is the authority
on every roll (spec 081), and a number agreed out loud is not a roll the
server made. And 5e is a full-time system that must be ready for field
testing, where these are the rules people notice first when they are
missing.

So a character's sheet records the facets that change its rolls, and the
server applies them. A **passive facet** always applies when it can, so the
server applies it while building the formula and nobody has to remember
it. A **spendable facet** costs something, so the server offers it after
the roll, to the player who made it, as a **Reroll**. The reroll spends
the resource and records a new roll, linked to the one it replaces. The
table sees both.

## What exists

Counted on 2026-10-07:

- **The rules version.** The pack follows the SRD 5.2.1
  (`packs/systems/dnd5e/system.json:10`), which is the 2024 rules. This
  spec follows the same version throughout:
  - **Great Weapon Fighting** treats a 1 or 2 on a damage die as a 3. It
    does not reroll.
  - **Lucky** gives Luck Points equal to the proficiency bonus. A point
    buys advantage on a d20 test.
  - **Heroic Inspiration** rerolls any die, and the new roll must be used.
- **Checks.** Every declared check is `"1d20 + MODIFIER"` (`system.json:135`
  onward). `rollCheck` (`crates/thunderforge-server/src/graphql/mutations_roll_check.rs`)
  matches the id, binds the modifier, then calls `roll_and_settle` with
  `check.formula` (around `:297` to `:312`). The formula is never seen or
  chosen by the client. If the system has an adjudicator, it judges the
  result.
- **Attacks** (spec 046). `attack_formulas`
  (`crates/thunderforge-combat/src/attack.rs:57`) picks the `attack_roll`
  and `damage` effects. `part_from` (`crates/thunderforge-server/src/combat/weapon.rs`)
  turns them into a `Part`. `record_attack` (`combat/attack.rs`) rolls the
  to-hit, judges it against the target's defence and, on a hit, rolls the
  damage. Depending on auto-apply, it then offers the damage or applies it,
  all in one transaction.
- **The dice crate already speaks every facet.** It has `r{cond}` (one
  reroll), `kh`/`kl` (advantage and disadvantage) and `min{n}` (a per-die
  floor, `eval.rs` `apply_clamp`). A die's `rolls` keeps every value it
  produced, so a reroll's history is already representable. No grammar
  work is needed.
- **The sheet.**
  - `trait_data.feats` and `trait_data.traits` are free-text string arrays
    (`system.json:756-769`), and `trait_data.race` is free text, so nothing
    the server can read says "halfling".
  - Heroic Inspiration is a boolean, `inspiration` (`system.json:839`). The
    table can switch it off with the world setting `inspiration`
    (`system.json:1164`).
  - There is no Luck Points field and no rest mechanism.
- **Items carry no properties.** `world_items` has reach, range and costs,
  but nothing says Two-Handed or Versatile. That is where Great Weapon
  Fighting's condition lives.
- **The roll record** (`world_roll_records`) records who rolled, the
  formula, the detail, visibility, label and reveal (spec 081). It does not
  record which actor the roll was for.
- **Per-pack hooks.** `SystemContribution`
  (`crates/thunderforge-canvas-core/src/system_contribution.rs`) is how a
  pack adds rules without shared code naming it (spec 032, spec 066). 5e
  contributes validators, rules and content refinement, but no
  adjudicator.
- **The demo** answers checks and attacks in the browser
  (`apps/demo/src/backend/handlers/dice.ts`, `combatAttacks.ts`), mirroring
  the server.

## Facets

| Facet                     | Kind      | Applies to                                                                | What it does                                                             |
| ------------------------- | --------- | ------------------------------------------------------------------------- | ------------------------------------------------------------------------ |
| **Advantage**             | per roll  | a d20 test                                                                | `1d20` becomes `2d20kh1`                                                 |
| **Disadvantage**          | per roll  | a d20 test                                                                | `1d20` becomes `2d20kl1`                                                 |
| **Halfling Luck**         | passive   | a d20 test                                                                | a natural 1 is rerolled once and the new roll used: `r1` on the d20 term |
| **Great Weapon Fighting** | passive   | the damage dice of a melee attack with a Two-Handed weapon                | a 1 or 2 on a damage die counts as 3: `min3` on each damage dice term    |
| **Heroic Inspiration**    | spendable | your own d20 test, after the roll                                         | rerolls the lowest d20, and the new value is used                        |
| **Lucky** (a Luck Point)  | spendable | your own d20 test, after the roll, when it was not rolled at disadvantage | rolls one more d20 and keeps the highest: advantage, after the fact      |

A **d20 test** is a declared check (ability checks, skills, saving throws
and initiative, whatever the pack declares as a check) or an attack's
to-hit roll. A free formula typed into the dice roller is not one. A player
who wants advantage on a free roll types `2d20kh1`, which the formula
guide (spec 081 follow-up) already teaches.

Advantage and disadvantage together cancel, as the rules say: the roll is
a plain `1d20`.

## User Scenarios & Testing

### User Story 1 - Roll with advantage or disadvantage (Priority: P1)

The rogue hides behind a pillar, and the GM says "roll Stealth with
advantage". The player picks Advantage beside the check and rolls. The
server rolls two d20s and keeps the higher. The chat shows both dice, with
the dropped one marked, and the word "Advantage".

**Why this priority**: This is the most common modifier in 5e, and every
other facet in this spec builds on the same formula transform.

**Independent Test**: Roll a check with each of Normal, Advantage and
Disadvantage. The recorded formula, the dice and the total match
`1d20`, `2d20kh1` and `2d20kl1`.

**Acceptance Scenarios**:

1. **Given** a character's Stealth check,
   **When** the player rolls it with Advantage,
   **Then** the server records `2d20kh1 + MOD`. Both dice show in the
   chat, the lower one marked dropped, and the entry is tagged
   "Advantage".
2. **Given** an attack,
   **When** the player picks Disadvantage before attacking,
   **Then** the to-hit is `2d20kl1 + …`, and the hit or miss is judged on
   the kept die. The damage is unchanged.
3. **Given** the choice was Advantage for the last roll,
   **When** the next roll is made,
   **Then** it is Normal again. Advantage is a circumstance, not a
   setting.
4. **Given** a declared check or attack whose formula has no `1d20` term
   (homebrew content),
   **When** Advantage is asked for,
   **Then** the server refuses with a sentence saying the roll has no d20
   to roll twice. It does not silently roll it plain.

---

### User Story 2 - Halfling Luck applies itself (Priority: P1)

A halfling rolls a natural 1 on a Dexterity save. The server rerolls the
d20 on its own and uses the new value. The chat shows the 1, struck
through, beside the die that counted, and tags the entry "Halfling Luck".
Nobody had to remember.

**Why this priority**: This is the owner's first example of a passive
facet, and the one that proves the server reads facets from the sheet.

**Independent Test**: With a seeded die that lands on a 1, a halfling's
check records `1d20r1 + MOD`. The die's history is `[1, n]`, and the total
uses `n`. A character without the facet keeps the 1.

**Acceptance Scenarios**:

1. **Given** a character whose sheet has Halfling Luck,
   **When** a d20 test lands on a natural 1,
   **Then** the d20 is rerolled once, the new value is used even if it is
   also a 1, and the entry shows both values.
2. **Given** the same character rolling with Advantage,
   **When** either d20 lands on a 1,
   **Then** that die is rerolled once before the higher is kept
   (`2d20r1kh1`).
3. **Given** a damage roll or a free formula,
   **When** a 1 is rolled,
   **Then** nothing is rerolled. Halfling Luck is for d20 tests only.

---

### User Story 3 - Heroic Inspiration buys a reroll (Priority: P1)

The fighter fails an Athletics check by two. They have Heroic Inspiration.
Their own entry in the chat offers **Reroll (Inspiration)**. They click
it. The server spends their Inspiration, rerolls the d20, and records a
new roll linked to the first. Every chat shows the first roll struck
through, with the new one beneath it, and every board animates the new
roll.

**Why this priority**: The owner named it. It is the spendable facet every
5e table uses.

**Independent Test**: A character with Inspiration rolls a check, then
rerolls it. The sheet's Inspiration is false, a new roll record points at
the first, and a second reroll with Inspiration is refused.

**Acceptance Scenarios**:

1. **Given** a check rolled for a character with Heroic Inspiration,
   **When** the player who rolled it clicks Reroll (Inspiration),
   **Then**, in one transaction:
   - the character's Inspiration becomes false;
   - a new roll is recorded with `rerollOf` set to the first roll, and
     with its d20 rerolled and its other dice and modifier unchanged;
   - a `ROLL_MADE` event is recorded for it.
2. **Given** a roll with Advantage, where the d20s were 14 and 6,
   **When** it is rerolled with Inspiration,
   **Then** the 6 is the die rerolled. The lowest d20 is always the one
   rerolled, which is the best choice under advantage and under
   disadvantage alike. The keep is applied again.
3. **Given** a roll that has already been rerolled with Inspiration,
   **When** another Inspiration reroll is asked for, from any tab,
   **Then** it is refused, and nothing is spent.
4. **Given** another player's roll, or the GM looking at a player's roll,
   **When** a reroll is asked for,
   **Then** the server refuses. Only the person who made a roll may
   reroll it.
5. **Given** a table that turned the `inspiration` setting off,
   **When** a roll is made,
   **Then** no Inspiration reroll is offered, and the server refuses one.
6. **Given** a check the 5e adjudicator judged,
   **When** it is rerolled,
   **Then** the new roll is judged again, and the chat shows the new
   verdict.

---

### User Story 4 - A missed attack can be rerolled (Priority: P2)

The paladin misses by one. They spend Inspiration. The new to-hit is judged
against the same target and the same defence as the first. On a hit, the
damage is rolled and offered or applied exactly as for a first-time hit.

**Why this priority**: An attack roll is where Inspiration is spent most
often. It needs more than US3 because an attack's hit and damage are
decided in the same action.

**Independent Test**: A seeded miss, then a reroll that hits. One damage
offer exists, made by the reroll. The first attack row stays a miss and
points at its replacement.

**Acceptance Scenarios**:

1. **Given** an attack that missed,
   **When** its maker rerolls the to-hit with Inspiration or a Luck Point,
   **Then** the reroll is judged against the target and defence recorded
   with the first attack. A hit rolls damage, and offers or applies it as
   spec 046 does, with events 29 and 30.
2. **Given** an attack that hit,
   **When** a reroll is asked for,
   **Then** it is refused. A hit has already dealt or offered its damage,
   and undoing it is out of scope.
3. **Given** a reroll that misses too,
   **Then** nothing more happens, and the resource stays spent.
4. **Given** a multiattack,
   **When** one missed part is rerolled,
   **Then** only that part is rerolled, and the other parts stand.
5. **Given** the first attack spent the attacker's action,
   **When** it is rerolled,
   **Then** nothing more is spent from the action budget. The reroll is
   the same attack.

---

### User Story 5 - The Lucky feat (Priority: P2)

A character with the Lucky feat has Luck Points equal to their proficiency
bonus. After a d20 test they dislike, their entry offers **Reroll (Luck
Point)**. The server rolls one more d20 and keeps the highest, records the
new roll linked to the first, and marks one point used. The sheet shows
the points left.

**Why this priority**: The owner named it, and it is the second resource
a reroll can spend, so it proves the reroll is not hard-wired to
Inspiration.

**Independent Test**: A level 5 Lucky character has 3 points. Three Luck
rerolls on three rolls succeed, and the fourth is refused. Resetting the
used count on the sheet allows another.

**Acceptance Scenarios**:

1. **Given** a character with Lucky and points left,
   **When** they reroll their own d20 test with a Luck Point,
   **Then** one d20 is added to the d20 term, the highest is kept, the
   other dice and the modifier are unchanged, and `luck_points_used` goes
   up by one, in one transaction.
2. **Given** a roll made with Disadvantage,
   **Then** Reroll (Luck Point) is not offered, and the server refuses it.
   In the 2024 rules this makes the roll a plain d20, so the point would
   buy nothing a player could see.
3. **Given** a roll already rerolled with a Luck Point,
   **When** another Luck reroll of that same roll is asked for,
   **Then** it is refused.
4. **Given** a roll rerolled with Inspiration,
   **When** the new roll is rerolled with a Luck Point,
   **Then** it is allowed. Each resource may be spent once on a roll, and
   a reroll is a roll.
5. **Given** a character with every point used,
   **Then** no Luck reroll is offered, and the server refuses one.
6. **Given** the sheet,
   **When** the player or the GM sets the used count back to 0 (a long
   rest),
   **Then** the points are back. Nothing resets them automatically.

---

### User Story 6 - Great Weapon Fighting (Priority: P3)

A fighter with Great Weapon Fighting hits with a greatsword. Its `2d6`
damage is rolled as `2d6min3`. A rolled 1 or 2 shows as a 3, with the
rolled value beside it, and the entry is tagged "Great Weapon Fighting".

**Why this priority**: The owner named it, but it needs a new piece of
data, the weapon's Two-Handed property. It can ship after the rest.

**Independent Test**: A seeded damage roll of `[1, 5]` with a Two-Handed
melee weapon records final values `[3, 5]`. The same weapon thrown, or a
one-handed weapon, records `[1, 5]`.

**Acceptance Scenarios**:

1. **Given** a character with Great Weapon Fighting attacking in melee
   with a weapon whose properties include Two-Handed,
   **When** it hits,
   **Then** every damage dice term gets `min3`. Flat bonuses are
   untouched.
2. **Given** a ranged attack (the part has a range and no reach), or a
   weapon without Two-Handed,
   **Then** the damage is rolled as declared.
3. **Given** a weapon item,
   **When** the GM edits it,
   **Then** they can mark it Two-Handed. SRD weapons imported from the
   compendium arrive with it marked.

---

### Edge Cases

- **Facets meet.** Advantage, Halfling Luck and an attack's Great Weapon
  Fighting all apply to one attack: `2d20r1kh1 + …` to hit and
  `2d6min3 + …` for damage. The order of modifiers in the formula is the
  dice crate's, and the transform emits it in one canonical order.
- **Two tabs reroll at once.** The first spend wins. The second is refused
  by the database, because one roll can have only one reroll per resource,
  and nothing is spent twice.
- **The resource changes between the roll and the reroll.** The GM grants
  Inspiration after the roll, or takes it away. The reroll reads the sheet
  when it is asked for, not when the roll was made.
- **A hidden roll** (spec 081). A reroll keeps the original's visibility.
  A GM's eyes roll rerolled stays GM's eyes. Revealing either roll reveals
  the chain, so the table never sees a revealed roll whose replacement is
  still hidden. The masked shape gains no fields: to other players, a
  reroll of a GM's eyes roll is one more "rolled for the GM".
- **A reroll from the sheet page in another tab.** The reroll is offered
  wherever the roll's entry is shown to its maker: the chat and the
  in-pane result. The board animates it from its event, as for every roll.
- **The GM's NPCs.** An NPC's sheet can carry facets too. The GM rolling
  for it may spend its Inspiration or Luck Points.
- **A roll older than the session.** A reroll is offered only on the
  latest roll in its chain, and only for a short time after it. Planning
  sets the time, on the order of a minute or until the next roll in the
  world, so last week's failed check cannot be rerolled tonight.
- **A pause** (spec 071). A reroll is a roll. While play is paused it is
  refused, exactly like `rollCheck`.
- **Other systems.** None of this runs for a world on another system. Roll
  for Shoes and every other pack roll exactly as before.

## Requirements

### Functional Requirements

**The sheet**

- **FR-001**: 5e `trait_data` MUST gain `facets`, a list drawn from a
  fixed set of ids: `halfling_luck`, `great_weapon_fighting` and `lucky`.
  The validator MUST refuse an unknown id. `feats` and `traits` stay free
  text, for what is written rather than played.
- **FR-002**: 5e `trait_data` MUST gain `luck_points_used`, an integer of
  0 or more. The maximum is the proficiency bonus, derived and never
  stored, as every derived value is.
- **FR-003**: The 5e sheet MUST show the facets as checkboxes to whoever
  may edit the actor. It MUST show Luck Points left (max minus used), with
  a way to set the used count back to 0, when Lucky is checked. The
  existing Heroic Inspiration toggle is unchanged.

**Passive facets**

- **FR-004**: A pack MUST be able to rewrite the formula of a d20 test or
  a damage roll before it is rolled. This is a new `SystemContribution`
  slot, so that shared code names no system. Its input is:
  - the kind of roll: a check, a to-hit or damage;
  - the actor's facets;
  - the per-roll choice (Normal, Advantage or Disadvantage);
  - for damage, whether the part is melee and the weapon's properties.

  It returns the formula to roll and the names of the facets it applied.
  A system without the slot rolls its formulas unchanged.

- **FR-005**: `rollCheck` MUST call it after binding and before
  `roll_and_settle`. The attack path MUST call it for each part's to-hit
  and damage, where `record_attack` rolls them.
- **FR-006**: The 5e transform MUST act only on the formula's first `1d20`
  term for d20 tests, and only on dice terms for damage. It MUST leave
  every other term, and every binding, untouched.
- **FR-007**: `rollCheck` and the attack mutation MUST take an optional
  `advantage` argument: `NORMAL`, `ADVANTAGE` or `DISADVANTAGE`, with
  `NORMAL` as the default. A formula with no `1d20` term refuses
  `ADVANTAGE` and `DISADVANTAGE` with a sentence (US1 scenario 4).
- **FR-008**: The roll record MUST store the formula as rolled, after the
  transform. It MUST also store the facets applied, as a list of ids that
  the chat renders as tags.

**Spendable facets**

- **FR-009**: The roll record MUST gain `actor_id` (nullable), set for
  checks and attacks. A roll with no actor cannot be rerolled.
- **FR-010**: A new mutation, `rerollRoll(worldId, rollId, spend)`, with
  `spend` one of `INSPIRATION` or `LUCK_POINT`, MUST in one transaction:
  1. refuse unless the caller made the roll and still has Editor on its
     actor;
  2. refuse unless the roll is a d20 test, is the latest in its chain and
     is within the reroll window;
  3. refuse unless the resource is available (Inspiration true and the
     world's `inspiration` setting on, or Luck Points left with Lucky on
     the sheet). For a Luck Point, it also refuses a roll made with
     disadvantage;
  4. spend the resource on the actor's sheet;
  5. build the new resolution from the original:
     - Inspiration rerolls the lowest d20, appending to that die's
       `rolls`;
     - a Luck Point adds a d20 and keeps the highest;
     - then the keep is applied again and the total recomputed;
  6. record it as a new roll with `reroll_of` and `reroll_spent`, the
     original's visibility, label, actor and facets, plus the one spent;
  7. judge it as the original was judged (an adjudicated check, or an
     attack's hit or miss per US4);
  8. record a `ROLL_MADE` event for it and the actor's update event, as
     any sheet change does.
- **FR-011**: The database MUST enforce one reroll per roll per resource,
  with a unique index on `(reroll_of, reroll_spent)`. A losing concurrent
  spend then fails without spending.
- **FR-012**: Rebuilding a resolution MUST be a dice-crate function, with
  its own tests. It takes a resolution, the die to reroll or the die to
  add, and an rng. It replays the term's keep, drop and floor modifiers,
  and never re-parses the formula.
- **FR-013**: `rerollRoll` MUST be classified GATED in
  `play_pause_surface_tables.rs`.
- **FR-014**: An attack reroll MUST be refused unless every part being
  rerolled missed (US4). A reroll that hits MUST roll damage and offer or
  apply it through the same code as a first-time hit, and spend no action
  budget.

**Showing it**

- **FR-015**: `WorldRoll` MUST expose `rerollOf`, `rerolledBy` (the later
  roll, if any), `spent` and `facets`. `MaskedRoll` gains nothing.
- **FR-016**: The chat MUST show a rerolled roll struck through, joined to
  its replacement, with the resource that was spent. It MUST show each die
  that a passive facet changed with both values, the rolled and the used.
- **FR-017**: The roller's own entry MUST offer a Reroll button for each
  resource the server would accept: the actor's facets, the resource
  left, the roll's kind and the window. Nobody else sees the button. The
  server's refusal is the rule; the button is a courtesy.
- **FR-018**: The check buttons (in-pane sheet, sheet page) and the attack
  flow MUST offer Normal, Advantage and Disadvantage. The choice resets to
  Normal after every roll.

**The demo**

- **FR-019**: The demo backend MUST mirror the transform, the reroll and
  its refusals in `handlers/dice.ts` and `combatAttacks.ts`. Its tests
  MUST cover the same cases as the server's.

### Key Entities

- **Facet**: a 5e id on a sheet (`halfling_luck`,
  `great_weapon_fighting`, `lucky`), or a per-roll choice (advantage,
  disadvantage), or a resource spent (`inspiration`, `luck_point`). It is
  owned by the 5e pack. Shared code carries the ids as opaque strings.
- **Roll record** (`world_roll_records`), existing, gains:
  - `actor_id` (nullable);
  - `facets` (text array, the ids applied, empty by default);
  - `reroll_of` (nullable, the roll replaced);
  - `reroll_spent` (nullable, the resource spent);
  - a unique index on `(reroll_of, reroll_spent)`.
- **Item property**: a weapon item gains a list of the pack's properties.
  Only `two_handed` is read in this spec. It lives where planning decides
  (a column on `world_items`, or a pack-validated data field), but it is
  declared by the pack, not by shared code.

## Success Criteria

### Measurable Outcomes

- **SC-001**: Every facet in the table changes the recorded formula and
  dice exactly as stated, under a seeded rng, on the server and in the
  demo alike.
- **SC-002**: A player at a 5e table never types a facet's formula by
  hand. Advantage is one click, Halfling Luck and Great Weapon Fighting
  are zero clicks, and a reroll is one click.
- **SC-003**: Across 100 concurrent reroll attempts on one roll with one
  resource, exactly one succeeds and the resource is spent exactly once.
- **SC-004**: A reroll reaches every member's chat and board within 1 s on
  a local stack, as any roll does (spec 081 SC-001).
- **SC-005**: Worlds on every other system roll byte-for-byte the formulas
  they rolled before.

### Proof

- **Dice-crate tests** for rebuilding a resolution: reroll the lowest
  d20, add a d20 and keep the highest, with keep, drop and floor replayed.
- **Pack tests** for the 5e transform: every facet, every combination in
  the edge cases, a formula with no d20, and damage left alone on a ranged
  part.
- **Server tests** for `rollCheck` with each advantage choice and with
  Halfling Luck, and for `rerollRoll`:
  - each resource;
  - each refusal (another's roll, a spent or missing resource, the
    setting off, a hit attack, disadvantage with a Luck Point, outside the
    window, paused);
  - the unique index under concurrency;
  - a reroll of a GM's eyes roll staying masked;
  - a missed attack's reroll hitting and offering damage once.
- **Demo tests** for the same cases against its handlers.
- **An e2e slice, `pnpm e2e:rolls`**. The new specs are named `rolls-facets-*.spec.ts`,
  so the rolls slice owns them:
  - a GM and a player;
  - Advantage on a check, both dice in both chats;
  - Inspiration spent from the chat, with the sheet's toggle off
    afterwards, the struck-through roll in the GM's chat, and a second
    reroll refused;
  - Halfling Luck shown on a seeded 1;
  - a missed attack rerolled.

## Assumptions

- **Rules version.** The SRD 5.2.1 (2024), because the pack is. A table on
  2014 rules would see Great Weapon Fighting floor rather than reroll, and
  Lucky act as advantage rather than as a third die to choose from. Another
  version is a later spec, if anyone asks.
- **Conditions do not grant advantage on their own in this spec.**
  Poisoned, Prone, Restrained, an invisible attacker and the like are
  applied by the player or GM picking the choice at roll time. Feeding
  conditions into the same transform is the natural next spec. The
  transform takes the choice as an input, so a condition can supply it
  later without changing the reroll or the record.
- **Lucky's other half** (spending a point to impose disadvantage on an
  attack against you) needs a prompt during someone else's roll. It is out
  of scope.
- **Heroic Inspiration on damage.** The 2024 rule allows rerolling any
  die, but this spec offers it on d20 tests only. Damage is applied in the
  same action as the hit, and rerolling it would mean undoing an applied
  change.
- **Versatile weapons** do not trigger Great Weapon Fighting in this spec.
  That needs "held in two hands" at attack time, which nothing records yet.
- **No rest mechanism.** Luck Points are reset by hand on the sheet. A
  long rest button belongs to a spec about rests.
- **The reroll window** is decided in planning, measured against how long
  a table takes to decide. A roll's maker sees the button only while the
  server would accept it.
