---
description: "Task list for A Board That Says It Stopped"
---

# Tasks: A Board That Says It Stopped

**Input**: [spec.md](./spec.md)

- [X] T001 Engine: a panic hook that keeps the console message and tells the window (FR-001)
- [X] T002 Web: `engineStopped.ts`, with its tests; `useCanvasEngine` returns `stopped` (FR-002)
- [X] T003 Web: `EngineStopped` over the board on the world page (FR-003)
- [X] T004 `debug_panic` in debug engine builds, `crashEngine` beside the other fault probe
- [X] T005 Proof from main: `canvas-engine-stopped.spec.ts` (SC-001, SC-002). The context-loss test passed first time; the panic test failed, and was right to — Bevy's `PanicHandlerPlugin` set its own hook over the engine's while the app was built, so the page was never told. Disabled in `e79c61f5`; both pass
