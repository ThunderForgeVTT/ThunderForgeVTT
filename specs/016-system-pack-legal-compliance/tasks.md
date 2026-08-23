---

description: "Task list for feature implementation"
---

# Tasks: System Pack Legal & Attribution Compliance

**Input**: Design documents from `/specs/016-system-pack-legal-compliance/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/manifest-legal-schema.md, quickstart.md (all present)

**Tests**: Not a separate TDD phase — a small Rust unit test confirming a manifest missing `legal` fails validation (mirroring `packs/systems/dnd5e/server/src/validators.rs`'s existing test conventions) is folded into T007 below.

**Organization**: Tasks are grouped by user story (spec.md priorities).

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (US1-US3)
- Every task includes an exact file path

## Path Conventions

Existing three-part layout: `src/server/` (Rust pack loader/validator), `apps/web/` (React), `packs/systems/*/` (per-system manifests). No `src/engine` changes.

**Important scope correction found while planning (not in plan.md, verified against the actual codebase)**: plan.md's Project Structure assumes "the system-selection step of world creation" already exists as a UI surface to hook `SystemLegalNotice` into. **It does not.** Spec 008 (`specs/008-seamless-onboarding-flow`) deliberately *removed* game-system selection from `CreateWorldPage.tsx` (see that file's own `T014 (US2)` comment — world creation is intentionally name+description only now) and no world-settings page or system-assignment UI exists anywhere else in the app today (`world.gameSystemId` is currently a read-only "Not yet assigned" display on `WorldDashboardPage.tsx`, with no way to actually set/change it). Re-adding a picker to `CreateWorldPage.tsx` would contradict spec 008's explicit decision. **Resolution**: this spec builds one new surface — a minimal per-world System Settings view — that is simultaneously (a) the FR-005 persistent settings location, AND (b) the FR-004 "point of selecting/changing a system" location, since system assignment doesn't happen anywhere else. This satisfies both FRs without reopening spec 008's onboarding-simplification decision. Tasks below (T012-T015) reflect this.

---

## Phase 1: Setup

**Not applicable as a separate phase** — this feature's only prerequisite is the manifest contract/type additions, folded into Phase 2.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: The `legal` field's shape, contract, and validation exist before anything renders it or any pack is required to supply it.

- [X] T001 [P] Add `SystemManifestLegal` type and extend `SystemManifest` with a required `legal: SystemManifestLegal` field in `apps/web/src/contexts/GameSystemContext.tsx` (contracts/manifest-legal-schema.md's TypeScript contract)
- [X] T002 [P] Add `SystemManifestLegal` struct (`#[serde(rename_all = "camelCase")]`, matching whatever existing convention `src/server/src/systems.rs` uses for manifest sub-structs) to `src/server/src/systems.rs`, and add `legal: SystemManifestLegal` to the manifest struct it currently deserializes `system.json` into
- [X] T003 Implement `validate_legal(manifest) -> Result<(), ValidationError>` in `packs/systems/dnd5e/server/src/validators.rs` (or wherever the shared/generic manifest-structural validation actually lives — check whether validation is per-pack or centralized in `src/server/src/systems.rs`/`system_hooks.rs` before choosing the file; contracts/manifest-legal-schema.md's Validator contract is the reference shape) — rejects a manifest with no `legal` object, or with an empty `licenseName`/`attributionText`, per FR-007 (depends on T002)
- [X] T004 Wire `validate_legal` into the pack-loading path so a non-compliant manifest is rejected/flagged at load time (fail closed, per Constitution Principle III/V), not merely available-but-unchecked — locate the actual load entry point in `src/server/src/systems.rs`/`system_hooks.rs` first (depends on T003)
- [X] T005 [P] Add a `#[tokio::test]` (or equivalent) confirming a manifest missing `legal` (and one with an empty `attributionText`) is rejected by T004's load path, alongside existing validator tests (depends on T004)
- [X] T006 [US2] Populate `packs/systems/dnd5e/system.json`'s new `legal` object directly from `research/system_dnd5e.json`'s existing `legal` object (research/ is gitignored/local-only, so read it locally — do not attempt to commit or reference it from application code), translating field names per contracts/manifest-legal-schema.md (`license_name`→`licenseName`, `attribution_text`→`attributionText`, `disclaimer`→`disclaimer`, `trademark_restrictions`→`trademarkRestrictions`, `required_ui_placement`→`requiredUiPlacement`; the digest's `display_name_safe_harbor`/`recommended_module_label`/`marketing_*_examples` fields are NOT part of the contracted shape — they're marketing guidance, not required manifest fields, and can be dropped or left as an internal comment/note, not invented into new schema fields) (depends on T001, T002)

**Checkpoint**: The `legal` field is typed, validated, enforced at load time, and `dnd5e` actually carries one. User story implementation (rendering) can begin.

---

## Phase 3: User Story 2 - A system pack author fills in one standard `legal` field and the app handles the rest (Priority: P1)

**Goal**: The manifest contract has one documented, schema-checked place for legal metadata; `dnd5e` demonstrates it; a manifest without it fails to load.

**Independent Test**: Author a `legal` block for a hypothetical new pack per the contract doc alone and confirm it validates; confirm `dnd5e`'s manifest now carries a complete `legal` object sourced from its research digest with no reshaping; confirm a manifest missing `legal` is rejected (quickstart.md).

*(Implementation for this story is entirely covered by Phase 2's T001-T006 — sequenced first because User Story 1's rendering has nothing to render without it. No additional tasks here; this section exists so the story is independently checkable per spec.md's priority ordering.)*

**Checkpoint**: A pack author has one clear place to declare legal metadata, and non-compliant packs can't silently ship.

---

## Phase 4: User Story 1 - A GM sees required legal/attribution notices when choosing a system for their world (Priority: P1) 🎯 MVP-critical

**Goal**: A GM can see `dnd5e`'s (and, once built, any future pack's) legal notice both at the point of assigning/changing a world's system and at any later time from a persistent settings location — per this file's scope-correction note above, both are the same new surface.

**Independent Test**: Open the new world System Settings view for a world with no system assigned yet, select `dnd5e`, confirm its full legal notice (attribution text, disclaimer, trademark restrictions) displays as part of confirming that selection; reload the page later and confirm the same notice is still reachable from that same settings view without re-selecting anything (quickstart.md).

### Implementation for User Story 1

- [X] T007 [P] [US1] Create `apps/web/src/components/game-systems/legal/SystemLegalNotice.tsx` per contracts/manifest-legal-schema.md's UI contract: props `{ legal: SystemManifestLegal; variant: "selection" | "settings" }`, renders license name, attribution text, `requiredNotice` (visually emphasized when present), `disclaimer`, and `trademarkRestrictions` (collapsed/expandable list when non-empty) — both variants render full content, `variant` only affects surrounding chrome/framing (depends on T001)
- [X] T008 [US1] Create `apps/web/src/api/gameSystems.ts` (or extend an existing systems API file if `game_systems`/`gameSystems` GraphQL calls already exist elsewhere — check `apps/web/src/api/` before creating a new file) exposing a `getGameSystems()` call returning available system packs (id, title, and enough to fetch/display `legal`) for the new settings view's picker
- [X] T009 [US1] Add an `updateWorldGameSystem(worldId, gameSystemId)` mutation call (check whether a server-side mutation for changing `world.gameSystemId` already exists — search `mutations_*.rs` for `game_system_id` writes — before assuming one needs to be added; if the server has no such mutation yet, add a minimal one following the existing world-update mutation pattern) to `apps/web/src/api/world.ts` (depends on T008)
- [X] T010 [US1] Create `apps/web/src/pages/world/settings/WorldSystemSettingsPage.tsx`: shows the world's currently-assigned system (if any) with its `SystemLegalNotice` (`variant="settings"`, satisfying FR-005), and a picker (from T008's system list) to assign/change it — selecting a new system shows that system's `SystemLegalNotice` (`variant="selection"`) as part of confirming the change (satisfying FR-004, per this file's scope-correction note), calling T009's mutation on confirm (depends on T007, T008, T009)
- [X] T011 [US1] Add a route for `WorldSystemSettingsPage` (e.g. `/world/:id/settings/system`) to `apps/web/src/routes/AppRoutes.tsx` and a lazy-loader entry in `pageLoaders.ts`, plus a discoverable link to it from `WorldDashboardPage.tsx` (replacing or augmenting the current static `{world.gameSystemId ?? "Not yet assigned"}` display at `WorldDashboardPage.tsx:209` with a real link into the settings view) and/or the Session Setup toolbar row (`WorldStagingPage.tsx`) so it's reachable within two interactions per SC-002 (depends on T010)
- [X] T012 [US1] Confirm (manually, per quickstart.md) that switching an already-assigned world from one system to another re-shows the new system's legal notice at the point of switching, not just at first assignment (spec.md Edge Cases) — this should fall out of T010's design naturally; this task is the explicit verification, not new code

**Checkpoint**: User Stories 1 and 2 both work independently — a GM has one real place to assign a world's system and see its legal notice, both at assignment time and persistently afterward.

---

## Phase 5: User Story 3 - The governing packaging contract documents the legal-metadata requirement (Priority: P2)

**Goal**: ADR 027 documents the `legal` object's shape and compliance rationale, so a future contributor doesn't need to rediscover it from source code or this spec.

**Independent Test**: Read ADR 027 alone (no other context) and correctly describe what legal metadata a new system pack must supply and why (quickstart.md).

### Implementation for User Story 3

- [X] T013 [US3] Update `docs/adrs/20260504-027-game_system_packaging_and_manifest_contract.md`: add the `legal` object's required/optional fields (mirroring contracts/manifest-legal-schema.md's shape) to whatever section documents the manifest's other fields (`skills`, `abilities`, `data_types`, etc.), and add a rationale paragraph stating that visible attribution is a condition of the CC-BY / ORC / Cypher System Open License / Free League FTL licenses the shipped/planned packs are built from, per FR-002 (depends on T006 existing, so the ADR can point at a real populated example)

**Checkpoint**: All three user stories complete.

---

## Phase 6: Polish & Cross-Cutting Concerns

- [X] T014 [P] Run `cargo check` and `cargo test` in `src/server` (and `packs/systems/dnd5e/server`) to confirm the new validator and its test pass (constitution Principle V)
- [X] T015 [P] Run `cargo clippy --all-targets` on touched native crates and fix any new warnings, keeping the workspace at 0 (per the recent full clippy pass)
- [X] T016 [P] Run `pnpm --filter @thunderforge/web build` and a scoped `eslint` check on new/touched frontend files (this repo has a large pre-existing baseline of unrelated lint problems — confirm no NEW ones, don't chase the baseline)
- [X] T017 Execute every scenario in `specs/016-system-pack-legal-compliance/quickstart.md` against a running local dev stack — actually assign `dnd5e` to a test world via the new settings view and confirm the legal notice renders correctly, not just that it compiles
- [X] T018 [P] Confirm `./scripts/check-file-length.sh` shows no new failures introduced by this feature's files

---

## Dependencies & Execution Order

- **Foundational (Phase 2)**: BLOCKS both user stories — nothing can render or enforce a `legal` field that doesn't exist yet.
- **US2 (Phase 3)**: Fully satisfied by Phase 2 itself; no additional code.
- **US1 (Phase 4)**: Depends on Phase 2 (needs `legal` typed/populated to render). Independent of US3.
- **US3 (Phase 5)**: Documentation-only; depends on Phase 2's `legal` shape being real (T006) so the ADR describes something that actually exists, but has no code dependency on US1.
- **Polish (Phase 6)**: Depends on US1 and US3 both being complete.

### Parallel Opportunities

- T001, T002 (Phase 2) can run in parallel (different files/languages).
- T007 (Phase 4) can start as soon as T001 lands, in parallel with T008/T009's backend-facing work.
- T014-T016, T018 (Phase 6) can run in parallel.

---

## Implementation Strategy

### MVP First

1. Phase 2 (Foundational) — `legal` exists, is validated, `dnd5e` has one.
2. Phase 4 (US1) — a GM can actually see it, via the new settings surface.
3. **STOP and VALIDATE**: quickstart.md against a running dev stack.
4. Phase 5 (US3) — document it in ADR 027 so it doesn't bit-rot.
5. Polish.

### Suggested Task Ordering for a Single Implementer

Sequential by phase (T001→T018) is safe; [P]-marked tasks within Phase 2/4/6 may be reordered or batched freely.
