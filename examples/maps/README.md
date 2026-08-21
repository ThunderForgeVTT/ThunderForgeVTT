# Example Maps (Universal VTT / `.dd2vtt`)

Fixtures for `specs/001-bevy-canvas-authoring`'s map-import capability.
Both files are the JSON-based Universal VTT export format (`format` field
`0.3`) produced by DungeonDraft and consumed by most VTTs (Foundry,
Owlbear Rodeo, etc.). Despite the `.dd2vtt` extension, the payload is
plain JSON with one large base64-encoded image string — not a binary
DungeonDraft project file. **That image is not always a PNG**: `demo.dd2vtt`'s
is genuinely WebP (verified via magic bytes: `RIFF....WEBP`), while
`chamber-of-echoing-grief.dd2vtt`'s is a genuine PNG (`\x89PNG\r\n\x1a\n`) —
an earlier draft of this doc assumed PNG-only, which broke real imports
(`src/server/src/map_import.rs`'s `detect_image_extension` now accepts
both, and `src/engine/Cargo.toml` enables Bevy's `"webp"` feature to
render either).

**Provenance / license note**: all files in this directory were pulled
from a local, personal map-asset collection (`vtt-maps`, cherry-picked
2026-08-21 for spec 003) for use as parser/editor test fixtures.
DungeonDraft map packs are frequently sold/licensed for personal use and
are not generally redistributable — do not assume these are safe to
publish, re-share, or ship inside a public release artifact without
confirming the original license. Treat this directory as dev/test-only.

## Files

- `demo.dd2vtt` — richest fixture: 8 line-of-sight (wall) polygons, 2
  portals (doors), 12 lights, baked ambient lighting. Use this one first
  when validating the import pipeline end-to-end.
- `chamber-of-echoing-grief.dd2vtt` — a single dungeon room: 1
  line-of-sight polygon, no portals/lights. Use this to validate the
  walls-only import path without lighting/portal handling in the way.
- `grassy-path-ambush.dd2vtt` — **the project's default "blank scene"
  gold-standard fixture** (spec 003). Zero walls, zero portals, zero
  lights — background art only. This is the reference fixture for
  from-scratch map authoring: import it for the background, then build
  every wall/door/window/torch by hand via edit mode (spec 003 US-new),
  rather than relying on pre-baked UVTT geometry. New scenes' default
  starting point should match this fixture's shape (image, no
  geometry), not necessarily this exact file.
- `azheim-meeting.dd2vtt` — smallest fixture (388KB), also zero
  geometry (background-only, same shape as `grassy-path-ambush.dd2vtt`).
  Use this one for fast unit tests that need a real-but-tiny
  background-only file and don't care which background art it shows.
- `road-side-in.dd2vtt` — richest real-world fixture in this set: 24
  line-of-sight polygons, 16 portals (doors), 4 lights. Use this to
  stress-test import/round-trip fidelity against a genuinely complex,
  multi-room building.
- `dwarven-forge.dd2vtt` — walls-only dungeon (13 line-of-sight
  polygons, no portals/lights) at a size between
  `chamber-of-echoing-grief.dd2vtt` and `road-side-in.dd2vtt`.
- `little-fish-academy.dd2vtt` — 27 walls, 24 portals, **non-default
  `environment.ambient_light`** (`fffff7e4`) with `baked_lighting: true`
  and zero explicit `lights[]` entries. The only fixture in this set
  that exercises the currently-parsed-but-unused `ambient_light` field
  (spec 003 US2) — every other fixture here uses the default
  `ffffffff` or omits it.

None of the 64 source files surveyed when picking this set had a
`freestanding: true` portal or a non-empty `objects_line_of_sight[]`
array — real DungeonDraft exports don't appear to populate either field
in practice. Testing those two spec-003 US2 field categories will need
a hand-crafted/synthetic fixture, not a real-world file.

## Top-level JSON shape (format 0.3)

```jsonc
{
  "format": 0.3,
  "resolution": {
    "map_origin": { "x": 0, "y": 0 },       // grid-cell offset, usually 0,0
    "map_size":   { "x": 35, "y": 20 },     // map size in grid cells
    "pixels_per_grid": 128                  // px per cell in the source image
  },
  "line_of_sight": [                        // array of polylines/polygons
    [ { "x": 32, "y": 12 }, { "x": 24, "y": 12 }, ... ],  // grid-space points
    ...
  ],
  "objects_line_of_sight": [ /* same shape, for movable-object occluders */ ],
  "portals": [
    {
      "position": { "x": 14.5, "y": 12 },   // door midpoint, grid-space
      "bounds":   [ { "x": 14, "y": 12 }, { "x": 15, "y": 12 } ], // endpoints
      "rotation": 0,
      "closed":   true,                     // door state at export time
      "freestanding": false
    }
  ],
  "environment": {
    "baked_lighting": true,                 // true = image already has lighting baked in
    "ambient_light":  "ffffffff"            // ARGB hex
  },
  "lights": [
    {
      "position":  { "x": 4.5, "y": 16.3 }, // grid-space
      "range":     5,                       // grid cells
      "intensity": 1,
      "color":     "ffeccd8b",              // ARGB hex
      "shadows":   true                     // occluded by line_of_sight walls
    }
  ],
  "image": "<base64 PNG or WebP, no data: prefix>"
}
```

## Mapping to ThunderForgeVTT entities (see `specs/001-bevy-canvas-authoring/data-model.md`)

| UVTT field | Maps to |
|---|---|
| `image` (decoded) | Scene background layer asset |
| `resolution.map_size` × `resolution.pixels_per_grid` | Scene `width`/`height` (px) |
| `resolution.pixels_per_grid` | Scene `grid_size`, with imported geometry scaled if the target scene already has a different `grid_size` |
| `line_of_sight[]` polygons | `Wall` segments (each consecutive point pair → one wall row), `blocks_vision = true`, `blocks_movement = false` by default |
| `objects_line_of_sight[]` | Same as above, tagged as object-sourced in `metadata` (movement-blocking left to the GM to confirm) |
| `portals[]` | `Wall` segments with door fields set (`door_state = closed`/`open` from `closed`) |
| `lights[]` | `LightSource` rows (`range` → `radius`, `color` ARGB → stored color, `shadows` → whether occlusion applies) |
| `environment.ambient_light` | Scene-level ambient light default (outside any `LightSource`) |

Coordinates in `line_of_sight`/`portals`/`lights` are in **grid units**,
not pixels — multiply by `pixels_per_grid` (or the target scene's
`grid_size` if rescaling) to get scene-local pixel coordinates.
