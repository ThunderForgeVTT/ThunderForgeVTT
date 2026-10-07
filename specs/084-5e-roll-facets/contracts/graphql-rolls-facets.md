# Contract: GraphQL for roll facets

This changes the shared schema, so it is a cross-cutting path. Regenerate
the contract with `node scripts/check-graphql-contract.mjs --schema --fix`.
The full e2e suite (`node ./scripts/e2e-parallel.mjs`) is the gate before
merge.

## New enum

```graphql
"The circumstance a d20 test is rolled under. The system decides what it means."
enum Advantage {
  NORMAL
  ADVANTAGE
  DISADVANTAGE
}
```

## Changed arguments

```graphql
type Mutation {
  rollCheck(worldId: UUID!, actorId: UUID!, checkId: String!, advantage: Advantage = NORMAL): RollResolution!
  makeAttack(input: AttackInput!): …   # AttackInput gains advantage: Advantage = NORMAL
  setItemAttack(itemId: UUID!, input: AttackFieldsInput!): …  # AttackFieldsInput gains properties: [String!] (absent = unchanged)
}
```

Refusals, each a GraphQL error with this exact sentence:

| When                                                     | Sentence                                   |
| -------------------------------------------------------- | ------------------------------------------ |
| `ADVANTAGE`/`DISADVANTAGE` on a formula with no d20 term | `This roll has no d20 to roll twice.`      |
| an item property the world's system does not declare     | `This system has no item property "<id>".` |

On a world whose system registers no `roll_facets`, `advantage` other than
`NORMAL` is refused with `This system does not roll with advantage.`, and
`NORMAL` rolls the declared formula untouched (SC-005).

## New mutation

```graphql
type Mutation {
  "Spend a resource to reroll one of your own d20 tests (spec 084). GATED by a pause."
  rerollRoll(worldId: UUID!, rollId: UUID!, spend: String!): WorldRoll!
}
```

The checks, in order. Each refusal is a sentence, and nothing is written:

1. The world exists and the caller is a member.
2. The world is not paused (`refuse_world_if_paused`).
3. The roll exists in this world, `triggered_by = caller`, and `actor_id`
   is set. Otherwise: `Only the person who made a roll may reroll it.`
4. The caller has Editor on `actor_id`. Otherwise:
   `You can no longer act for this character.`
5. `roll_kind` is `check` or `to_hit`. Otherwise:
   `Only a d20 test can be rerolled.`
6. Nothing has rerolled it: `This roll has already been rerolled.`
7. `created_at + 2 min ≥ now`: `It is too late to reroll this roll.`
8. The spend has not been used in the chain:
   `<label> has already been spent on this roll.`
9. A `to_hit` roll whose attack hit is refused with:
   `A hit cannot be rerolled.`
10. The pack's `reroll` accepts the spend. Its own sentences include:
    - `<name> has no Heroic Inspiration.`
    - `This table does not use Heroic Inspiration.`
    - `<name> has no Luck Points left.`
    - `A Luck Point does nothing on a roll made with disadvantage.`
    - `<name> does not have the Lucky feat.`
11. A spend id the pack does not list is refused with
    `This system has no reroll called "<id>".`

Then, in one transaction:

1. Lock the roll row, then the sheet row.
2. Write the sheet the pack returned.
3. Replay the dice.
4. Insert the new roll.
5. Judge it: a check through `judge_check`, or a to-hit as in research R7.
6. Record the events: `ROLL_MADE` (36), `ACTOR_SHEET_CHANGED` (26), and for
   an attack `ATTACK_MADE` (29), plus `OFFER_CHANGED` (30) if the reroll
   hit.

A unique violation on `reroll_of` maps to refusal 6.

## `WorldRoll` gains

```graphql
type WorldRoll {
  # existing fields unchanged
  facets: [RollFacet!]! # passive and per-roll facets applied, plus the spend on a reroll
  rerollOf: UUID # the roll this one replaces
  rerolledBy: UUID # the roll that replaced this one
  spent: RollFacet # what this reroll spent
  rerollOffers: [RollFacet!]! # only for the roll's maker: spends the server would accept now; [] for everyone else
  rerollUntil: String # RFC 3339; null when the roll cannot be rerolled at all
}

type RollFacet {
  id: String!
  label: String!
}
```

`rerollOffers` is computed per viewer, from the same pack function as the
mutation, with no write (research R9). It is empty for anyone but the
roll's maker, and empty for a roll that has a `rerolledBy`.

`MaskedRoll` is unchanged. A reroll of a GM's eyes roll is one more masked
"rolled for the GM" to the other players.

## `revealRoll` (changed behaviour, same signature)

Revealing any roll in a chain reveals every roll in it. `ROLL_REVEALED` (37)
is recorded once per roll revealed.

## `Item` / attack fields

`AttackFields` (the item's attack read) gains `properties: [String!]!`.
`systemItemProperties(worldId: UUID!): [RollFacet!]!` lists the world
system's declared properties for the editor, and is empty for a pack that
declares none.

## Pause classification

`crates/thunderforge-server/src/graphql/play_pause_surface_tables.rs`:

An entry in the gated list, beside `revealRoll` and `rollCheck`. The
pause is checked before the roll is looked up, so the world's id stands in
for the roll:

```rust
(
    "rerollRoll",
    r#"mutation { rerollRoll(worldId: "{world}", rollId: "{world}", spend: "inspiration") { __typename } }"#,
),
```
