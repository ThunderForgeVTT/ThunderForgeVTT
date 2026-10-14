# Tasks: Clocks

**Input**: [spec.md](./spec.md), [plan.md](./plan.md)
**Prerequisites**: none; the event flow, the dock and the manifest validator
exist as 029, 051 and 067 left them.

**Tests**: TDD. Each phase opens with its failing tests and closes with
its proof. Rust files stay under 1000 lines (`scripts/check-file-length.sh`
fails the build otherwise); split `clocks.rs` before it gets there.

**Organization**: phased server → store/sync → dock → proof, as the brief
asks. Story labels map to spec.md: US1 manifest, US2 create and tick, US3
everyone sees it, US4 hidden, US5 two systems.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependency on an
  unfinished task)
- **[Story]**: the user story this task serves

---

## Phase 1: Setup

- [ ] T001 Read `specs/029-in-engine-status-displays/spec.md` "Follow-up: clocks", `crates/thunderforge-pack-system-spec/src/conditions.rs` (the block to copy the shape of), `crates/thunderforge-server/src/world_events.rs` (`roll_event_reaches`), and `apps/web/src/engine/world/sync/shapes.ts` (the sync module to copy the shape of). No code
- [ ] T002 Add the `clocks` slice to `scripts/e2e/slices.json` (own `clocks-`; neighbours `status-systems.spec.ts`, `genie-puzzle-clock-rewards.spec.ts`; paths per plan.md) and `e2e:clocks` / `e2e:clocks:integration` to root `package.json`; run `pnpm verify`'s slice check and record it here

---

## Phase 2: Server — manifest block (US1, FR-001..FR-005)

**Proof**: `cargo test -p thunderforge-pack-system-spec`, `make check-packs`.

- [ ] T003 [US1] Failing tests in `crates/thunderforge-pack-system-spec/src/clocks.rs`: accepts `{id, label, segments: [4,6,8], color: "danger"}`; refuses duplicate ids, blank label, empty `segments`, a size of 1 or 13, duplicate sizes, a colour off the palette, an unknown field; each refusal names the kind
- [ ] T004 [US1] `SystemClockKind` (`deny_unknown_fields`), `clocks_from_manifest(&Value)`, `validate_clocks_content(&Value)` in `clocks.rs`; `clocks: Option<Vec<SystemClockKind>>` on `SystemManifest` and the validator call in `validate_system_manifest` (`lib.rs`)
- [ ] T005 [P] [US1] `packs/systems/blades_in_the_dark/system.json`: `clocks: [{ id: "progress", label: "Progress clock", segments: [4, 6, 8], color: "warning" }]`
- [ ] T006 [P] [US1] `packs/systems/genie/system.json`: `doom` (`[6, 8, 10, 12]`, `danger`) and `puzzle` (`[4, 6, 8]`, `arcane`), labels from Genie's own text
- [ ] T007 [P] [US1] `packs/systems/README.md`: a `### clocks` section after `conditions`, in that section's voice: what a kind is, the sizes rule, the colour list, "absent means no clocks tab", and that the host does the rest
- [ ] T008 [US1] Phase proof: `cargo test -p thunderforge-pack-system-spec` and `make check-packs`; record counts here

---

## Phase 3: Server — table, rule, events (US2, US4, FR-006..FR-009)

**Proof**: `make test-rust ARGS="-p thunderforge-server"`.

- [ ] T009 [US2] Migration `crates/thunderforge-server/migrations/2026-10-13-120000-0000_world_clocks/{up,down}.sql` per FR-006 (checks on `segments` and `filled`, cascades, two indexes); regenerate `src/schema.rs`; `WorldClock` / `NewWorldClock` in `src/models.rs`
- [ ] T010 [US2] Failing tests in `crates/thunderforge-server/src/clocks_tests.rs`: create at 0; create refused for an undeclared kind, a disallowed size, a scene of another world, a non-GM; tick clamps in SQL (+10 on a 6 lands at 6); tick refused at full and at empty with the right message; two concurrent +1 ticks make +2; tick refused while paused; show/hide/rename/delete gated; delete of a scene cascades its clocks and leaves world clocks
- [ ] T011 [US2] `crates/thunderforge-server/src/clocks.rs`: the rule, `create_clock`, `tick_clock` (one `UPDATE … RETURNING`, plan.md "Clamp in SQL"), `set_clock_shown`, `update_clock_label`, `delete_clock`, `clocks_visible_to`; gates through `thunderforge_authz::runs_the_world()` and the pause check 067 uses; `clock_kinds_for_system` in `systems.rs`
- [ ] T012 [US4] `EVENT_CODE_CLOCK_CHANGED = 43` and `clock_event_payload(clock_id, action, shown)` in `world_events.rs`; each write in `clocks.rs` records one event after commit (FR-009)
- [ ] T013 [US4] Failing tests then the change: `roll_event_reaches` becomes `event_reaches` dispatching by code (plan.md "Reach for code 43"); 43 with `shown: false` reaches only `is_gm_or_admin`; rolls unchanged; every other code reaches everyone. Update the subscription call site
- [ ] T014 [US2] Phase proof: `make test-rust ARGS="-p thunderforge-server"`; `scripts/check-file-length.sh`; record counts here

---

## Phase 4: Server — GraphQL (US2, US3, US4, FR-010..FR-014)

**Proof**: `make test-rust ARGS="-p thunderforge-server"` (the `_tests.rs` beside the module).

- [ ] T015 [US2] Failing tests in `crates/thunderforge-server/src/graphql/mutations_clocks_tests.rs`: each mutation's happy path, each refusal's message text (FR-014), a player's call refused
- [ ] T016 [US2] `crates/thunderforge-server/src/graphql/mutations_clocks.rs`: `createClock`, `tickClock`, `setClockShown`, `updateClockLabel`, `deleteClock`; `WorldClock` GraphQL object; register in the mutation root
- [ ] T017 [US3] [US4] `crates/thunderforge-server/src/graphql/queries/clocks.rs`: `worldClocks(worldId)` filtered by `shown` for non-GMs, member-gated; `worldSystemClockKinds(worldId)` from the registry; tests beside it
- [ ] T018 [US2] Regenerate the GraphQL schema artefact the web build reads (whatever `make` target does that today); phase proof: `make test-rust ARGS="-p thunderforge-server"`, `make lint`; record here

---

## Phase 5: Store and sync (US2, US3, FR-015..FR-019)

**Proof**: `pnpm -F @thunderforge/web test`.

- [ ] T019 [US2] `apps/web/src/engine/world/types.ts`: `WorldClock`, `clocks` on `WorldState`, commands `upsert_clock` / `remove_clock`, intents `create_clock` / `tick_clock` / `set_clock_shown` / `update_clock_label` / `delete_clock`; reducer cases in `store.ts` with tests (tick clamps locally; an unknown clock's remove is a no-op)
- [ ] T020 [P] [US3] `apps/web/src/api/clocks.ts`: `fetchWorldClocks(worldId)`, `fetchClock(clockId)`, `fetchClockKinds(worldId)` and the five mutations over `postGraphQL`
- [ ] T021 [US3] Failing tests in `apps/web/src/engine/world/sync/__tests__/clocks.test.ts`: event 43 `created`/`updated` fetches and upserts; `deleted` removes; `shown: false` received by a non-GM removes; `tick_clock` is optimistic and rolls back to the cached value on refusal, surfacing the message; create waits for the server
- [ ] T022 [US3] `apps/web/src/engine/world/sync/clocks.ts`: `startClockMutationBridge`, `applyClockWorldEvent`; register in `sync/index.ts` and wherever the other kinds' bridges and event syncs are started for a world; initial `fetchWorldClocks` on world load beside the other kinds' loads
- [ ] T023 [US3] Phase proof: `pnpm -F @thunderforge/web test`; `pnpm -F @thunderforge/web lint`; record counts here

---

## Phase 6: Dock (US2, US3, US4, FR-020..FR-024)

**Proof**: `pnpm -F @thunderforge/web test`, then the slice in Phase 7.

- [ ] T024 [US2] `apps/web/src/components/world/PlayDock/ClockRing.tsx`: `segments` wedges, `filled` in the kind's colour token, `aria-hidden`; a story-free unit test that 4 of 6 renders 6 wedges with 4 filled and no percentage anywhere in the DOM
- [ ] T025 [US2] [US4] `apps/web/src/components/world/PlayDock/ClocksPanel.tsx`: "This world" and current-scene groups (FR-021); GM controls and create form (FR-022); no controls for players; `aria-label` per FR-023; undeclared kind drawn neutral (FR-024). Reads and dispatches through the store only (FR-018)
- [ ] T026 [US1] `DockSectionId "clocks"` in `WorldDock.tsx`; `useDockSections.tsx` fetches `worldSystemClockKinds` once per world and mounts the section only when non-empty; test that a kind-less system mounts no tab
- [ ] T027 [US2] Phase proof: `pnpm -F @thunderforge/web test`; `pnpm -F @thunderforge/web lint`; record here

---

## Phase 7: End-to-end (US2..US5, SC-001..SC-005)

**Proof**: `pnpm e2e:clocks`.

- [ ] T028 [US5] `apps/web/e2e/clocks-systems.spec.ts`: loop `blades_in_the_dark` and `genie` the way `status-systems.spec.ts` loops four; per system: `worldSystemClockKinds` returns the manifest's kinds; create a clock of the first kind at its default size through the dock; tick; the dock reads the system's kind label and "1 of N". Plus `dnd5e`: no tab, `[]` (SC-001, SC-005)
- [ ] T029 [US2] [US3] `apps/web/e2e/clocks-tick.spec.ts`: GM and player sessions; create a shown scene clock; tick; player's dock updates within 2 s with no reload (SC-002); tick to full then past: refused, GM's dock unchanged, message shown (SC-004); scene switch hides the scene clock and keeps the world clock; delete leaves both docks
- [ ] T030 [US4] `apps/web/e2e/clocks-hidden.spec.ts`: GM creates a hidden clock and ticks twice; the player's `worldClocks` response has no such id and the player's subscription stream (captured as `rolls-*.spec.ts` captures it) carries no 43 for it; GM shows it; player receives one 43, fetches, dock reads 2 of N; GM hides; player's dock drops it (SC-003)
- [ ] T031 [US5] FR-005 grep step in `clocks-systems.spec.ts` or a Vitest beside it: the feature's host files contain neither `blades_in_the_dark` nor `genie`; record the list of files it scans
- [ ] T032 Phase proof: `pnpm e2e:clocks` alone with `--record-durations`; grep the log for ✘; record result and wall time here

---

## Phase 8: Polish and proof

- [ ] T033 [P] `docs/guides/clocks.md`: for Game Masters (create, tick, show, clear; world versus scene) and players (what they see); link from the guides index; CONTRIBUTING: the one rule for clock writes lives in `clocks.rs`, and the reach filter is the only place `shown` is read for delivery
- [ ] T034 [P] Tick F001 in `specs/029-in-engine-status-displays/tasks.md` — done at spec time, 2026-10-13, pointing here
- [ ] T035 `pnpm verify`, limited to what this feature added; `make lint`; record here
- [ ] T036 `pnpm e2e:which --diff` and run each slice it names; record each result here
- [ ] T037 Spec status to Complete with an Outcome paragraph, in 029's style

---

## Follow-up (not this feature)

- [ ] F001 Re-base Genie's session clocks (018 US7, 020 rewards) onto
      `world_clocks`, so `SessionClocks.tsx` draws through `ClockRing` and
      the Doom Clock's loss rule and the Puzzle Clock's rewards hang off a
      generic clock's `filled` by Genie's own server code. Its own spec,
      because it touches a full-time system's rules path.
- [ ] F002 Clocks a player may tick (their own long-term project) once 093
      has a capability finer than `runs_the_world()`.

---

## Dependencies

- Phase 1 → 2 → 3 → 4 → 5 → 6 → 7 → 8. T002 may land any time before
  T032.
- T005–T007 need T004. T012 and T013 need T011. T017 needs T011 and T012.
  T019–T022 need T018's schema. T025 needs T019 and T022. T028–T030 need
  T026.
- US1 (manifest) is deliverable alone after Phase 2; US2 after Phase 6;
  US3 and US4 are proven only by Phase 7; US5 is Phase 7's loop.

## Parallel Example

- T005, T006, T007 together once T004 is in.
- T020 beside T019.
- T024 beside T019–T022 (it takes props, not the store).
- T028, T029, T030 are separate files and separate stacks.

## Implementation Strategy

- MVP is Phases 2–6 with T028's Blades half: a GM creates and ticks a
  clock and sees it. Ship nothing to main before Phase 7, though: US3 and
  US4 are the feature's reason to exist and are only proven end-to-end.
- Write the hidden-clock e2e (T030) before the dock is polished; it is the
  assertion most likely to find a server mistake.

## Notes

- Never carry `filled` in the event payload (FR-007). A player who is shown
  a clock later must not be able to read its history from the stream.
- "4 of 6", never "67%". A percentage anywhere in the DOM is a test failure
  (T024).
- A system with no kinds gets no tab (FR-020). Do not add an empty state.
