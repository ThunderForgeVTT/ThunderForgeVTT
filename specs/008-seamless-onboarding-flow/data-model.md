# Phase 1 Data Model: Seamless Sign-Up-to-Canvas Onboarding Flow

## No schema changes

This feature introduces no new tables and no new columns. Both entities it touches — `worlds` and `scenes` — are unchanged in shape (specs 001-006 remain their authority).

## Behavioral change: `create_world` now always produces exactly one `Scene`

| Before | After |
|---|---|
| `create_world` inserts one `worlds` row. A world can exist with zero `scenes` rows (the common case today for a freshly created world). | `create_world` inserts one `worlds` row **and** one `scenes` row, in the same DB transaction. A world created through this mutation can never have zero scenes. |

### The auto-created default scene

Uses `create_scene`'s existing default values verbatim (`graphql.rs:969`, research.md §1) — no new defaulting logic:

| Field | Value |
|---|---|
| `scene_id` | fresh `Uuid::now_v7()` |
| `world_id` | the just-inserted world's id |
| `name` | the world's own name (e.g. a world named "The Ember Crown" gets a scene named "The Ember Crown") |
| `description` | `None` |
| `type_` | `"battlemap"` |
| `grid_size` | `5` |
| `grid_type` | `"square"` |
| `width` / `height` | `100` / `100` |
| `metadata` | `None` |
| `owner_id` | the authenticated user creating the world (same as `worlds.created_by`) |
| `background_image_path` / `background_asset_id` | `None` (no default background — an empty grid, consistent with what `create_scene`'s dialog already produces today) |

### Validation rules (new)

- The world-insert and scene-insert MUST both succeed or both fail (single transaction) — no code path may commit a world without its default scene, or a scene without its world.
- No new validation on the scene's fields beyond what `create_scene` already enforces (name derived from the already-validated world name, so no separate empty-name case is possible).

## Entity summary (maps to spec.md's Key Entities)

- **World**: existing entity, unchanged shape. `create_world`'s side effect changes (now also creates a scene), not its own fields.
- **Scene**: existing entity, unchanged shape. One row is now created automatically per world, using entirely pre-existing default values.
- **User Session / Landing State**: confirmed not a new persisted entity (spec.md) — the zero-vs-nonzero-worlds landing decision (research.md §2) is computed client-side from `getMyWorlds()`'s existing response, not stored.

## State transition

```text
(no world) --user submits create-world form (name only)--> world + 1 default scene, both committed atomically
                                                          --> user lands directly on /world/:id/play
                                                          --> WorldPage's existing scene-gating logic
                                                              (scenes.length > 0 || isSceneOwner) already
                                                              renders the scene picker + canvas content,
                                                              unchanged from how it behaves for any world
                                                              that already has scenes today
```
