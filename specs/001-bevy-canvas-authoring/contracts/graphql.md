# GraphQL Contract Additions

Existing `Wall`/`WallMutation`/`walls` query are unchanged (Phase 6,
already shipped) and are the structural template for everything below.

## Types

```graphql
type LightSource {
  lightId: ID!
  sceneId: ID!
  x: Float!
  y: Float!
  radius: Float!
  intensity: Float!
  color: String
  attachedTokenId: ID
  metadata: JSON
  createdBy: ID!
  updatedBy: ID!
  createdAt: DateTime!
  updatedAt: DateTime!
}

enum AnnotationKind {
  STROKE
  SHAPE
  TEXT
}

type Annotation {
  annotationId: ID!
  sceneId: ID!
  kind: AnnotationKind!
  geometry: JSON!
  text: String
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
  annotations(sceneId: ID!): [Annotation!]!
}
```

- `lightSources`: returns all lights for the scene to any authenticated
  scene participant — light effects are player-visible by nature (FR-005),
  there is no GM-only light data.
- `annotations`: returns all annotations to the scene owner/GM caller;
  returns only `visibleToPlayers = true` annotations to any other
  authenticated participant (FR-009). Unauthenticated callers get the
  existing scene-query auth error, unchanged.

## Mutations

```graphql
input CreateLightSourceInput {
  sceneId: ID!
  x: Float!
  y: Float!
  radius: Float!
  intensity: Float
  color: String
  attachedTokenId: ID
  metadata: JSON
}

input UpdateLightSourceInput {
  x: Float
  y: Float
  radius: Float
  intensity: Float
  color: String
  attachedTokenId: ID
  metadata: JSON
}

input CreateAnnotationInput {
  sceneId: ID!
  kind: AnnotationKind!
  geometry: JSON!
  text: String
  visibleToPlayers: Boolean
  metadata: JSON
}

input UpdateAnnotationInput {
  geometry: JSON
  text: String
  visibleToPlayers: Boolean
  metadata: JSON
}

extend type Mutation {
  createLightSource(input: CreateLightSourceInput!): LightSource!
  updateLightSource(lightId: ID!, input: UpdateLightSourceInput!): LightSource!
  deleteLightSource(lightId: ID!): Boolean!

  createAnnotation(input: CreateAnnotationInput!): Annotation!
  updateAnnotation(annotationId: ID!, input: UpdateAnnotationInput!): Annotation!
  deleteAnnotation(annotationId: ID!): Boolean!
}
```

All six mutations enforce the identical scene-ownership check
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

## Real-time propagation

The existing PostgreSQL LISTEN/NOTIFY + world-event-subscription transport
(already used for invites/token-adjacent world events, per ADR-000/ADR-020)
is the correct mechanism to reuse — **but wall changes are not yet wired
into it**: `mutations_walls.rs` currently only writes to Postgres and does
not NOTIFY. To satisfy FR-003 ("propagate ... within a few seconds"),
this feature must:

1. Add NOTIFY emission to `create_wall`/`update_wall`/`delete_wall` (a gap
   in the already-shipped Phase 6 work, not new scope invented here).
2. Add the same NOTIFY emission to the new light/annotation mutations.
3. Extend `engine/world/sync` to subscribe to and dispatch these event
   types, the same way it already dispatches token events, so the engine's
   `WallSet`/`LightSet`/`AnnotationSet` resources update reactively.

No new transport, channel-naming scheme, or subscription type is
introduced — this closes an existing gap using the established pattern.
