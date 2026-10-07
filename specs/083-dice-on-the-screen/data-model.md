# Data Model: Dice on the Screen

Nothing new is stored in a table. Two shapes grow:

- the stored JSON of a die, which gains `steps`;
- one GraphQL type, which gains `bindings`.

Everything else is derived, or lives only in the engine.

## Stored: `DieOutcome` (crates/thunderforge-dice/src/lib.rs)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ChainStep {
    Reroll,
    Explode,
}

pub struct DieOutcome {
    pub sides: DieSides,
    pub rolls: Vec<i64>,
    /// Why each value after the first was rolled: `steps[i]` explains
    /// `rolls[i + 1]`. Empty on a resolution stored before spec 083.
    #[serde(default)]
    pub steps: Vec<ChainStep>,
    pub kept: bool,
    pub final_value: i64,
}
```

- **Invariant.** Either `steps.len() == rolls.len() - 1`, or `steps` is
  empty because the die was stored before this spec.
- **Clamped.** Derived: `final_value != *rolls.last()`.
- **Where it is stored.** Inside `world_roll_records.detail` (JSONB), as
  serde writes it. Old rows have no `steps` key, and read as empty. No
  migration is needed.

## Exposed: GraphQL

```graphql
enum DieStep {
  REROLL
  EXPLODE
}

type GraphQLDieOutcome {
  # … existing fields
  "Why each value after the first was rolled. Empty for rolls stored before spec 083."
  steps: [DieStep!]!
}

type RollBinding {
  placeholder: String!
  value: Float!
}

type WorldRoll {
  # … existing fields
  "The values the server substituted for the formula's placeholders."
  bindings: [RollBinding!]!
}
```

- `bindings` comes from `world_roll_records.bindings`, a nullable JSONB
  object of name to number. It is written by `mutations_roll_check.rs:297-313`
  and `combat/attack.rs:268-271`. A null or non-object value reads as `[]`.
  The list is sorted by placeholder, so the output is stable.
- `MaskedRoll` is unchanged. It still cannot be built from a row.

## Derived: `Breakdown` (crates/thunderforge-dice/src/breakdown.rs)

```rust
pub enum Breakdown {
    /// A sum or difference of dice terms and constants.
    Sum(Vec<Addend>),
    /// A success-count roll: for each die in resolution order, whether it
    /// succeeded.
    Successes(Vec<bool>),
}

pub struct Addend {
    pub negative: bool,
    pub kind: AddendKind,
}

pub enum AddendKind {
    /// A kept die, by its index in `RollResolution::dice`.
    Die { index: usize, value: i64 },
    /// A literal, or a substituted placeholder (with its name).
    Constant { value: f64, placeholder: Option<String> },
}

pub fn breakdown(
    formula: &str,
    resolution: &RollResolution,
    bindings: &PlaceholderBindings,
) -> Option<Breakdown>;
```

The function returns `None` for any of these:

- a pool;
- `*` or `/`;
- a math function;
- a dice count that is not a number or a bound placeholder;
- a resolution whose dice do not match the formula's terms in number;
- a missing binding.

It returns `Successes` for a formula whose only dice term carries `cs` or
`cf`.

A die that keep/drop dropped is not an addend. Its index is skipped.

## Derived: the readout (crates/thunderforge-canvas-core/src/dice_throw/readout.rs)

```rust
pub fn readout(throw: &ThrowSpec) -> String;  // ASCII only (research R6)
pub fn chip(drawn: usize, total_dice: usize) -> Option<String>;  // "+20 more"
```

- **Prefix.** `"{roller}: {label}   "`, or `"{roller}   "` when there is no
  label.
- **`Sum`.**
  - The first addend prints bare. Each later addend prints as `+ v` or
    `- v`, and a negative constant flips its sign, so a bonus of `-1` reads
    `- 1`, never `+ -1`.
  - Then ` = total`.
  - If the addends do not add up to the server's total, the text falls back
    to the `formula = total` form below.
- **`Successes`.** `N successes`, or `1 success`.
- **`None`.** `formula = total`. Each placeholder in the formula is replaced
  by its bound value, matched as a whole identifier.
- **Totals.** Printed as integers when they are whole, and with up to two
  decimals otherwise.

## Engine-local: Throw

| Field                 | Source                                                                   |
| --------------------- | ------------------------------------------------------------------------ |
| `roll_id`             | payload                                                                  |
| `seed`                | `fnv1a64(roll_id)` (research R3)                                         |
| `dice: Vec<ThrowDie>` | one per drawn die, the first 20 dice of the resolution, expanded (below) |
| `hidden_count`        | dice past the 20th                                                       |
| `readout`, `chip`     | `readout()`, `chip()`                                                    |
| `state`               | `Waiting` → `Tumbling` → `Landed` → `Fading` → despawned; or `Skipped`   |
| `started_at`          | engine time when it left `Waiting`                                       |

`ThrowDie`:

| Field          | Meaning                                                                                                                                         |
| -------------- | ----------------------------------------------------------------------------------------------------------------------------------------------- |
| `shape`        | `ShapeKind`, from the die's sides (table below)                                                                                                 |
| `outcome`      | the index of the source `DieOutcome`                                                                                                            |
| `segments`     | the faces it lands on in turn. A reroll segment is struck when the next one starts. A clamped final value adds a struck segment with no tumble. |
| `explosion_of` | `Some(outcome index)` for a die added by an explosion step                                                                                      |
| `kept`         | `false` dims it                                                                                                                                 |
| `succeeded`    | `Some(bool)` when the breakdown is `Successes`; `false` dims it                                                                                 |
| `rest`         | resting place in screen pixels, from the seed                                                                                                   |

Expanding a `DieOutcome` into drawn dice:

1. The die lands `rolls[0]`.
2. For each step:
   - **`Reroll`**: struck through, then another segment on the same die.
   - **`Explode`**: a new die of the same shape, which lands that value.
3. An empty `steps` treats every step as `Reroll`.
4. A clamp adds a final segment showing `final_value`, with the previous
   value struck.

The 20-die cap counts drawn dice, explosions included.

`ShapeKind` from `DieSides`:

| Sides                  | Shape                             | Face labels                                                 |
| ---------------------- | --------------------------------- | ----------------------------------------------------------- |
| `Numeric(4/6/8/12/20)` | Tetra, Cube, Octa, Dodeca, Icosa  | 1 to n                                                      |
| `Numeric(10)`          | Trapezohedron                     | `1` to `9`, and `0` for 10                                  |
| `Numeric(100)`         | Two trapezohedra (tens and units) | tens `00` to `90`; units `0` to `9`. 100 reads `00` and `0` |
| `Numeric(3)`           | Cube                              | 1, 2, 3, 1, 2, 3                                            |
| `Fate`                 | Cube                              | `+`, `+`, blank, blank, `-`, `-` (value 1, 1, 0, 0, -1, -1) |
| `Coin`, `Numeric(2)`   | Disc                              | `H`/`T`, or `1`/`2`                                         |
| `Numeric(other)`       | Disc                              | the landed value on its front face                          |

## Engine-local: Queue

`ThrowQueue { playing: Option<Throw>, waiting: VecDeque<Throw> (≤ 4), fading: Vec<Throw> }`.
Its operations:

- `push(throw) -> Option<Throw /* skipped */>`
- `landed()`
- `fade_done(roll_id)`

## Engine-local: landed log (probe)

Up to 50 `LandedThrow` entries, the oldest dropped first. The fields are in
contracts/engine-dice.md.
