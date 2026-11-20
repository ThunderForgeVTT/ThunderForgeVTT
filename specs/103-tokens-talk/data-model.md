# Data Model: Tokens Talk

## Chat line (`world_chat_messages`, extended)

| Field | Type | Notes |
|---|---|---|
| speaker_token_id | UUID NULL → tokens(id) ON DELETE SET NULL | Which token it was said as; null = said as the typist ("Game Master" for the GM) |
| speaker_label | TEXT NULL | The token's real name at send time; null when `speaker_token_id` is null at send |

Existing fields are unchanged (author_user_id, author_label, body, gm_only,
scene_id, …). Rows that already exist get nulls; they keep showing the typist
(spec assumption).

**Validation** (server, on send; research R4):

- the token exists, and its scene's world is the message's world
- when `scene_id` is given, the token is on that scene
- the sender is the world's Game Master, or `controllers::may_move(sender, token)`
- anything else is `FORBIDDEN`

**Read rule** (research R5): `speakerLabel` is returned verbatim to the Game
Master. A player gets it only when `player_may_read_token_name` holds, and
`"Someone"` otherwise.

## Roll (`world_roll_records`, extended)

The same two columns, with the same validation, set only by `rollDice` (the
dice tray) when the composer has a token speaker. Sheet rolls leave them
null; their float is placed by `actor_id`.

## Speaker choice (client only)

| Field | Type | Notes |
|---|---|---|
| key | `tf.speaker.<worldId>` | localStorage, try/catch |
| value | `"gm"` \| `"self"` \| token id | Falls back to the default (FR-004) when the token is no longer on the scene or no longer controlled |

## Float (client only, never stored)

| Field | Type | Notes |
|---|---|---|
| id | string | `chat:<messageId>` or `roll:<rollId>[:reveal]` |
| kind | `"line"` \| `"roll"` | |
| name | string | The token label, the typist's name, or "Game Master" |
| text | string | The line (cut with … past 140 chars on the map), or `"<label or kind> <result>"` with no formula |
| tokenId | string \| null | The resolved token on the current scene, or null |
| placement | `"over"` \| `"edge"` | The engine decides for a token; null → `"edge"` directly |
| expiresAt | number | `now + TALK_FLOAT_MS` (6000) |

### Lifecycle

```text
arrival (live, fresh, visible to me, chat closed)
   ├─ tokenId on current scene → engine `say` ─┬─ over (engine draws; fades itself)
   │                                           └─ speech_placed{edge} → edge stack
   └─ no token → edge stack
edge stack: push (newest at bottom), cap 4 (drop oldest), remove at expiresAt
chat opens → `clear_speech` + edge stack emptied; arrivals marked seen, not floated
```

### Roll → token resolution (FR-009, FR-010)

1. `speaker_token_id`, if it is on the current scene.
2. Otherwise a token on the current scene whose `actorId` is the roll's
   `actor_id`: the roller's own controlled one first, else the first.
3. Otherwise none (edge, under the roller's name).
