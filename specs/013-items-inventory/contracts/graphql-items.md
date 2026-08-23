# Contract: Item CRUD, Effects, and Compendium Catalog (new)

## Shape

```graphql
enum ItemEffectType {
  HEAL
  DAMAGE
  MODIFIER
  ATTACK_ROLL
}

enum ItemEffectTrigger {
  ON_USE
  PASSIVE
}

type GraphQLItemEffect {
  id: ID!
  effectType: ItemEffectType!
  formula: String!
  target: String!
  triggerKind: ItemEffectTrigger
  sortOrder: Int!
}

type GraphQLItem {
  id: ID!
  worldId: ID!
  name: String!
  description: String
  iconUrl: String
  effects: [GraphQLItemEffect!]!
  myPermissionLevel: ItemPermissionLevel!   # reuses ActorPermissionLevel's enum shape
  createdAt: String!
  updatedAt: String!
}

input CreateItemInput {
  worldId: ID!
  name: String!
  description: String
}

input UpdateItemInput {
  itemId: ID!
  name: String
  description: String
}

input ItemEffectInput {
  effectType: ItemEffectType!
  formula: String!
  target: String!
  triggerKind: ItemEffectTrigger
  sortOrder: Int
}

# Queries
worldItems(worldId: ID!, search: String): [GraphQLItem!]!
item(itemId: ID!): GraphQLItem!
suggestItemName(worldId: ID!, name: String!): [GraphQLItem!]!   # "did you mean?" (research.md §3), max 5 results

# Mutations
createItem(input: CreateItemInput!): GraphQLItem!
updateItem(input: UpdateItemInput!): GraphQLItem!
deleteItem(itemId: ID!): Boolean!

addItemEffect(itemId: ID!, effect: ItemEffectInput!): GraphQLItemEffect!
updateItemEffect(effectId: ID!, effect: ItemEffectInput!): GraphQLItemEffect!
removeItemEffect(effectId: ID!): Boolean!
```

## Behavior

- `worldItems(worldId, search)`: returns every Item in the world visible to the caller (i.e., all of them — default Viewer access means every world member can see every item, per FR-008/FR-018), optionally filtered by `search` matching `name`/`description` (instant-as-you-type, mirrors the NPC catalog's existing search per spec 011 FR-003). Results are NOT deduplicated or ordered by name-collision (FR-019 — duplicate names simply both appear).
- `item(itemId)`: returns full detail including all effects, ordered by `sortOrder`. Denies access (per FR-018) if the caller has no ownership-block row and is not a world member with default-Viewer access (mirrors actor/lore detail-access rule).
- `suggestItemName(worldId, name)`: read-only, non-authoritative helper for the creation form's "did you mean?" prompt (FR-020, research.md §3); returns up to 5 near-name-matches via `pg_trgm` similarity, empty array if none clear the similarity threshold. Never blocks `createItem`.
- `createItem`: only a DM (Owner/GM role on `worldId`, FR-002) may call this. Creates the `world_items` row with the caller as `created_by`; `description`/icon are optional (Clarifications). No `name` uniqueness check is performed (FR-019) — this is a create, not an upsert.
- `updateItem` / `deleteItem`: require Editor-or-Owner (`updateItem`) / Owner-level (`deleteItem`) effective permission on that specific Item, per the reused ownership-block model (FR-003/FR-018). `deleteItem` is never blocked by outstanding lore links or inventory references (FR-017) — see `contracts/item-lore-links.md` and `contracts/graphql-inventory.md` for how each side reacts afterward.
- `addItemEffect` / `updateItemEffect` / `removeItemEffect`: require Editor-or-Owner on the parent Item (FR-005). `formula` is validated server-side (structural dice-grammar check, research.md/data-model.md `world_item_effects`) and rejected with a clear error if empty/malformed (FR-006) — no row is written on validation failure.

## Authorization

- `createItem`: caller MUST hold DM-level access (Owner or GM role) on `worldId` (FR-002).
- `worldItems` / `item` / `suggestItemName`: caller MUST be an authenticated member of `worldId` (or the world's DM); every member sees every item at at least Viewer level by default (FR-008).
- `updateItem`, `addItemEffect`, `updateItemEffect`, `removeItemEffect`: caller's effective `ItemPermissionLevel` for the target item MUST be `EDITOR` or `OWNER`.
- `deleteItem`: caller's effective `ItemPermissionLevel` for the target item MUST be `OWNER` (includes the DM's always-implicit Owner access).

## Non-goals

- No dice-rolling, formula evaluation, or effect triggering — `formula`/`target`/`triggerKind` are stored and returned as-authored only (Assumptions, FR-004a).
- No ruleset-specific validation of `target` against a stat/resource vocabulary (Assumptions).
- No item-name uniqueness enforcement — `suggestItemName` is advisory only (FR-019/FR-020).
