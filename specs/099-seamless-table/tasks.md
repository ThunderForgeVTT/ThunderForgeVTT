# Tasks: A Seamless Table

**Input**: [spec.md](./spec.md), [plan.md](./plan.md)

## Phase 1: Glide (US1)

- [x] T001 [US1] Core `glide.rs` and its tests (FR-002, FR-003).
- [x] T002 [US1] Engine `token_glide.rs`: track hops, draw glides, carried
  lights from the drawn token, `drawnX`/`drawnY` (SC-001, SC-002).
- [x] T003 [US1] Reduced motion reaches the glide.

## Phase 2: Clock (US2)

- [x] T004 [US2] Core `daylight.rs`: curve, OKLab tint, clock, and tests
  (SC-003).
- [x] T005 [US2] Engine: `TimeOfDay`, `set_time_of_day`, darkness from the
  clock, `time_of_day` events (SC-004).

## Phase 3: Night lights and map (US3)

- [x] T006 [US3] `byNight` lights.
- [x] T007 [US3] `nightImagePath` crossfade.

## Phase 4: Landing (US4)

- [x] T008 [US4] `hourAt` and the clock on the page. Remove the fade loop,
  torch steps and swaps.
- [x] T009 [US4] e2e: the walker glides (SC-005), and 098 SC-006 holds.

## Phase 5: The table's time (US5)

- [x] T010 [US5] Migration, GraphQL mutation and broadcast.
- [ ] T011 [US5] Web: apply the scene's time, plus the GM control.
- [ ] T012 [US5] e2e (SC-006).
