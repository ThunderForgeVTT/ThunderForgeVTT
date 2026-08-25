# Phase 0 Research: Scene Management Overhaul

All items below were resolved by reading the existing codebase (no NEEDS CLARIFICATION remain in Technical Context). Two research passes fed this: a survey of the scene/map/image pipeline, and a targeted investigation of real-time sync.

## 1. dd2vtt (DungeonDraft VTT) import

**Decision**: Reuse the existing importer as-is; this feature only adds a UI entry point in the new Scenes section.

**Rationale**: `src/server/src/map_import/` (mod.rs + geometry.rs, image.rs, parse.rs, types.rs, warnings.rs) is a complete, working UVTT (`.dd2vtt`, format 0.3) importer already wired as `POST /api/scenes/{scene_id}/import/uvtt` (multipart, 50MB cap). It parses `line_of_sight`, `portals`, `lights`, and the embedded base64 `image`, converts them into wall/light rows, and calls `save_background_image`. A frontend component (`apps/web/src/components/canvas-tools/MapImportTool/MapImportTool.tsx`) already drives it, currently reachable from the canvas map-editor tooling. This feature's only new work here is surfacing that same upload flow from the new Scenes section (a GM picks/creates a scene, then imports into it) rather than only from inside the canvas editor.

**Alternatives considered**: Building a new importer — rejected, redundant with working code. Wiring import through GraphQL instead of REST — rejected as unnecessary churn; the REST endpoint already returns a structured `ImportResult` the new UI can consume directly.

## 2. Scene summary (Markdown)

**Decision**: Add `summary_markdown TEXT` and `summary_rendered_html TEXT` columns to `scenes`, rendered server-side through the same Markdown pipeline (comrak + ammonia sanitization) already used for lore entries (`src/server/src/markdown/`), and edited client-side with the existing `LoreMarkdownEditor`/`LoreMarkdownRenderer` components generalized to accept a non-lore entity, or a thin scene-specific wrapper around the same CodeMirror extension set.

**Rationale**: Lore entries already solve "Markdown edit + sanitized server-rendered HTML + revision-safe update" end to end. Scenes have no comparable field today (`description TEXT` exists but is plain, unrendered text, displayed as-is nowhere important). Reusing the render pipeline (not re-implementing it) keeps sanitization guarantees (Principle III doesn't cover XSS directly, but reusing a hardened pipeline is the obvious lower-risk choice) and matches the spec's explicit ask to reuse "the same Markdown edit/view editor used for Lore entries."

**Alternatives considered**: Repurposing the existing `description` field as Markdown — rejected; `description` is already used elsewhere as plain text (research didn't find a markdown renderer wired to it), so overloading its meaning risks breaking existing display code and conflates "internal GM note" with "player-facing summary." A distinct `summary_markdown` field keeps the two concerns separate and matches the Key Entities section of spec.md.

## 3. Hidden flag

**Decision**: Add `hidden BOOLEAN NOT NULL DEFAULT true` to `scenes`. Query-time filtering (GM sees all, players see `hidden = false` only) follows the same owner-vs-player branching pattern already used for `shapes.visible_to_players` in `queries/scene.rs`'s shape resolver.

**Rationale**: No scene-level visibility flag exists today; `shapes.visible_to_players` is the closest analog and already establishes the filtering pattern to copy at the scene-list resolver. `DEFAULT true` at the column level matches "hidden by default" from the spec's Clarifications (a scene inserted with no explicit `hidden` value is hidden).

**Alternatives considered**: Reusing/repurposing `metadata JSONB` to stash a hidden flag — rejected; a first-class boolean column is directly indexable/filterable and matches the project's existing convention of promoting stable, queried-on fields to real columns rather than JSONB (see `allow_player_created_actors`, `genie_resource_carryover_enabled`).

## 4. Grid type ("None"/"Squares"/"Hexagons") — CORRECTED during implementation

**Decision (superseding the original research pass below)**: `scenes.grid_type` is **not** an unvalidated free-form column — it already carries a DB-level `CHECK (grid_type IN ('square', 'hex', 'gridless'))` constraint, added by migration `2026-08-23-195654-0000_widen_scene_grid_type_gridless` (spec 018, Genie's Wish-Warped Zone). The UI's "None" option maps directly onto the existing `"gridless"` value — no new migration, no new mutation-layer validation beyond what already exists.

More importantly, **the "disables snapping, no grid lines" behavior is already fully implemented end-to-end**, not new work:
- `src/engine/src/plugins/grid.rs`: `GridType::Gridless => ()` — no grid is rendered.
- `src/engine/src/movement.rs::apply_grid_snapping`: returns early (no-op) when `scene.grid_type == GridType::Gridless`, so tokens keep free-form positions.

The one part of the Clarification with no existing code to change is "distance-measuring tools report raw pixel distances instead of grid units" — **no measurement/ruler tool exists anywhere in this codebase yet** (confirmed: no `Ruler`/measurement UI component in `apps/web/src/`, no player-facing measuring feature). This requirement is therefore satisfied vacuously today and becomes binding only once a measurement tool is eventually built — it is not blocking for this feature and needs no task now.

**Rationale**: Always verify assumptions about "unvalidated"/"no existing behavior" against the actual schema and engine code before scoping new work — the original research pass below relied on a grep pattern that missed the CHECK constraint and the engine's existing `Gridless` handling (both added by a different, more recent spec than the researcher's search initially surfaced). This correction removes an entire engine-plugin task from the implementation (see tasks.md T039, revised to a verification-only task).

**Alternatives considered**: Re-implementing gridless behavior from scratch — rejected outright once discovered to already exist; this would have been pure duplication.

<details>
<summary>Original (superseded) research pass — kept for the record of what was investigated and corrected</summary>

**Original decision**: Keep `scenes.grid_type` as the existing free-form `TEXT` column (no migration needed for the column itself), but constrain the values accepted through the new UI and the scene-create/update mutations to exactly `"gridless" | "square" | "hex"`. "None" is a full behavioral mode, not just a visual toggle: it disables grid rendering, disables token-snap-to-grid, and switches distance-measuring tools to raw pixel units (per Clarifications).

**Original rationale**: `grid_type` is already an unvalidated `Text` column everywhere in the codebase (confirmed: every existing write path defaults to the literal `"square"`, nothing enforces a fixed set today). Formalizing the three accepted values at the mutation/UI layer is the minimal change; a DB-level CHECK constraint is unnecessary and would risk breaking any existing row with an unexpected value. The "None disables snapping and measurement" behavior is engine-owned (Principle I) — it touches whatever Bevy plugin/system currently reads `gridType` for snapping and measurement (grid/token-movement systems under `src/engine/src/plugins/`), not new React logic.

**Original alternatives considered**: A Postgres enum type — rejected as unnecessary rigidity given the column is already untyped `Text` in production data; a client-side-only restriction (Select options) without touching the engine's snap/measurement behavior — rejected, contradicts the explicit Clarification that "None" must actually disable snapping/measurement, not just hide grid lines.

</details>

## 5. Scene preview thumbnails

**Decision**: Reuse `src/server/src/storage/transcode.rs`'s existing resize + WebP-encode helpers (the `image` crate, already a dependency) to generate a scene preview image, following the exact pattern already used for lore image thumbnails (`LORE_IMAGE_THUMBNAIL_DIMENSION`-style max-dimension resize, not a literal fixed 1/16 ratio — source maps vary wildly in resolution, so a capped-max-dimension approach is what "roughly 1/16 scale" means in practice for typical map sizes). Serve it via a new route mirroring `lore_assets_serve.rs` (e.g. `/scene-assets/{asset_id}/thumb`). Trigger generation wherever a scene's background image is set: `map_import/image.rs::save_background_image` (dd2vtt import path) and the canvas image-asset upload path (`mutations_assets.rs`), whichever a given scene's map arrived through.

**Rationale**: The image-processing capability (resize, WebP encode, serve-by-computed-URL) already exists and is proven in production for lore images; this is a direct application of that pattern to a new asset kind, not new infrastructure.

**Alternatives considered**: Generating thumbnails client-side (canvas resize + upload) — rejected; server-side is already the established pattern and avoids trusting the client to produce a correctly-sized/sanitized image. A literal 1/16-ratio resize — rejected in favor of max-dimension capping, consistent with the existing lore-thumbnail approach and more robust across wildly different source map resolutions.

## 6. Live "launch scene" broadcast to everyone in Play

**Decision**: Add a server-authoritative `worlds.active_scene_id UUID NULL` column and a `launchScene(worldId, sceneId)` mutation (GM/Owner-only, following the existing one-mutation-per-setting pattern) that updates it and emits a new `world_events` row (new event code, next free value after the existing 1-5/10-15 range) through the **already-live** `record_world_event` → `pg_notify('world_events_channel', ...)` → `network/listener.rs` → `broadcast::Sender<WorldEvent>` → `SubscriptionRoot::world_events_created` → `/api/ws` GraphQL-WS → `apps/web/src/engine/world/sync/subscriptionClient.ts` pipeline. Client-side, `WorldPage.tsx`'s existing `subscribeToWorldEvents(id)` loop (already open for wall/token/light/shape events) gets one more handler that calls `setSelectedSceneId(newSceneId)` on the new event — which is already the exact state variable that drives every scene-content loader effect (`load*IntoStore`) in that file, so once it updates from a pushed event instead of only local `SceneSwitcher` UI, the unload/load behavior is close to automatic.

**Rationale**: A full research pass confirmed this transport is real and already carries live traffic for tokens/walls/lights/shapes/genie-session events — it is not the aspirational/dead-code path a stale comment on `useWorldMembers.ts` suggested (that comment describes a *different*, never-wired member-presence channel, not this one). Today, scene selection is 100% local per-browser-tab React state with zero cross-client sync and no server-side "active scene" concept at all — "launch" requires introducing both, but can do so as an extension of live infrastructure that already works, not a new transport.

**Alternatives considered**: Polling for the active scene on an interval — rejected, strictly worse than the already-available push transport and would add latency the spec's SC-006 ("within seconds") doesn't require accepting. A dedicated WebSocket/channel just for scene switches — rejected, redundant with the general-purpose `world_events` channel already carrying comparable events.

**Constitutional flag**: introducing a server-authoritative "active scene" concept plus a new live-broadcast event type is an architecturally significant decision (new ownership boundary: which scene is "live" is now server state, not purely client-local) — per Constitution Principle IV, this needs an ADR under `docs/adrs/` landing in the same change set as the implementation, not just this spec.

## 7. World-level default scene grid type

**Decision**: Add `worlds.default_scene_grid_type TEXT NOT NULL DEFAULT 'square'` and a single-purpose `updateWorldDefaultSceneGridType(worldId, gridType)` mutation, mirroring the exact shape of `genie_resource_carryover_enabled` (migration → schema.rs → models.rs → `GraphQLWorld` field + `From<World>` → dedicated input type → `_impl` fn gated by `is_dm_of_world` → thin mutation wrapper). `createScene` reads this world setting as its default `grid_type` when the caller doesn't explicitly pass one.

**Rationale**: This project already has an established, repeated pattern (two prior examples) for "one new boolean/enum-ish world setting, one dedicated mutation" — following it keeps this change reviewable and consistent rather than introducing a generic `updateWorldSettings(partial)` mutation shape that doesn't exist anywhere else in the codebase.

**Alternatives considered**: A generic partial-update `updateWorld` mutation covering many fields at once — rejected, inconsistent with the existing one-mutation-per-setting convention and would need broader review of what else it should cover.

## 8. Scenes section navigation and page structure

**Decision**: Add a "Scenes" entry to `WorldSidebarNav`'s category list (between Session Setup and NPCs), backed by a new route (e.g. `/world/:id/scenes`) rendered inside the existing `WorldSectionShell`, reusing its GM-vs-player branching pattern (compare `WorldCompendiumPage`'s `isGm`-gated create/edit controls). GM view: full scene list/management (create, import, summary, hidden toggle, Launch). Player view: table of non-hidden scenes + detail view (summary + thumbnail).

**Rationale**: This directly extends the existing sidebar-nav/shell pattern built for Compendium/System-settings in the immediately preceding feature — no new navigation paradigm needed.

**Alternatives considered**: A tab inside the existing Compendium page (alongside NPCs/Lore/Items/Abilities) — rejected; scenes have GM-only mutation-heavy actions (launch, import, hidden toggle) that don't fit the Compendium's mostly-symmetric GM/player browsing model, and the spec explicitly asks for the sidebar's own destination, "alongside" Compendium's categories, not nested inside it.

## Outstanding technical risk (for tasks phase, not blocking planning)

- The engine-side change for grid-type "None" (disable snap-to-grid, switch measurement to raw pixels) requires touching whatever Bevy plugin currently owns grid/measurement logic under `src/engine/src/plugins/`. This wasn't inventoried in research (out of scope for this pass) and should be scoped explicitly as its own task with a `cargo check --target wasm32-unknown-unknown` verification step (Constitution Principle V).
