# Contract: Actor Creation and Field Editing (new)

## Shape

```graphql
enum ActorPermissionLevel {
  VIEWER
  EDITOR
  OWNER
}

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
  ownedBy: ID!             # unchanged field, no longer authorization-relevant (research.md §4)
  myPermissionLevel: ActorPermissionLevel!   # NEW — server-computed, see data-model.md
  createdAt: String!
  updatedAt: String!
}

input CreateActorInput {
  worldId: ID!
  label: String!
  isNpc: Boolean!
  actorType: String          # defaults to "npc" / "character" based on isNpc if omitted
  gameSystemId: String
}

input UpdateActorInput {
  actorId: ID!
  label: String
  isNpc: Boolean
  actorType: String
}

createActor(input: CreateActorInput!): GraphQLWorldActor!
updateActor(input: UpdateActorInput!): GraphQLWorldActor!
```

## Behavior

- `createActor`: inserts a new `world_actors` row — `scene_id` defaults to the target world's earliest-created scene (research.md §6), `owned_by` and `created_by` are both set to the caller, no `world_actor_permissions` rows are created (the creator, as DM, already has implicit `Owner` per `myPermissionLevel`'s resolution — an explicit row would be redundant).
- `updateActor`: only fields provided in the input are changed; omitted fields are left as-is (same partial-update convention as `UpdateActorSystemDataInput`).
- `worldActors(worldId)` (existing query, unchanged shape except the added `myPermissionLevel` field) continues to return every actor in the world to every member — permission gates *editing*, not *listing* (same precedent as spec 009's contract).

## Authorization

- `createActor`: caller MUST be DM (Owner or GM role) of `input.worldId` (FR-019). Non-DM callers get the same rejection shape existing DM-only mutations use.
- `updateActor`: caller's `myPermissionLevel` for `input.actorId` MUST be `EDITOR` or `OWNER` (FR-010, FR-011) — computed via `require_actor_permission(..., minimum: Editor)` (research.md §4). A `VIEWER`-level caller (default or explicit) is rejected; the frontend's `/edit` route MUST NOT even be reachable in that case (FR-011 — redirect to `/view` before attempting the mutation).
- Both require the caller to be a member (or admin) of the actor's world at all, via the same `require_visible_world`/`require_world_member` pattern every other world-scoped mutation already uses.

## Non-goals

- No delete-actor mutation is specified by this feature (not requested; actors persist once created).
- No scene-reassignment field on `UpdateActorInput` — moving an actor between scenes is an existing, separate concern untouched by this feature (research.md §6).
