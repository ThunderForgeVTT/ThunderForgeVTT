# Feature Specification: A Map In Tiles

**Feature Branch**: `075-a-map-in-tiles`
**Created**: 2026-10-05
**Status**: Draft
**Input**: The owner, 2026-10-05, after the demo panicked on a browser that reports WebGL2's guaranteed minimum texture size: "this is why we were engineering a tiling system of the map image … the tiling would be huge because it would enable a lot of devices with limited scopes."

## Why

A battle map is one image, and the engine hands it to the GPU as one
texture. That works on the hardware we develop on and fails on a great deal
of the hardware players own.

WebGL2 guarantees a 2048px texture and nothing more. Phones, integrated
graphics in older laptops, and hardened browsers (LibreWolf's
minimum-capability mode reports exactly 2048, and the owner hit it on
2026-10-05) all live at that ceiling. 4096 is common; 8192 and up is a
desktop GPU. Our maps run to 6144px on the long side and every one of the
demo's seven is wider than 2048.

Today the system meets this in three places, none of which is the answer:

- **The importer resizes.** `storage/transcode.rs` caps every stored
  background at `MAX_CANVAS_TEXTURE_DIMENSION = 4096` and its own comment
  says why that number is a compromise and that "serving larger art means
  tiling, not raising this number." Detail the artist drew is discarded at
  import, permanently, for every player, to suit a GPU most of them do not
  have.
- **The engine resamples.** Since `1cd31851`, `CachedAssetsPlugin` resamples
  any image larger than the device's `max_texture_dimension_2d` before the
  render world sees it. On a 2048 device a 4080x2295 map is drawn at
  2048x1152 — the whole map decoded at full size in memory first, then
  halved. It no longer panics. It is also blurry at any zoom, and it still
  downloads and decodes every byte of a map it will show a quarter of.
- **The whole map is always resident.** Zoomed in on one room of a 6144px
  map, the GPU holds all 81MB of it. On the devices this spec is for, that
  alone can be the reason a tab is killed.

Tiling answers all three at once, and it is why `crates/thunderforge-mapforge`
exists. Its `tiles.rs` already defines the pyramid (512px tiles, level 0 at
full resolution, each level halving), `Pyramid::describe`, `tile_rect`,
`level_for_scale`; its `server.rs` already serves `/maps/{name}/tile/{level}/{col}/{row}`
from the example maps for development. What is missing is everything that
would let a player see it: nothing produces tiles for an imported map, the
main server does not serve them, and the engine does not draw them.

## What exists

Counted on 2026-10-05:

- `crates/thunderforge-mapforge`: `tiles.rs` (geometry, tested),
  `source.rs` (loads `examples/maps/*.dd2vtt`, builds the level images in
  memory), `server.rs` (Axum routes `/maps`, `/maps/{name}`,
  `/maps/{name}/tile/{level}/{col}/{row}`, `/maps/{name}/full`), and the
  `apps/mapforge-server` binary. Development only: no authentication, no
  object store, reads from disk.
- `crates/thunderforge-server/src/storage/transcode.rs`: the import path,
  one WebP per background, capped at 4096, fingerprinted. Spec 059 delivers
  the source in pieces; the stored result is still one file.
- `crates/thunderforge-server/src/assets_serve/canvas.rs`:
  `GET /canvas-assets/{asset_id}`, immutable by fingerprint.
- `crates/thunderforge-engine/src/systems/background.rs`: one
  `BackgroundSprite` with `custom_size` = the scene's size, texture from the
  spec 028 cache (`cached_assets.rs`, `try_cached`) or `asset_server.load`.
- `map_import/alignment.rs`: knows that a stored background may be smaller
  than its source and corrects `grid_size` for it. Tiles make that whole
  class of problem go away, because the stored size is the source size again.
- `apps/demo`: imports maps in the page (`src/map/`), serves them from an
  in-page `/api/canvas-assets/` and has no server at all.

## Decisions already made

- **Tiles are the owner's chosen answer**, not an option among several.
  The resample in `cached_assets.rs` is a stopgap and this spec removes it
  (FR-012).
- **The geometry is mapforge's.** 512px tiles, halving levels, level 0 full
  resolution. The engine, the server and the demo all compute addresses with
  `Pyramid` from `thunderforge-mapforge`; nobody writes a second one. A
  tile is also at most 512px, so it fits any device with a WebGL2 context
  at all.
- **The source is kept at full resolution.** With tiles there is no reason
  to resize at import. `MAX_CANVAS_TEXTURE_DIMENSION` stops applying to
  backgrounds; the stored size is the source size, and the `alignment.rs`
  correction becomes history rather than a live rule.
- **The background stays one asset to the rest of the system.** A scene
  still names one `background_asset_id`; the tiles are how that asset is
  served and drawn, not a new kind of thing a Game Master manages.
- **Not a client-side database.** Tiles are cached by the spec 028 cache the
  way the whole image is today, by fingerprint, nothing new in the browser.

## Requirements

### Producing tiles

- **FR-001** When a background is imported (spec 059's assembly step, and the
  plain-image path), the server stores the source image at full resolution
  and a tile pyramid for it under the same asset id, each tile a WebP of at
  most 512x512, at every level down to the one that fits in a single tile.
- **FR-002** The pyramid's shape (width, height, tile size, levels) is
  recorded with the asset so a client learns it in one request, before it
  asks for any tile.
- **FR-003** A background that already exists when this ships gets its
  pyramid on first request (lazily, from the stored file, then kept), so no
  migration has to touch every world before any scene can open. A source
  that was resized by the old cap is tiled as it is; nothing can restore
  what the cap discarded.

### Serving tiles

- **FR-004** `GET /canvas-assets/{asset_id}/tiles` answers the pyramid's
  shape; `GET /canvas-assets/{asset_id}/tiles/{level}/{col}/{row}` answers one
  tile. Both carry the same authorization and immutable caching as
  `GET /canvas-assets/{asset_id}` does today, and an address outside the
  pyramid is 404.
- **FR-005** `GET /canvas-assets/{asset_id}` keeps working and keeps
  answering an image a single texture can hold, for every client that has not
  learned tiles yet (the actor sheet's map preview, the scene card, exports).
- **FR-006** The demo's in-page backend answers the same two routes from the
  same `Pyramid` arithmetic (compiled to the page through the existing wasm
  boundary or re-stated in TypeScript against the crate's tests, decided in
  planning), so the demo exercises the same engine code path as a real
  world.

### Drawing tiles

- **FR-007** The engine draws a scene's background from tiles: for the
  camera's current scale it picks the level whose pixels are nearest one
  screen pixel (`level_for_scale`), and for the visible rectangle plus one
  tile of margin it has those tiles resident and drawn at their `tile_rect`
  in scene space. The scene's `width`/`height` and every wall, light, token
  and grid line are where they were; only what fills the background changes.
- **FR-008** Tiles not on screen for a settled interval are released from
  the GPU; the total resident at any scale is bounded by what the viewport
  can show plus the margin, never by the map's size.
- **FR-009** While a level's tiles are arriving, the coarser level already
  resident is drawn beneath them, so zooming in sharpens rather than flashes
  empty.
- **FR-010** Tiles go through the spec 028 cache like the whole image does
  today: fingerprinted, fetched once, and a scene opened a second time draws
  from the cache before the network answers.
- **FR-011** A device whose texture ceiling is below 512 is not supported and
  is told so by the existing WebGL2 gate, not by a panic.
- **FR-012** The resample in `cached_assets.rs` (`fit_images_under_texture_ceiling`)
  is removed once backgrounds come as tiles. Token art, portraits and other
  images stay bounded by their own existing limits.

### Proof

- **FR-013** The demo's `limited-device.spec.ts` keeps passing with its
  resample assertion replaced by a tile assertion: on a faked 2048 device in a
  2560px window, the board draws, no texture larger than 512 is requested for
  the background, and the engine does not panic.
- **FR-014** An engine-side test proves tile selection: given a viewport,
  camera scale and pyramid, the set of tile ids requested is exactly the
  visible set plus margin at the right level, and panning one tile's width
  requests one column and releases one.

## Success Criteria

- **SC-001** Every one of the demo's seven maps opens on a 2048px device
  without a panic, at full source detail when zoomed in.
- **SC-002** On a 6144x3456 map, zoomed to one room, GPU memory for the
  background is under 16MB (against ~81MB today), measured with the engine's
  existing `frame_trace()` export or a counter added for it.
- **SC-003** Opening a scene a second time draws the first visible tiles from
  cache with no tile request on the wire (spec 028's existing proof pattern).
- **SC-004** A map imported after this ships is stored at its source size:
  `scenes.width`/`height` equal the source, and `alignment.rs` reports
  nothing to correct.
- **SC-005** The full web e2e suite and the demo suite are green with the
  resample stopgap deleted.

## Open questions for the owner

- Existing backgrounds: tile lazily on first request (FR-003, chosen) or
  run a one-time job across every world's assets at deploy time? Lazy costs
  the first viewer a few seconds on a large map, once.
- Should the demo build its tiles at import time in the page (seven maps at
  up to 4080px, a second or two of work each on a phone) or should the image
  build precompute them into `/srv/thunderforge/demo` beside the maps? The
  latter keeps the demo fast on exactly the devices this spec is for.

## What this spec does not do

- It does not change how maps are uploaded (spec 059) or what a Game Master
  sees in the import dialog.
- It does not tile token art, portraits or lore images; those are small.
- It does not make `apps/mapforge-server` a production service. Its routes
  are the model; the production routes live on the main server under
  `/canvas-assets/`.
- It does not add a client-side database. See AGENTS.md.
