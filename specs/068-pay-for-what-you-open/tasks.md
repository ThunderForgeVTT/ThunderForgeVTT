---
description: "Task list for Pay for What You Open"
---

# Tasks: Pay for What You Open

**Input**: [spec.md](./spec.md), [ADR-091](../../docs/adrs/20260907-091-instance_configuration_is_rows.md)

**Tests**: written with the code they hold. Each story ends on the e2e slices its changes touch, run from the main checkout.

**Baseline** (commit `0314d369`, `vite build`): 152 chunks, 2,569,524 bytes raw. Entry static closure 504,029 raw / 138,941 brotli. Beyond the entry: scene detail 818,729 / 196,025; the board 487,458 / 124,083; world staging 234,266 / 64,397; compendium 173,258 / 48,724; a lore entry 842,344 / 201,618.

## Format: `[ID] [P?] [Story] Description`

---

## Phase 1: The measure (Story 1)

- [X] T001 [US1] `scripts/check-bundle-budget.mjs`: follow static imports from the entry and from named route chunks in a build directory, report raw and brotli, fail over budget or on a forbidden module; its own tests under `pnpm test:scripts` (FR-009)
- [X] T002 [US1] Budgets recorded from the baseline, so the check passes before anything is split and every later task lowers a number

## Phase 2: The splits (Story 1)

- [X] T003 [US1] The three static CodeMirror imports go behind `React.lazy` with a read-only fallback: `SceneSummaryEditor`, `LoreMarkdownEditor` (two callers) (FR-002, FR-003)
- [X] T004 [US1] The feedback dialog loads on press, from the launcher and from the help panel; log capture stays in the entry (FR-001)
- [X] T005 [US1] A boundary for a dynamic import that fails, used by every `lazy` this spec adds (FR-007)
- [X] T006 [US1] System panels and sheets: chunks of their own, fetched for the world's system where one is mounted; slot presence known without loading, the dock title from that system's dock file (FR-004, FR-005)
- [X] T007 [US1] The board's authoring tools load when opened (FR-006)
- [X] T008 [US1] `manualChunks` loses the tldraw/RxDB/RxJS rule (FR-008)
- [X] T009 [US1] Budgets lowered to what the build now measures; CodeMirror and the six authoring tools on the forbidden list (SC-001 – SC-004). Measured, brotli: entry 131,608; scene detail 20,036; the board 95,749; world staging 39,940; compendium 50,979; a lore entry 26,196
- [X] T010 [US1] Proof: web unit, `pnpm test:scripts`, `make lint`; the slices that cover scenes, lore, feedback, the board and game systems, from main (SC-005). Scenes, lore, feedback, game-systems, genie, canvas, interactive and lighting pass. Lighting first failed about one run in three at `carried-light.spec.ts`: a walk lost a step. The splits did not cause it, they changed a board's timing enough to show it — the engine applied the web app's queued commands and read the frame's keys in no declared order, so a position read-back could undo a step in the frame it was taken. Ordered in `e7bb5575`; eight runs of the spec and the slice pass

## Phase 3: Flags (Story 2)

- [X] T011 [US2] The first flag chosen with the owner (FR-015): source-book import, `feature.book_import`, on by default (owner, 2026-10-04)
- [X] T012 [US2] Registry: a `Features` group, a public marker, `settings::flag_on` (FR-010, FR-011)
- [X] T013 [US2] `featureFlags` read, public to anyone and whole to a member (FR-012)
- [X] T014 [US2] Web: `api/featureFlags.ts`, `useFeatureFlag`, refreshed on the administrator's change (FR-013)
- [X] T015 [US2] The first flag wired on the server and in the web app, with its e2e (SC-006). `instance-feature-flags.spec.ts`; the instance (42), book-import (17) and collections (30) slices pass. The e2e found that a visitor could not reach the read at all — it now goes to `/api/graphql/public`
- [X] T016 [US2] `docs/CONTRIBUTING.md`: when a feature takes a flag, how to declare one, when to remove it (FR-014)
