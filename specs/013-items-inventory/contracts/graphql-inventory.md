# Contract: Actor Inventory (new)

## Shape

```graphql
type GraphQLInventoryEntry {
  id: ID!
  actorId: ID!
  itemId: ID          # null if the referenced item has been deleted
  itemName: String!   # itemNameSnapshot — always populated, even when itemId is null
  itemIconUrl: String # null if item deleted or has no icon
  quantity: Int!
}

input AddItemToInventoryInput {
  actorId: ID!
  itemId: ID!
  quantity: Int!       # MUST be >= 1
}

input AdjustInventoryQuantityInput {
  inventoryEntryId: ID!
  quantity: Int!        # absolute new value; MUST be >= 0
}

# Queries
actorInventory(actorId: ID!): [GraphQLInventoryEntry!]!

# Mutations
addItemToInventory(input: AddItemToInventoryInput!): GraphQLInventoryEntry!
adjustInventoryQuantity(input: AdjustInventoryQuantityInput!): GraphQLInventoryEntry   # null return = entry removed (quantity hit 0)
removeInventoryEntry(inventoryEntryId: ID!): Boolean!
```

## Behavior

- `actorInventory(actorId)`: returns every inventory row for the actor (deleted-item rows included, rendered via `itemName`/`itemIconUrl` snapshot data per data-model.md's `item_name_snapshot`). Any user with at least Viewer access to the actor may call this (FR-013).
- `addItemToInventory`: if an entry for `(actorId, itemId)` already exists, this is an upsert that **adds** `quantity` to the existing row (never a duplicate row, per the DB unique constraint + `ON CONFLICT` upsert in data-model.md), refreshing `itemNameSnapshot` from the Item's current name. If no entry exists, creates one with the given quantity.
- `adjustInventoryQuantity`: sets the entry's quantity to the given absolute value (not a delta) — this is the control used for "decrease from 3 to 2." A resulting quantity of `0` deletes the row server-side and the mutation returns `null` (FR-011); the client is expected to treat a `null` result as "entry removed" and update its list accordingly.
- `removeInventoryEntry`: deletes the row outright regardless of its quantity (explicit "remove from inventory" action, distinct from adjusting to zero, though both end in the same deleted-row state).
- Every mutation here re-resolves the caller's effective permission against `actorId` at call time — never trusts a client-cached permission level from an earlier `actorInventory` read.

## Authorization

- `actorInventory`: caller MUST have at least Viewer-level effective permission on `actorId` (FR-013's read half).
- `addItemToInventory`, `adjustInventoryQuantity`, `removeInventoryEntry`: caller MUST have Editor-or-Owner effective permission on `actorId` — **not** on the referenced Item. Holding only Viewer (or no) access to the Item itself does not block adding it to an Actor the caller can edit (spec Assumptions: "inventory management is fundamentally an operation on the Actor, not on the Item").
- `addItemToInventory`'s `itemId` MUST reference an Item the caller has at least Viewer access to (i.e., can see well enough to know it exists and pick it from an item-picker UI) — since every world member has default Viewer access to every item (FR-008), this is effectively "any item in the same world," and is checked to guard against cross-world `itemId` values, not to add a new permission gate.

## Non-goals

- No "use item" mutation (rolling an effect and auto-decrementing quantity) — explicitly deferred (Clarifications, spec Assumptions). All quantity changes in this contract are explicit, manual actions.
- No stacking limits/max-quantity cap — `quantity` is an unbounded non-negative integer.
- No "equip" state on an inventory entry (spec Assumptions) — an entry is Item + quantity only.
