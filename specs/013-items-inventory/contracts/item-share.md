# Contract: Item Sharing and Cross-World Copy (new)

Direct structural mirror of `specs/010-world-staging-actors/contracts/actor-share.md` — see research.md §5 for why this is a deliberate precedent-copy rather than a new design. Only Item-specific differences are called out; unlabeled behavior is identical to the actor contract.

## Shape

```graphql
type GraphQLItemShareLink {
  id: ID!
  itemId: ID!
  shareCode: String!
  revoked: Boolean!
  createdAt: String!
}

type SharedItemPreview {
  name: String!
  description: String
  iconUrl: String
  effects: [GraphQLItemEffect!]!   # reused type from graphql-items.md
}

# Deliberately excludes id/worldId/createdBy/ownership block (mirrors actor-share.md's SharedActorPreview)

createItemShareLink(itemId: ID!): GraphQLItemShareLink!
revokeItemShareLink(shareId: ID!): Boolean!

sharedItem(shareCode: String!): SharedItemPreview!   # query — "not available" error if revoked/not found
# myDmWorlds reused as-is from spec 010 — already world-type-agnostic, no item-specific variant needed

input CopySharedItemInput {
  shareCode: String!
  destinationWorldId: ID!
}

copySharedItemToWorld(input: CopySharedItemInput!): GraphQLItem!   # mutation
```

## Behavior

- `createItemShareLink`: generates a new opaque `share_code` (same scheme as `world_actor_shares.share_code`) and a `world_item_shares` row with `revoked = false`. Calling it again for the same item creates an *additional*, independently-revocable link (matches the actor precedent — no exactly-one-link-per-item requirement).
- `sharedItem(shareCode)`: resolves the link, returns `SharedItemPreview` if `revoked = false` and the item still exists; otherwise a clear "not available" error, rendered by the client as a "no longer available" state (mirrors FR-024 of spec 010, applied here per FR-023).
- `copySharedItemToWorld`: resolves the share link (same validity check as `sharedItem`), re-verifies the caller holds DM-level access on `destinationWorldId` server-side (never trusts a prior `myDmWorlds` read), then in one transaction: inserts a new `world_items` row (fresh id, `world_id` = destination world, `created_by` = caller, no ownership-block rows) and clones every `world_item_effects` row belonging to the source item onto the new item id. Returns the newly created Item. Icon/image, if present, is also copied (implementation detail: either re-referenced if the underlying asset store supports read-shared assets, or re-uploaded — a tasks.md-level decision, not a data-model concern).

## Authorization

- `createItemShareLink`: caller's effective `ItemPermissionLevel` for `itemId` MUST be `OWNER` (FR-022 — includes the DM's implicit Owner).
- `revokeItemShareLink`: caller MUST be either the link's `created_by` OR DM of the item's world (FR-027's "Item-level Owner (or DM)" — deliberately slightly broader than `createItemShareLink`'s gate, mirroring the actor precedent's rationale: a DM must always be able to shut down a link even one they didn't personally create).
- `sharedItem`: caller MUST be authenticated; no world-membership check (mirrors actor-share.md's rationale — a share link is meant to be viewable by anyone logged in, regardless of world).
- `copySharedItemToWorld`: caller MUST be authenticated AND hold DM-level access (Owner or GM) on `destinationWorldId`, re-verified server-side.

## Non-goals

- No usage cap or expiry on the share link (persistent, uncapped by default, only `revoked` gates access — mirrors spec 010 Assumptions).
- No live/referential relationship between source and copy after `copySharedItemToWorld` completes (FR-026) — the copy is fully independent from the moment it's created.
