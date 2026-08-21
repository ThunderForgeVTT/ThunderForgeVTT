# Phase 1 Data Model: dd2vtt Map Fidelity & From-Scratch Map Editor Tooling

## No schema changes

This feature introduces **zero new tables, columns, or migrations**. Every entity it touches already exists and already has the shape it needs (see research.md §1-2). This section documents the existing shape relevant to this feature's verification/test work, not new design.

## Existing entities referenced by this feature

### Wall segment (`walls` table, unchanged)

| Column | Type | Relevance to this feature |
|---|---|---|
| `wall_id` | UUID (PK) | Round-trip identity check (US2) |
| `scene_id` | UUID (FK → `scenes`) | Scoping for round-trip fixtures and GM-only enforcement (reused, unchanged) |
| `x1, y1, x2, y2` | numeric | Round-trip field-equality check (US2) |
| `blocks_vision` | bool | **US1's passability toggle** — already independently settable; verify, don't build |
| `blocks_movement` | bool | **US1's passability toggle** — already independently settable; verify, don't build |
| `door_state` | text (`"none"` / `"open"` / `"closed"`) | Unchanged; doors remain their own existing mechanic, not touched by US1's passability work |
| `metadata` | nullable jsonb | Round-trip check should confirm this survives unchanged if populated by import |
| `created_by`, `updated_by`, `created_at`, `updated_at` | existing provenance | Not directly asserted by this feature's tests beyond existing convention |

### Light source (`light_sources` table, unchanged)

Referenced only for US1's "torch" verification and US2's round-trip coverage. No fields relevant beyond what's already persisted (position, range, intensity, color, shadows per spec 001's data model).

### Shape annotation, token (unchanged)

Included in US2's round-trip verification (FR-008) for completeness since they belong to a scene, but this feature does not modify their shape or add authoring capability for them (that's spec 002, already merged).

### Canvas image asset / scene background (`canvas_image_assets`, `scenes.background_asset_id`, unchanged, spec 002)

US2's round-trip verification confirms a scene's background reference resolves to the same image content after reload — read-only check against this existing table, no new field.

## New (in-memory only) shape: import result warnings (User Story 3)

**Not persisted** — this is a response-shape addition to `map_import.rs`'s `import_uvtt` REST handler, not a database entity.

Current response shape (`map_import.rs:604-611`):
```json
{
  "wallsCreated": 8,
  "doorsCreated": 2,
  "lightsCreated": 12,
  "backgroundImageSet": true,
  "skippedDegeneratePolygons": 0
}
```

Extended shape (exact field name/structure decided at implementation time — either is compatible with FR-012/FR-013):
```json
{
  "wallsCreated": 8,
  "doorsCreated": 2,
  "lightsCreated": 12,
  "backgroundImageSet": true,
  "skippedDegeneratePolygons": 0,
  "warnings": [
    "1 freestanding portal was not imported (not attached to a wall)",
    "ambient_light was present but is not yet applied to scene lighting"
  ]
}
```

**Validation rule**: `warnings` MUST be empty (or absent, if the field is made optional) whenever the source file uses none of the three currently-unhandled field categories — per spec.md FR-014, no new noise for the common case.

**Populated from** (already-parsed, currently-`#[allow(dead_code)]` fields, per research.md §6):
- `UvttPortal.freestanding: bool` (`map_import.rs:92-94`) — when `true`, and no wall/door was created from it.
- `UvttEnvironment.ambient_light: Option<String>` (`map_import.rs:102`) — when present and non-default.
- `UvttFile.objects_line_of_sight: Vec<...>` — when non-empty.

## Test/reference fixtures (not schema, but feature-relevant data)

`examples/maps/` (5 newly added + 2 existing, see spec.md Assumptions and the directory's `README.md` for full per-file notes):

| File | Walls | Doors | Lights | `ambient_light` | Primary use in this feature |
|---|---|---|---|---|---|
| `grassy-path-ambush.dd2vtt` | 0 | 0 | 0 | default | US1 "blank canvas" starting point (the gold standard) |
| `azheim-meeting.dd2vtt` | 0 | 0 | 0 | default | US1 alternate blank-canvas fixture, smallest file |
| `road-side-in.dd2vtt` | 24 | 16 | 4 | default | US2 round-trip stress test (richest real fixture) |
| `dwarven-forge.dd2vtt` | 13 | 0 | 0 | default | US2 round-trip, walls-only case |
| `little-fish-academy.dd2vtt` | 27 | 24 | 0 | non-default (`fffff7e4`) | US3's `ambient_light` disclosure test — the only real-world fixture with this field set |
| `demo.dd2vtt` (existing) | 8 | 2 | 12 | baked | US2 round-trip (richest all-around, already spec 001's primary import fixture) |
| `chamber-of-echoing-grief.dd2vtt` (existing) | 1 | 0 | 0 | default | US2 round-trip, minimal walls-only case |

Plus one new hand-crafted synthetic fixture (not yet created — implementation-time work) exercising `freestanding: true` and a non-empty `objects_line_of_sight`, per research.md §7.
