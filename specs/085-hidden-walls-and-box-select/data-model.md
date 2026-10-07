# Data Model: Hidden Walls and Box Select

**No persisted data changes.** There is no migration, no new column and no
new GraphQL type. Hidden is the existing `walls.secret`, already returned
by the wall queries and already on the web's `WorldWall.secret`.

What changes is session state in the engine and the world store.

## Engine

### `GroupSelection` (resource, new)

| Field    | Type          | Meaning                                |
| -------- | ------------- | -------------------------------------- |
| `tokens` | `Vec<String>` | Selected token ids, in selection order |
| `walls`  | `Vec<String>` | Selected wall ids                      |
| `lights` | `Vec<String>` | Selected light ids                     |
| `shapes` | `Vec<String>` | Selected shape ids                     |

- `len()` is the total across kinds. A group exists when `len() >= 2`.
- **Its relation to the single selections:**
  - `SelectedToken.0 == tokens`;
  - `SelectedWall.0 == walls.first()`, and the same for lights and shapes.
- **Writers:**
  - the box's release;
  - any single selection, which resets the group to that one item or to
    none;
  - a token changing hands (R9);
  - a removed item, which drops out.
- **It is never sent to the server.**

### `TokenOwner` (component, new)

`TokenOwner(Option<String>)` sits on every token entity, filled from
`WorldTokenPayload.owner_user_id`.

| Payload value | Effect                |
| ------------- | --------------------- |
| absent        | keeps the current one |
| `null`        | clears it             |
| a string      | sets it               |

### `BoxDrag` (resource, new)

`Option<{ anchor_screen: Vec2, current_screen: Vec2, past_threshold: bool }>`.
It lives while a box is being dragged.

### `GroupDrag` (resource, new)

It holds the following while a group drag is in progress:

- the press point;
- each member's pre-drag position. That is the token position, the wall's
  `x1`, `y1`, `x2` and `y2`, the light's `x` and `y`, or the shape's
  geometry.
- the group id.

## World store (`apps/web/src/engine/world/types.ts`)

| State field        | Type             | New?                 |
| ------------------ | ---------------- | -------------------- |
| `selectedTokenIds` | `string[]`       | exists               |
| `selectedWallId`   | `string \| null` | exists (the primary) |
| `selectedWallIds`  | `string[]`       | new                  |
| `selectedLightIds` | `string[]`       | new                  |
| `selectedShapeIds` | `string[]`       | new                  |

- `select_wall`, `select_light` and `select_shape` keep setting their
  primary, and they set their list to `[id]` or `[]`.
- `select_group` sets all four lists and the three primaries.
- `remove_wall`, `remove_light` and `remove_shape` drop the id from the
  lists as well as from the primary.

## Server

Unchanged. For the record, these existing rules are what this spec relies
on:

- **`walls.secret`**, set only by `setDoorSecret`. The setter must be the
  scene's DM, and the scene must not be paused.
- **Token moves**, checked per token in `mutations_tokens.rs:73`.
- **Shape edits**, decided by spec 082's `shape_authority`.
