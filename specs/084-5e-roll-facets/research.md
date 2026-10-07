# Research: 5e Roll Facets

Each decision below was made against the code as it stood on 2026-10-07.

## R1. The pack's hook is one new `SystemContribution` slot

**Decision**: `SystemContribution` gains
`roll_facets: Option<&'static RollFacets>`. `RollFacets` is a struct of
plain functions and label tables, declared in a new
`crates/thunderforge-canvas-core/src/roll_facets.rs`:

- `shape(&ShapeInput) -> Result<Option<Shaped>, String>` rewrites a formula
  before it is rolled. `Ok(None)` means "roll it as declared".
- `reroll(&RerollInput) -> Result<RerollPlan, String>` decides whether a
  spend is allowed, what it does to the dice, and what the sheet looks like
  after it.
- `spends: &'static [FacetLabel]` lists the spendable ids in the order the
  chat offers them (`inspiration`, `luck_point`).
- `labels: &'static [FacetLabel]` names every id the pack can put on a roll,
  so the server returns `{ id, label }` and the web never names a 5e facet.

`SystemContribution::new` sets it to `None`. 5e registers it in
`packs/systems/dnd5e/server/src/lib.rs` beside its validators.

**Why**: shared code must never name a system (specs 032 and 066). One slot
holding both halves keeps the passive and spendable rules in the same pack
file, `roll_facets.rs`, where one test file proves them together. A system
without the slot never reaches either function, so its formulas are
byte-for-byte unchanged (SC-005).

**Alternatives rejected**:

- An adjudicator-style `fn` per behaviour (three new slots). This spreads one
  rule set over three registrations, and each slot would need its own
  `None` path in shared code.
- Facets declared as data in `system.json` and interpreted by shared code.
  Shared code would then know what "reroll the lowest d20" means for one
  system. That is a rules engine, and it is out of scope.

## R2. The formula is rewritten through the dice crate's AST, not by string edits

**Decision**: the dice crate gains `rewrite.rs`, a public
`rewrite_dice_terms(formula, edit) -> Result<String, FormulaError>`. It
parses the formula, shows each dice term to `edit` as a `TermView`, applies
the returned `TermEdit`, and prints the AST back. A `TermView` has the term's
index, its literal count, its literal sides and whether it already has
modifiers. A `TermEdit` holds a new count and the modifiers to add. The
printer emits modifiers in one canonical order, which is the order `eval.rs`
applies them in: rerolls, then keep and drop, then clamps (`2d20r1kh1`,
`2d6min3`). Its tests prove `parse(print(ast)) == ast` for every modifier.

The 5e pack gains a dependency on `thunderforge-dice` for this. The dice crate
is pure Rust and already builds for wasm32.

The 5e transform:

| Roll   | Input                   | Rewrite of the first `1d20`/`Nd20` term with no keep or drop                                                                      |
| ------ | ----------------------- | --------------------------------------------------------------------------------------------------------------------------------- |
| d20    | Advantage               | count 2, `kh1`                                                                                                                    |
| d20    | Disadvantage            | count 2, `kl1`                                                                                                                    |
| d20    | both                    | cancelled, no change (and neither id is recorded)                                                                                 |
| d20    | `halfling_luck`         | `r1`                                                                                                                              |
| damage | `great_weapon_fighting` | `min3` on every dice term, when the part is melee (R6) and the weapon's properties include `two_handed`                           |
| d20    | a formula with no d20   | Advantage or Disadvantage is refused: "This roll has no d20 to roll twice." Halfling Luck alone does nothing and records nothing. |

`Shaped` carries the new formula and the facet ids applied, including
`advantage` and `disadvantage` when they changed the roll.

**Why**: the AST is private (`mod ast` in `lib.rs`), and a string edit on
`"1d20 + MODIFIER"` breaks on the first homebrew formula with spaces, a
`1d20` inside a placeholder name, or an existing modifier. A printer is also
what R3 needs.

**Alternatives rejected**:

- Make `ast` public and let the pack edit it. That exposes every grammar
  change as a breaking API change for packs.
- Expose source spans from the parser and splice the string. The parser keeps
  no spans, and splicing still has to know the modifier order.

## R3. A reroll is a replay of the recorded dice through the formula

**Decision**: the dice crate gains `replay.rs`:

```rust
pub fn replay<R: Rng>(
    original: Recorded<'_>,        // formula, bindings, resolution, all as stored
    reshaped: Option<&str>,        // a new formula with the same terms, or None
    edit: ReplayEdit,              // None | RerollDie(index)
    rng: &mut R,
) -> Result<RollResolution, FormulaError>
```

It parses the formula (the reshaped one, or else the original) and evaluates
it with an `EvalCtx` whose die draws come from a queue rather than from the
rng. The queue is the original's `DieOutcome`s, grouped by term. The groups
are recovered by parsing the original formula and counting each term's dice,
because `eval_dice_term` pushes one outcome per die in draw order.

- **A recorded die** keeps its whole `rolls` chain: rerolls and explosions
  are copied, never redrawn. Then the term's keep, drop and clamp are applied
  again, exactly as `eval.rs` applies them.
- **`RerollDie(i)`** (Heroic Inspiration) copies die `i`'s chain, appends one
  fresh face from the rng and makes it the die's value. The term's
  reroll and explode modifiers do not apply to the new face, because the
  rule says the new roll must be used. So a halfling's Inspiration reroll of
  a 1 stays a 1.
- **A reshaped term with more dice** (a Luck Point: `1d20` becomes
  `2d20kh1`, and `2d20r1kh1` becomes `3d20r1kh1`) takes its recorded dice
  first, then draws the extra ones fresh. A fresh die gets the term's own
  modifiers, so Halfling Luck rerolls a 1 on a Luck die too.
- Every other term is copied, so the damage and the modifier never change.
- Terms that do not line up (a different term count, or a term with fewer
  dice than recorded) are an error. Only the server's own transform produces
  a reshaped formula, so this is a bug guard, not a user path.

Which die Inspiration rerolls is decided by the server's shared code from
the pack's plan, `RerollEdit::RerollLowest { sides: 20 }`: the die with the
lowest `final_value` among the `Numeric(20)` dice, taking the first on a
tie.

**Spec correction**: FR-012 said the rebuild "never re-parses the formula".
It must, because the resolution's dice list is flat and keeps no term
structure. What it parses is the formula the server itself rolled and stored,
never client input, so the reason behind the original wording still holds.

**Coordination with spec 083**: 083 adds `DieOutcome.steps`, one
`ChainStep` per value in `rolls`. `replay` copies `steps` with `rolls` and
pushes `ChainStep::Reroll` beside the Inspiration face. Whichever spec lands
second adds that line. Its test is listed in tasks.md (T012).

**Alternatives rejected**:

- Reroll by rolling the whole formula again. That redraws the damage and the
  other d20, which the rules do not allow.
- Edit the stored JSON directly: replace one `final_value` and recompute the
  total. Keep and clamp would then have to be reimplemented outside the
  crate, and a success-count formula would be totalled wrongly.

## R4. The roll record says what the roll was for

**Decision**: `world_roll_records` gains:

- `actor_id`;
- `roll_kind`, one of `check`, `to_hit` or `damage`, or null for a free
  formula;
- `check_id`, the declared check's id when `roll_kind = 'check'`;
- `facets TEXT[] NOT NULL DEFAULT '{}'`;
- `reroll_of`;
- `reroll_spent`.

An attack's roll is found from `world_attacks.to_hit_roll_id`, which already
exists. The roll does not need to point back at the attack.

**Why**: re-judging needs the source. A check is re-judged by calling
`judge_check` with its `check_id`. A to-hit is re-judged against its attack
row (R7). Only `check` and `to_hit` are d20 tests, and so only they can be
rerolled.

**Alternative rejected**: a `source` JSONB. It cannot be checked by the
database, and every reader would parse it.

## R5. The reroll window is two minutes, on the latest roll in its chain

**Decision**: a roll can be rerolled until two minutes after its
`created_at`, and only while nothing has rerolled it. The constant is
`REROLL_WINDOW` in `crates/thunderforge-server/src/rolls/reroll.rs`.
`WorldRoll.rerollUntil` sends the deadline, so the button disappears when
the server would refuse.

**Why**: a table decides about a failed check within a few seconds, and it
can take most of a minute if the GM is still describing what happened. Two
minutes covers that with room to spare. It is also short enough that
nothing from an earlier scene, or from last session, can be rerolled.

**Alternatives rejected**:

- "Until the next roll in the world." At a busy table another player's roll
  lands within seconds and steals the window. Measuring "next roll by the
  same actor" fails for a GM rolling for several NPCs.
- One minute. That is too tight once the GM is narrating.

## R6. The weapon's properties are a column, declared by the pack

**Decision**:

- `world_items` gains `properties TEXT[] NOT NULL DEFAULT '{}'`.
- The pack declares its vocabulary in `system.json`:
  `"itemProperties": [{ "id": "two_handed", "label": "Two-Handed" }, …]`.
  5e declares the 2024 weapon properties: Ammunition, Finesse, Heavy, Light,
  Loading, Reach, Thrown, Two-Handed and Versatile. Only `two_handed` is read
  in this spec.
- `setItemAttack` (`AttackFieldsInput`) gains `properties`. The server
  refuses an id the world's pack does not declare.
- `Part` (`combat/weapon.rs`) carries the item's properties. An ability's
  part carries none.

A part is **melee** when it has a reach and the target, as measured at
attack time (`Measured.distance`), is within it. So a thrown greatsword at
range is not melee, and Great Weapon Fighting does not apply to it.

**Why**: `world_items` has no data JSON column, only typed columns
(`reach`, `range_normal`, `action_cost`…). A text array matches
`multiattack`, the existing list column. The vocabulary is the pack's, as
the spec asks, and shared code only stores and checks ids.

**Spec correction**: US6 scenario 3 says SRD weapons "imported from the
compendium arrive with it marked". No weapon compendium exists. Nothing in
the repository lists SRD weapons, and `srd.rs` holds skills, classes and
spell slots only. The scenario now says the GM marks the property in the
item's attack fields. Importing SRD weapons belongs to a compendium spec.

**Alternatives rejected**:

- A `data JSONB` on items, validated by the pack. That is a new validator
  slot and a sheet for items, which is far more than one property needs.
- Reusing `world_item_effects`. Effects are formulas with targets, not tags.

## R7. A missed attack's reroll is a new attack row judged against the old one

**Decision**:

1. `rerollRoll` on a `to_hit` roll finds the attack whose `to_hit_roll_id`
   is that roll. It refuses unless the attack's `outcome` is a miss.
2. The new to-hit is judged with `judge(target.is_some(), attack.defence,
total)`, using the defence stored on that row. It does not read the
   target's current AC.
3. A new `world_attacks` row copies the target, labels, distance, flags,
   `action_cost`, combat and multiattack parent. It has the new
   `to_hit_roll_id` and `reroll_of = <old attack id>`, and it emits
   `ATTACK_MADE` (29).
4. On a hit, damage is rolled from the part's current declaration, which is
   the same `parts_of` and `part_from` the attack used. It is shaped by the
   facets (R2, so Great Weapon Fighting applies), recorded, and then offered
   or applied through the code `record_attack` uses.
5. Nothing is spent from the action budget. `spend_for_attack` is not
   called.

To share step 4 without growing `attack.rs` (830 lines), the per-part
"hit → damage → offer or auto-apply" block moves out of `record_attack`
into `combat/attack_hit.rs` as `settle_hit`. `record_attack` and the reroll
both call it. The reroll itself lives in `combat/attack_reroll.rs`.

`world_attacks` gains `reroll_of UUID NULL` with a unique index. The first
row stays a miss. The attack feed shows the replacement under it.

**Why**: the reroll is the same attack (US4.5), so it keeps the defence it
was judged against. A GM who changes the AC between the roll and the reroll
does not change what the first roll needed. A new row, rather than an
update, keeps the audit trail ("store every mutation") and the first roll's
verdict.

**Alternative rejected**: update the old attack row in place. That loses the
miss that the chain shows struck through, and an `ATTACK_MADE` consumer
would see a row change outcome under it.

## R8. One replacement per roll, and one use of each resource per chain

**Decision**:

- `reroll_of` is **unique** on its own, so a roll has at most one
  replacement whatever was spent. That rule is "the latest in its chain".
- Each resource may be spent once per chain (US5.3 and US5.4). The rule is
  checked in the transaction by walking `reroll_of` back to the first roll.
  A chain has at most three rolls.
- The transaction first takes `SELECT … FOR UPDATE` on the rolled row, then
  the actor's `world_actor_system_data` row, in that order. Two tabs then
  serialise, and the loser sees the replacement and is refused. The unique
  index is the backstop: a violation maps to the same refusal, and the
  transaction rolls back, so nothing is spent.

**Spec correction**: FR-011's unique `(reroll_of, reroll_spent)` would let
Inspiration and a Luck Point both replace the same roll, which forks the
chain. US5.4 only needs "Luck on the _new_ roll". The index is now on
`reroll_of` alone.

## R9. The spend is a sheet write by the pack's rules, through shared code

**Decision**: the 5e `reroll` function receives:

- the actor's `trait_data` and proficiency inputs (`level`, or the
  challenge rating for an NPC);
- the world's effective system settings;
- the roll's `facets` and `roll_kind`;
- the spend id.

It returns either a refusal sentence or a `RerollPlan`, which holds:

- the edit (`RerollLowest { sides: 20 }` or `Reshape { formula }`);
- the new `trait_data`;
- the facet id to record.

Shared code then validates the new `trait_data` through the pack's own
`trait_data` validator, writes it, and records `ACTOR_SHEET_CHANGED` (26)
with the payload `updateActorSystemData` writes. The 5e rules:

- `inspiration`: refused unless `trait_data.inspiration` is true and the
  world setting `inspiration` is not false. Sets it to false.
- `luck_point`: refused unless `facets` has `lucky` and `luck_points_used` is
  below the proficiency bonus (`rules::proficiency_bonus`). Also refused if
  the roll's facets include `disadvantage`. Adds one to `luck_points_used`.

`WorldRoll.rerollOffers` calls the same function with a dry flag for the
roll's maker only, so the button and the refusal cannot disagree.

**Why**: the resource lives on the sheet the pack owns. The write goes
through the pack's validator like any other sheet write, and through the
same event, so open sheets update.

## R10. A reroll keeps the original's visibility, and a reveal reveals the chain

**Decision**: the reroll copies `visibility` and `label`. `revealRoll` on
any roll in a chain sets `revealed_at` and `revealed_by` on every roll in the
chain, in one statement, and emits `ROLL_REVEALED` for each. `view_of`
(`rolls/visibility.rs`) is unchanged: a reroll is a roll. `MaskedRoll`
gains nothing.

**Why**: FR edge case "A hidden roll". Revealing one roll and not its
replacement would show the table a number that was not the final one.

## R11. Advantage reaches the server as an argument, and resets on the client

**Decision**: `rollCheck(…, advantage: Advantage = NORMAL)` and
`AttackInput.advantage: Advantage = NORMAL`. The enum is
`NORMAL | ADVANTAGE | DISADVANTAGE`. It is shared vocabulary: any d20 system
can read it, and the pack decides what it means. The web's choice is local
state in `CharacterRollButtons`, set back to `NORMAL` after each roll is
sent (FR-018).

## R12. The demo mirrors the transform in TypeScript and replays through wasm

**Decision**:

- `apps/demo/src/backend/handlers/facets.ts` mirrors the 5e transform for
  the demo's own formulas. The demo's checks are `1d20 ± n`, and its attacks
  are its seeded abilities, so string building is enough there.
- The replay comes from the crate itself: `crates/thunderforge-dice/src/wasm.rs`
  gains `replayRoll(formula, bindings, detail, reshaped, rerollDie, seed)`.
- The handler tests use `seedDice` to land a 1 and a miss.

**Why**: the replay is the delicate half, and running the crate's own code
keeps the demo honest. The transform on the demo's fixed formulas is a few
lines.

## R13. The e2e cannot seed the server's dice

**Decision**: the server rolls from `StdRng::from_rng(&mut rand::rng())`
(`mutations_roll.rs:328`, `mutations_attacks.rs:333`) and has no test seed.
The e2e specs therefore assert what is deterministic:

- the formula as rolled (`r1`, `2d20kh1`);
- the facet tags;
- the chain and the spent resource;
- the sheet's toggle after a spend;
- the refusal of a second spend.

A missed attack is made certain with a target whose AC is 99. Seeded cases
(a natural 1, a reroll that hits) are proven in the server and demo tests.

**Spec correction**: the Proof's "Halfling Luck shown on a seeded 1" becomes
"Halfling Luck shown on the formula and tag". The seeded 1 is in the server
and demo tests.

## R14. 5e has no adjudicator, so re-judging a check is generic

**Decision**: `rerollRoll` on a `check` roll calls `judge_check` when the
world's system has an adjudicator, exactly as `rollCheck` does. 5e
registers none, so a 5e check reroll records no outcome, as its first roll
did not.

**Spec correction**: US3.6 said "a check the 5e adjudicator judged". It
now says "a check its system's adjudicator judged".

## R15. Spends are opaque strings in GraphQL

**Decision**: `rerollRoll(worldId, rollId, spend: String!)`, where `spend`
is one of the pack's `spends` ids. It is not a GraphQL enum.

**Why**: an enum with `INSPIRATION` and `LUCK_POINT` would put 5e
vocabulary in the shared schema. The spec itself says shared code carries
these ids as opaque strings. **Spec correction** to FR-010.
