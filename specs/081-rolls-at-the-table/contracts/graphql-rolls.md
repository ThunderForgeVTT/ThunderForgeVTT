# Contract: GraphQL for rolls

```graphql
enum RollVisibility {
  EVERYONE
  GM_EYES
  GM_ONLY
}

input RollDiceInput {
  worldId: UUID!
  formula: String!
  bindings: [PlaceholderBindingInput!]
  "Defaults to EVERYONE. GM_EYES: players only. GM_ONLY: the GM only."
  visibility: RollVisibility
  "What the roll was for, at most 80 characters."
  label: String
}

type WorldRoll {
  id: UUID!
  rollerId: UUID!
  rollerName: String!
  label: String
  formula: String!
  "The same resolution `rollDice` answers: dice, kept dice, total or successes."
  resolution: GraphQLRollResolution!
  visibility: RollVisibility!
  createdAt: DateTime!
  revealedAt: DateTime
  revealedByName: String
}

"Another player's roll for the GM's eyes. Carries nothing about the dice."
type MaskedRoll {
  id: UUID!
  rollerName: String!
  createdAt: DateTime!
  visibility: RollVisibility!
}

union WorldRollEntry = WorldRoll | MaskedRoll

type Query {
  "One roll as the caller may see it. Null if it does not exist or is hidden from the caller."
  worldRoll(worldId: UUID!, rollId: UUID!): WorldRollEntry
  "Newest first, before `before` (exclusive). Any member. `limit` 1–100, default 50."
  worldRolls(worldId: UUID!, before: DateTime, limit: Int): [WorldRollEntry!]!
}

type Mutation {
  rollDice(input: RollDiceInput!): GraphQLRollResolution! # unchanged answer
  "GM or admin. Idempotent: an already-public roll answers as it is and records nothing."
  revealRoll(worldId: UUID!, rollId: UUID!): WorldRoll!
}
```

## Refusals

| Case                                     | Message                                     |
| ---------------------------------------- | ------------------------------------------- |
| player rolls `GM_ONLY`                   | Only the GM can roll for their eyes only    |
| GM rolls `GM_EYES`                       | The GM rolls GM only, not for the GM's eyes |
| non-GM reveals                           | Only the GM can reveal a roll               |
| reveal of a roll in another world / none | Roll not found                              |
| label over 80 characters                 | A roll's label is at most 80 characters     |

A refused roll records nothing: no row, no event.

## What a payload may contain

`{ "rollId": "<uuid>", "visibility": "everyone" | "gm_eyes" | "gm_only" }`
and nothing else. A server test asserts the key set.
