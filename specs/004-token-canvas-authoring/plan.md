# Implementation Plan: Canvas-Native Token Authoring & Scene-Switch Loading Feedback

**Branch**: `004-token-canvas-authoring` | **Date**: 2026-08-20 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/004-token-canvas-authoring/spec.md`

**Note**: This template is filled in by the `/speckit-plan` command; its definition describes the execution workflow.

## Summary

Bring tokens up to the same canvas-native authoring tier walls/shapes/lights already have (spec 001), plus fix a genuine usability gap in scene switching (no loading/error feedback exists today). Research surfaced a real architectural problem along the way: the canvas engine and the existing `TokenPanel.tsx` currently operate on two entirely disconnected tables (`tokens`, scene-scoped, engine-facing vs. `world_tokens`, world-scoped, panel-facing) — moving a token in one has never affected the other. This plan unifies both onto the scene-scoped `tokens` table (extended with `owner_user_id`, `is_primary`, `photo_url`, `health`, `max_health`), retires `world_tokens` as unread legacy data, and builds new Bevy-side drag/resize/rotate handles for tokens mirroring the existing wall/shape handle pattern, plus a client-side loading/error state machine around scene switching. This is a genuinely larger plan than spec 003's "mostly verification" scope — real schema change, real new engine systems, real new mutations, and an ADR requirement (Constitution Principle IV) for the backing-store unification.

## Technical Context

**Language/Version**: Rust 2024 edition (engine crate → `wasm32-unknown-unknown` via Bevy 0.18; server crate native), TypeScript/React (`apps/web`) — unchanged from specs 001-003.

**Primary Dependencies**: Existing stack only — Bevy 0.18 (engine), Axum + `async-graphql` + Diesel/PostgreSQL (server), Playwright (`apps/web/e2e/`). RxDB continues to back `TokenPanel.tsx`'s sync, now pointed at `tokens` instead of `world_tokens`. No new dependency.

**Storage**: PostgreSQL. One new migration on the existing `tokens` table (five new columns + one partial unique index — see data-model.md); `world_tokens` table left in place, unread, not migrated (research.md §1).

**Testing**: `cargo test` (server — new migration, new mutations, ownership-filter tests), `cargo check --target wasm32-unknown-unknown` (engine — new token plugin/systems), Playwright (`apps/web/e2e/`, new `token-authoring.spec.ts` plus scene-switch loading/error coverage), manual quickstart walkthrough for the two-browser-context scenarios.

**Target Platform**: Linux server + WASM in-browser (unchanged).

**Project Type**: Web application — existing `src/engine` / `src/server` / `apps/web` three-part layout, unchanged.

**Performance Goals**: Token drag/resize/rotate at interactive framerate on the canvas (matching existing wall/shape drag feel); cross-client sync within the same few-seconds bar already established and verified for walls/shapes/lights (SC-002).

**Constraints**: Constitution Principle I — token position/size/rotation authoring must live in Bevy systems, not React/RxDB state, once the new `TokenTool` exists (TokenPanel keeps only non-canvas fields: health, photo, bulk create/delete). Principle III — reuse the existing scene-owner DB-level filter pattern for GM mutations; the two new player-facing mutations (`move_own_token`, `set_own_primary_token_photo`) introduce no new authorization *mechanism*, only a new column (`owner_user_id`) filtered the same way. Principle IV — the `world_tokens`→`tokens` backing-store unification is architecturally significant and REQUIRES a new ADR before implementation diverges across files (see research.md §1); this plan does not itself author that ADR, but implementation MUST NOT proceed past the schema-migration task until it exists.

**Scale/Scope**: Builds on specs 001 (wall/light/shape hand-drawn authoring engine plugins, the pattern this feature's token handles mirror) and the existing (if disconnected) token infrastructure from ADR-033/the `tokens` table migration. Retires part of ADR-033's original `world_tokens` design in favor of its own already-existing-in-practice `tokens` successor.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

- **Principle I (ECS owns simulation)**: PASS, with a design correction required. Today, token position is partially owned by React/RxDB (`TokenPanel.tsx`'s direct `moveToken` calls against `world_tokens`) — a Principle I violation already latent in the codebase. This feature does not introduce that violation; it removes it, by making the Bevy engine (via the new `TokenTool`/token plugin) the only path that writes `tokens.x/y/rotation/scale`, and narrowing `TokenPanel.tsx` to fields Principle I doesn't govern (health, photo, create/delete which are inherently server-mutation-triggered, not simulation state).
- **Principle II (Plugin-modular engine)**: PASS. `src/engine/src/plugins/token.rs` (currently 19 lines, a placeholder) becomes a real plugin following `WallPlugin`/`ShapePlugin`'s shape: its own systems for drag input (extending what already exists in `selection.rs`), new resize-handle input, new rotate-handle input, and visual sync — chained in the plugin, not scattered.
- **Principle III (Ownership/authorization at the data boundary)**: PASS. GM mutations reuse the existing scene-owner filter verbatim. The two new player-facing mutations add exactly one new filter shape (`tokens.owner_user_id = requester`, optionally `AND is_primary` for the photo mutation) — same DB-level-filter pattern ADR-033 and Principle III both already mandate, applied to a new but structurally identical column.
- **Principle IV (ADRs before divergent implementation)**: **FLAGGED — REQUIRED, not yet written.** The `world_tokens`→`tokens` unification is exactly the kind of architecturally significant, established-pattern-changing decision this principle exists for (it formally retires part of ADR-033). An ADR must be authored (e.g. `docs/adrs/<date>-0XX-unify_token_backing_store.md`) before the schema-migration task begins, referencing and partially superseding ADR-033. This is called out explicitly as a required precondition task, not silently assumed.
- **Principle V (Verify before done)**: Plan calls for `cargo check --target wasm32-unknown-unknown` (engine), native `cargo check`/`cargo test` (server, including a migration up/down check and the new mutations' authorization-filter tests), `tsc`/build (web), and live Playwright + manual two-browser-context verification for the drag/resize/rotate/sync and loading/error scenarios (matching spec 003's precedent that live verification is not optional when the premise is "does this actually work end-to-end").

**Gate result**: PASS, conditional on the Principle IV ADR being authored before the migration task lands — recorded here so `/speckit-tasks` includes it as an explicit blocking task, not an assumption.

## Project Structure

### Documentation (this feature)

```text
specs/004-token-canvas-authoring/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md         # Phase 1 output
├── quickstart.md         # Phase 1 output
├── contracts/            # Phase 1 output
│   ├── token-mutations.md
│   └── scene-load-state.md
└── tasks.md              # Phase 2 output (/speckit-tasks — not created by this command)
```

### Source Code (repository root)

```text
docs/adrs/
└── (new) <date>-0XX-unify_token_backing_store.md   # REQUIRED before migration task — Principle IV gate

src/server/
├── migrations/
│   └── (new) <timestamp>_add_ownership_and_photo_to_tokens/   # owner_user_id, is_primary, photo_url, health, max_health + partial unique index
├── src/schema.rs                          # regenerated after migration
├── src/models.rs                          # Token/NewToken/TokenUpdate structs extended
└── src/graphql/mutations_tokens.rs        # update_token input extended; + move_own_token, + set_own_primary_token_photo

src/engine/src/
├── plugins/token.rs                       # grows from 19-line placeholder into a real TokenPlugin (drag/resize/rotate/sync systems chained)
├── systems/selection.rs                   # existing handle_token_drag extended/relocated into the token plugin's own systems module
└── systems/wall.rs, systems/shape.rs      # reference-only — handle-rendering pattern mirrored, not modified

apps/web/src/
├── components/canvas-tools/TokenTool/     # (new) TokenTool.tsx, mirrors WallTool.tsx's worldStore-dispatch convention
├── components/TokenPanel.tsx              # rewired off world_tokens RxDB collection onto tokens; slimmed to health/photo/bulk-CRUD
├── engine/world/sync/tokens.ts            # sync shape extended with the 5 new fields
└── pages/world/WorldPage.tsx              # scene-load state machine (loading/ready/error/retry) wraps the 4 existing per-scene loaders + background fetch

apps/web/e2e/
└── (new) token-authoring.spec.ts          # US1-US3 live verification; scene-load feedback covered here or a sibling spec file
```

**Structure Decision**: Existing three-part layout (`src/engine`, `src/server`, `apps/web`) reused as-is. Unlike spec 003, this feature does add new backend schema (one migration), a real new engine plugin (token drag/resize/rotate handles), and two new GraphQL mutations — it is not primarily a verification pass, though it does verify/reuse the existing (already-correct) `rotation`/`scale` columns and `update_token` mutation rather than re-deriving them.

## Complexity Tracking

> **Fill ONLY if Constitution Check has violations that must be justified**

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| Principle IV: ADR required before migration task | Retiring part of ADR-033's design (`world_tokens`'s role) and introducing a new ownership column/authorization surface is architecturally significant per the constitution's own definition | Skipping the ADR was considered (spec 003 needed none) but rejected — spec 003 introduced no new subsystem or ownership boundary; this feature does both (new `owner_user_id` authorization surface, retired data model), which is exactly what Principle IV's trigger condition names |
