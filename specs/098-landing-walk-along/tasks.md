# Tasks: The Hero Walks the Page With You

**Input**: [spec.md](./spec.md), [plan.md](./plan.md)
**Prerequisites**: 096 (down), 097 (`plan_path`) in the wasm the landing loads.

**Tests**: landing e2e (`apps/landing/e2e`), desktop project for each
phase. Phone and tablet run at the end of each phase to prove FR-001.

## Phase 1: Route (US1, US3)

- [x] T001 [US1] Send the tavern's walls and south door from
  `tavern-sample.json`, scaled to the page cell.
- [x] T002 [US3] `route.ts`:
  - stations;
  - legs (plan_path or gutter);
  - `sAt(scroll)`;
  - `pointAt(s)`.
  Unit-test it in node.
- [x] T003 [US1] PageTable eases `s` and draws the walker at `pointAt(s)`.
- [x] T004 [US1] [US3] e2e `walk-along.spec.ts`: the door exit (SC-001)
  and the centre band (SC-003).

## Phase 2: Goblins (US2)

- [x] T005 [US2] Add `goblins.ts`: three tokens below the tavern, and
  `defeated` from `s`.
- [x] T006 [US2] e2e (SC-002).

## Phase 3: Map selection (US4)

- [x] T007 [US4] `MapLegend` publishes the pick, and the station follows
  it.
- [x] T008 [US4] e2e (SC-004).

## Phase 4: Dungeon (US5)

- [ ] T009 [US5] `pageRoom.ts` adapter, with node tests.
- [ ] T010 [US5] Wire the wide Room to the page table: play gives a top
  entry; not played walks round.
- [ ] T011 [US5] e2e (SC-005): the existing room specs pass on desktop.

## Phase 5: Campfire (US6)

- [ ] T012 [US6] `camp-sample` is in `examples/maps` and
  `prepare-static.mjs`'s ROOMS; add `Campfire.tsx` beside "Pull up a chair".
- [ ] T013 [US6] e2e: the unused heroes are on the logs, and the walker parks on (4,1).

## Phase 6: Dusk (US7)

- [ ] T014 [US7] Rework the page-distance ramp with a slew cap.
- [ ] T015 [US7] e2e at 1920 and 2560 (SC-006).

## Phase 7: Greenlight

- [ ] T016 Owner tests on a phone and a desktop (SC-008). Only then push.
