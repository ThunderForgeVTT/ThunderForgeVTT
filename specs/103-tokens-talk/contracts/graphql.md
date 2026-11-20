# Contract: GraphQL changes

## `SendChatMessageInput`

```graphql
input SendChatMessageInput {
  worldId: UUID!
  sceneId: UUID
  body: String!
  gmOnly: Boolean
  """
  Spec 103: the token this line is said as. Omitted or null = said as the
  sender. Refused with FORBIDDEN when the sender may not speak as it
  (players: tokens they may move; Game Master: any token of the world), or
  when it is not on `sceneId`.
  """
  speakerTokenId: UUID
}
```

## `RollDiceInput`

```graphql
input RollDiceInput {
  # … existing fields …
  """Spec 103: as on SendChatMessageInput; the dice tray sends the composer's speaker."""
  speakerTokenId: UUID
}
```

## Chat message and roll outputs

These fields are added to the chat message type (the one with `authorLabel`)
and to the roll record type:

```graphql
  """Spec 103: the token it was said as, or null."""
  speakerTokenId: UUID
  """
  Spec 103: that token's name when said. A player reads "Someone" when the
  token's name is withheld from players (the nameplate rule).
  """
  speakerLabel: String
```

## Errors

| Case | Error |
|---|---|
| The token is not in this world, or not on `sceneId` | `FORBIDDEN` "You can't speak as that token" |
| A player names a token they may not move | `FORBIDDEN` (same message) |
| The token does not exist | `FORBIDDEN` (same message; does not reveal existence) |

## Demo backend

`apps/demo/src/backend/handlers.ts` `sendChatMessage` and `rollDice` accept
`speakerTokenId` and apply the same rules against the demo's in-memory
tokens, returning the same error code.
