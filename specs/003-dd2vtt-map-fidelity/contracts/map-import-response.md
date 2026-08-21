# Contract: `.dd2vtt` Import Response (extended)

**Endpoint**: `POST /api/scenes/{scene_id}/import/uvtt` (existing, unchanged route/auth — scene-owner only, per `src/server/src/map_import.rs`)

## Why this is the only contract change

User Stories 1 and 2 add no new GraphQL mutation, query, or REST endpoint — they reuse `update_wall`/`create_light_source` (already contract-stable) and add read-only verification. User Story 3 is the one place this feature changes an existing contract: the JSON body returned by the import endpoint on success.

## Before (current, unchanged fields)

```json
{
  "wallsCreated": 8,
  "doorsCreated": 2,
  "lightsCreated": 12,
  "backgroundImageSet": true,
  "skippedDegeneratePolygons": 0
}
```

## After

```json
{
  "wallsCreated": 8,
  "doorsCreated": 2,
  "lightsCreated": 12,
  "backgroundImageSet": true,
  "skippedDegeneratePolygons": 0,
  "warnings": []
}
```

- `warnings`: array of human-readable strings (or a small structured object per entry — implementation's call), one per skipped field *category* actually present in the source file (not one per individual skipped element — e.g. one warning for "freestanding portals," not one per portal).
- **Backward compatible**: existing consumers reading only `wallsCreated`/`doorsCreated`/etc. are unaffected; `warnings` is additive. Per FR-014, `warnings` MUST be empty for every existing fixture that doesn't use the three currently-unhandled field categories (`demo.dd2vtt`, `chamber-of-echoing-grief.dd2vtt`, and 4 of this feature's 5 new fixtures) — a regression here would fail SC-004.
- Error responses (unsupported format version, oversized upload, malformed JSON) are unchanged — `warnings` only appears in a *successful* import's response, distinguishing "fully imported" from "imported with some fields skipped" per FR-012, never conflated with an outright failure.

## Verification

- `little-fish-academy.dd2vtt` (this feature's new fixture with a non-default `ambient_light`) MUST produce a `warnings` entry mentioning ambient light.
- The hand-crafted synthetic fixture (research.md §7) MUST produce `warnings` entries for `freestanding` and `objects_line_of_sight`.
- All other fixtures in `examples/maps/` MUST produce an empty `warnings` array.
