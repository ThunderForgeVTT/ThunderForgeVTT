# Phase 1 Data Model: Hand-Drawn Authoring & Per-Campaign Asset Storage

## Existing entities (unchanged)

### Wall segment
Already persisted (spec 001). No schema change. This feature adds no new columns —
hand-authoring reuses the create/update/delete mutations in
`src/server/src/graphql/mutations_walls.rs` exactly as map-import does.

- `id`, `scene_id`, `x1`, `y1`, `x2`, `y2`, `is_door`, `door_open` (existing columns)
- Authorization: `scenes::owner_id.eq(user_id)` (existing, unchanged by this feature)

### Shape annotation
Already persisted (spec 001, `ShapePlugin`). No schema change.

- `id`, `scene_id`, `kind` (freehand/rect/ellipse/line/text), geometry payload, style
- Authorization: `scenes::owner_id.eq(user_id)` (existing, unchanged)

### World membership (`world_members`, `world_invites`)
Existing tables (`src/server/src/schema.rs:352`, `:367`; models at `models.rs:850`,
`:883`). No schema change. This feature adds a new *consumer* (the asset-write
authorization guard) but no new columns or states.

## New entity: Canvas image asset

Represents one uploaded/pasted image placed on a scene. Backed by a new Diesel
migration under `src/server/migrations/`.

| Column | Type | Notes |
|---|---|---|
| `id` | `uuid` (v7, PK) | matches existing PK convention (`uuid::v7`) |
| `world_id` | `uuid`, FK → `worlds.id` | owning campaign; scoping unit for STS credential |
| `scene_id` | `uuid`, FK → `scenes.id`, nullable | null only for a background image staged before scene placement, if that path is kept; otherwise required |
| `owner_user_id` | `uuid`, FK → `users.id` | first path segment; matches `created_by` convention (ADR `20260504-009`) |
| `created_by` | `uuid`, FK → `users.id` | existing provenance convention (may equal `owner_user_id`) |
| `updated_by` | `uuid`, FK → `users.id` | existing provenance convention |
| `storage_path` | `text` | `{owner_user_id}/{world_id}/{scene_id}/{id}.webp`, unique |
| `original_format` | `text` | source format as uploaded (e.g. `png`, `jpeg`), for diagnostics only |
| `width_px` / `height_px` | `int4` | decoded dimensions, used for canvas placement sizing |
| `byte_size` | `int8` | stored (post-transcode) size |
| `kind` | enum (`background`, `pasted`) | distinguishes map-import background vs. paste-to-canvas assets; both use this one table per FR-018 |
| `created_at` / `updated_at` | `timestamptz` | existing convention |

**Validation rules**:
- `storage_path` MUST be derivable purely from `(owner_user_id, world_id, scene_id, id)` — never client-supplied, to prevent path-traversal into another campaign's prefix (supports FR-014).
- `byte_size` MUST NOT exceed the existing `MAX_UPLOAD_BYTES` (50MB) ceiling reused from `map_import.rs` (FR-013).
- Row is only inserted after a successful RustFS write (no row for a rejected/oversized/failed upload) — no partial asset persisted (FR-013).

**Relationships**:
- `canvas_image_assets.world_id` → `worlds.id` (many assets per world)
- `canvas_image_assets.scene_id` → `scenes.id` (many assets per scene; a scene's background is one row with `kind = background`, referenced the same way `scenes.background_image_path` does today — see migration note below)
- `canvas_image_assets.owner_user_id` → `users.id`

**Migration note (FR-018)**: `scenes.background_image_path` (existing `text` column,
`schema.rs:204`) is migrated to reference a `canvas_image_assets` row (`kind =
background`) instead of a bare filesystem path string, so map-import and paste-to-canvas
share one storage mechanism and one authorization path. Existing rows get backfilled by
a one-time migration step that uploads each existing local-filesystem background image
into RustFS under the new path convention and inserts the corresponding
`canvas_image_assets` row.

**State transitions**: none — an asset is either present (after a successful write) or
absent (rejected/never written); deletion/lifecycle cleanup is explicitly out of scope
(Assumptions).
