# Contract: the pack's roll-facets slot and the dice crate's rewrite and replay

## Canvas core: `crates/thunderforge-canvas-core/src/roll_facets.rs`

Plain data and function pointers. No dice, Diesel or network types, so
canvas core stays wasm-clean.

```rust
pub struct FacetLabel { pub id: &'static str, pub label: &'static str }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Advantage { Normal, Advantage, Disadvantage }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RollKind { Check, ToHit, Damage }

pub struct ShapeInput<'a> {
    pub kind: RollKind,
    pub formula: &'a str,                 // as declared, placeholders unbound
    pub trait_data: &'a serde_json::Value, // the actor's, or Null for none
    pub advantage: Advantage,              // Normal for Damage
    pub melee: bool,                       // Damage only (research R6)
    pub item_properties: &'a [String],     // Damage only
}

pub struct Shaped { pub formula: String, pub facets: Vec<String> }

pub struct RerollInput<'a> {
    pub spend: &'a str,
    pub kind: RollKind,                    // Check or ToHit
    pub formula: &'a str,                  // as rolled
    pub facets: &'a [String],              // the roll's
    pub actor_name: &'a str,
    pub sheet: &'a serde_json::Value,      // every slot of the actor's system data, keyed by slot name
    pub settings: &'a serde_json::Value,   // the world's effective system settings, by id
}

pub enum RerollEdit {
    /// Reroll the die with the lowest final value among dice of these sides.
    RerollLowest { sides: u32 },
    /// Roll this formula instead, replaying the recorded dice into it.
    Reshape { formula: String },
}

pub struct RerollPlan {
    pub edit: RerollEdit,
    pub trait_data: serde_json::Value,     // the sheet after the spend
}

pub type ShapeFn = fn(&ShapeInput<'_>) -> Result<Option<Shaped>, String>;
pub type RerollFn = fn(&RerollInput<'_>) -> Result<RerollPlan, String>;

pub struct RollFacets {
    pub shape: ShapeFn,
    pub reroll: RerollFn,
    pub spends: &'static [FacetLabel],
    pub labels: &'static [FacetLabel],
}
```

The host hands over the whole sheet (`trait_data`, `ability_data`,
`resource_data`, …) rather than a proficiency bonus. The pack works the
bonus out itself with `rules::proficiency_bonus` (from `trait_data.level`) or
`rules::proficiency_bonus_for_challenge` (for an NPC). The host never learns
what a proficiency bonus is. `RerollPlan.trait_data` is the only slot a
spend may write.

`SystemContribution` gains `pub roll_facets: Option<&'static RollFacets>`,
which `new` sets to `None`. Every `SystemContribution { … }` literal in the
tests and the Roll for Shoes pack picks the field up through `new(..)` or
gets `roll_facets: None` added.

### What the host does with it

`crates/thunderforge-server/src/rolls/facets.rs`:

```rust
/// The formula to roll and the facets it carries. A system without the slot,
/// or `Ok(None)` from it, rolls `formula` untouched.
pub fn shape_roll(system_id: &str, input: ShapeInput<'_>) -> Result<Shaped, String>;
pub fn facet_labels(system_id: &str, ids: &[String]) -> Vec<(String, String)>;
```

With no slot, `Advantage::Normal` gives `Shaped { formula, facets: [] }`
and anything else is refused (contracts/graphql-rolls-facets.md).

## 5e: `packs/systems/dnd5e/server/src/roll_facets.rs`

- `shape`:
  - d20 tests (`Check`, `ToHit`) get advantage or disadvantage and
    `halfling_luck`;
  - `Damage` gets `great_weapon_fighting` when `melee` holds and
    `item_properties` contains `two_handed`;
  - a `Damage` roll never takes advantage, so a choice other than `Normal`
    is a host bug and is refused.
- `reroll`: the rules in research R9.
  - `inspiration` returns `RerollLowest { sides: 20 }`.
  - `luck_point` returns `Reshape { formula: add_one_d20(formula) }`, where
    `add_one_d20` is the same `rewrite_dice_terms` call: count + 1 and
    `kh1` on the first d20 term.
- `spends`: `inspiration` "Heroic Inspiration", then `luck_point` "Luck
  Point".
- `labels`: every id in data-model.md's facet table.

## Dice crate: `crates/thunderforge-dice/src/rewrite.rs`

```rust
pub struct TermView { pub index: usize, pub count: Option<u32>, pub sides: Option<u32>,
                      pub keeps: bool, pub rerolls: bool, pub clamps: bool }

pub enum AddModifier { KeepHighest(u32), KeepLowest(u32), RerollOnceEq(i64), Min(i64) }

pub struct TermEdit { pub count: Option<u32>, pub add: Vec<AddModifier> }

pub fn rewrite_dice_terms(
    formula: &str,
    edit: impl FnMut(&TermView) -> Option<TermEdit>,
) -> Result<String, FormulaError>;
```

- It returns the formula **unchanged, byte for byte**, when `edit` returns
  `None` for every term.
- It prints the canonical modifier order: rerolls, then keep and drop, then
  clamps.
- Placeholders, numbers and operators print as written, normalised to single
  spaces around binary operators.

## Dice crate: `crates/thunderforge-dice/src/replay.rs`

```rust
pub struct Recorded<'a> {
    pub formula: &'a str,
    pub bindings: &'a PlaceholderBindings,
    pub resolution: &'a RollResolution,
}

pub enum ReplayEdit { None, RerollDie(usize) }

pub fn lowest_die(resolution: &RollResolution, sides: u32) -> Option<usize>;

pub fn replay<R: Rng>(
    original: Recorded<'_>,
    reshaped: Option<&str>,
    edit: ReplayEdit,
    rng: &mut R,
) -> Result<RollResolution, FormulaError>;
```

The guarantees, each a test in `replay_tests.rs`:

1. `replay(r, None, ReplayEdit::None, _)` equals `r.resolution`, for every
   formula in the crate's existing test corpus. Nothing is drawn: the test
   uses an rng that panics.
2. `RerollDie(i)` changes only die `i`. Its `rolls` is the old chain plus
   one value, and its `final_value` is that value. Keep and clamp are then
   reapplied: under `2d20kh1` the keep can move to the other die, and under
   `min3` a new 1 reads 3.
3. The new face under `r1` is not rerolled again.
4. `reshaped = "2d20kh1 + 5"` from `1d20 + 5` keeps die 0's chain, draws die
   1 fresh, and keeps the higher. From `2d20r1kh1` to `3d20r1kh1`, the fresh
   die is rerolled on a 1.
5. A reshaped formula whose terms do not line up is an error.
6. Spec 083: when `DieOutcome.steps` exists, it is copied, and the
   Inspiration face pushes `ChainStep::Reroll`.

## Wasm: `crates/thunderforge-dice/src/wasm.rs`

```rust
#[wasm_bindgen(js_name = replayRoll)]
pub fn replay_roll(formula: &str, bindings: &str, detail: &str,
                   reshaped: Option<String>, reroll_die: Option<u32>, seed: &[u32])
    -> Result<String, JsError>;
#[wasm_bindgen(js_name = lowestDie)]
pub fn lowest_die_js(detail: &str, sides: u32) -> Option<u32>;
```

It is the demo's only path to a reroll (research R12).
