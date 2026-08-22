# Contract: `createWorld` GraphQL Mutation (behavior change, no shape change)

## Existing, unchanged input/output shape

```graphql
input GraphQLCreateWorldInput {
  name: String!
  description: String
  gameSystemId: String
  interfacePackId: String
}

createWorld(input: GraphQLCreateWorldInput!): GraphQLWorld!
```

Neither the input nor the `GraphQLWorld` output type gains, loses, or renames any field. `gameSystemId`/`interfacePackId` remain accepted (already optional) — the frontend simply stops sending them (research.md §4); a caller that still sends them (e.g. a future admin tool) continues to work unchanged.

## Behavior change

- **Before**: inserts one `worlds` row. No `scenes` row is created.
- **After**: inserts one `worlds` row **and** one `scenes` row (data-model.md's default-scene shape), in a single DB transaction. Both succeed or both fail — a caller of `createWorld` can rely on the returned world always having exactly one scene immediately queryable via the existing `scenes(worldId: ID!)` query, with no separate call required.
- Authorization is unchanged: still gated by `authenticated_user(ctx)`, the returned world's `createdBy` is still the calling user, and the new scene's `ownerId` is the same user — no new authorization surface, no new client-supplied trust decision.
- Error behavior: if the scene insert fails for any reason (should not happen under normal operation, since it uses no new validation beyond the already-validated world name), the world insert is rolled back too — the client sees a single mutation error, not a partially-created world.

## Unrelated, unchanged: `createScene` mutation

`createScene(input: GraphQLCreateSceneInput!)` is untouched — still available for adding further scenes to an existing world (e.g. via `SceneSwitcher`'s "New scene" dialog), with its own existing defaults (data-model.md's table mirrors them exactly, since `create_world`'s new scene-insert reuses this same defaulting logic internally rather than duplicating it).
