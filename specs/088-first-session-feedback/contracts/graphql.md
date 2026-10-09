# Contract: GraphQL changes

Every change is additive, apart from the auth on `worldByInviteCode` and
`alreadyMember`, and the new error codes. `schema.graphql` is regenerated,
and the web and demo clients are updated in the same commit.

## US1 World links

```graphql
type Query {
  # Now needs a signed-in caller. Signed out: null, no error detail.
  worldByInviteCode(code: String!): InvitePreview
  alreadyMember(code: String!): Boolean!
}

type Mutation {
  # maxUses 1..50 or null (no limit); expiresAt RFC 3339 or null. Defaults:
  # no limit and now + 7 days when the client leaves them out.
  # WorldInvite.maxUses becomes nullable; usedCount counts joins.
  generateInviteCode(worldId: UUID!, maxUses: Int, expiresAt: String): WorldInvite!
  revokeInviteCode(...): WorldInvite!   # unchanged
  joinWorld(code: String!): GraphQLWorld!                              # error codes below
}
```

`generateInviteCode` refusals:

| Case | Message |
| --- | --- |
| `maxUses` outside 1 to 50 | "A link can be used 1 to 50 times." |
| `expiresAt` not RFC 3339 | "That expiry is not a date." |
| `expiresAt` in the past | "That expiry has already passed." |

`joinWorld` errors, as `extensions.code`:

| Code | Message |
| --- | --- |
| `LINK_REVOKED` | "The GM has withdrawn this link. Ask them for a new one." |
| `LINK_EXPIRED` | "This link has expired. Ask your GM for a new one." |
| `LINK_USED_UP` | "This link has already been used. Ask your GM for a new one." Only a limited link, and also when a concurrent join took its last use. |
| `LINK_UNKNOWN` | "There is no world behind this link. Check it was copied whole." |
| `ALREADY_MEMBER` | not an error: `joinWorld` returns the world, uses nothing, and the page says "You're already in this world." The world's owner counts as a member. |

A use is counted only when `joinWorld` makes a new membership, in the same
transaction. `worldByInviteCode`, `alreadyMember`, and every refusal above
count nothing.

The join page reads the code to show the message. The rate limit on
`joinWorld` is unchanged, and it is what keeps `LINK_UNKNOWN` from being a
useful oracle.

## US2 Base maps

```graphql
type BaseMap {
  id: ID!
  name: String!
  width: Int!
  height: Int!
  gridSize: Int!
  thumbnailUrl: String!   # /api/base-maps/<id>.thumb.webp
  credit: MapCredit!
}

type MapCredit {
  author: String!
  licence: String!
  licenceUrl: String!
  source: String!
  catalog: String!
  shareAlike: String!
}

type Query {
  baseMaps: [BaseMap!]!          # signed-in; [] when the directory is missing
  defaultBaseMapId: ID           # null when the default is not available
}

input GraphQLCreateWorldInput {
  # ...existing fields
  baseMapId: ID                  # MaybeUndefined: absent = default, null = none
}

# createWorld(input: GraphQLCreateWorldInput!): GraphQLWorld!  -- return type unchanged

type GraphQLScene {
  # ...existing fields
  backgroundCredit: MapCredit    # null unless the background is a base map
}
```

`createWorld` refuses an unknown `baseMapId`, before creating anything,
with "That map is not available on this instance."

If the map cannot be applied after the world is created, `createWorld`
still returns the world, and adds a non-fatal error to the response
(`ctx.add_error`):

| `extensions.code` | Message |
| --- | --- |
| `STARTING_MAP_FAILED` | "Your world is ready, but its map could not be added. You can import it from the scene." |

`data.createWorld` is set, so a client that ignores errors still works.

## US5 Clear rolls

```graphql
type Mutation {
  clearWorldRolls(worldId: UUID!): ClearRollsResult!
}

type ClearRollsResult {
  clearedAt: DateTime!
}
```

| Case | Refusal |
| --- | --- |
| caller does not run the world, and is not a site admin | "Only the GM can clear the rolls." |
| play is paused | the existing paused message |

`revealRoll` and `rerollRoll` gain one refusal: "That roll was
cleared." The world event is `ROLLS_CLEARED` (39), payload
`{ clearedAt }` (data-model.md).

## US6 Edge walls

```graphql
input GraphQLUpdateSceneLevelInput {   # updateSceneLevel(levelId, input)
  # ...existing fields
  wallEdges: Boolean = true      # used only when backgroundAssetId changes
}
```

The REST map import (`POST /scenes/{scene_id}/import/uvtt`, `map_import/mod.rs:81`) takes a `wallEdges`
form field (`true` when absent) and adds to its JSON answer:

```json
{ "wallsCreated": 4, "perimeterWallsCreated": 4, "...": "unchanged" }
```

`wallsCreated` includes the perimeter. The `MAP_IMPORTED` payload gains
`perimeter_walls_created`.

## US7 Mail

No schema change. The form sends the existing
`updateInstanceSetting(key, value)`, once per changed key.

## US8 Hero polish

No schema change.
