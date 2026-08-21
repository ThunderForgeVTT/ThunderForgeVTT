# Implementation Plan: Token Authoring Polish — Real Resize/Rotate Handles & Reliable Ownership Assignment

**Branch**: `006-token-authoring-polish` | **Date**: 2026-08-20 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/006-token-authoring-polish/spec.md`

**Note**: This template is filled in by the `/speckit-plan` command; its definition describes the execution workflow.

## Summary

Closes spec 004's three documented open items. User Story 1 replaces the keyboard-shortcut resize/rotate stand-in with real canvas-rendered drag handles, which requires first restructuring `src/engine/src/plugins/token.rs` from its current 18-line placeholder into a proper plugin (Constitution Principle II), mirroring `WallPlugin`/`ShapePlugin`'s established shape. User Story 2 fixes a still-open Radix Popover auto-dismissal race in `TokenPanel`'s ownership-assignment UI that two prior fix attempts narrowed but didn't resolve — this plan calls for live instrumentation (not another guess) as the research method. Closing both un-skips spec 004's one remaining test and, per research.md §4, makes the full automated suite's run itself the "connected walkthrough" spec 004's T039 asked for — no separate manual QA pass is needed as its own deliverable.

## Technical Context

**Language/Version**: Rust 2024 edition (engine crate → `wasm32-unknown-unknown` via Bevy 0.18), TypeScript/React (`apps/web`) — unchanged from specs 001-005.

**Primary Dependencies**: Existing stack only — no new dependency. Radix UI (`Popover`) is already in use; this feature only changes how it's used/instrumented, not which library.

**Storage**: PostgreSQL — no schema change. This feature touches interaction mechanism and UI reliability only; `update_token`'s existing shape (spec 004) is unchanged.

**Testing**: `cargo check --target wasm32-unknown-unknown` (engine — new handle systems), Playwright (`apps/web/e2e/token-authoring.spec.ts` — un-skip the existing test once User Story 2 lands, update the resize/rotate tests to drive drag handles instead of keyboard shortcuts for User Story 1), manual React DevTools Profiler session for User Story 2's research phase (not itself a shippable artifact, but a required research step per research.md §3).

**Target Platform**: Linux server + WASM in-browser (unchanged).

**Project Type**: Web application — existing `src/engine` / `apps/web` layout. No `src/server` changes (FR-008 explicitly rules out server-side authorization changes).

**Performance Goals**: Handle drag responsiveness matching existing wall/shape handle drag feel (interactive framerate, no perceptible lag). Popover interaction completing within "a couple of seconds" per FR-007 — matching normal UI responsiveness, not a new stricter target.

**Constraints**: Constitution Principle II — `token.rs`'s plugin restructuring (research.md §1) must happen before or alongside the new handle systems, not after, since building handles on top of the current disconnected `selection.rs`/`token.rs` split would perpetuate the exact debt Principle II exists to prevent. Constitution Principle V — this feature's User Story 2 research step (live instrumentation) is itself a verification activity the plan calls for explicitly, not an implementation detail to skip.

**Scale/Scope**: Directly closes out spec 004 (canvas-native token authoring, merged to main). Does not touch spec 005 (subscription transport, separately tracked, not yet implemented).

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

- **Principle I (ECS owns simulation)**: PASS. Resize/rotate remain engine-owned `Transform.scale`/`.rotation` changes dispatched through the same `upsert_token`/`update_token` path spec 004 established; TokenPanel's popover fix is a UI-reliability change, not a new source of truth for token state.
- **Principle II (Plugin-modular engine)**: PASS, and this is the plan's central structural fix. `token.rs` grows from a placeholder into a real plugin per research.md §1, with drag/resize/rotate/sync systems chained inside it, mirroring `WallPlugin`.
- **Principle III (Ownership/authorization at the data boundary)**: PASS, unchanged — FR-008 explicitly rules out any authorization-logic change; this feature is interaction/UI-reliability only.
- **Principle IV (ADRs before divergent implementation)**: N/A — no new subsystem, dependency, or ownership boundary. This is a structural refactor within an already-established plugin pattern and a UI bug fix, not an architecturally significant decision requiring a new ADR.
- **Principle V (Verify before done)**: Plan calls for `cargo check --target wasm32-unknown-unknown` (engine, new handle systems), Playwright live verification (both user stories, including the previously-skipped test un-skipped and passing repeatedly per SC-003), and an explicit live-instrumentation research step for User Story 2 before attempting a fix — not another blind guess.

**Gate result**: PASS, no violations, no Complexity Tracking entries needed.

## Project Structure

### Documentation (this feature)

```text
specs/006-token-authoring-polish/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── quickstart.md         # Phase 1 output
└── tasks.md              # Phase 2 output (/speckit-tasks — not created by this command)
```

No `data-model.md` or `contracts/` — no schema change, no new API contract (per Technical Context above).

### Source Code (repository root)

```text
src/engine/src/
├── plugins/token.rs        # grows from 18-line placeholder into a real TokenPlugin, mirroring plugins/wall.rs
├── systems/token.rs         # (new) handle_token_drag + handle_token_resize_rotate_keyboard relocated from selection.rs, plus new handle_token_resize_drag/handle_token_rotate_drag and sync_token_visuals
└── systems/selection.rs     # token-specific systems removed (relocated above); non-token selection logic (if any) stays

apps/web/src/
└── components/TokenPanel.tsx  # Popover dismissal fix (User Story 2) — exact change TBD pending research.md §3's live-instrumentation step

apps/web/e2e/
└── token-authoring.spec.ts   # US1: resize/rotate tests updated to drive drag handles instead of keyboard shortcuts; US2: the test.skip removed once the fix lands, confirmed passing 3x consecutively (SC-003)
```

**Structure Decision**: Existing `src/engine` / `apps/web` layout, unchanged. This feature touches no `src/server` files at all (confirmed by FR-008 and the Technical Context's Storage/Testing sections) — it is purely an engine-plugin restructuring plus a frontend UI-reliability fix.

## Complexity Tracking

> **Fill ONLY if Constitution Check has violations that must be justified**

No violations — table intentionally omitted.
