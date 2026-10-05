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
