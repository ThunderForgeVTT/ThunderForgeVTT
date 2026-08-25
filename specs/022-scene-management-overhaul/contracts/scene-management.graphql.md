# Contract: Scene Management GraphQL Surface

This documents the GraphQL contract additions/changes this feature introduces. It extends the existing `GraphQLScene`/`GraphQLWorld` types and their mutations rather than introducing a parallel API surface.

## Type changes

### `GraphQLScene` (additive fields)

```graphql
type GraphQLScene {
  # ...existing fields unchanged (sceneId, worldId, name, description, gridSize, gridType, width, height, ...)
  summaryMarkdown: String
  summaryRenderedHtml: String
  hidden: Boolean!
  previewUrl: String        # computed, e.g. "/scene-assets/{previewAssetId}/thumb"; null if no preview generated yet
}
```

### `GraphQLWorld` (additive fields)

```graphql
type GraphQLWorld {
  # ...existing fields unchanged
  defaultSceneGridType: String!   # "gridless" | "square" | "hex"
  activeSceneId: UUID             # null when nothing has been launched yet
}
```

## Query changes

### `scenes(worldId: UUID!): [GraphQLScene!]!`

Existing query — behavior changes only:
- **GM/Owner caller**: unchanged — returns every scene in the world, hidden or not (FR-009).
- **Player caller**: now excludes scenes where `hidden = true` (FR-008), mirroring the existing GM-vs-player branching already used for `shapes.visible_to_players`.

## Mutations

### `createScene(input: GraphQLCreateSceneInput!): GraphQLScene!`

Existing mutation — input unchanged; behavior changes only:
- When `input.gridType` is omitted, the server now applies `World.default_scene_grid_type` instead of always defaulting to `"square"` (FR-015).
- The created scene's `hidden` starts `true` regardless of input (FR-003) — not settable at creation time, only via `updateSceneHidden` below, to keep "create" and "publish" as distinct, deliberate actions.

### `updateScene(sceneId: UUID!, input: GraphQLUpdateSceneInput!): GraphQLScene!`

Existing mutation — input gains two optional fields:

```graphql
input GraphQLUpdateSceneInput {
  # ...existing fields unchanged (name, description, gridSize, gridType, width, height, metadata)
  summaryMarkdown: String   # when present, server re-renders summaryRenderedHtml
}
```

Authorization unchanged (owner-only, per existing `updateScene` implementation) — GM/Owner requirement already satisfied by that check.

### `updateSceneHidden(sceneId: UUID!, hidden: Boolean!): GraphQLScene!` — NEW

GM/Owner-only (server-enforced). Toggles `Scene.hidden`. Rejects with an authorization error for non-GM/Owner callers (FR-019).

### `launchScene(worldId: UUID!, sceneId: UUID!): GraphQLWorld!` — NEW

GM/Owner-only (server-enforced, FR-002c). Validates `sceneId` belongs to `worldId`. Sets `World.active_scene_id`, records a new `world_events` row via the existing `record_world_event`/`pg_notify` path (research.md §6), and returns the updated `GraphQLWorld`. This is the single mutation behind the Scenes section's "Launch" button (FR-002a) and is idempotent (launching the already-active scene is a no-op broadcast, per spec.md's Edge Cases).

### `updateWorldDefaultSceneGridType(worldId: UUID!, gridType: String!): GraphQLWorld!` — NEW

GM/Owner-only (server-enforced). Validates `gridType ∈ {"gridless", "square", "hex"}`. Mirrors the shape of the existing `updateWorldGenieResourceCarryover` single-purpose mutation (FR-014).

### dd2vtt import — unchanged

`POST /api/scenes/{scene_id}/import/uvtt` (REST, existing, `map_import` module) is reused as-is. The Scenes section's "Import" action calls this exact endpoint; no contract change.

## Subscription changes

### `worldEventsCreated(worldId: UUID!)` — existing subscription, new event payload variant

The existing `WorldEventsCreated` subscription (already delivering wall/token/light/shape/genie-session events over `/api/ws`) gains one more event code representing "scene launched," carrying `{ worldId, sceneId }`. Every client already subscribed for a given world (which today includes every open `/world/:id/play` tab) receives it with no new subscription to open — see research.md §6 and data-model.md's World Event section.

## Access control summary

| Action | Caller requirement |
|---|---|
| `scenes` query (unfiltered vs filtered) | Any world member; filtering branches on GM/Owner vs Player |
| `createScene` | GM/Owner |
| `updateScene` (incl. summary) | GM/Owner (existing owner check) |
| `updateSceneHidden` | GM/Owner |
| `launchScene` | GM/Owner |
| `updateWorldDefaultSceneGridType` | GM/Owner |
| dd2vtt import REST endpoint | GM/Owner (existing check, unchanged) |
