# Phase 1 Data Model: Dice Rolling Engine

Two distinct layers: (1) in-memory/wire types owned by `crates/thunderforge-dice` (no database involvement — pure Rust structs, `serde`-serializable so they cross the GraphQL boundary as-is), and (2) the one new persisted table in `src/server` (Diesel/Postgres) that durably records a resolved roll (FR-014).

## Crate types (`crates/thunderforge-dice`, in-memory / wire-format only)

### `DiceFormula`

A parsed, validated representation of a formula string (research.md §2). Not persisted directly — a `world_item_effects.formula` (spec 013) or any other formula-bearing field stays a plain string in storage; parsing happens at resolution time. Exposed for callers that want to validate a formula (FR-011) without resolving it (e.g. the frontend validating an Item Effect's formula as it's typed, per spec 013's `addItemEffect`).

| Field | Type | Notes |
|---|---|---|
| `source` | `String` | The original formula text, retained for display/audit |
| `ast` | internal `ast::Expr` | Not exposed across the GraphQL boundary; internal to the crate |

**Validation rules**: Constructing a `DiceFormula` (via `DiceFormula::parse(source: &str) -> Result<Self, FormulaError>`) is the only way to obtain one — an unparseable string never produces a value, satisfying FR-011.

### `PlaceholderBindings`

A caller-supplied map from placeholder name to numeric value, substituted into the formula before evaluation (User Story 3, FR-010).

| Field | Type | Notes |
|---|---|---|
| (map) | `HashMap<String, f64>` | e.g. `{"STAT": 3.0, "MODIFIERS": 1.0}` for a formula like `1d20 + STAT + MODIFIERS` |

**Validation rules**: `resolve()` returns `FormulaError::MissingPlaceholder(name)` if the formula references a name absent from the map — never defaults to 0 (FR-010).

### `RollResolution`

The result of one `resolve()` call — the crate's core output type, and the shape returned by the `rollDice` GraphQL mutation (contracts/graphql-roll.md) and persisted (denormalized to JSON) in `world_roll_records.detail`.

| Field | Type | Notes |
|---|---|---|
| `formula` | `String` | Echoes the resolved formula (post-placeholder-substitution, for audit clarity) |
| `dice` | `Vec<DieOutcome>` | Every individual die actually rolled, including every reroll/explosion (FR-013) |
| `kind` | `ResolutionKind` enum: `Total(f64)` \| `SuccessCount(i64)` | A summed-total result (most formulas) or a success/failure count (dice-pool formulas, FR-008) — the shape depends on the formula's own notation, not a caller's expectation (per spec.md Edge Cases) |

### `DieOutcome`

One individual die's full history within a resolution — the unit the presentation/animation layer (User Story 4) renders.

| Field | Type | Notes |
|---|---|---|
| `sides` | `DieSides` enum: `Numeric(u32)` \| `Fate` \| `Coin` | What kind of die this was |
| `rolls` | `Vec<i64>` | Every value this die produced, in order — index 0 is the original roll, subsequent entries are rerolls/explosions of *this* die (FR-013's "full chain of rolls... not just the final kept value") |
| `kept` | `bool` | Whether this die's final value contributed to the aggregated result (false for a die dropped by a keep/drop modifier, per User Story 2 Acceptance Scenario 2 — dropped dice are shown, not hidden) |
| `final_value` | `i64` | The value that actually counted (last entry of `rolls`, or the value after min/max clamping) |

### `FormulaError`

| Variant | When |
|---|---|
| `ParseError { message, position }` | Malformed syntax (unbalanced grouping, unknown modifier, bad condition) — FR-011 |
| `DivisionByZero` | An arithmetic sub-expression divides by zero — FR-011 |
| `NonFiniteResult` | A math function produces NaN/infinity — FR-011 |
| `MissingPlaceholder(String)` | A referenced placeholder has no supplied binding — FR-010 |
| `DiceCountExceeded` / `IterationCapExceeded` | The FR-012 bound would be exceeded — never a hang |

## Persisted entity (`src/server`, Diesel/Postgres)

### `world_roll_records`

One row per completed, authoritative Roll Resolution (FR-014). Analogous in spirit to spec 012's `world_lore_revisions` — an immutable, append-only audit log, never updated after insert.

| Column | Type | Notes |
|---|---|---|
| `id` | `UUID` PK | |
| `world_id` | `UUID` FK → `worlds.id`, cascade delete | Scopes the record; matches every other world-scoped table's convention |
| `triggered_by` | `UUID` FK → users | Who/what triggered the roll |
| `formula` | `TEXT NOT NULL` | The formula actually resolved (post-substitution) |
| `bindings` | `JSONB`, nullable | The `PlaceholderBindings` map supplied, if any — kept for audit ("what STAT value produced this result") |
| `detail` | `JSONB NOT NULL` | The full `RollResolution` (every `DieOutcome`, per FR-013/FR-014), serialized as-is via `serde_json` |
| `result_kind` | `TEXT NOT NULL` | `"total"` or `"success_count"`, mirroring `ResolutionKind`, so a list/history query can filter/render without deserializing `detail` |
| `result_value` | `DOUBLE PRECISION NOT NULL` | The final numeric value (total or count), denormalized out of `detail` for cheap sorting/display in a roll-history list |
| `created_at` | `TIMESTAMPTZ` | |

**Validation rules**: Row is only inserted *after* `thunderforge_dice::resolve()` succeeds server-side — a rejected/errored formula (FR-011) never produces a row, matching the "no dice rolled, no partial result" guarantee. Never updated or deleted by application code (immutable log, same convention as spec 012's revisions).

**Access**: Readable by at least the world's DM (FR-014); this spec's Assumptions leave broader read-access policy (e.g. whether players see their own roll history) as an implementation-level default consistent with the existing world-visibility pattern used elsewhere (any world member can read; only the DM's read-access is a hard requirement) — `contracts/graphql-roll.md` states the concrete gate chosen.

## Entity relationship summary

```text
World ──1:N── RollRecord ──N:1── User (triggered_by)

RollRecord.detail (JSONB) ⊃ RollResolution ⊃ [DieOutcome, DieOutcome, ...]
  (crate types, not their own tables — persisted only as the record's JSON payload)
```

## Reused, unmodified entities

- **`worlds`** / world-membership (spec 009/010): supplies `world_id` scoping and the authorization check the `rollDice` mutation runs before resolving (contracts/graphql-roll.md).
- **spec 013's `world_item_effects.formula`**: unchanged by this spec — this feature makes that already-stored string resolvable (by parsing/resolving it through this crate when a future trigger mechanism calls `rollDice`), without requiring any change to spec 013's schema (SC-005).
