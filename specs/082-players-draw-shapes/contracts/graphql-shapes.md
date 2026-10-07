# Contract: GraphQL for Players Draw Shapes

## New

```graphql
type ShapeCreator {
  userId: UUID!
  "users.username"
  displayName: String!
  "false once they left or were removed from the world"
  isMember: Boolean!
  "their shapes on this scene, every level"
  shapeCount: Int!
}

extend type Query {
  """
  Each creator of a shape on the scene who is not a DM of its world today,
  by displayName. DM of the scene only; anyone else gets NotFound.
  """
  shapeCreators(sceneId: UUID!): [ShapeCreator!]!
}

extend type Mutation {
  """
  DM of the scene only; anyone else gets NotFound. Gated by the pause.
  createdBy absent: every shape on the scene, every level.
  createdBy given: only those creators' shapes; [] deletes nothing.
  One transaction, one SHAPE_CHANGED event per shape with action "deleted".
  Answers the number deleted.
  """
  clearShapes(sceneId: UUID!, createdBy: [UUID!]): Int!
}
```

## Changed rules, unchanged signatures

```graphql
extend type Mutation {
  "DM of the scene, or a member holding `shapes`. A non-DM's shape is stored visibleToPlayers: true."
  createShape(input: CreateShapeInput!): Shape!

  "DM of the scene, or the shape's creator holding `shapes`. A non-DM's visibleToPlayers is ignored."
  updateShape(id: UUID!, input: UpdateShapeInput!): Shape!

  "DM of the scene, or the shape's creator holding `shapes`. Refused: false."
  deleteShape(id: UUID!): Boolean!
}
```

Refusals are what each mutation answers today for a missing shape or a
non-member: `createShape` and `updateShape` an error ("… not found or not
owned by you"), `deleteShape` `false`. A refusal changes no row and records
no event.

Every accepted write records `EVENT_CODE_SHAPE_CHANGED` (12) with
`{action, shape_id, scene_id}`, as today.

## Tool grants

```graphql
extend type Query {
  "The caller's effective tools: all six for a DM, defaults − revocations + grants for a member."
  authoringTools(worldId: UUID!): [String!]!

  """
  DM only. One entry per non-DM member, every member listed, with that
  member's effective tools. A member with no rows shows ["select","shapes"].
  """
  authoringToolGrants(worldId: UUID!): [MemberAuthoringTools!]!
}

extend type Mutation {
  """
  DM only, gated by the pause. Signature unchanged.
  tool select|shapes: granted false writes a revocation, true removes it.
  any other tool: granted true writes a grant, false removes it.
  Answers the member's effective tools.
  """
  setAuthoringToolGrant(
    worldId: UUID!
    worldMemberId: UUID!
    tool: String!
    granted: Boolean!
  ): [String!]!
}
```

`setAuthoringToolGrant` answers the member's tools today; it now answers
their effective tools, so the card can read the answer as it does.

## Pause surface

`clearShapes` joins `createShape`, `updateShape` and `deleteShape` in the
GATED table of `play_pause_surface_tables.rs`:

```rust
("clearShapes", r#"mutation { clearShapes(sceneId: "{scene}") }"#),
```
