# Contract: The Engine's Viewer and Shape Ownership

## Command

```rust
// crates/thunderforge-engine/src/payloads.rs, ExternalCommand
SetViewerUser { user_id: Option<String> },
```

- SDK name `set_viewer_user` (`sdk.rs`), handled in `app.rs` beside
  `SetIsGameMaster`; it replaces the `ViewerUserId` resource.
- Web: `setViewerUser(userId: string | null)` in
  `apps/web/src/engine/bevy/index.ts`, called from `WorldPage.tsx` in the
  effect that calls `setIsGameMaster`, so it is re-sent when the engine
  becomes ready.
- Never synced to other clients.

## Payload

`WorldShapePayload.createdBy: string | null`, copied into the core
`Shape.created_by`.

## Rule

```rust
pub fn may_edit_shape(is_gm: bool, viewer: Option<&str>, shape: &Shape) -> bool
```

True for a GM; otherwise true only when `viewer` and `shape.created_by` are
both present and equal.

| System (`systems/shape.rs`, `plugins/context_menu.rs`) | Was            | Now                                                       |
| ------------------------------------------------------ | -------------- | --------------------------------------------------------- |
| `handle_shape_tool_selection`                          | `IsGameMaster` | `tool_is_allowed(Shapes)`                                 |
| `handle_shape_input` (draw)                            | `IsGameMaster` | `tool_is_allowed(Shapes)`                                 |
| `handle_shape_input` (hit-test, drag)                  | `IsGameMaster` | `tool_is_allowed(Select or Shapes)` and `may_edit_shape`  |
| `handle_shape_keyboard_toggles` (visibility)           | `IsGameMaster` | GM only (a player's shape is always visible)              |
| `handle_shape_keyboard_toggles` (other), delete        | `IsGameMaster` | `may_edit_shape` on the selection                         |
| `handle_shape_undo`                                    | `IsGameMaster` | `tool_is_allowed(Shapes)`; the stack holds only own edits |
| `sync_shape_visuals` (GM-only tint)                    | `IsGameMaster` | unchanged: only a GM sees hidden shapes                   |
| shape context menu                                     | `is_gm`        | `may_edit_shape`; the visibility item stays GM only       |

A shape the viewer may not edit is skipped by hit-testing, so a click on it
falls through to whatever is beneath.
