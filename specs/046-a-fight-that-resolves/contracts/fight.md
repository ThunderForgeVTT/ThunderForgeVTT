# Contract: A Fight That Resolves

**Spec**: [../spec.md](../spec.md) | **Data model**: [../data-model.md](../data-model.md)

The interfaces this feature adds or changes: GraphQL, world events, the engine
command the web sends, and the pack manifest. Every clause is something an e2e
or a server test can check. `src/app/schema.graphql` is regenerated with each
phase, and `pnpm verify`'s `graphql-schema` and `graphql-ops` steps must stay
green.

## 1. GraphQL

### Queries

```graphql
# Phase 5. Every token in the scene that has a footprint other than 1.
tokenGrid(sceneId: UUID!): [TokenGrid!]!
type TokenGrid { tokenId: UUID!  footprint: Float! }

# Phase 4. Built per viewer (section 3). Newest first, 50 per page.
attack(id: UUID!): Attack
sceneAttacks(sceneId: UUID!, before: UUID): [Attack!]!

# Phase 4. The caller's own pending offers, for reconnect (FR-008).
# A Game Master receives every pending offer in the world.
pendingOffers(worldId: UUID!): [Offer!]!

# Phase 4. A warning before rolling. Judges nothing it would refuse.
previewAttack(input: AttackInput!): AttackPreview!
type AttackPreview {
  distance: Float          # system units; null when there is no target
  flags: [AttackFlag!]!    # what makeAttack would record
  turn: TurnCheck!         # whether makeAttack would be refused for turn order
  reach: Float             # Phase 7: the attack's own figures, and their unit,
  rangeNormal: Float       #   so a warning can say "Out of reach: 20 ft,
  rangeLong: Float         #   reach 5 ft"
  unit: String!            # "ft"; empty when nothing was measured
}
```

### Mutations

```graphql
# Phase 4.
makeAttack(input: AttackInput!): [Attack!]!   # several for a multiattack
input AttackInput {
  attackerTokenId: UUID!
  abilityId: UUID          # exactly one of abilityId, itemId
  itemId: UUID
  targetTokenId: UUID      # null: "a roll into the air"
  targets: [UUID!]         # per part of a multiattack; overrides targetTokenId
  actionCost: ActionCost   # defaults to the ability's; REACTION bypasses turn order
  bindings: [PlaceholderBindingInput!]
}

# Phase 4. By a controller of the offer's token, or a Game Master.
resolveOffer(offerId: UUID!, take: Boolean!): Offer!

# Phase 1. Game Master only (FR-014).
changeHitPoints(tokenId: UUID!, kind: HitPointChange!, amount: Int!): TokenHitPoints!
enum HitPointChange { DAMAGE HEALING }

# Phase 3. Game Master only.
setTokenLink(tokenId: UUID!, linked: Boolean!): Token!
setActorUnique(actorId: UUID!, unique: Boolean!): Actor!

# Phase 4. Game Master only.
updateWorldAutoApplyNpcDamage(input: { worldId: UUID!, enabled: Boolean! }): World!
setCombatAutoApply(combatId: UUID!, enabled: Boolean): Combat!   # null = world's

# Phase 7. Game Master only.
addLairCombatant(combatId: UUID!, label: String!): Combat!

# Phase 4 (implemented in tasks Phase 6). Editor on the ability or item.
# What it is as an attack; every field is written, a null clears it.
setAbilityAttack(abilityId: UUID!, attack: AttackFieldsInput!): Boolean!
setItemAttack(itemId: UUID!, attack: AttackFieldsInput!): Boolean!
input AttackFieldsInput { reach: Float  rangeNormal: Float  rangeLong: Float
  needsLineOfSight: Boolean!  actionCost: ActionCost!  legendaryCost: Int!
  multiattack: [UUID!]! }
```

*Implementation note (tasks T056):* the attack fields are set through their
own two mutations rather than added to `updateAbility`/`updateItem`'s inputs:
those inputs are "omitted means unchanged", and a reach or range must be
clearable. `Offer` and `Attack` carry `sceneId`, and `Offer` its `attackId`
and `createdAt`, so a client can place an offer on screen without a second
read; none of them identifies a party.

### Types

```graphql
type Attack {
  id: UUID!
  sceneId: UUID!
  attacker: AttackParty!     # redacted per viewer (section 3)
  target: AttackParty        # null: no target; redacted per viewer
  abilityName: String        # null when the attacker is redacted
  toHit: RollResolution!
  damage: RollResolution     # hits only
  defence: Int               # null when the target has none, or is redacted
  outcome: AttackOutcome!    # HIT MISS NO_DEFENCE NO_TARGET
  distance: Float
  flags: [AttackFlag!]!
  actionCost: ActionCost!
  offer: Offer               # null on a miss or with no target
  multiattackOf: UUID
  createdAt: DateTime!
}
type AttackParty { tokenId: UUID  label: String! }   # tokenId null and label "Unknown" when redacted
enum AttackFlag { OUT_OF_REACH LONG_RANGE BEYOND_RANGE NO_LINE_OF_SIGHT
                  NO_REACH_DECLARED OVERSPENT LEGENDARY_ON_OWN_TURN }
enum ActionCost { ACTION BONUS_ACTION REACTION LEGENDARY FREE }

type Offer {
  id: UUID!  sceneId: UUID!  attackId: UUID  createdAt: DateTime!
  kind: HitPointChange!  amount: Int!
  target: AttackParty!
  status: OfferStatus!             # PENDING TAKEN DECLINED APPLIED
  resolvedBy: String               # a display name; "Game Master" when on behalf
  resolvedOnBehalf: Boolean!
  mayResolve: Boolean!             # true for the viewer's controllers and Game Masters
}

type TokenHitPoints { tokenId: UUID!  current: Int!  max: Int!  temporary: Int! }
type TurnCheck { allowed: Boolean!  activeLabel: String }
```

`Token` gains `linked: Boolean!`. `Actor` gains `isUnique: Boolean!`.
`GraphQLCreateTokenInput` gains `linked: Boolean`. `Token.health` and
`Token.maxHealth` are removed (phase 3). `Combatant` gains `downedBy`,
`kind` and `budget: TurnBudget`. `Combat` gains `autoApply: Boolean` and
`effectiveAutoApply: Boolean!`. `World` gains `autoApplyNpcDamage: Boolean!`.
`Ability` and `Item` gain `reach`, `rangeNormal`, `rangeLong`,
`needsLineOfSight`, `actionCost`, `legendaryCost`, `multiattack`.

```graphql
type TurnBudget {
  action: BudgetLine!  bonusAction: BudgetLine!  reaction: BudgetLine!
  movement: BudgetLine!             # in system units
  legendary: BudgetLine             # null when the creature has none
  unit: String!                     # Phase 8: the system's unit ("ft"), so movement can be said
}
type BudgetLine { allowed: Float!  spent: Float!  remaining: Float! }  # remaining may be negative
```

*Implementation note (tasks Phase 8):* `Combatant.budget` is null when the
world's system declares no `turnStructure.budget`. It carries numbers only, so
a combatant redacted to "Unknown" shows its budget without identifying itself.
`makeAttack` spends once per call (a multiattack's parts together) and flags
every part `OVERSPENT` when the line it spent is past its allowance;
`previewAttack` flags `OVERSPENT` when one more would be. `moveOwnToken`,
`updateToken` and a replayed queued move spend movement for a combatant in a
running combat in the scene: a route by the footprint's steps (cells × units),
a move with no route by the footprint's displacement from old position to new
(research R13). Every spend records event 18.

## 2. Rules each mutation enforces

| # | Rule | Where | Refusal text |
|---|---|---|---|
| C1 | A player's `makeAttack`, `moveOwnToken` and queued offline move are refused when a combat is running in the scene, the acting token is a combatant, and it is not the active combatant — unless `actionCost = REACTION`. Game Masters are never refused. | server | `It is <label>'s turn` (label "Unknown" per the tracker's rule) |
| C2 | `makeAttack` requires the caller to be a controller of `attackerTokenId`, or a Game Master. | server | `You do not control that creature` |
| C3 | Reach, range and line of sight never refuse; they add flags. | server | — |
| C4 | A miss, or no target, creates no offer. | server | — |
| C5 | A hit creates a `pending` offer, unless auto-apply holds (research R15), in which case the offer is `applied` and hit points change in the same transaction. Damage to a creature any player controls is always `pending`. | server | — |
| C6 | `resolveOffer` succeeds once. A second call, by anyone, is refused. | server | `That offer has already been resolved` |
| C6a | Taking an offer whose token was relinked or unlinked after the offer was made is refused; declining it is not (research R18). | server | `That creature was relinked after this offer was made, so its hit points are a different record now. …` |
| C7 | Taking damage spends temporary hit points first, bounds current at 0; healing bounds at max. | server | — |
| C8 | A hit-point change to 0 marks the combatant `active = false, downed_by = hit_points`; above 0 reactivates only `downed_by = hit_points`. | server | — |
| C9 | Overspending a budget is recorded and shown, never refused: `spent` may exceed `allowed`, `remaining` may be negative, and the attack is flagged `OVERSPENT`. | server | — |
| C10 | Every mutation above refuses while the world's play is paused (`refuse_if_paused`), like every other play mutation. | server | the existing pause text |

## 3. Per-viewer redaction (FR-002a)

For a viewer V who is not a Game Master, an `AttackParty` for token T is
redacted — `tokenId: null`, `label: "Unknown"` — when either:

- `T.name_visible_to_players = false`, or
- no token V controls in the scene sees T under
  `thunderforge_canvas_core::vision::visibility_of`.

When the **attacker** is redacted, `abilityName` is also null. When the
**target** is redacted, `defence` is also null. Redaction is decided when the
answer is built, so the same attack reads differently to different viewers
and can change as tokens move. No redacted id, name or label appears in any
field, in any event payload, or in `pendingOffers`.

A target's controller always sees their own target (they control it), and
receives the offer even when the attacker is "Unknown".

*Phase 7 (tasks T078):* "sees" here is judged from token **centres**, as the
engine draws tokens, while an attack's `NO_LINE_OF_SIGHT` flag is judged from
the squares each creature fills. They can disagree about a Large creature half
round a corner, and that is deliberate: a player's log never names a creature
their board does not draw. See research R11.

## 4. World events

| Code | Name | Payload | Clients re-read |
|---|---|---|---|
| 14 | `TOKEN_CHANGED` (existing) | unchanged | tokens, **tokenStatus**, **tokenGrid** |
| 18 | `COMBAT_CHANGED` (existing) | unchanged | combat, including budgets |
| 26 | `ACTOR_SHEET_CHANGED` (existing) | unchanged | tokenVision, **tokenStatus** (new), **tokenGrid** (new) |
| **29** | `ATTACK_MADE` | `{attackId}` | `attack(id)` |
| **30** | `OFFER_CHANGED` | `{offerId}` | `pendingOffers`, and `attack(id)` if on screen |

Payloads carry ids only. Nothing a redaction would hide is ever in a payload.

## 5. Engine command (phase 5)

The web drives the engine's existing `set_token_grid` (`src/engine/src/sdk.rs:193-200`):

```ts
interface SetTokenGridCommand {
  type: "set_token_grid";
  tokenId: string;
  footprint: number;   // from tokenGrid; 1 when absent
  snap?: boolean;      // unchanged: default true
}
```

`sync/tokenGrid.ts` dispatches it for every token in the scene on load, with
an explicit `1` for tokens `tokenGrid` leaves out, and re-reads on events 14
and 26 — the same shape as `sync/tokenVision.ts`. No new engine system.

## 6. Pack manifest (`combat` block and `turnStructure.budget`)

Shapes in [../data-model.md](../data-model.md#pack-manifest-additions-phases-1-4-5-6-7).
Contract clauses:

- **M1**: every block is optional. A pack without `combat.hitPoints` has no
  damage operation; its attacks still roll and record, and a hit's damage is
  shown as a number with no offer.
- **M2**: `hitPoints`, `defence`, `sizes.source` and `legendary` name a slot
  and a field that exist in the pack's `data_types`; `validate_system_manifest`
  refuses a manifest that names one that does not.
- **M3**: `sizes.categories[].footprint` is ≥ 0.5 (canvas-core's
  `MIN_FOOTPRINT`); ids are unique.
- **M4**: `turnStructure.budget.movement.speed` names a key of the pack's
  `movement` block.
- **M5**: shared code names no system's fields; `check-system-registry.mjs`
  stays green.
