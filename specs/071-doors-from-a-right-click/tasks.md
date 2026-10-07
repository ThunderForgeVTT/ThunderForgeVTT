---
description: "Task list for Doors From a Right-Click"
---

# Tasks: Doors From a Right-Click

**Input**: [spec.md](./spec.md)

- [x] T001 Engine: `wall_under`, with its tests; `canvas_context_menu` carries `wallId` (FR-001)
- [x] T002 Web: `doorMenuActions`, `isGmLocked`, `wallName`, with their tests (FR-002, FR-003, FR-005)
- [x] T003 Web: `isDoubleRightClick` in the menu's hook, with its tests; a doubled request locks as a wall (FR-004)
- [x] T004 Web: `doorActions.ts` carries out what the menu offered, through spec 030's mutations (FR-006)
- [x] T005 `docs/guides/doors-and-walls.md`
- [x] T006 Proof from main: `interactive-door-menu.spec.ts`; the interactive and canvas slices (SC-001, SC-002). The spec passed on its first run; interactive (12) and canvas (34) pass

## User Story 3 — a door shows what it can do (FR-007..FR-010)

- [x] T007 Canvas core: `door_icon`, `IconPlace`, `icon_shown`, with their tests written first (FR-007, FR-008). The icon is a share of a grid square, never under 24 screen pixels: an unmapped scene keeps a 5-unit grid
- [x] T008 Engine: `DoorIconsPlugin` spawns and despawns icon entities only on change, claims a press on an icon before token drag sees it, and emits `door_icon_pressed`; `door_icons()` mirrors what is drawn for the probe
- [x] T009 Web: `doorIconAction` from the menu's own rule, with a test that it is always an item the menu offers; `useDoorIcons` carries a press out through `performDoorAction`. A press is heard after the engine's frame returns, since the action drives the engine
- [x] T010 Demo: `activateInteractive` refuses a player on a locked door (FR-010)
- [x] T011 Proof: `interactive-door-icons.spec.ts` (SC-003) and the demo's own e2e, a door's icon shuts it and a locked door stays shut for a player (SC-004)
