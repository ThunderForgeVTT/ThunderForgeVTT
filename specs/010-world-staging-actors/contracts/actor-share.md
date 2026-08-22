# Contract: Actor Sharing and Cross-World Copy (new)

## Shape

```graphql
type GraphQLActorShareLink {
  id: ID!
  actorId: ID!
  shareCode: String!
  revoked: Boolean!
  createdAt: String!
}

type SharedActorPreview {
  label: String!
  actorType: String!
  isNpc: Boolean!
  gameSystemId: String
  systemData: GraphQLActorSystemData   # existing type, reused as-is (ability/resource/proficiency/trait/spell data)
}

# Deliberately excludes id/worldId/sceneId/createdBy/ownedBy (research.md §9)

createActorShareLink(actorId: ID!): GraphQLActorShareLink!
revokeActorShareLink(shareId: ID!): Boolean!

sharedActor(shareCode: String!): SharedActorPreview!   # query — 404-equivalent error if revoked/not found
myDmWorlds: [GraphQLWorld!]!                            # query — see research.md §8

input CopySharedActorInput {
  shareCode: String!
  destinationWorldId: ID!
}

copySharedActorToWorld(input: CopySharedActorInput!): GraphQLWorldActor!   # mutation
```

## Behavior

- `createActorShareLink`: generates a new, opaque `share_code` (same style as `world_invites.invite_code`) and a `world_actor_shares` row with `revoked = false`. Calling it again for the same actor creates an *additional* link (independently revocable) rather than reusing/replacing an existing one — the spec does not require exactly-one-link-per-actor.
- `sharedActor(shareCode)`: resolves the link, returns the `SharedActorPreview` projection if `revoked = false` and the actor still exists; otherwise returns a clear "not available" error (FR-024's edge case) — the client renders this as a "no longer available" state, not a generic error page.
- `myDmWorlds`: returns worlds where the caller is Owner (`created_by`) or holds an accepted `GM` `world_members` row (research.md §8) — used to populate the "Copy to World" destination picker; a caller with zero results sees an empty, explanatory state instead of the picker (spec Edge Cases).
- `copySharedActorToWorld`: resolves the share link (same validity check as `sharedActor`), verifies the caller holds DM-level access on `destinationWorldId` (re-checked here even though `myDmWorlds` already filtered to it — the server never trusts the client's earlier read), then in one transaction: inserts a new `world_actors` row (fresh id, `world_id`/`scene_id` = destination world's default scene, `owned_by`/`created_by` = caller, no permission rows) and clones every `world_actor_system_data` row belonging to the source actor onto the new actor id. Returns the newly created actor.

## Authorization

- `createActorShareLink`: caller's `myPermissionLevel` (see `actor-crud.md`) for `actorId` MUST be `OWNER` (FR-023 — includes the DM's implicit Owner).
- `revokeActorShareLink`: caller MUST be either the link's `created_by` OR DM of the actor's world (FR-029's "actor-level Owner (or DM)" — note this is deliberately slightly broader than `createActorShareLink`'s gate, since a DM must always be able to shut down a link even one they didn't personally create).
- `sharedActor`: caller MUST be authenticated (any logged-in user) — no world-membership check, by design (research.md §9's whole point).
- `myDmWorlds`: caller MUST be authenticated; returns only the caller's own worlds, never another user's.
- `copySharedActorToWorld`: caller MUST be authenticated AND hold DM-level access (Owner or GM) on `destinationWorldId`, re-verified server-side regardless of what `myDmWorlds` returned earlier.

## Non-goals

- No usage cap or expiry field on the share link (spec Assumptions: persistent, uncapped by default) — only `revoked` gates access.
- No "who has viewed/copied this link" analytics — out of scope.
- No live/referential relationship recorded between source and copy after `copySharedActorToWorld` completes (FR-027) — nothing in this contract's response or any table links the two beyond the one-time copy operation itself.
