# Quickstart: Validating the Dice Rolling Engine

Prerequisites: local dev stack running (`docker compose up` for Postgres, server on its configured port, `apps/web` dev server running, `src/engine` built for `wasm32-unknown-unknown` and loaded), at least one world with a logged-in member.

## US1 — A roll is resolved fairly, with no client-supplied outcome ever trusted

1. As any world member, trigger `rollDice` with formula `1d20`. **Expect**: a `GraphQLRollResolution` comes back with one `DieOutcome` whose `finalValue` is between 1 and 20, and a `world_roll_records` row now exists matching it exactly (verify via `worldRollRecords` as the DM).
2. Attempt to call `rollDice` with any extra/forged field suggesting a specific result (there is none in `RollDiceInput` — confirm the schema itself has no such field, per contracts/graphql-roll.md). **Expect**: impossible by construction — the mutation only accepts `worldId`/`formula`/`bindings`.
3. Trigger two `1d20` rolls back-to-back as two different users. **Expect**: independent results (no correlation), each with its own `world_roll_records` row.
4. As the world's DM, open roll history (`worldRollRecords`). **Expect**: every prior roll is listed with its full formula, per-die detail, and timestamp.

## US2 — The full formula grammar resolves correctly

1. `4d6+2` → total = sum of the 4 dice + 2; all 4 `DieOutcome`s present.
2. `4d6kh3` → exactly 3 of the 4 dice are `kept: true`, contributing to the total; the 4th is present with `kept: false`.
3. `2d6r1` → any die that rolled a 1 shows a `rolls` array with more than one entry (the reroll), `final_value` reflecting the kept (rerolled) result.
4. `1d6x` → an exploding die that hits its max face shows `rolls` with the original roll plus every explosion in sequence.
5. `8d10cs>=7` → `resultKind = SUCCESS_COUNT`; `resultValue` equals the count of dice with `finalValue >= 7`.
6. `4dF` → 4 `DieOutcome`s with `sidesKind = FATE`, each `finalValue` in `{-1, 0, 1}`.
7. `(2d4)d8` → resolves correctly (inner `2d4` determines how many d8s are rolled).
8. `floor(1d20/2)` → total is the floored division result.
9. Submit `1d20 +` (malformed) via `rollDice`. **Expect**: a specific `FormulaError`-derived GraphQL error, no dice rolled, no `world_roll_records` row created.
10. Submit `1d6x>=1` (never-terminating explode condition). **Expect**: rejected with a dice/iteration-cap error (FR-012), not a hang.

## US3 — Placeholder substitution

1. Call `rollDice` with formula `1d20 + STAT` and `bindings: [{name: "STAT", value: 3}]`. **Expect**: resolves with `STAT` substituted as 3 before evaluation (verify by comparing against the same formula with `STAT` bound to a different value — the totals should differ by exactly the delta).
2. Call `rollDice` with formula `1d20 + STAT` and no `STAT` binding. **Expect**: a `MissingPlaceholder` error, not a result treating `STAT` as 0.
3. Call `rollDice` with formula `2d8` (no placeholders) and no `bindings`. **Expect**: resolves normally — placeholders are opt-in, not required machinery.
4. Via `validateDiceFormula`, check a spec 013 Item Effect formula (e.g. `1d20 + STAT + MODIFIERS`) parses successfully without needing any binding or resolution (parse-only check).

## US4 — Dice-bouncing animation reveals, never precedes, the real result

1. Trigger a roll from the play canvas. **Expect**: a bouncing-dice animation plays; each animated die comes to rest on the exact `finalValue` (and, for a multi-entry `rolls` chain, visibly represents the reroll/explosion) from the `rollDice` response.
2. Inspect the UI mid-animation. **Expect**: no total/result is shown until the animation completes.
3. Simulate the animation being unavailable (e.g. a context with no canvas surface). **Expect**: the resolved result is still delivered and displayed — the animation's absence never blocks or hides it.

## Cross-cutting checks

- **wasm32 build**: `cargo check --target wasm32-unknown-unknown -p thunderforge_engine` (or the workspace's equivalent command) succeeds with `thunderforge_dice` as a dependency, confirming the crate is genuinely WASM-clean.
- **Crate unit tests run with no server/DB**: `cargo test -p thunderforge_dice` passes standalone (seeded/mock `RngCore`), with no Postgres or Axum involvement — proving the crate is independently testable per Constitution Principle II.
- **Spec 013 unlock**: resolve an actual spec 013 Item Effect's stored `formula` (e.g. `2d8` from a "Longsword" damage effect) through `rollDice` with zero changes needed to spec 013's `world_item_effects` schema (SC-005).
