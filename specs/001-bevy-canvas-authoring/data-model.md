# Phase 1 Data Model: Native Canvas — Full tldraw Replacement

All new entities are per-scene, UUID-v7-keyed, and carry
`created_by`/`updated_by` provenance per Constitution Principle III,
mirroring the shipped `walls` table exactly in shape/conventions.

## Wall (existing, extended with door fields)

Base fields already implemented (Phase 6). Door fields are new, added by
this feature (FR-017) via an additive migration (`ALTER TABLE walls ADD
COLUMN ...`), not a new table — a door is structurally a wall.

| Field | Type | Notes |
|---|---|---|
| `wall_id` | UUID PK | existing |
| `scene_id` | UUID FK → `scenes` | existing, `ON DELETE CASCADE` |
| `x1, y1, x2, y2` | double precision | existing, segment endpoints, scene-local coords |
| `blocks_vision` | bool, default true | existing |
| `blocks_movement` | bool, default false | existing |
| `door_state` | text, `CHECK (door_state IN ('none', 'open', 'closed'))`, default `'none'` | **NEW**. `'none'` = ordinary wall; `'open'`/`'closed'` = a door. Import maps UVTT `portals[].closed` → `'closed'`/`'open'`. |
| `metadata` | jsonb, nullable | existing, free-form GM notes/tags |
| `created_by`, `updated_by` | UUID FK → `users` | existing |
| `created_at`, `updated_at` | timestamp | existing |

**Door semantics**: when `door_state = 'open'`, the engine treats
`blocks_vision`/`blocks_movement` as `false` regardless of their stored
value (the stored flags describe the door's *closed* behavior, not its
current behavior); when `'closed'`, the stored flags apply normally; when
`'none'`, the flags always apply (unchanged, existing behavior).

**Validation** (already enforced for the base fields, unchanged): caller
must own the parent scene (`scenes.owner_id = auth_user.user_id`) for
create/update/delete. Toggling `door_state` goes through `update_wall`,
so it inherits the same ownership check — no separate "toggle door"
mutation is needed (spec Assumption: door toggling is GM-only in this
iteration).

## LightSource (new)

| Field | Type | Notes |
|---|---|---|
| `light_id` | UUID PK, default `gen_random_uuid()` | |
| `scene_id` | UUID FK → `scenes` | `ON DELETE CASCADE` |
| `x, y` | double precision | position, scene-local coords |
| `radius` | double precision, `CHECK (radius > 0)` | illumination radius |
| `intensity` | double precision, default `1.0`, `CHECK (intensity >= 0)` | 0 = off, relative brightness |
| `color` | text, nullable | e.g. `"#ffcc66"`; nullable = engine default warm-white |
| `attached_token_id` | UUID, nullable, FK → `tokens` | when set, `x`/`y` are ignored by the engine in favor of the token's live position; `ON DELETE SET NULL` so deleting a token doesn't cascade-delete the light |
| `casts_shadows` | bool, default true | **NEW**, FR-027. Maps UVTT `lights[].shadows`; when false, the light ignores wall occlusion (matches UVTT semantics for ambient/non-shadowed lights). |
| `metadata` | jsonb, nullable | |
| `created_by`, `updated_by` | UUID FK → `users` | |
| `created_at`, `updated_at` | timestamp | |

**Validation**: same scene-ownership check as walls. `radius > 0` and
`intensity >= 0` enforced by CHECK constraints (fail fast at the data
boundary rather than relying on client validation, per FR-004's degenerate
"zero radius" edge case).

**State/relationships**: A light with `attached_token_id` set is
"token-attached" (FR-006); one with it null is "static". No other state
machine — a light is on whenever `intensity > 0`.

## Shape (new — replaces the "Annotation" concept from the pre-expansion draft)

| Field | Type | Notes |
|---|---|---|
| `shape_id` | UUID PK, default `gen_random_uuid()` | |
| `scene_id` | UUID FK → `scenes` | `ON DELETE CASCADE` |
| `kind` | text, `CHECK (kind IN ('stroke', 'rect', 'ellipse', 'line', 'text'))` | matches FR-007's tldraw-parity tool set |
| `geometry` | jsonb NOT NULL | interpreted by the engine, opaque to the server (contract below) |
| `text` | text, nullable | label (for `kind = 'text'`, or an optional caption on any shape) |
| `style` | jsonb, nullable | color/line-weight/fill, opaque to the server |
| `visible_to_players` | bool NOT NULL, default false | FR-008 — GM-only by default |
| `metadata` | jsonb, nullable | |
| `created_by`, `updated_by` | UUID FK → `users` | |
| `created_at`, `updated_at` | timestamp | |

**Validation**: same scene-ownership check as walls, applied to writes.
Reads (the `shapes` scene query) additionally filter
`visible_to_players = true` for callers without scene-owner/GM standing —
mirroring how player-facing queries already narrow GM-only data elsewhere
in the codebase, and directly implementing FR-009/FR-010's "players never
see GM-only authoring content" at the server, not just the client.

**Geometry contract** (`geometry` jsonb shape, opaque to Diesel/Postgres):
- `kind = "stroke"`: `{ "points": [[x, y], ...] }`
- `kind = "rect" | "ellipse"`: `{ "x": ..., "y": ..., "w": ..., "h": ... }`
- `kind = "line"`: `{ "x1": ..., "y1": ..., "x2": ..., "y2": ... }` (also covers arrows via `style`)
- `kind = "text"`: `{ "x": ..., "y": ... }` (anchor position), label in `text`

## Scene (existing table, extended)

| Field | Type | Notes |
|---|---|---|
| `background_image_path` | text, nullable | **NEW**. Relative path under `state.directories.asset_directory` to the imported/uploaded background image; `NULL` = no background art (existing scenes unaffected). Not a full URL — the web app resolves it against the existing static-asset serving route. |

No change to `grid_size`/`width`/`height`/`grid_type` — map import writes
into these existing fields (FR-023) rather than adding parallel ones.

## Map Import (not a table — a request/response shape + transactional write)

Not persisted as its own entity (spec: "a creation event, not a live
document"). One `POST` (multipart) request produces, in a single DB
transaction:

1. Zero or one background-image write (validated PNG, saved under
   `asset_directory`, `scenes.background_image_path` updated).
2. Zero or more `Wall` rows: each `line_of_sight[]`/`objects_line_of_sight[]`
   polygon's consecutive point pairs become one wall row each
   (`door_state = 'none'`); each `portals[]` entry becomes one wall row
   from its `bounds` pair with `door_state` derived from `closed`.
3. Zero or more `LightSource` rows, one per `lights[]` entry
   (`radius` ← `range`, `casts_shadows` ← `shadows`, `color` ← ARGB hex).
4. All coordinates scaled by `target_scene.grid_size / source.resolution.pixels_per_grid`
   before insertion (FR-023) — grid-space source coordinates are
   multiplied by `pixels_per_grid` first to reach source pixel-space, then
   by the scale ratio to reach the target scene's pixel-space.

Rejected/degenerate input (unsupported `format` version; a `line_of_sight`
polygon with fewer than 2 points) causes the whole transaction to roll
back — no partial scene data (FR-024).

## Canvas Layer (engine-side ordering concept, not persisted)

An ordered list, fixed for v1 (not user-reorderable), that every rendering
system in this feature consumes instead of choosing its own z-value:

1. Background/map art (scene image, including imported background)
2. Grid
3. Walls (GM-only visual — endpoints/handles only, never for players)
4. Lighting (the illumination effect itself — visible to players; editing handles are GM-only)
5. Shapes
6. Tokens
7. Fog-of-war

Each layer additionally carries a GM/player visibility rule (e.g. layer 3's
editing handles are GM-only per FR-009, while its *effect* — vision
occlusion — is not a rendered layer at all, it's a computation that feeds
the fog layer). This generalizes the ad hoc layer stack already sketched
in ADR-032 into a first-class resource (`CanvasLayers`, plan.md) other
plugins read rather than hardcode.

## Engine-side (non-persisted) resources

These live only in the Bevy `World`, rebuilt from GraphQL/RxDB data on
scene load and updated incrementally — not part of the database schema.

- **`WallSet`**: spatial index (e.g. segment list + simple grid/BVH) over
  the scene's walls (including door state), used by both vision and light
  occlusion.
- **`LightSet`**: current light sources (including live positions for
  token-attached lights, resolved each frame from the token's `Transform`).
- **`ShapeSet`**: current shapes plus a bounded per-session undo stack
  (research.md §4) — the undo stack itself is never persisted.
- **`CanvasLayers`**: the ordered layer list + visibility rules above.
