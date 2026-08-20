# Example Maps (Universal VTT / `.dd2vtt`)

Fixtures for `specs/001-bevy-canvas-authoring`'s map-import capability.
Both files are the JSON-based Universal VTT export format (`format` field
`0.3`) produced by DungeonDraft and consumed by most VTTs (Foundry,
Owlbear Rodeo, etc.). Despite the `.dd2vtt` extension, the payload is
plain JSON with one large base64-encoded PNG string — not a binary
DungeonDraft project file.

**Provenance / license note**: these two files were pulled from a local,
personal map-asset collection for use as parser test fixtures. DungeonDraft
map packs are frequently sold/licensed for personal use and are not
generally redistributable — do not assume these are safe to publish,
re-share, or ship inside a public release artifact without confirming the
original license. Treat this directory as dev/test-only.

## Files

- `demo.dd2vtt` — richest fixture: 8 line-of-sight (wall) polygons, 2
  portals (doors), 12 lights, baked ambient lighting. Use this one first
  when validating the import pipeline end-to-end.
- `chamber-of-echoing-grief.dd2vtt` — a single dungeon room: 1
  line-of-sight polygon, no portals/lights. Use this to validate the
  walls-only import path without lighting/portal handling in the way.

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
  "image": "<base64 PNG, no data: prefix>"
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
