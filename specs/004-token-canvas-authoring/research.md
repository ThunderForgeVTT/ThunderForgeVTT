# Phase 0 Research: Canvas-Native Token Authoring & Scene-Switch Loading Feedback

## 1. Token backing store is currently split in two — unify onto the scene-scoped `tokens` table

**Decision**: Extend and standardize on the scene-scoped `tokens` table (`src/server/src/schema.rs:254-266`) as the single source of truth for tokens. Rewire `TokenPanel.tsx` off the legacy `world_tokens` table (`schema.rs:408-423`) onto `tokens`. Do not attempt to auto-migrate existing `world_tokens` rows (no clean 1:1 mapping — `world_tokens` is world-scoped, `tokens` is scene-scoped); leave the `world_tokens` table and its data in place, unread, as a retired legacy artifact.

**Evidence**:
- The canvas engine already renders/syncs only the `tokens` table — `apps/web/src/engine/world/sync/tokens.ts:5-9`, `src/engine/src/systems/selection.rs`'s `handle_token_drag` (lines 48-120) already drags/persists against it via an `upsert_token`-style call.
- `TokenPanel.tsx` (`apps/web/src/components/TokenPanel.tsx`) instead binds to the legacy `world_tokens` RxDB collection (line 68) and calls `moveToken`/`createWorldToken`/`deleteWorldToken` — mutations that touch a table the canvas never reads. Today, moving a token in the panel and dragging a token on the canvas are two unrelated rows in two unrelated tables.
- ADR-033 (`docs/adrs/20260505-033-token_data_model_and_ownership.md`) documents the original `world_tokens` design, predating scenes as their own first-class concept (its own `world_tokens` schema sketch even includes a `scene_id` field that was never actually added to the real table — schema.rs's `world_tokens` has no `scene_id` column at all). `tokens` (`create_tokens_table` migration, 2026-05-05-010001-0002) is the newer, scene-correct design that superseded it in practice without a formal retirement.
- Constitution Principle I (ECS owns simulation) and its rationale explicitly calls out this exact class of problem — a prior split between two competing stores (tldraw vs. Bevy) that had to be consolidated. The `world_tokens`/`tokens` split is the same failure mode recurring for tokens specifically.

**Rationale**: FR-005 requires the canvas and the panel to never show conflicting state for the same token. That is only possible if they operate on the same row. Unifying is real, in-scope work for this feature, not incidental cleanup — without it, "primary token" and "GM grants control of an additional token" (User Story 3) have no single table to live in.

**Alternatives considered**: Keep both tables and sync between them — rejected; introduces exactly the dual-source-of-truth bug class Principle I's rationale was written to prevent, and every future canvas token feature would inherit the split. Migrate `world_tokens` data into `tokens` — rejected as an automatic step; there is no scene to assign historical `world_tokens` rows to (they're world-scoped), so a blind migration would require guessing a scene. Left as manual/out-of-scope cleanup if any production `world_tokens` data turns out to matter (per Assumptions, this is a pre-release codebase with no such data at stake).

**Process requirement**: Per Constitution Principle IV, this is an architecturally significant change (retiring a data model, ADR-033's actual implementation target) and MUST be recorded in a new ADR before implementation diverges across files — e.g. `docs/adrs/<date>-0XX-unify_token_backing_store.md`, marking ADR-033 superseded for `world_tokens`'s ownership/ADR content while ADR-033's still-relevant general patterns (event-driven mutation logging, ownership-at-DB-level) continue to apply to the unified `tokens` table.

## 2. Rotation and scale already exist server-side — no new columns needed for User Story 2's core ask

**Decision**: `tokens.rotation` and `tokens.scale` (`schema.rs:254-266`, both `Float8`) already exist and are already accepted by `update_token`'s `GraphQLUpdateTokenInput` (`mutations_tokens.rs:120-127`). User Story 2 (resize/rotate) needs no schema change for these two fields — only new engine-side UI (handles) and, per the size clarification, an engine-side snap to whole grid-cell multiples before the value is sent (the column itself stays a float — grid-cell count × cell size — so no schema constraint is added; the *constraint* is enforced client-side by the resize handle's drag math, consistent with how existing wall/shape tools snap during drag rather than at the database layer).

**Evidence**: `mutations_tokens.rs:106-140` (`update_token`), `GraphQLUpdateTokenInput` fields `x, y, rotation, scale, actor_id, metadata` (lines 120-127).

**Rationale**: Avoids a needless migration for the one part of this feature that turns out to already be fully wired server-side — mirrors spec 003's research pattern of "verify what's already built before assuming a gap."

## 3. New columns genuinely needed on `tokens`: ownership/control, primary designation, photo, health

**Decision**: Add four nullable/defaulted columns to `tokens` via a new Diesel migration:
- `owner_user_id UUID NULL` (FK `users.id`) — the player who controls this token (their primary token, or a token the GM additionally granted them).
- `is_primary BOOLEAN NOT NULL DEFAULT false` — true for exactly one token per `(scene_id, owner_user_id)` pair (enforced by a partial unique index `WHERE is_primary`).
- `photo_url TEXT NULL` — player- and GM-editable override image; when absent, the client continues to fall back to the existing deterministic Dicebear URL computed from `token_id` (`TokenPanel.tsx`'s `getTokenAvatar`, lines 194-196) — so tokens that never set a custom photo keep working exactly as today.
- `health INTEGER NULL`, `max_health INTEGER NULL` — ported concept from `world_tokens` (`schema.rs:408-423`), since `TokenPanel.tsx`'s health bar is a User-Story-relevant feature of the panel this spec keeps.

**Rationale**: These are the fields FR-009/FR-009a (primary token, player-editable photo) and the "TokenPanel stays for ... health-bar editing" assumption require, and none exist today on either table in a scene-correct shape.

**Alternatives considered**: A separate `token_ownership` join table instead of a column — rejected as overengineering for a 1:1 (token → controlling user) relationship; a plain nullable FK column is simpler and matches the existing `actor_id` column's own nullable-FK convention on the same table.

## 4. Field-level authorization: GM vs. controlling-player, via two narrow mutations rather than one generic patch

**Decision**: Keep `update_token` (existing, scene-owner/GM-only, unchanged authorization) as the full-control path for GMs — position, size, rotation, health, `owner_user_id`, `is_primary`, `photo_url`, everything. Add two new, narrowly-scoped mutations for player-initiated changes, each enforcing its own DB-level filter (per ADR-033's "ownership enforced at the DB level, not in Rust code" pattern, and Constitution Principle III):
- `move_own_token(token_id, x, y)` — succeeds only where `tokens.owner_user_id = <requesting user>`; touches only `x`/`y`. No resize/rotate/health/photo access.
- `set_own_primary_token_photo(token_id, photo_url)` — succeeds only where `tokens.owner_user_id = <requesting user> AND tokens.is_primary = true`.

**Evidence**: `mutations_walls.rs`/`mutations_lighting.rs`/`mutations_tokens.rs`'s existing convention is already "one purpose-built mutation per capability," not a single generic patch endpoint (e.g. `update_wall` vs. a hypothetical door-only mutation never introduced) — the two new mutations follow that same house style rather than inventing per-field permission-matrix logic inside `update_token`.

**Rationale**: A single generic `update_token` reachable by non-GMs would need to re-derive, per field, which caller is allowed to touch it — more error-prone than two small, single-purpose, independently-testable mutations whose entire authorization story is one `.filter()` clause each.

**Alternatives considered**: Extending `update_token` with an internal permission matrix — rejected as harder to unit-test exhaustively and a departure from the existing narrow-mutation convention.

## 5. Engine-side drag/resize/rotate: extend the existing wall/shape handle pattern, not `token_sync_d2.rs`

**Decision**: Build token resize/rotate handles by directly mirroring `src/engine/src/systems/wall.rs`'s endpoint-handle pattern (`WallHandle` marker, `WallDragMode::MovingEndpoint`, GM-gated handle spawn in `sync_wall_visuals` lines 627-657) and `src/engine/src/systems/shape.rs`'s corner-resize handle pattern (`handle_shape_input` line 261, `sync_shape_visuals` line 617). Extend the existing (currently trivial, 19-line) `src/engine/src/plugins/token.rs` into a proper plugin chaining: existing drag input (already in `selection.rs`'s `handle_token_drag`, to be moved/renamed into the token plugin's own systems module for consistency with Principle II) + new resize-handle input + new rotate-handle input + visual sync, gated by the existing `IsGameMaster` resource for handle rendering, and by a new "is this my controlled token" check for player-drag eligibility. `src/engine/src/systems/token_sync_d2.rs` (the `wasm32`-gated stub) is legacy/superseded and should not be extended further — new logic goes in the new/renamed token systems module instead.

**Evidence**: `wall.rs:47` (`WallHandle`), `:63` (`WallDragMode::MovingEndpoint`), `:627-657` (GM-gated handle spawn); `shape.rs:261` (`handle_shape_input`), `:617` (`sync_shape_visuals`); `selection.rs:48-120` (`handle_token_drag`, position-only today), `:106-115` (emits token mutation on release), `:125-140` (`render_selection_feedback`, opacity/z-order only, no handles today).

**Rationale**: Constitution Principle II requires new engine capability to ship as a self-contained plugin with its own systems module — `token.rs`'s current 19 lines is a placeholder, not yet a real plugin in the sense walls/shapes are.

## 6. Frontend: new canvas-native `TokenTool`, following `WallTool.tsx`'s worldStore-dispatch convention

**Decision**: Add a new `apps/web/src/components/canvas-tools/TokenTool/TokenTool.tsx` (net-new, mirroring `WallTool.tsx`'s structure: `worldStore.dispatch({type:"update_token", ...}, "ui")`, `Panel`/`Checkbox` primitives, `selectedTokenId` prop, GM-only mounting) for GM-facing resize/rotate/full-control affordances. `TokenPanel.tsx` is rewired (per research §1) onto the `tokens` table and slimmed to the responsibilities the spec keeps for it: bulk create/delete, health editing, and (new) primary-token photo editing — it stops being the place position is set, since canvas drag now owns that for both GM and player-controlled tokens.

**Evidence**: `WallTool.tsx:53-69` (`worldStore.dispatch` convention), `:95` (`selectedWallId` prop pattern), `:33-36` (GM-only-mount self-documentation); `WorldPage.tsx:506` (`{isSceneOwner && sceneId ? (<WallTool .../>) : null}`, same guard reused at lines 486/519/538 for sibling tool panels) — the same `isSceneOwner && sceneId` guard gates mounting `TokenTool` and, inside the engine, gates handle rendering (mirroring `wall.rs`'s `IsGameMaster`-gated handle spawn at line 634).

**Rationale**: Keeps the new tool consistent with the established generation of canvas tools (walls/shapes/lighting), rather than extending the older TokenPanel/RxDB/direct-GraphQL generation the constitution's Principle I rationale already flagged as the pattern to move away from.

## 7. Scene-switch loading/error feedback: currently no UI state at all

**Decision**: Add a loading/error state machine around the four per-scene loader calls in `WorldPage.tsx` (`loadWallsIntoStore` line 282, `loadTokensIntoStore` line 298, `loadLightsIntoStore` line 314, `loadShapesIntoStore` line 340, plus background image loading), replacing their current `.catch((error) => console.error(...))`-only handling (no UI signal at all today) with a single aggregate loading/error state surfaced over the canvas area, plus a retry action (per the clarified FR-013a) that re-invokes the same four loaders (and background fetch) for the current `sceneId` without changing scenes.

**Evidence**: `WorldPage.tsx:282-284, 298-300, 314-316, 340-342` (`.catch` with only a `console.error`, no state update); `SceneSwitcher.tsx:55-77` (existing loading state exists only for scene *creation*, not for switching to an existing scene).

**Rationale**: Directly closes the gap User Story 4 names — there is genuinely no existing loading/error UI to reuse or extend beyond the create-scene dialog's own local state, confirmed by reading the actual scene-switch code path rather than assuming.

**Alternatives considered**: Per-widget loading states (one spinner per wall/light/token/background load) — rejected as more visual noise than a single aggregate "scene is loading" / "scene failed to load, retry" state, which is what the spec's acceptance scenarios describe (a single loading indicator, a single error state).
