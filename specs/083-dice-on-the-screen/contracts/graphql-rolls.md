# Contract: GraphQL, rolls

This changes `apps/thunderforge/schema.graphql`. The slices
`pnpm e2e:which --diff` names gate the merge (owner decision 2026-10-07). Regenerate it with
`node scripts/check-graphql-contract.mjs --schema --fix`.

## Additions

```graphql
enum DieStep {
  REROLL
  EXPLODE
}

type GraphQLDieOutcome {
  sidesKind: DieSidesKind!
  numericSides: Int
  rolls: [Int!]!
  "Why each value after the first was rolled: steps[i] explains rolls[i + 1]. Empty for rolls stored before spec 083."
  steps: [DieStep!]!
  kept: Boolean!
  finalValue: Int!
}

type RollBinding {
  placeholder: String!
  value: Float!
}

type WorldRoll {
  id: UUID!
  rollerId: UUID!
  rollerName: String!
  label: String
  formula: String!
  "The values the server substituted for the formula's placeholders, sorted by placeholder."
  bindings: [RollBinding!]!
  resolution: GraphQLRollResolution!
  visibility: RollVisibility!
  createdAt: String!
  revealedAt: String
  revealedByName: String
}
```

`MaskedRoll` is unchanged: `id`, `rollerName`, `createdAt`, `visibility`.

## Who sees `bindings`

`WorldRoll` reaches exactly the viewers that `rolls/visibility.rs::view_of`
lets see the whole roll. `bindings` adds nothing a viewer could not already
work out, because the total is visible to them. A `MaskedRoll` carries no
bindings, and has no field that could hold them.

## Every producer of `GraphQLDieOutcome`

| Producer                                                 | Change                                                                               |
| -------------------------------------------------------- | ------------------------------------------------------------------------------------ |
| `types_dice.rs` `From<&DieOutcome>`                      | maps `steps` to `DieStep`                                                            |
| `mutations_roll_check_tests.rs:592`                      | constructs `DieOutcome`, so it gains `steps: vec![]`                                 |
| `apps/demo/src/backend/handlers/dice.ts` `resolutionRow` | maps the crate's `"Reroll"`/`"Explode"` to `REROLL`/`EXPLODE`, with `[]` when absent |

## Every query that selects dice, and must select the new fields

| Query                                               | Change                                                      |
| --------------------------------------------------- | ----------------------------------------------------------- |
| `apps/web/src/api/roll.ts` `dice {` fragment        | adds `steps`                                                |
| `apps/web/src/api/roll.ts` `worldRoll`/`worldRolls` | adds `bindings { placeholder value }` on `... on WorldRoll` |

## Errors

None. An unreadable `bindings` column reads as `[]`. A die with no stored
steps reads as `[]`.
