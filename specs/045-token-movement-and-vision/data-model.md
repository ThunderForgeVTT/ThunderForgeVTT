# Data Model: Token Movement and Vision

What already exists, what changes, and what is new. Nothing here stores a
player's explored areas on the server: that is theirs, in their browser
(decision 3).

## Existing, unchanged

- **Wall** (`walls`): endpoints, `blocks_vision`, `blocks_movement`,
  `door_state` (`none` | `closed` | `open`), `locked`, `secret`. This feature
  finally *reads* `blocks_movement`; it adds no column.
- **Token** (`tokens`): position, `owner_user_id`, `is_primary`, `scale`,
  `actor_id`. Movement writes position as it does today.
- **Scene** (`scenes`): grid kind and size, `ambient_light`. Exploration adds
  two fields below.

## Changed

### Scene

| Field | Type | Default | Why |
|---|---|---|---|
| `exploration_enabled` | bool | `false` | FR-070: off for every existing and new scene until a Game Master turns it on. |
| `exploration_epoch` | int | `0` | Bumped by a reset. A browser holding an older epoch drops what it kept (FR-078). |

A per-player reset needs to reach one player only, so the epoch is paired with
an optional per-player stamp:

| Field | Type | Why |
|---|---|---|
| `scene_exploration_resets(scene_id, user_id, epoch)` | row | A reset for one player. A client takes the greater of the scene's epoch and its own row. |

### Move (`moveOwnToken`)

Gains an optional **path**: the ordered points or cells the token passed
through. Absent means "a straight line from where it is to where it is going".
The token's stored position remains the authority for where the move began.

### Light source (T067, 2026-09-15)

| Field | Type | Default | Why |
|---|---|---|---|
| `bright_radius` | double | `radius * 0.5` for existing rows; half the radius on a create that names none | FR-061: a placed light's bright reach. `radius` stays its dim reach. Never negative, never beyond `radius` (a check constraint). FR-062: a light saved with one radius keeps its look. |

## New concepts

### Move judgement (server, `src/server/src/movement/`)

Not a table — a decision, computed per request.

| Field | Meaning |
|---|---|
| `from` | the token's stored position |
| `to` | the requested position |
| `path` | the segments judged, derived from the request's path or the straight line |
| `verdict` | `allowed`, or `refused` with the wall that stopped it |

Rules: a segment that properly crosses a wall with `blocks_movement`, or a
closed door, is refused; passing through the point where two walls meet counts
as crossing; a Game Master is never judged (decision 1).

### Vision profile (per token, resolved)

Resolved by the server from the actor's system data, using the pack's
declaration. Darkvision is handed to the engine by the existing
`set_token_vision`; the carried light by `set_carried_light`, as a light
attached to the token rather than part of its profile (spec decision 6,
2026-09-14).

| Field | Meaning | Today |
|---|---|---|
| `darkvision` | how far darkness is seen as dim light | exists in the engine (`VisionProfile`), set by nothing |
| `light_bright` / `light_dim` | the reach of a light the character carries | an engine light attached to the token, id `carried:<tokenId>`, with its own bright reach (T065) |
| `facing` / `fov` / `max_range` | a vision cone and a hard limit | exist in the engine; not used by this feature |

### Explored area (browser only)

Stored in the IndexedDB the world cache already uses, never on the server.

| Field | Meaning |
|---|---|
| key | `(user id, world id, scene id)` |
| `cells` | a coarse grid of what this player's token has seen, packed |
| `epoch` | the scene epoch this was accumulated under |
| `updated_at` | for housekeeping and for the storage panel's figures |

State: a client drops its `cells` when the scene's epoch (or its own reset row)
is newer than the stored `epoch`, and when the player clears their browser
storage — which is theirs to lose (FR-073).

## Relationships

```text
Scene 1───* Wall            (walls block movement and sight; doors are walls)
Scene 1───* Token           (a token moves within a scene)
Token *───1 Actor           (an actor's system data is where vision comes from)
Actor  1───1 VisionProfile  (resolved per token, by the pack's declaration)
Player 1───* ExploredArea   (one per scene, in that player's browser)
Scene  1───1 exploration epoch ──▶ invalidates ExploredArea
```

## Validation

- A path longer than a bound (proposed: 64 segments) is refused rather than
  judged, so a client cannot make the server do unbounded geometry.
- An explored-area record beyond a size bound is dropped rather than grown.
- `exploration_enabled` and a reset are Game-Master-only (existing scene
  authorisation).
- A move refused by the server leaves the token at its stored position, and the
  refusal names a wall, never a coordinate the client did not already have.
