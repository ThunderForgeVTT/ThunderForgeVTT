# Phase 0 Research: Native Canvas Authoring

No `[NEEDS CLARIFICATION]` markers remained in the spec or Technical
Context, so this phase resolves the open technical-approach questions
identified while drafting the plan, rather than ambiguous requirements.

## 1. Wall backend reuse vs. rebuild

**Decision**: Reuse the existing `walls` table, `WallMutation`
(`create_wall`/`update_wall`/`delete_wall`), and `walls` scene query
unchanged. This feature only adds the missing authoring UI + engine-side
rendering/occlusion on top of it.

**Rationale**: Verified `mutations_walls.rs` already implements full CRUD
with scene-ownership enforcement matching Constitution Principle III, and
`queries/scene.rs::walls` already returns per-scene wall lists. No backend
gap exists for walls — confirmed by direct code inspection, not assumption.

**Alternatives considered**: Rebuilding wall mutations as part of this
feature — rejected, would duplicate working, already-shipped code (Phase 6
commit).

## 2. Light source and annotation persistence shape

**Decision**: Model `light_sources` and `annotations` as their own Diesel
tables/GraphQL types, structurally parallel to `walls` (per-scene,
`created_by`/`updated_by`, UUID v7 primary key), rather than folding them
into a generic "scene object" table.

**Rationale**: Each entity has a distinct, stable shape (light: position +
radius + intensity + optional token attachment; annotation: stroke/shape
geometry + visibility flag) and distinct query patterns (players never
query raw annotation/light authoring data, only rendered effects for
lights and player-visible annotations). A generic polymorphic table would
need a JSON payload per type anyway, adding indirection without saving
real duplication — three tables of ~8 columns each is not enough
repetition to justify a shared abstraction (project convention: prefer
three similar things over one premature generalization).

**Alternatives considered**: Single `scene_objects` table with a `kind`
discriminator and JSON payload — rejected as premature abstraction; would
also complicate ownership/occlusion queries that need typed columns
(radius, blocks_vision) for server-side or spatial-index filtering later.

## 3. Vision/light occlusion algorithm

**Decision**: Use 2D shadow-casting (ray/segment intersection against wall
segments) computed in the engine (WASM), per the existing `FogPlugin`
composite-rendering slot reserved in ADR-032, re-triggered whenever the
`WallSet` or `LightSet` resource changes (not every frame) plus once per
moved token/light.

**Rationale**: Standard, well-understood technique for tile/segment-based
VTT vision (used by Foundry VTT and similar tools); bounding the recompute
to "on change" rather than "every frame" keeps it cheap at the stated
tens-to-low-hundreds per-scene wall/light count (Technical Context: Scale/Scope).

**Alternatives considered**: GPU shader-based shadow volumes — rejected
for v1 as unnecessary complexity given the entity counts involved; can be
revisited later as a pure rendering-layer optimization without changing
the wall/light data model, so it isn't foreclosed.

## 4. Undo scope

**Decision**: Session-local undo stack per authoring plugin (wall, light,
annotation each keep their own bounded stack), applied by re-issuing the
inverse mutation (e.g. undo of "move wall" re-issues "update_wall" with
prior coordinates) through the same GraphQL path as a normal edit.

**Rationale**: Spec Assumption already scopes undo to "current GM's
current editing session" (FR-012). Routing undo through the normal
mutation path (rather than a special client-only rollback) means undone
edits propagate to other clients exactly like any other edit — no new
sync mechanism needed, consistent with existing optimistic-mutation /
conflict-handling systems already in `systems/optimistic.rs` and the
Phase 4.9.C client-conflict-handling work.

**Alternatives considered**: Client-only local undo (no server round-trip)
— rejected because it would leave other connected clients' fog/light view
stale/wrong until their next unrelated sync.

## 5. RxDB collection pattern

**Decision**: Add `worldWallsCollection.ts`, `worldLightsCollection.ts`,
`worldAnnotationsCollection.ts` following the exact structure of the
existing `worldTokensCollection.ts` (schema, replication pull/push,
world-store wiring).

**Rationale**: Consistency with the established sync layer; no new sync
transport or conflict-resolution strategy is introduced by this feature.

**Alternatives considered**: Skipping RxDB and querying GraphQL directly
per canvas load — rejected, breaks the existing offline-capable /
optimistic-update pattern all other world entities rely on.

## 6. tldraw removal sequencing

**Decision**: Remove `WorldWhiteboard.tsx` and the `tldraw` package
dependency only after `AnnotationPlugin` (User Story 3, P3) reaches
parity with the freeform-drawing capability tldraw currently provides —
not at the start of the feature. Walls (P1) and lighting (P2) do not
depend on tldraw today (it was never wired to walls/lighting), so they can
ship and be used independently and first.

**Rationale**: Matches FR-013 (each tool independently usable) and avoids
a period where GMs have neither the old drawing tool nor a working
replacement.

**Alternatives considered**: Removing tldraw immediately and building
annotations first — rejected because it inverts the spec's stated
priority order (walls are P1 for a reason: the backend already exists and
is the highest-value gap).

## 7. Universal VTT (`.dd2vtt`) format scope and parsing location

**Decision**: Support format version `0.3` only (verified against both
fixtures in `examples/maps/`, both of which declare `"format": 0.3`).
Parse the file entirely server-side, inside the new `POST
/api/scenes/{scene_id}/import/uvtt` handler (contracts/graphql.md), not in
the browser/engine — the file is plain JSON with a large base64 image
payload; parsing/decoding server-side avoids shipping that work (and a
JSON+image-decode dependency) into the WASM bundle, and lets the import
run as one DB transaction close to the database.

**Rationale**: `examples/maps/README.md` documents the exact shape
verified by inspecting both fixture files with `python3 -m json.tool`
equivalent tooling (top-level keys: `format`, `resolution`,
`line_of_sight`, `objects_line_of_sight`, `portals`, `environment`,
`lights`, `image`). One fixture (`chamber-of-echoing-grief.dd2vtt`) uses
an older exporter for some fields but still declares `format: 0.3`,
confirming 0.3 is the correct version to target, not a moving target per
file.

**Alternatives considered**: Supporting older format versions (0.1/0.2,
referenced in some public UVTT documentation) — rejected for v1 scope;
FR-024 requires rejecting unsupported versions with a clear error rather
than guessing at a compatible shape, so adding versions later is additive,
not a breaking change. Parsing in the engine/WASM — rejected due to
bundle-size and transaction-locality concerns above.

## 8. Coordinate scaling between import and target scene

**Decision**: `resolution.pixels_per_grid` in the source file is the
source's px-per-grid-cell; all `line_of_sight`/`portals`/`lights`
coordinates are in grid units. Convert to the target scene's pixel space
with `scene_px = grid_units * pixels_per_grid * (target_scene.grid_size /
pixels_per_grid)`, which simplifies to `scene_px = grid_units *
target_scene.grid_size` — i.e. once normalized to grid units, the source
file's own `pixels_per_grid` cancels out and only the *target* scene's
`grid_size` matters. `resolution.map_size` (in grid cells) ×
`target_scene.grid_size` gives the scene's `width`/`height` in pixels.

**Rationale**: This is the simplest correct mapping and matches
data-model.md's "Map Import" section; it also means importing the same
file into scenes with different `grid_size` values naturally produces
correctly-scaled results without per-import configuration, addressing the
"resolution mismatch" edge case in spec.md directly.

**Alternatives considered**: Always adopting the source file's
`pixels_per_grid` as the scene's `grid_size` (i.e. resize the scene to
fit the import instead of fitting the import to the scene) — rejected
because it would silently change an existing scene's grid to match
whatever was last imported, which is surprising when importing into a
scene that already has hand-drawn content at a specific scale.

## 9. Background image storage

**Decision**: Decode and re-save the imported base64 PNG under the
server's existing `state.directories.asset_directory`
(`src/server/src/config/mod.rs`), the same directory already used for
other served assets, and store only the relative path in
`scenes.background_image_path` (data-model.md) — not the base64 blob
itself in Postgres.

**Rationale**: Reuses infrastructure that already exists (directory
creation, static-file serving) rather than introducing object storage or
storing multi-megabyte blobs in the database, which would bloat every
`scenes` row read/replicated to RxDB even when the background image
isn't needed.

**Alternatives considered**: Storing the base64 string in a `scenes`
column or a dedicated `scene_assets` table — rejected as unnecessary
database bloat and slower replication for something the existing static
asset directory already serves well.
