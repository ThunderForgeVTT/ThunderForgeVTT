# Contract: Token GraphQL Mutations (extended + new)

## Existing, unchanged authorization — extended input shape

### `update_token` (GM/scene-owner only, existing mutation, extended input)

```graphql
input GraphQLUpdateTokenInput {
  tokenId: ID!
  x: Float
  y: Float
  rotation: Float
  scale: Float
  actorId: ID
  metadata: JSON
  # New this feature:
  ownerUserId: ID
  isPrimary: Boolean
  photoUrl: String
  health: Int
  maxHealth: Int
}

updateToken(input: GraphQLUpdateTokenInput!): Token!
```

- Authorization unchanged: succeeds only when the requesting user owns the scene (`scenes.owner_id`), exactly like `mutations_walls.rs`'s `update_wall`.
- Setting `isPrimary: true` for a `(sceneId, ownerUserId)` pair that already has a primary token replaces the prior primary (the DB partial unique index `tokens_one_primary_per_owner_per_scene` prevents two simultaneous primaries — the mutation must clear the prior one in the same transaction, not error).
- All fields optional/nullable in the input; only fields present are updated (existing `AsChangeset` convention).

## New mutations — player-initiated, narrowly scoped

### `move_own_token` (token-controller only)

```graphql
moveOwnToken(tokenId: ID!, x: Float!, y: Float!): Token!
```

- Succeeds only where `tokens.owner_user_id == <authenticated user>`. No effect (returns an authorization error, consistent with existing mutation error conventions — not a silent no-op that returns 200) if the token is not owned by the requester.
- Touches only `x`, `y`. Does not accept or alter `rotation`, `scale`, `photoUrl`, `health`, or `ownerUserId` — a request attempting to set any of those fields on this mutation is a client error (the mutation's input type has no such fields at all, so this is enforced by the GraphQL schema shape itself, not runtime validation).
- This is the mutation User Story 3 / FR-009 route through.

### `set_own_primary_token_photo` (primary-token owner only)

```graphql
setOwnPrimaryTokenPhoto(tokenId: ID!, photoUrl: String!): Token!
```

- Succeeds only where `tokens.owner_user_id == <authenticated user> AND tokens.is_primary == true` for the given `tokenId`.
- Touches only `photoUrl`.
- This is the mutation FR-009a routes through.

## Unchanged mutations

- `create_token`, `delete_token` — GM/scene-owner only, unchanged (players cannot create tokens, per FR-009b).

## Verification

- A non-owning player calling `moveOwnToken` on a token they don't control MUST receive an authorization error and the token's position MUST be unchanged on re-query (SC-003).
- A GM calling `update_token` MUST still be able to set every field, including the five new ones, exactly as before this feature for pre-existing fields.
- Setting a second token's `isPrimary: true` for the same `(sceneId, ownerUserId)` MUST leave exactly one primary token for that pair after the mutation completes (verified by re-query, not just the mutation's return value — per the spec 003 round-trip-verification convention).
