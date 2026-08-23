# Contract: Dice Roll Resolution (new)

## Shape

```graphql
enum DieSidesKind {
  NUMERIC
  FATE
  COIN
}

type GraphQLDieOutcome {
  sidesKind: DieSidesKind!
  numericSides: Int          # set iff sidesKind = NUMERIC (e.g. 20 for a d20)
  rolls: [Int!]!              # full chain: original roll + every reroll/explosion of this die
  kept: Boolean!               # false if dropped by a keep/drop modifier
  finalValue: Int!
}

enum RollResultKind {
  TOTAL
  SUCCESS_COUNT
}

type GraphQLRollResolution {
  formula: String!             # resolved formula (post placeholder-substitution)
  dice: [GraphQLDieOutcome!]!
  resultKind: RollResultKind!
  resultValue: Float!           # the total, or the success count, per resultKind
}

type GraphQLRollRecord {
  id: ID!
  worldId: ID!
  triggeredBy: ID!
  resolution: GraphQLRollResolution!
  createdAt: String!
}

input PlaceholderBindingInput {
  name: String!
  value: Float!
}

input RollDiceInput {
  worldId: ID!
  formula: String!
  bindings: [PlaceholderBindingInput!]     # optional; omit for a formula with no placeholders
}

# Mutation — the ONLY way to produce an authoritative roll result.
rollDice(input: RollDiceInput!): GraphQLRollResolution!

# Query — roll-history retrieval (FR-014)
worldRollRecords(worldId: ID!, limit: Int): [GraphQLRollRecord!]!

# Query — formula validation only, never produces a "real" result (client-side UX nicety, FR-011)
validateDiceFormula(formula: String!): Boolean!   # true if parseable; a non-parseable formula returns false, never an error, so the client can show inline "invalid formula" state without a round-trip failure
```

## Behavior

- `rollDice`: verifies the caller is a member of `worldId` (world-visibility check, same as every other world-scoped mutation), then calls `thunderforge_dice::resolve(formula, bindings, &mut real_os_backed_rng)` server-side. **No field of `RollDiceInput` can express a pre-computed result — the input shape itself makes client-supplied outcomes structurally impossible, not just policy-rejected.** On success, inserts a `world_roll_records` row (data-model.md) and returns the `GraphQLRollResolution`. On a `FormulaError` (invalid formula, missing placeholder, division by zero, non-finite result, or the FR-012 dice/iteration cap exceeded), returns a specific GraphQL error identifying which — no row is ever inserted for a failed resolution.
- `worldRollRecords`: returns the world's roll history, newest first, capped at `limit` (default a reasonable page size, e.g. 50). Every returned record's `resolution` is reconstructed from the persisted `detail` JSONB (data-model.md), so history views get full per-die detail, not just the final value.
- `validateDiceFormula`: a pure parse-only check (`DiceFormula::parse`, no evaluation, no RNG, no persistence) — safe to call from any client, including a WASM engine build, since it never touches randomness. Used for inline "is this a well-formed formula" UX (e.g. while authoring a spec 013 Item Effect's `formula` field) without needing a full roll.

## Authorization

- `rollDice`: caller MUST be a member of `worldId` (any role — this is a general capability, not DM-gated; a specific future gameplay trigger, e.g. "only the item's owner can trigger its effect," is that trigger's own concern, not this general-purpose mutation's).
- `worldRollRecords`: caller MUST be at least the world's DM (FR-014's stated minimum-guarantee — "retrievable by at least the world's DM"). Whether to widen this to all world members (e.g. so players can see their own roll history) is left as an implementation-time default consistent with the rest of the app's read-access posture; this contract states the floor, not a ceiling.
- `validateDiceFormula`: no world-scoping or authorization — pure, stateless parse check, safe for any authenticated (or even unauthenticated, if the app's general GraphQL posture allows) caller.

## Non-goals

- No mutation or query in this contract accepts a client-supplied roll *result* in any form — only a formula (+ optional bindings) goes in, only a resolved `GraphQLRollResolution` comes out. This is deliberate, not an oversight (FR-001/FR-002).
- No subscription/real-time push is specified here for broadcasting a roll to other players' screens — that's a presentation-layer concern (research.md §6) built on top of this mutation's response, likely reusing whatever existing live-session fan-out mechanism the app already has (e.g. NOTIFY/LISTEN, per the constitution's Technology Constraints) rather than this contract inventing a new one; left to `tasks.md` to wire concretely.
- No formula-authoring/autocomplete UI contract — that's spec 013's Item Effect editor's concern (or any other future caller's), not this engine's.
