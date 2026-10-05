---
description: "Task list for A Board You Can Touch"
---

# Tasks: A Board You Can Touch

**Input**: [spec.md](./spec.md)

- [X] T001 `plugins/touch.rs`: the gesture as a pure step function over the frame's fingers, with its tests (FR-001 – FR-004)
- [X] T002 `TouchInputPlugin`: the step applied to the mouse buttons, the pointer and the camera, in `PreUpdate` after input is read; app-level tests sending `TouchInput` (FR-001, FR-003, FR-005)
- [X] T003 `Pointer`: the eight systems that read the window's cursor ask it instead (tokens, walls, shapes, lights, placement, context menu, camera)
- [X] T004 `canvas-touch.spec.ts`: real touches through the browser (SC-001). First run: drag, pan and pinch passed; the hold failed because the spec had panned and zoomed the token off the page before holding on it. Reordered
- [X] T005 Proof from main: `canvas-touch` passes; the canvas (34), interactive (11) and lighting (8) slices pass (SC-001, SC-002)
