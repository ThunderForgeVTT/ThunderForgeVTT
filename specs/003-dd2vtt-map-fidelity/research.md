# Phase 0 Research: dd2vtt Map Fidelity & From-Scratch Map Editor Tooling

## 1. Wall passability (User Story 1) — already fully built, not a gap

**Decision**: Treat User Story 1's passability requirement as a **verification** task, not a build task. No new column, mutation, or engine system.

**Evidence**:
- `src/server/src/schema.rs:313-328` — the `walls` table already has independent `blocks_vision: Bool` and `blocks_movement: Bool` columns, separate from `door_state: Text`.
- `src/server/src/graphql/input_types.rs:87-101` (`GraphQLCreateWallInput`/`GraphQLUpdateWallInput`) already accept `blocks_vision: Option<bool>` and `blocks_movement: Option<bool>` independently.
- `src/server/src/graphql/mutations_walls.rs:41-42, 73-74, 128-129` — `update_wall` already writes both fields independently to the DB.
- `crates/thunderforge-canvas-core/src/wall.rs:72-87` — `Wall::currently_blocks_vision()`/`currently_blocks_movement()` already compute the live blocking state from these two independent flags (with `door_state == Open` as a temporary override for movement/vision alike).
- `apps/web/src/components/canvas-tools/WallTool/WallTool.tsx:101-121` — the property panel shown on wall selection already renders two independent checkboxes ("Blocks vision", "Blocks movement") wired to `updateSelectedWall({ blocksVision })`/`updateSelectedWall({ blocksMovement })`.
- `src/engine/src/systems/wall.rs:334` — a `B` keyboard shortcut already exists to toggle `blocks_movement` on the selected wall directly in the engine.

**Rationale**: The only gap between what exists and what spec.md's User Story 1 describes is the *interaction pattern* — the user described "right-click a wall segment," while today's flow is "left-click to select, then use the side panel checkbox (or press `B`)." That's a UX-affordance question, not a missing capability. Given spec 002's precedent (T014 found `IsGameMaster` was *completely* unwired despite looking done at the code level), this feature's job for User Story 1 is to actually exercise the existing flow live — select a wall, toggle each checkbox, confirm the live-sync and second-session behavior described in spec.md's Acceptance Scenarios 2 and 4 — before concluding no code change is needed. If live verification finds it *does* work end-to-end, the only remaining decision is whether to add a right-click context-menu shortcut as a UX enhancement (optional, not required by any FR).

**Alternatives considered**: Building a new "window" wall-state enum variant — rejected after direct clarification (see spec.md's corrected User Story 1); the existing two independent boolean flags already express everything a "window" would need.

## 2. Torch (light source) placement (User Story 1) — already fully built

**Decision**: Same treatment as passability — verify live, do not build.

**Evidence**: `src/engine/src/systems/lighting.rs:138-194` (`handle_light_input`, GM-gated at line 148-150) already handles click-to-place, emitting a `create_light` event server-ward; `src/server/src/graphql/mutations_lighting.rs:20` (`create_light_source`) is already scene-owner-scoped identically to walls; `src/engine/src/plugins/lighting.rs:22-42` already chains input/resize/keyboard-toggle/undo/sync/illumination systems.

**Rationale**: No functional requirement in spec.md asks for anything beyond "GM places a torch, it lights up live for everyone" — which is exactly what's already built (spec 001).

## 3. GM-only enforcement — reuse verbatim

**Decision**: Any new code this feature does need (the import `warnings` field, the round-trip test) reuses the identical scene-owner authorization pattern already used by `mutations_walls.rs`/`mutations_lighting.rs`: `walls::scene_id.eq_any(scenes::table.filter(scenes::owner_id.eq(user_id)).select(scenes::scene_id))`.

**Rationale**: Constitution Principle III requires server-side enforcement at the data boundary; copying the existing, already-tested pattern (rather than writing a new check) is both faster and keeps the enforcement surface consistent, per the same reasoning spec 002's `require_world_member` guard used for a *new* mutation family — but here, unlike spec 002, no new mutation family exists at all, just reuse of existing ones.

## 4. Round-trip persistence test pattern (User Story 2)

**Decision**: Model the round-trip test(s) directly on `src/server/src/graphql/mutations_assets.rs:288-327`'s `upload_canvas_image_happy_path_produces_webp_asset` test: build fixtures via `test_support.rs`'s `insert_test_user`/`insert_test_world`/`insert_test_scene` (spec 002 convention), perform the write (either the import handler directly, or a sequence of wall/light/shape/token mutations for the "hand-built" half of User Story 2's Acceptance Scenario 3), then **re-query the row(s) via a fresh `SELECT ... .first::<T>(&mut conn)`** (not just trust the mutation's return value) and assert field-for-field equality against what was written.

**Rationale**: This is the one template in the codebase that actually re-fetches from the DB rather than trusting an in-memory return value — the real distinction spec.md's User Story 2 draws between "parsing succeeded with the right counts" (existing `map_import.rs` tests) and "the *persisted* data is identical" (what's missing today). `mutations_walls.rs`'s `door_state_round_trips_through_db_string_representation` test and `map_import.rs`'s existing tests use an older, more verbose manual-insert style — not used as the template, since the fixture-based `mutations_assets.rs` style is more consistent with this session's established conventions and less code per test.

**Alternatives considered**: Snapshot/golden-file testing (serialize a scene to JSON, compare against a checked-in golden file) — rejected as heavier tooling than needed; direct field-assertion tests are the established local pattern and sufficient for FR-008's requirement.

## 5. Import result "skipped field" disclosure (User Story 3) — genuinely new work

**Decision**: Add a `warnings: Vec<String>` (or a small structured `{ field: String, reason: String }` list — decided at implementation time based on what reads better in the existing REST JSON response) to `map_import.rs`'s `import_uvtt` response shape (currently `{"wallsCreated", "doorsCreated", "lightsCreated", "backgroundImageSet", "skippedDegeneratePolygons"}`, `map_import.rs:604-611`), populated when a freestanding portal, non-default `ambient_light`, or non-empty `objects_line_of_sight` is present in the source file and not applied.

**Rationale**: `skippedDegeneratePolygons` (an existing integer count) is the only precedent for "count of something silently dropped" in this response — there is no existing warnings/notices array to extend, confirmed by inspecting the full `MapImportError` enum and response struct. This is the one piece of genuinely new server-side work in this feature.

**Alternatives considered**: A separate GraphQL query to fetch import warnings after the fact — rejected as unnecessary indirection; the import is a single synchronous REST call today (per spec.md's edge cases, unchanged by this feature), so returning warnings inline in that same response is simplest and matches FR-013's "without requiring the GM to inspect server logs."

## 6. UVTT field parsing — confirmed dead-but-present fields

**Decision**: No parser change needed to *detect* the three field categories — they're already parsed into the `Uvtt*` structs, just marked `#[allow(dead_code)]` and never consumed. User Story 3's work is entirely about *reading* these already-parsed values at the point the import response is constructed and populating `warnings` accordingly — not about changing what's parsed.

**Evidence**: `src/server/src/map_import.rs:78-94` (`UvttPortal.position`/`.rotation`/`.freestanding`, all `#[allow(dead_code)]`), `:98-102` (`UvttEnvironment.baked_lighting`/`.ambient_light`, same attribute), with an explanatory block comment at `:51-61` already anticipating this exact follow-up ("kept for parser correctness/round-tripping and as the natural place to add ambient-light or portal-orientation handling later").

**Rationale**: Confirms spec.md's Assumption that this is "detect and disclose," not "fully implement" — the detection already exists; only the disclosure (populating `warnings`) is new.

## 7. Reference fixtures — real-world coverage gap for two field categories

**Decision**: `objects_line_of_sight` and `freestanding` portals need a **hand-crafted fixture**, not a real-world file, for User Story 3's tests.

**Rationale**: All 64 source files surveyed when cherry-picking this feature's 5 reference maps (see spec.md Assumptions) were checked programmatically for these two fields; none had a non-empty `objects_line_of_sight` array or a `freestanding: true` portal. Only `ambient_light` has real-world coverage (`little-fish-academy.dd2vtt`, `ambient_light: "fffff7e4"`). A minimal synthetic `.dd2vtt` JSON fixture (hand-written, not exported from DungeonDraft) is needed to exercise the other two Acceptance Scenarios in User Story 3.
