# Data Model: Scene Management Overhaul

## Scene (extends existing `scenes` table)

| Field | Type | Notes |
|---|---|---|
| `scene_id` | UUID (PK) | existing, unchanged |
| `world_id` | UUID (FK → worlds) | existing, unchanged |
| `name` | Text | existing, unchanged |
| `description` | Text, nullable | existing, unchanged — **not** reused for the new Markdown summary (see research.md §2) |
| `grid_size` | Int4 | existing, unchanged |
| `grid_type` | Text | existing column, **already DB-CHECK-constrained** (migration `2026-08-23-195654-0000_widen_scene_grid_type_gridless`) to `"gridless" \| "square" \| "hex"` — the UI's "None" option maps to the existing `"gridless"` value; no new constraint needed (see research.md §4, corrected) |
| `width`, `height` | Int4 | existing, unchanged |
| `background_image_path`, `background_asset_id` | nullable | existing, unchanged |
| **`summary_markdown`** | Text, nullable | **NEW.** GM-authored Markdown source for the scene's player-facing summary. Null/empty renders as the "no summary yet" placeholder (FR-013-adjacent; Edge Cases). |
| **`summary_rendered_html`** | Text, nullable | **NEW.** Server-rendered, sanitized HTML of `summary_markdown`, produced via the existing lore Markdown pipeline (`markdown/` module) at write time — mirrors how lore entries store `rendered_html`. Kept in sync with `summary_markdown` on every update; never rendered client-side. |
| **`hidden`** | Boolean, NOT NULL, DEFAULT `true` | **NEW.** Player-facing visibility flag. `true` = excluded from the non-GM scenes table (FR-008); GM/Owner always see all scenes regardless (FR-009). Defaults to hidden per Clarifications (a newly created scene is not visible until the GM explicitly un-hides it). |
| **`preview_asset_id`** | UUID, nullable, FK → the project's existing image-asset table | **NEW.** Points at the generated reduced-size preview image (research.md §5). Null until a background image has been set and a thumbnail successfully generated; the detail view falls back to a placeholder when null (FR-013). |

**Validation rules**:
- `grid_type` MUST be one of `"gridless" | "square" | "hex"` when set through `createScene`/`updateScene`; reject other values with a clear error (extends, doesn't replace, the column's existing free-form storage — old rows with other values are left alone).
- `summary_markdown` has no server-enforced length cap beyond whatever limit lore entries already use (reuse, don't invent a new one).
- `hidden`, `launch`, `summary` mutations all require the caller to be GM/Owner of `world_id` — same authorization check already applied to `updateScene` (owner-filtered) and other world-scoped GM actions (Constitution Principle III).

**Lifecycle**: Scene creation → (optional) dd2vtt import replaces background/walls/lights, and preview generation re-runs whenever the background image changes → GM writes/edits summary any time → GM toggles `hidden` any time → GM "launches" the scene (see World.active_scene_id below) any time, independent of `hidden` (a GM can launch a still-hidden scene for prep/testing without exposing it in the player table — Edge Cases already covers "hiding a currently-active scene doesn't interrupt Play").

## World (extends existing `worlds` table)

| Field | Type | Notes |
|---|---|---|
| **`default_scene_grid_type`** | Text, NOT NULL, DEFAULT `'square'`, CHECK IN `('gridless','square','hex')` | **NEW.** Read by `createScene` to seed a new scene's `grid_type` when the caller doesn't pass one explicitly (FR-015). Changing it never retroactively touches existing scenes (FR-016). |
| **`active_scene_id`** | UUID, nullable, FK → scenes(scene_id) | **NEW.** The world's server-authoritative "currently launched" scene for Play (research.md §6). Null = nothing launched yet, in which case Play shows the empty/unloaded canvas state per Clarifications. Set only via the `launchScene` mutation. |

**Validation rules**:
- `default_scene_grid_type` MUST be one of `"gridless" | "square" | "hex"` — same constraint as `Scene.grid_type`.
- `active_scene_id`, if set, MUST reference a scene belonging to this same `world_id` (defensive check in the `launchScene` mutation, mirroring existing scene/world ownership checks elsewhere).
- Setting `active_scene_id` has no `hidden`-state precondition — a GM may launch a hidden scene (see Scene lifecycle above).

**Reconciliation with spec 010 (found during implementation)**: spec 010 (FR-004) already guarantees a freshly created world has its auto-generated default scene ready to play immediately, with no separate "create a scene" step. `create_world` now also sets `active_scene_id` to that same default scene within the same insert transaction, so a brand-new world is never stuck in FR-002d's empty/unloaded-canvas state — that state is reachable only for a world where a GM has genuinely never launched anything (not the normal post-creation case). A one-time backfill migration (`2026-08-24-200000-0002_backfill_world_active_scene`) sets `active_scene_id` to each pre-existing world's oldest scene, so worlds created before this feature shipped aren't retroactively left showing an empty canvas either.

## World Event (extends existing `world_events` catalog, `world_events.rs`)

| Field | Notes |
|---|---|
| New event code | Next free integer after the existing 1-5/10-15 range (exact value assigned during implementation, tracked in `world_events.rs`'s existing code table) — represents "scene launched." |
| Payload | `{ worldId, sceneId }` — the newly-active scene, delivered through the existing `WorldEventsCreated` subscription payload shape (mirrors how token/wall/light/shape events already carry their own minimal payload). |

**State transition it represents**: `World.active_scene_id: previous → new`, broadcast to every subscriber of that `world_id`'s `world_events_created` subscription — which today already includes every open Play tab for that world (research.md §6). No new transport; this is one more event type on the existing channel.

## Scene Preview Image (new concept, not a new table)

Not a first-class row of its own — it's a generated *rendition* of whatever asset already backs `Scene.background_image_path`/`background_asset_id`, referenced by the new `Scene.preview_asset_id`. Generation happens synchronously (or best-effort async, to be decided in tasks) at the same point the background image itself is set: `map_import/image.rs::save_background_image` (dd2vtt path) and the canvas image-asset upload path (`mutations_assets.rs`), whichever a given scene's map arrived through (research.md §5). Served via a new computed-URL route, not stored as a separate uploaded file the client controls.

## Frontend types (mirrors backend 1:1, additive only)

- `SceneRecord` (`apps/web/src/types/scene.ts`) gains: `summaryMarkdown: string | null`, `summaryRenderedHtml: string | null`, `hidden: boolean`, `previewUrl: string | null` (a computed URL, not a raw asset id, matching how `LoreImageAssetRecord.thumbnailUrl` already works).
- `WorldRecord` (`apps/web/src/types/world.ts`) gains: `defaultSceneGridType: string`, `activeSceneId: string | null`.
- `CreateSceneInput` gains an optional `gridType` override (already exists) — no new field needed; when omitted, the server applies `World.default_scene_grid_type`.
- New `UpdateSceneInput`-shaped calls for summary/hidden edits (the backend `updateScene` mutation already exists per research; frontend `api/scenes.ts` currently has no wrapper for it — one is added, extended with the two new fields).
