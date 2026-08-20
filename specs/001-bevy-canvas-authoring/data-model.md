# Phase 1 Data Model: Native Canvas Authoring

All three entities are per-scene, UUID-v7-keyed, and carry
`created_by`/`updated_by` provenance per Constitution Principle III,
mirroring the shipped `walls` table exactly in shape/conventions.

## Wall (existing — no changes)

Already implemented (Phase 6). Documented here only for cross-reference.

| Field | Type | Notes |
|---|---|---|
| `wall_id` | UUID PK | |
| `scene_id` | UUID FK → `scenes` | `ON DELETE CASCADE` |
| `x1, y1, x2, y2` | double precision | segment endpoints, scene-local coords |
| `blocks_vision` | bool, default true | |
| `blocks_movement` | bool, default false | |
| `metadata` | jsonb, nullable | free-form GM notes/tags |
| `created_by`, `updated_by` | UUID FK → `users` | |
| `created_at`, `updated_at` | timestamp | |

**Validation** (already enforced): caller must own the parent scene
(`scenes.owner_id = auth_user.user_id`) for create/update/delete.

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

## Annotation (new)

| Field | Type | Notes |
|---|---|---|
| `annotation_id` | UUID PK, default `gen_random_uuid()` | |
| `scene_id` | UUID FK → `scenes` | `ON DELETE CASCADE` |
| `kind` | text, `CHECK (kind IN ('stroke', 'shape', 'text'))` | matches FR-007's "strokes/shapes with optional text" |
| `geometry` | jsonb NOT NULL | point list (stroke) or shape descriptor (shape); interpreted by the engine, opaque to the server |
| `text` | text, nullable | optional label |
| `visible_to_players` | bool NOT NULL, default false | FR-008 — GM-only by default |
| `metadata` | jsonb, nullable | |
| `created_by`, `updated_by` | UUID FK → `users` | |
| `created_at`, `updated_at` | timestamp | |

**Validation**: same scene-ownership check as walls, applied to writes.
Reads (the `annotations` scene query) additionally filter
`visible_to_players = true` for callers without scene-owner/GM standing —
mirroring how player-facing queries already narrow GM-only data elsewhere
in the codebase, and directly implementing FR-009/FR-010's "players never
see GM-only authoring content" at the server, not just the client.

**Geometry contract** (`geometry` jsonb shape, opaque to Diesel/Postgres):
- `kind = "stroke"`: `{ "points": [[x, y], ...] }`
- `kind = "shape"`: `{ "shape": "rect" | "ellipse" | "line", "x": ..., "y": ..., "w": ..., "h": ... }` (rect/ellipse) or `{ "shape": "line", "x1":..., "y1":..., "x2":..., "y2":... }`
- `text` kind reuses `geometry` for anchor position: `{ "x": ..., "y": ... }`, with the label in `text`.

## Engine-side (non-persisted) resources

These live only in the Bevy `World`, rebuilt from GraphQL/RxDB data on
scene load and updated incrementally — not part of the database schema.

- **`WallSet`**: spatial index (e.g. segment list + simple grid/BVH) over
  the scene's walls, used by both vision and light occlusion.
- **`LightSet`**: current light sources (including live positions for
  token-attached lights, resolved each frame from the token's `Transform`).
- **`AnnotationSet`**: current annotations plus a bounded per-session undo
  stack (Research §4) — the undo stack itself is never persisted.
