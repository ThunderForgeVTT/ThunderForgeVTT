# GraphQL Contract: Actor Claiming

## Types

```graphql
type GraphQLActorClaim {
  actorId: ID!
  actor: GraphQLActor!
  worldMemberId: ID!
  claimedByUserId: ID!
  claimedAt: DateTime!
}

extend type GraphQLActor {
  availableForClaim: Boolean!
  claimedBy: GraphQLWorldMember   # nullable
}

extend type GraphQLWorld {
  allowPlayerCreatedActors: Boolean!
}
```

## Queries

### `myActorClaim(worldId: ID!): GraphQLActorClaim`

- **Authorization**: any authenticated world member (any role). Returns `null` for the GM/Owner (they are never gated by this feature, FR-003) and for a non-GM member with no claim yet.
- **Behavior**: joins `world_actor_claims` through the caller's own `world_members` row for `worldId`. Pure read, no side effects.

### `availableActors(worldId: ID!): [GraphQLActor!]!`

- **Authorization**: any world member (used both by the Actor Selection screen for non-GM members and potentially by a GM reviewing what's on offer).
- **Behavior**: `world_actors` rows in `worldId` where `is_npc = false AND available_for_claim AND` no row exists in `world_actor_claims` for that actor. Excludes moderated actors per the existing `moderation::filter_visible` convention used elsewhere in this codebase.

## Mutations

### `setActorAvailability(actorId: ID!, available: Boolean!): GraphQLActor!`

- **Authorization**: caller MUST hold Owner-level permission on the actor (`auth::actor_permissions::require_actor_permission`, spec 010's existing check — reused verbatim, no new authority).
- **Behavior**: rejects with a specific error if the target actor is NPC-classified (`is_npc = true`) — only PC actors may be marked available (spec.md Assumptions). Sets `world_actors.available_for_claim`. Idempotent (setting `true` on an already-`true` row, or `false` on `false`, is not an error).

### `claimActor(worldId: ID!, actorId: ID!): GraphQLActorClaim!`

- **Authorization**: caller MUST be a member of `worldId` with a non-GM role, and MUST NOT already hold a claim in `worldId` (checked server-side even though the client-side Actor Selection screen also gates on this — Principle III).
- **Behavior**: single transaction — verify the actor belongs to `worldId`, is `available_for_claim = true`, and is not already claimed (`NOT EXISTS` check backed by the table's `UNIQUE(actor_id)` constraint as the concurrency backstop per research.md §4); insert the claim row. On a unique-constraint violation from the backstop (lost the race), returns a specific `"This character was just claimed by someone else"` error — the client re-fetches `availableActors` and re-renders (Edge Cases).
- Returns the created `GraphQLActorClaim`.

### `createAndClaimActor(worldId: ID!, name: String!, description: String): GraphQLActorClaim!`

- **Authorization**: caller MUST be a non-GM member of `worldId` with no existing claim, AND `worlds.allow_player_created_actors` MUST currently be `true` for `worldId` — re-checked at call time regardless of what the client UI showed (FR-008/FR-009).
- **Behavior**: single transaction — inserts a new `world_actors` row (`is_npc = false`, `available_for_claim = true`, `owned_by`/`created_by` = the creating user, per the existing `world_actors` ownership convention) and its claim row together. No race is possible (the actor doesn't exist until this transaction commits).

### `unclaimActor(actorId: ID!): GraphQLActor!`

- **Authorization**: caller MUST hold Owner-level permission on the actor (same check as `setActorAvailability` — GM authority, per spec 010, per Clarifications Q3).
- **Behavior**: deletes the `world_actor_claims` row for this actor, if one exists (no-op, not an error, if the actor has no active claim). Does NOT change `available_for_claim` — it remains whatever it was, so the actor becomes visible in `availableActors` again automatically if it was still flagged available (data-model.md's validation rules).

## Errors

| Condition | Error |
|---|---|
| `claimActor`/`createAndClaimActor` by a member who already has a claim in that world | `"You have already claimed a character in this world"` |
| `claimActor` on an actor no longer available (lost race, or GM un-flagged it) | `"This character was just claimed by someone else"` (or a distinct message when un-flagged — client treats both as "refresh the list") |
| `createAndClaimActor` when the world setting is off | `"This world's GM has not enabled player-created characters"` |
| `setActorAvailability`/`unclaimActor` without Owner-level Actor permission | Existing `require_actor_permission` error (unchanged) |
| `setActorAvailability(available: true)` on an `is_npc = true` actor | `"Only player characters can be marked available for claiming"` |
