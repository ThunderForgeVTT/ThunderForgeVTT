# Data Model: Bevy 0.20

No persisted data changes. There is no migration, no GraphQL type, no
world event and no store shape involved. This file records the two
engine-side shapes the upgrade must keep.

## EngineStats (unchanged)

These are the engine's frame counters, mirrored to `engine_stats_slot()`
and read by `apps/web/src/engine/bevy/stats.ts`.

| Field           | Meaning                                         | Source on 0.20                           |
| --------------- | ----------------------------------------------- | ---------------------------------------- |
| `frame_time_ms` | Smoothed frame time                             | Unchanged (diagnostics)                  |
| `fps`           | Smoothed frames per second                      | Unchanged                                |
| `sprites`       | Entities with `Sprite` in the main world        | Unchanged (`Query<(), With<Sprite>>`)    |
| `lights`        | Lights on the board                             | Unchanged                                |
| `walls`         | Walls on the board                              | Unchanged                                |
| `tokens`        | Tokens on the board                             | Unchanged                                |
| `tokens_culled` | Tokens outside the view                         | Unchanged                                |
| `shadow_quads`  | Shadow quads drawn                              | Unchanged                                |
| `frames`        | Frames since start                              | Unchanged                                |

**Rule**: no field is added, removed or renamed. These read them:

- `engine-limits.spec.ts`, `engine-status-limits`, `engine-token-culling`,
  `engine-interaction-limits` and `engine-lighting-limits`;
- `rolls-dice-on-screen.spec.ts` and `fixtures/lightingProbe.ts`;
- `stats.ts`, `world/probe.ts` and `EngineMonitor.tsx`;
- `apps/engine-sandbox`;
- spec 086 T075 (`framesSummary.ts`).

## Render probe line (meaning unchanged, sources change)

contracts/render-probe.md has the detail. The counts keep their names and
meanings, and they are read from the 0.20 sprite pipeline instead of the
0.19 one.
