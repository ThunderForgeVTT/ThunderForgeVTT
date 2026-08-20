# API Contract Additions (GraphQL + one REST upload endpoint)

Existing `Wall`/`WallMutation`/`walls` query are unchanged in shape (Phase
6, already shipped) except for the new door field, and remain the
structural template for everything below.

## Types

```graphql
type Wall {
  wallId: ID!
  sceneId: ID!
  x1: Float!
  y1: Float!
  x2: Float!
  y2: Float!
  blocksVision: Boolean!
  blocksMovement: Boolean!
  doorState: DoorState!        # NEW
  metadata: JSON
  createdBy: ID!
  updatedBy: ID!
  createdAt: DateTime!
  updatedAt: DateTime!
}

enum DoorState {               # NEW
  NONE
  OPEN
  CLOSED
}

type LightSource {
  lightId: ID!
  sceneId: ID!
  x: Float!
  y: Float!
  radius: Float!
  intensity: Float!
  color: String
  attachedTokenId: ID
  castsShadows: Boolean!
  metadata: JSON
  createdBy: ID!
  updatedBy: ID!
  createdAt: DateTime!
  updatedAt: DateTime!
}

enum ShapeKind {
  STROKE
  RECT
  ELLIPSE
  LINE
  TEXT
}

type Shape {
  shapeId: ID!
  sceneId: ID!
  kind: ShapeKind!
  geometry: JSON!
  text: String
  style: JSON
  visibleToPlayers: Boolean!
  metadata: JSON
  createdBy: ID!
  updatedBy: ID!
  createdAt: DateTime!
  updatedAt: DateTime!
}
```

## Queries (extend `queries/scene.rs`)

```graphql
extend type Query {
  lightSources(sceneId: ID!): [LightSource!]!
  shapes(sceneId: ID!): [Shape!]!
}
```

- `lightSources`: returns all lights for the scene to any authenticated
  scene participant — light effects are player-visible by nature (FR-005),
  there is no GM-only light data.
- `shapes`: returns all shapes to the scene owner/GM caller; returns only
  `visibleToPlayers = true` shapes to any other authenticated participant
  (FR-009). Unauthenticated callers get the existing scene-query auth
  error, unchanged.
- The existing `walls` query gains `doorState` in its returned type; no
  change to the query signature or the player-facing filtering it already
  performs (walls are already visible to all scene participants, only
  editing is GM-only).
- The existing scene payload gains `backgroundImagePath` (nullable
  String), resolved by the web app against the existing static-asset
  route (data-model.md's `Scene.background_image_path`).

## Mutations

```graphql
input UpdateWallInput {
  x1: Float
  y1: Float
  x2: Float
  y2: Float
  blocksVision: Boolean
  blocksMovement: Boolean
  doorState: DoorState        # NEW field on the existing input
  metadata: JSON
}

input CreateLightSourceInput {
  sceneId: ID!
  x: Float!
  y: Float!
  radius: Float!
  intensity: Float
  color: String
  attachedTokenId: ID
  castsShadows: Boolean
  metadata: JSON
}

input UpdateLightSourceInput {
  x: Float
  y: Float
  radius: Float
  intensity: Float
  color: String
  attachedTokenId: ID
  castsShadows: Boolean
  metadata: JSON
}

input CreateShapeInput {
  sceneId: ID!
  kind: ShapeKind!
  geometry: JSON!
  text: String
  style: JSON
  visibleToPlayers: Boolean
  metadata: JSON
}

input UpdateShapeInput {
  geometry: JSON
  text: String
  style: JSON
  visibleToPlayers: Boolean
  metadata: JSON
}

extend type Mutation {
  createLightSource(input: CreateLightSourceInput!): LightSource!
  updateLightSource(lightId: ID!, input: UpdateLightSourceInput!): LightSource!
  deleteLightSource(lightId: ID!): Boolean!

  createShape(input: CreateShapeInput!): Shape!
  updateShape(shapeId: ID!, input: UpdateShapeInput!): Shape!
  deleteShape(shapeId: ID!): Boolean!
}
```

`updateWall`'s existing signature is unchanged (`wallId: uuid::Uuid,
input: GraphQLUpdateWallInput`); `GraphQLUpdateWallInput` simply gains an
optional `door_state` field, exactly like any other optional patch field
already in that struct.

All mutations above enforce the identical scene-ownership check
`mutations_walls.rs::create_wall` already implements: the caller must own
the parent scene, checked server-side via
`scenes::table.filter(scenes::owner_id.eq(user_id))`, returning
`DieselError::NotFound` (surfaced as a generic GraphQL error, not a
detailed permissions message, matching existing wall-mutation behavior)
when the check fails. This is FR-010 made concrete.

`radius > 0` and `intensity >= 0` are validated by the database CHECK
constraints (data-model.md); the mutation returns a GraphQL error if
violated, same pattern as any other Diesel constraint violation in this
codebase.

## Map Import (REST, not GraphQL — binary upload)

GraphQL is not a good fit for a multi-megabyte binary/base64 file upload;
this reuses the existing Axum `Multipart` upload pattern already
implemented for game-system package uploads (`src/server/src/systems.rs`)
instead of forcing it through GraphQL.

```text
POST /api/scenes/{scene_id}/import/uvtt
Content-Type: multipart/form-data
  field "file": the .dd2vtt JSON file

200 OK
{
  "wallsCreated": 42,
  "doorsCreated": 2,
  "lightsCreated": 12,
  "backgroundImageSet": true
}

400 Bad Request   — malformed JSON, unsupported format version, or a
                     degenerate line_of_sight polygon (FR-024)
403 Forbidden      — caller does not own the target scene (FR-025)
413 Payload Too Large — file exceeds the configured upload size cap
```

Authorization: identical scene-ownership check as GraphQL wall/light/shape
mutations, applied once for the request (not per created row) before any
writes begin. On any validation failure, nothing is written — the handler
parses and validates the entire file before opening the DB transaction
that performs the batch insert described in data-model.md's "Map Import"
section.

The response counts (not the created rows themselves) are returned
directly; the web app re-fetches `walls`/`lightSources`/the scene payload
via the normal queries afterward — no separate "import result" GraphQL
type is introduced.

## Real-time propagation

The existing PostgreSQL LISTEN/NOTIFY + world-event-subscription transport
(already used for invites/token-adjacent world events, per ADR-000/ADR-020)
is the correct mechanism to reuse — **but wall changes are not yet wired
into it**: `mutations_walls.rs` currently only writes to Postgres and does
not NOTIFY. To satisfy FR-003 ("propagate ... within a few seconds"),
this feature must:

1. Add NOTIFY emission to `create_wall`/`update_wall`/`delete_wall` (a gap
   in the already-shipped Phase 6 work, not new scope invented here).
2. Add the same NOTIFY emission to the new light/shape mutations, and to
   the map-import handler (one NOTIFY per created batch is sufficient —
   clients re-fetch the affected queries rather than replaying every row).
3. Extend `engine/world/sync` to subscribe to and dispatch these event
   types, the same way it already dispatches token events, so the engine's
   `WallSet`/`LightSet`/`ShapeSet` resources update reactively.

No new transport, channel-naming scheme, or subscription type is
introduced — this closes an existing gap using the established pattern.
