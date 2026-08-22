# Contract: Actor Ownership Block (new)

## Shape

```graphql
type GraphQLActorPermission {
  actorId: ID!
  userId: ID!
  level: ActorPermissionLevel!
  updatedAt: String!
}

input SetActorPermissionInput {
  actorId: ID!
  userId: ID!
  level: ActorPermissionLevel!
}

actorPermissions(actorId: ID!): [GraphQLActorPermission!]!   # query
setActorPermission(input: SetActorPermissionInput!): GraphQLActorPermission!   # mutation
removeActorPermission(actorId: ID!, userId: ID!): Boolean!   # mutation
```

## Behavior

- `actorPermissions(actorId)`: returns only the *explicit* rows in `world_actor_permissions` for that actor — members with default Viewer access (no row) are NOT synthesized into this list. The ownership-block editing UI is responsible for rendering "every world member plus the DM" (FR-015) by combining this list with the full `worldMembers(worldId)` roster client-side; any member absent from this query's result is shown as default Viewer in the UI.
- `setActorPermission`: UPSERT on `(actor_id, user_id)` — creates the row if absent, updates `level` if present.
- `removeActorPermission`: deletes the explicit row if present (idempotent — returns `true` whether or not a row existed), reverting that member to default Viewer.

## Authorization

- All three operations require the caller to be DM (Owner or GM role) of the actor's world (FR-014) — this is the one part of the actor-permission system where "does the caller hold Owner-level on the actor itself" is irrelevant; only DM status matters. A caller who holds `Owner` via an explicit `world_actor_permissions` row but is not DM (e.g., a player owning their own PC) MUST be rejected from all three operations (spec Edge Cases: "ownership-block changes are DM-only regardless of the requester's own permission level").
- `setActorPermission`'s `input.userId` MUST resolve to an existing member of the actor's world (any role, including the DM assigning themselves an explicit row, which is legal but redundant given FR-017).

## Non-goals

- No bulk/batch variant — the ownership-block UI calls `setActorPermission`/`removeActorPermission` once per changed row, matching the existing one-mutation-per-field-change convention used elsewhere (e.g. `updateMemberRole`).
- No notification/audit trail beyond the existing `world_events` mechanism already used for membership changes — wiring ownership-block changes into that log is not required by this spec.
