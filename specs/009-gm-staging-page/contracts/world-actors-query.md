# Contract: `worldActors` GraphQL Query (new)

## Shape

```graphql
type GraphQLWorldActor {
  id: ID!
  worldId: ID!
  sceneId: ID!
  actorType: String!
  gameSystemId: String
  label: String!
  isPublic: Boolean!
  isNpc: Boolean!
  createdBy: ID!
  ownedBy: ID!
  createdAt: String!
  updatedAt: String!
}

worldActors(worldId: ID!): [GraphQLWorldActor!]!
```

Field-for-field mirror of the existing `WorldActor` Rust struct (`src/server/src/models.rs`) — no field is invented or renamed relative to the underlying table.

## Behavior

- Returns every `world_actors` row where `world_id` matches the argument — both NPCs and player characters (distinguished by `isNpc`); the staging page and sidebar's NPC roster UI filter client-side to `isNpc == true` (or the caller may filter server-side in a later iteration — this contract returns the full set to keep the query reusable for any future non-NPC actor listing without a second query).
- Ordering is not contractually specified (undefined order is acceptable — the UI is responsible for any display sort).
- An empty result (`[]`) is a valid, expected response for a world with no actors yet — not an error (spec FR-004: a real empty state, not a placeholder).

## Authorization

- Requires an authenticated session (`authenticated_user(ctx)`), same as every other world-scoped query.
- Requires the caller to pass `require_visible_world(state, user_id, is_admin, world_id)` — identical to the existing `scenes(worldId)` query (`graphql/queries/scene.rs`). This means: the world's owner, any accepted `world_members` row for that world (any role), or an admin. A non-member gets the same `Forbidden`-style error `scenes`/`worldMembers` already return today — no new error shape is introduced.
- No role distinction at the query level (GM vs Player both see the same actor list) — role-based gating of *editing* (not viewing) NPCs is a UI-only concern per spec FR-012, matching how `worldMembers` is also readable by any member regardless of role.

## Non-goals

- No mutation is added in this contract — creating/editing/deleting NPC actors from the staging page or sidebar is not specified by spec 009 (only *listing* is required by FR-003); if the staging page's NPC section needs to support editing in a later pass, that is a separate mutation contract.
- No pagination — worlds are not expected to have actor counts large enough to require it yet; if that changes, pagination can be added without breaking this contract (adding optional arguments is backward-compatible).
