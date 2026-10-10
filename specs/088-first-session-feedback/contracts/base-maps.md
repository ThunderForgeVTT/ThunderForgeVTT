# Contract: the base-maps directory

## Where it comes from

A Dockerfile stage after `build`:

```dockerfile
FROM build AS base-maps
RUN /app/target/release/thunderforge-demo-maps /app/examples/maps /out/base-maps \
 && cp /app/examples/maps/credit.json /out/base-maps/credit.json
```

The `server` stage copies `/out/base-maps` to `/srv/base-maps` and sets
`THUNDERFORGE_BASE_MAPS_DIR=/srv/base-maps`. Locally, `make base-maps`
writes the same thing to `target/base-maps`, and `.env.example` points
there.

## Layout

```text
<dir>/
├── maps.json              # [BaseMap] (data-model.md), walls include the perimeter
├── credit.json            # the one MapCredit
├── NOTICE.txt             # the licence notice, as thunderforge-demo-maps writes it
├── <id>.webp              # the background, full size
└── <id>.thumb.webp        # 480 px wide
```

## Reading it

- Read once at start-up into memory (`base_maps::BaseMaps`). A change on
  disk takes effect on restart.
- Missing, empty, unreadable, or a `maps.json` that does not parse: one
  `warn` line naming the path and the reason, and an empty set. The server
  still starts.
- A map whose image is missing is left out, with a `warn` naming it.
- The default is `grassy-path-ambush` (Open item 1). If it is not in the
  set, `defaultBaseMapId` is `null` and the form selects **None**.
- An operator can name another default, or `none`, with
  `THUNDERFORGE_BASE_MAPS_DEFAULT` (`--base-maps-default`). The e2e harness
  sets `none`, so the worlds its specs make keep a blank Starting Scene while
  the maps are still offered (`scripts/e2e/base-maps.mjs`).

## HTTP

| Route | Answer |
| --- | --- |
| `GET /api/base-maps/<id>.thumb.webp` | the thumbnail, `Cache-Control: public, max-age=31536000, immutable` |
| `GET /api/base-maps/<id>.webp` | the full image, same cache |
| `GET /api/base-maps/NOTICE.txt` | the notice, `text/plain` |
| anything else | 404 |

These need a signed-in caller, as `baseMaps` does. The id is matched
against the loaded set, never joined into a path, so `..` cannot escape the
directory.

## Applying one to a world

1. Upload `<id>.webp` through the storage adapter, to the world's own
   path, as a `BACKGROUND` asset with `base_map_id = <id>`.
2. Set the Starting Scene's width, height and grid from the map.
3. Insert the walls, doors and lights from `maps.json`, perimeter
   included.
4. Record `MAP_IMPORTED` (13).

Steps 2 to 4 are one transaction. Step 1 comes first, and if it fails,
nothing else happens.

## The credit, wherever a base map is shown

> Map by **MBRound18**, from [vtt-maps](https://github.com/mbround18/vtt-maps)
> ([catalog](https://vtt-maps.dnd-apps.dev/catalog)), under
> [CC BY-SA 4.0](https://creativecommons.org/licenses/by-sa/4.0/). The
> copies here are offered under the same licence.

The board's credit line is shorter, and expands on focus or hover:

> Map: MBRound18, CC BY-SA 4.0
