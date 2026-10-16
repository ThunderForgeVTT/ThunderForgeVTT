# Tasks: The World's Front Door

**Input**: [spec.md](./spec.md), [plan.md](./plan.md)

## Phase 1: The landing opens the table (US1)

- [x] T001 [US1] `DEMO` points at the demo world's `/play`.
- [x] T002 [US1] Landing e2e: demo links' `href` and the board behind
  them (SC-001).

## Phase 2: One front door (US2)

- [x] T003 [US2] `WorldOverviewPage` at `/world/:id` from the two pages'
  parts; remove "Quick actions", the ids and "Enter world".
- [x] T004 [US2] `/staging` redirects; move every staging link; delete
  `WorldDashboardPage` and `WorldStagingPage`.
- [x] T005 [US2] e2e: two clicks to the table, the redirect, and the
  actor-claim gate (SC-002, SC-003). Update the demo's route headings.

## Phase 3: The overview gets the session ready (US3)

- [x] T006 [US3] `SceneStrip` with "Play here" and map credits.
- [x] T007 [US3] `PartyStrip`, the notes beside it, "Resume in …".
- [x] T008 [US3] Empty states, and e2e for "Play here" (SC-004).

## Phase 4: Width (US4)

- [x] T009 [US4] Breakpoints, `--page-max`, and the shared width on the
  header, footer, `Container` and the shells.
- [x] T010 [US4] `auto-grid` on players, scenes, compendium and overview;
  prose capped at 75ch.
- [x] T011 [US4] e2e at three widths (SC-005).

## Phase 5: The character page (US5)

- [x] T012 [US5] Rail and sheet layout on `ActorDetailPage`; imagery in
  the rail.
- [x] T013 [US5] Region `@container`s and the 4-column sheet in the 5e
  pack; the same wrapper for the other system sheets.
- [x] T014 [US5] e2e: ability inputs inside their cells, sheet columns
  (SC-006); the play dock's sheet still one column.

## Phase 6: Demo notice (US6)

- [ ] T015 [US6] Collapsible pill below `sm`, and its e2e (SC-007).

## Phase 7: Review

- [ ] T016 Screenshots before and after at 390, 1440 and 3440 for the
  owner, phone included (SC-008). Ships only on the owner's greenlight.
