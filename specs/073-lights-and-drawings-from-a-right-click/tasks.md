---
description: "Task list for Lights and Drawings From a Right-Click"
---

# Tasks: Lights and Drawings From a Right-Click

**Input**: [spec.md](./spec.md)

- [x] T001 Engine: `light_under` and `shape_under`, with their tests; `canvas_context_menu` carries `lightId` and `shapeId` for a Game Master (FR-001)
- [x] T002 Web: `menuSubjectOf`, with its tests (FR-002)
- [x] T003 Web: `lightMenuActions`, `shapeMenuActions`, `shapeName`, with their tests (FR-003, FR-004, FR-005)
- [x] T004 Web: the menu carries out what it offered as world-store intents (FR-006)
- [x] T005 `docs/guides/lights-and-drawings.md`
- [x] T007 Web: the shape sync drops a drawing the server stops returning, with its test (FR-007)
- [x] T006 Proof from main, 2026-10-05: `canvas-light-and-drawing-menu.spec.ts` passes; lighting 8 of 8; interactive 12 of 12; canvas 34 of 35, the one failure being `canvas-authoring.spec.ts:638` (a wall toggled to a door and deleted), which uses no right-click, light or drawing and passed with the rest of its file, 15 of 15, when run alone. That failure is not root-caused (SC-001, SC-002)
