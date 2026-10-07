# Contract: The Engine's Box, Group and Probe

All of this is engine ↔ web. None of it reaches the server.

## canvas-core (`crates/thunderforge-canvas-core/src/box_select.rs`)

```rust
pub struct ScreenBox { pub min: Vec2, pub max: Vec2 }        // world units, ordered
impl ScreenBox { pub fn from_corners(a: Vec2, b: Vec2) -> Self; }

pub enum Footprint {
    Point(Vec2),                       // a light
    Segment(Vec2, Vec2),               // a wall
    Rect { min: Vec2, max: Vec2 },     // a token's footprint, a shape's bounds
}

/// Every point of `f` lies inside `b`, edges inclusive.
pub fn wholly_inside(b: &ScreenBox, f: &Footprint) -> bool;

/// Without toggle: `hits`. With toggle: `current` minus the hits it holds,
/// then the hits it did not hold, in order. No duplicates either way.
pub fn apply_box(current: &[String], hits: &[String], toggle: bool) -> Vec<String>;
```

`crates/thunderforge-canvas-core/src/shape_geometry.rs` (after 082):

```rust
pub fn translate(kind: ShapeKind, geometry: &Value, delta: Vec2) -> Value;  // moved from systems/shape.rs
pub fn shape_bounds(kind: ShapeKind, geometry: &Value) -> Option<(Vec2, Vec2)>;
```

## Candidate rule (`crates/thunderforge-engine/src/systems/box_select.rs`)

```rust
pub enum BoxItem<'a> {
    Token { owner: Option<&'a str>, seen: bool },
    Wall,
    Light,
    Shape(&'a Shape),
}

pub fn box_candidate(
    is_gm: bool,
    viewer: Option<&str>,          // spec 082 ViewerUserId
    filter: &SelectionFilter,
    item: BoxItem<'_>,
) -> bool;
```

| Viewer | Token                     | Wall           | Light           | Shape                                  |
| ------ | ------------------------- | -------------- | --------------- | -------------------------------------- |
| GM     | `filter.tokens`           | `filter.walls` | `filter.lights` | `filter.shapes`                        |
| Player | `owner == viewer && seen` | never          | never           | `may_edit_shape(false, viewer, shape)` |

- `seen` is `Visibility != Hidden` on the token entity.
- A player with no viewer id takes nothing.

## Gestures (Select mode only)

| Input                                                    | Result                                                        |
| -------------------------------------------------------- | ------------------------------------------------------------- |
| Press on empty board, release within the click threshold | Clear the selection (today's behaviour), unless shift is held |
| Press on empty board, drag past the threshold, release   | Group = `apply_box(current, hits, shift)` per kind            |
| Shift-click on a token or a group member                 | Toggle that one item in the group                             |
| Press on a member of a group of 2 or more, drag, release | Move every member by one snapped offset (R6)                  |
| Delete or Backspace, group of 2 or more                  | `delete_*` for each member the viewer may delete (R10)        |

The box is drawn while dragging as a gizmo rectangle and is never sent.

## Events (engine → web, through the existing event channel)

```jsonc
// new
{ "type": "select_group",
  "tokenIds": ["…"], "wallIds": ["…"], "lightIds": ["…"], "shapeIds": ["…"] }

// existing per-item events gain an optional stamp when part of a group
{ "type": "upsert_token", "token": { … }, "group": { "id": "g-17", "size": 4 } }
{ "type": "update_wall",  "wallId": "…", "changes": { "x1":…, "y1":…, "x2":…, "y2":… }, "group": { … } }
{ "type": "update_light", "lightId": "…", "changes": { "x":…, "y":… }, "group": { … } }
{ "type": "update_shape", "shapeId": "…", "changes": { "geometry": … }, "group": { … } }
{ "type": "delete_wall" | "delete_light" | "delete_shape", "…Id": "…", "group": { … } }
```

- **The id.** A group id is a per-engine counter (`g-<n>`). It only has to
  be unique per board session.
- **The size.** `size` counts the events actually sent. A player's token
  refused at a wall is not sent, and not counted. The engine puts it back
  itself.
- **The door click** (`systems/wall.rs`): its `update_wall` changes are
  `{ doorState: "closed", locked: false }`. They no longer carry
  `secret: false`.

## Commands (web → engine)

```rust
// crates/thunderforge-engine/src/payloads.rs, ExternalCommand
DeleteSelection,                        // SDK delete_selection(); the Select bar's Delete
```

`WorldTokenPayload` gains:

```rust
#[serde(default, rename = "ownerUserId", deserialize_with = "absent_or_null")]
pub(crate) owner_user_id: Option<Option<String>>,
```

## Probe (`window.__engineProbe`, `apps/web/src/engine/bevy/index.ts`)

```ts
selection(): { tokens: string[]; walls: string[]; lights: string[]; shapes: string[] };
drawnWalls(): string[];   // wall ids this canvas draws a line for
```

The wasm sources are `selection_state()` and `wall_visuals()` in `sdk.rs`.
Each returns JSON, and both are present in every build, like the other
probe readers.
