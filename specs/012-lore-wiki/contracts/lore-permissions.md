# Contract: Lore Entry Ownership Block (new)

## Shape

```graphql
type GraphQLLorePermission {
  loreEntryId: ID!
  userId: ID!
  level: ActorPermissionLevel!   # reused enum: VIEWER / EDITOR / OWNER
  updatedAt: String!
}

input SetLorePermissionInput {
  loreEntryId: ID!
  userId: ID!
  level: ActorPermissionLevel!
}

setLorePermission(input: SetLorePermissionInput!): GraphQLLorePermission!
loreEntryPermissions(loreEntryId: ID!): [GraphQLLorePermission!]!   # every world member + their effective level (explicit or default Viewer)
```

## Behavior

- `setLorePermission`: upserts a `world_lore_permissions` row for `(loreEntryId, userId)` — re-setting an existing subject's level updates in place (unique constraint per data-model.md), never inserts a duplicate row.
- `loreEntryPermissions`: returns one entry per world member (all players plus the DM), each showing either their explicit assigned level or an indication they're at the implicit default (`VIEWER`) — mirrors `actorPermissions`'s existing shape from spec 010.

## Authorization

- `setLorePermission`: caller MUST be DM (Owner or GM role) of the entry's world (spec 012 FR — mirrors FR-014 from spec 010: only the DM changes an ownership block, regardless of the caller's own Owner-level grant on that entry). No other permission level, including entry-level Owner, may open or change this.
- `loreEntryPermissions`: same DM-only restriction — a non-DM caller (even one holding Owner on that specific entry) cannot view the block, matching the actor precedent exactly.

## Non-goals

- No self-service "leave this entry's ownership block" action for a player — removal only happens via DM edit or the member being removed from the world entirely (cascade, per data-model.md).
