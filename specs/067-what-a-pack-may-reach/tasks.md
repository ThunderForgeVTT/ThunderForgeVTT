---
description: "Task list for What a Pack May Reach"
---

# Tasks: What a Pack May Reach

**Input**: [spec.md](./spec.md), [the pack contract](../../packs/systems/README.md), [ADR-063](../../docs/adrs/20260903-063-a_pack_owns_the_tables_it_writes.md), [ADR-091](../../docs/adrs/20260907-091-instance_configuration_is_rows.md), [ADR-108](../../docs/adrs/20260923-108-a_generic_world_settings_surface_is_deferred_not_rejected.md)

**Tests**: written with the code they hold. Each story ends on an e2e slice run from the main checkout.

**Baseline** (commit `5573f270`): `make test-rust` 1,867 passed; web unit 727; scripts 129.

## Format: `[ID] [P?] [Story] Description`

---

## Phase 1: Settings (Story 1)

- [X] T001 [US1] `crates/pack_system_spec/src/settings.rs`: the `settings` block — declaration, per-type value check, lenient reader, strict validation wired into `validate_system_manifest` (FR-001, FR-002)
- [X] T002 [US1] Migration `world_system_settings` and `world_system_setting_changes`; `schema.rs` (FR-003, FR-004)
- [X] T003 [US1] `SystemContribution::world_setting`, the pack's own validator (FR-007)
- [X] T004 [US1] `crates/thunderforge-server/src/world_system_settings.rs`: effective read, inert stale rows, transactional write with its change row, `effective_value` for server code (FR-003, FR-008, FR-011)
- [X] T005 [US1] `worldSystemSettings` / `setWorldSystemSetting`, event code 35, the pause-surface entry (FR-005, FR-006, FR-009)
- [X] T006 [US1] Web: `api/worldSystemSettings.ts`, `useWorldSystemSettings` (reads, writes, re-reads on event 35), exported from `@thunderforge/host` (FR-009, FR-011)
- [X] T007 [US1] Web: the generic form on the world's System settings page, above the pack's `world-settings` panel (FR-010)
- [X] T008 [US1] The first setting: 5e declares `inspiration` (on by default; a table that does not award it turns it off), and its sheet shows the Inspiration control only when the world plays with it (FR-013)
- [X] T009 [US1] e2e in the `game-systems` slice: a Game Master turns Inspiration off, a player's open sheet loses the control without a reload, a player cannot change it
- [X] T010 [US1] The pack contract documents `settings`; ADR-112 records the shared table and marks ADR-108's deferral closed
- [X] T011 [US1] Proof: `make test-rust`, web unit, `pnpm test:scripts`, `make lint`, the pre-commit checks; the slice from main

## Phase 2: Panel slots (Story 2)

- [X] T020 [US2] `scripts/check-packs.mjs`: refuse a `panels/` file whose name is not a slot, reading the slot list from `apps/web/src/host/index.ts`; test first (FR-022)
- [X] T021 [US2] Rename the slot `clocks` to `dock`; a panel module's optional `title` export reaches the dock tab; no tab when unfilled (FR-020, FR-021)
- [X] T022 [US2] Roll for Shoes' dock panel is titled "Table"; Genie's "Clocks"
- [X] T023 [US2] The pack contract's slot table (FR-023)
- [X] T024 [US2] Proof, and the `game-systems` slice from main

## Phase 3: Rules (Story 3)

- [X] T030 [US3] Confirm with the owner: the adjudicator's signature and where an outcome is stored — verdict plus label, stored with the roll
- [X] T031 [US3] `check-packs`: refuse a top-level manifest key outside the contract's list (FR-030)
- [X] T032 [US3] Restate or remove the six single-pack roll keys, and the four other keys nothing read (FR-031)
- [ ] T033 [US3] The roll adjudicator on `SystemContribution`; Roll for Shoes' outcome decided on the server (FR-032, FR-033)
- [ ] T034 [US3] Proof

## Phase 4: Conditions (Story 4)

- [X] T040 [US4] Confirm with the owner: a condition lives on the token or on the actor behind it — on the actor
- [ ] T041 [US4] `conditions` in the manifest; Genie's restated, 5e's declared (FR-040, FR-043)
- [ ] T042 [US4] Apply and clear, per-viewer delivery through the world store (FR-041, FR-042)
- [ ] T043 [US4] The engine draws a marker from identifier and marker alone (FR-042)
- [ ] T044 [US4] Proof
