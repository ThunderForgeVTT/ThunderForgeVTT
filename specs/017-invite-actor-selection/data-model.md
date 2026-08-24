# Data Model: Player Onboarding — Invite-to-Actor Selection

## New table: `world_actor_claims`

| Column | Type | Constraints | Notes |
|---|---|---|---|
| `id` | `UUID` | PK, default `gen_random_uuid()` (matches existing table convention) | |
| `actor_id` | `UUID` | `NOT NULL`, `UNIQUE`, `REFERENCES world_actors(id) ON DELETE CASCADE` | one active claim per actor (FR-006); deleting the actor removes the claim (Edge Cases — "GM deletes a claimed character") |
| `world_member_id` | `UUID` | `NOT NULL`, `UNIQUE`, `REFERENCES world_members(id) ON DELETE CASCADE` | one active claim per (world, member) — enforced via the member row, which is itself world-scoped (FR-014); member leaving/removed cascades the claim away |
| `claimed_at` | `TIMESTAMPTZ` | `NOT NULL DEFAULT now()` | |

No `world_id` column — it's derivable via `world_actors.world_id` or `world_members.world_id` (both point to the same world by construction; the claim mutation verifies this before inserting). Avoiding a denormalized `world_id` here follows the same reasoning as other join tables in this schema that key off their FKs' own scoping rather than duplicating it.

## Column additions

### `world_actors`

| Column | Type | Constraints | Notes |
|---|---|---|---|
| `available_for_claim` | `BOOLEAN` | `NOT NULL DEFAULT false` | GM-set via `setActorAvailability`; independent of `is_npc`/claim state (research.md §2) |

### `worlds`

| Column | Type | Constraints | Notes |
|---|---|---|---|
| `allow_player_created_actors` | `BOOLEAN` | `NOT NULL DEFAULT false` | GM-set; default false per Clarifications Q1 |

## Derived/computed values (not stored)

- **"Is this actor available to claim right now?"** = `world_actors.available_for_claim AND NOT EXISTS (SELECT 1 FROM world_actor_claims WHERE actor_id = world_actors.id)`. This is the `availableActors(worldId)` query's filter — never persisted as its own flag, to avoid a second source of truth that could drift from the claims table (research.md §2's whole point: availability and claimedness are independent inputs, not one stored value).
- **"What is my claim state in this world?"** = `myActorClaim(worldId)`: `NULL` if the caller has no row in `world_actor_claims` joined through their `world_members` row for that world; otherwise the claimed `GraphQLActor`.

## GraphQL types (new)

```graphql
type GraphQLActorClaim {
  actorId: ID!
  actor: GraphQLActor!
  worldMemberId: ID!
  claimedByUserId: ID!
  claimedAt: DateTime!
}
```

## Extensions to existing GraphQL types

- `GraphQLActor` (in `types.rs`) gains two `#[graphql(complex)]`-resolved fields, mirroring the `linked_from_lore`/`myPermissionLevel` pattern already established on `GraphQLItem`/`GraphQLActor`:
  - `availableForClaim: Boolean!` — the flat column, DM-and-Owner visible (not secret, but only meaningfully actionable by a GM)
  - `claimedBy: GraphQLWorldMember` (nullable) — who currently has this actor claimed, if anyone (FR-012); visible to a caller with Owner-level Actor permission, i.e. the GM
- `GraphQLWorld` gains `allowPlayerCreatedActors: Boolean!` (flat field, no join needed — it's a direct column).

## Validation / state rules

- `available_for_claim` MUST only ever be set to `true` on a PC-classified actor (`is_npc = false`) — enforced in the `setActorAvailability` resolver, not the database (matches how `is_npc` validity is enforced elsewhere: application-level, not a CHECK constraint, since Rust owns the actor-kind business rules).
- A claim row's existence implies `available_for_claim` becomes irrelevant for display purposes (an available-but-claimed actor is excluded from `availableActors`, per data-model's derived-value note above) but the flag itself is left `true` in storage — un-claiming does not require re-flipping availability, it becomes visible again automatically. This matches spec.md's Edge Case 4 ("un-marks... without it being claimed... otherwise unaffected") and Acceptance Scenario US3.3 ("becomes available again") without needing to track "was this auto-hidden or explicitly turned off" as separate state.
- `createAndClaimActor` is rejected server-side (independent of the client-side UI hiding the option) if `worlds.allow_player_created_actors` is `false` at the time of the call — FR-008's "evaluated at the time... not cached" applies to the mutation's authorization check, not just the screen's rendering.
