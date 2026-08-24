# Contract: Genie Session Resource Economy (Grants, Shops, Clock Rewards)

Extends `specs/018-genie-house-system/contracts/genie-session-loop.md`. All
mutations below broadcast a `world_events` row with `event_code = 15`
(`EVENT_CODE_GENIE_SESSION_STATE`) on success, with a new `kind` value per
mutation family (research.md R3) — consumed by the existing
`worldEventsCreated(worldId)` subscription; no new subscription defined.

## Shape

```graphql
enum GenieShopPriceKind { RESOURCE, ITEM }
enum GenieRewardRecipientMode { TRIGGERING_ACTOR, WHOLE_PARTY }

type GenieShopListing {
  id: ID!
  actorId: ID!
  itemId: ID!
  priceKind: GenieShopPriceKind!
  priceResourceType: String
  priceResourceAmount: Int
  priceItemId: ID
  priceItemQuantity: Int
  # Derived, not stored: world_actor_inventory.quantity for (actorId, itemId).
  stockQuantity: Int!
}

type GeniePuzzleClockReward {
  id: ID!
  clockId: ID!
  triggerSegment: Int!
  rewardResourceType: String
  rewardResourceAmount: Int
  rewardItemId: ID
  rewardItemQuantity: Int
  recipientMode: GenieRewardRecipientMode!
  grantedAt: String
}

type Mutation {
  # GM-only (is_dm_of_world, same pattern as spendWish/advanceDoomClock).
  # Rejects if no active session exists for sessionId (FR-001, Scenario 3).
  # kind = "resource_grant"
  grantSessionResource(
    sessionId: ID!, actorId: ID!, resourceType: String!, amount: Int!
  ): GenieResourceHolding!

  # No new mutation for item grants — reuse existing addItemToInventory
  # unchanged (FR-002). Listed here only for contract completeness.
  # addItemToInventory(actorId: ID!, itemId: ID!, quantity: Int!): InventoryEntry!

  # GM-only. Creates a listing on an NPC's existing inventory item.
  createShopListing(
    actorId: ID!, itemId: ID!,
    priceKind: GenieShopPriceKind!,
    priceResourceType: String, priceResourceAmount: Int,
    priceItemId: ID, priceItemQuantity: Int
  ): GenieShopListing!

  # Buyer-callable (any world member controlling the buying actor).
  # Atomic: verify afford -> deduct/transfer price -> transfer listed item
  # -> atomic conditional stock decrement (FR-005, FR-005a). "Stock" is
  # world_actor_inventory.quantity, not a separate counter.
  # kind = "purchase"
  purchaseFromShop(listingId: ID!, buyerActorId: ID!): GenieShopListing!

  # GM-only. Configures one reward entry on a Puzzle Clock (data-model.md).
  configurePuzzleClockReward(
    clockId: ID!, triggerSegment: Int!,
    rewardResourceType: String, rewardResourceAmount: Int,
    rewardItemId: ID, rewardItemQuantity: Int,
    recipientMode: GenieRewardRecipientMode!
  ): GeniePuzzleClockReward!

  # CHANGED from spec 018: gains optional actorId (FR-006a, backward-compatible
  # -- every existing caller omitting actorId is unaffected). When advancing
  # crosses a configured reward's triggerSegment, that reward grants exactly
  # once in the same transaction as the segment update (FR-006). A
  # "triggering_actor" reward crediting requires actorId; if omitted, falls
  # back to whole_party for that grant (FR-006a, Configuration section).
  # kind = "clock_reward" (in addition to the existing puzzle-clock-state kind,
  # when at least one reward fires on this advance)
  advancePuzzleClock(clockId: ID!, delta: Int!, actorId: ID): GeniePuzzleClock!
}
```

## Authorization summary

| Mutation | Caller | Enforcement |
|---|---|---|
| `grantSessionResource` | GM only | `is_dm_of_world`, same as `spendWish`/`advanceDoomClock` |
| `createShopListing` | GM only | `is_dm_of_world` |
| `purchaseFromShop` | Any world member controlling `buyerActorId` | `caller_controls_actor` (spec 018's claim-or-owned_by check) — NOT GM-only, this is player-initiated |
| `configurePuzzleClockReward` | GM only | `is_dm_of_world` |
| `advancePuzzleClock` (with new `actorId`) | GM only (unchanged from spec 018) | `is_dm_of_world`; `actorId` is informational (who to credit), not an authorization signal — the GM is still the one clicking Advance |

## Error cases (contract-level, mirrors spec.md Acceptance Scenarios)

- `grantSessionResource` with no active session → error, no partial state (FR-001, US1 Scenario 3).
- `grantSessionResource`/`createShopListing`/`configurePuzzleClockReward` called by a non-GM → rejected (US1 Scenario 4).
- `purchaseFromShop` with insufficient resource balance or barter-item quantity → rejected, no state change on either side (US2 Scenarios 2, 4).
- `purchaseFromShop` racing another buyer for the last unit → exactly one succeeds; the loser gets a clean "out of stock" error, no partial deduction (FR-005a, US2 Scenario 5).
- `advancePuzzleClock` reaching a segment with no configured reward rows → behaves exactly as spec 018/019 today (US3 Scenario 4).
