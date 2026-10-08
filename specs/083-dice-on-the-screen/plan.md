# Implementation Plan: Dice on the Screen

**Branch**: `083-dice-on-the-screen` | **Date**: 2026-10-07 | **Spec**: [spec.md](./spec.md)
**Input**: Feature specification from `specs/083-dice-on-the-screen/spec.md`

## Summary

A roll that reaches a board stops being a row of yellow squares at the world
origin. It becomes polyhedral dice, drawn by the engine, that tumble in from
the bottom of the screen, land on the faces the server rolled, and are then
explained by a readout such as `Ayla: Stealth   13 + 3 = 16`. The server
still decides every number. The engine only stages the throw.

- **Dice crate.** Each die records why each value after the first is in its
  chain (`steps`: reroll or explosion). A new pure `breakdown()` turns a
  resolved roll, its formula and its bindings into signed addends or success
  marks, or into nothing when the formula is not a plain sum.
- **Server.** `GraphQLDieOutcome.steps` and `WorldRoll.bindings`.
  `MaskedRoll` does not change.
- **Canvas core.** The throw's logic lives here, because the engine crate
  cannot run host tests. That covers the die shapes and face tables, the
  landing orientation, the seeded tumble, the queue and the readout text.
- **Engine.** `plugins/dice_roll.rs` becomes `plugins/dice/`. It holds:
  - a queue resource;
  - one `Mesh2d` per drawn die, mutated in place each frame and projected
    on the CPU;
  - `Text2d` numerals and readout, anchored to the camera;
  - the `dice_landed()` and `dice_timings()` wasm getters;
  - the `set_reduced_motion` command.
- **Web.** The roll sync passes the whole `WorldRoll` to the board.
  `triggerDiceRollAnimation` builds the payload from it, `__engineProbe`
  gains `diceLanded()`, the viewer's `prefers-reduced-motion` reaches the
  engine, and the dice roller panel reads its delay from the engine.
- **Demo.** Its dice handler stores bindings and maps step kinds, so its
  boards play the same throw.

### What is deliberately not built

- How the dice crate totals an exploding die (research R4). The throw draws
  every value in the chain, and the readout reports the crate's total.
- Dice skins, per-player colours, sound, a critical-hit effect, or a
  per-user switch to turn dice off (spec Assumptions).
- A server-built readout string. The readout is derived data (AGENTS.md §5),
  computed by the engine from the breakdown.
- Physics. The landing is scripted to the result.

## Technical Context

**Language/Version**: Rust 2024 (dice crate, canvas core, server, engine on
Bevy 0.19.1 for wasm32); TypeScript 5 / React 19 (web, demo)
**Primary Dependencies**: Bevy with its current features
(`bevy_sprite_render` for `Mesh2d` and `ColorMaterial`, `bevy_text` with
`default_font`). `glam` is already used by canvas core. async-graphql and
Diesel stay on the server. No new crates, and no new Bevy features.
**Storage**: none new. `world_roll_records.detail` gains `steps` inside its
JSON, defaulted when absent. `world_roll_records.bindings` already exists.
**Testing**:

- `cargo test -p thunderforge-dice`;
- `cargo test -p thunderforge-canvas-core dice_throw`;
- `make test-rust ARGS="-p thunderforge-server --lib roll"`;
- `make lint` (host, plus wasm32 for the engine);
- web and demo vitest;
- `pnpm e2e:rolls`, then every slice `pnpm e2e:which --diff` names. Slices
  are the gate (owner decision 2026-10-07); the full suite is not run.

**Target Platform**: browsers running the wasm engine, and the self-hosted
server.
**Project Type**: web (Rust server, wasm engine, React app, in-browser demo)
**Performance Goals**:

- a plain roll lands within 1.2 s, and within 150 ms under reduced motion;
- a 20-die throw keeps 55 fps;
- the engine grows by less than 150 KB brotli.

**Constraints**:

- every board plays the same throw;
- masked rolls never reach a board;
- the default font is ASCII only (research R6);
- `.rs` files must stay under 1000 lines (`eval.rs` is 944, `app.rs` 885).

**Scale/Scope**: at most 20 drawn dice per throw, and at most 4 throws
waiting.

## Constitution Check

| Principle                      | How this plan holds it                                                                                                                                                                                                                              |
| ------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| I. The server is the authority | Every value drawn is the server's: sides, chain, step kinds, kept and final value. The engine picks only orientation and path.                                                                                                                      |
| II. Circular flow              | A board plays a roll only from its `ROLL_MADE` or `ROLL_REVEALED` event, through `startRollSync` (spec 081). The panel that asked never animates.                                                                                                   |
| III. Agnostic core             | The shapes, orientation, tumble, queue and readout are pure, in `thunderforge-canvas-core`. The breakdown is pure, in `thunderforge-dice`. The engine plugin only maps them onto entities.                                                          |
| IV. Tests first                | Every phase opens with its failing tests: crate, canvas core, server, web and demo vitest, then e2e.                                                                                                                                                |
| V. Docs by audience            | `docs/guides/rolls.md` says what players see. CONTRIBUTING's Rolls section says where the throw is built and how to tune it in the sandbox.                                                                                                         |
| VI. Proven by its own slice    | `apps/web/e2e/rolls-dice-on-screen.spec.ts` sits in the `rolls` slice. The demo's `rolls-across-tabs.spec.ts` asserts `diceLanded()`. `pnpm e2e:rolls` is green, then every slice `pnpm e2e:which --diff` names; slices are the gate (owner decision 2026-10-07). |

No violations.

## Project Structure

### Documentation (this feature)

```text
specs/083-dice-on-the-screen/
├── spec.md
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
├── contracts/
│   ├── graphql-rolls.md
│   └── engine-dice.md
└── tasks.md
```

### Source Code (touched)

```text
crates/thunderforge-dice/src/
├── lib.rs                  # ChainStep, DieOutcome.steps, pub mod breakdown
├── eval.rs                 # push a step beside every reroll/explosion
├── eval_steps_tests.rs     # new: step kinds, clamps, old JSON
├── breakdown.rs            # new: Breakdown, Addend, breakdown()
└── breakdown_tests.rs      # new

crates/thunderforge-canvas-core/
├── Cargo.toml              # + thunderforge-dice (pure, wasm-compatible)
└── src/dice_throw/         # new
    ├── mod.rs              # ThrowDie, ThrowSpec, timings
    ├── shapes.rs           # vertex/face tables, face labels
    ├── landing.rs          # orientation that shows a face
    ├── tumble.rs           # seed, path, spin, resting place
    ├── queue.rs            # play one, wait four, skip oldest
    ├── readout.rs          # ASCII readout and chip text
    └── *_tests.rs

crates/thunderforge-server/src/graphql/
├── types_dice.rs           # DieStep enum, GraphQLDieOutcome.steps
├── types_rolls.rs          # RollBinding, WorldRoll.bindings
└── roll_bindings_tests.rs  # new

crates/thunderforge-engine/src/
├── payloads.rs             # TriggerDiceRoll { roll }, SetReducedMotion
├── sdk.rs                  # parse both, dice_landed(), dice_timings()
├── app.rs                  # route both to the dice resources
└── plugins/dice/           # replaces plugins/dice_roll.rs
    ├── mod.rs              # DicePlugin, resources, system order
    ├── mesh.rs             # per-die Mesh2d, CPU projection, shading
    ├── throw.rs            # spawn, animate, land, fade, despawn
    ├── readout.rs          # Text2d readout, chip, struck values
    └── probe.rs            # landed-log mirror for dice_landed()

apps/thunderforge/schema.graphql
apps/web/src/
├── types/roll.ts, api/roll.ts           # steps, bindings
├── engine/bevy/diceThrow.ts             # new: payload, reduced motion, timings
├── engine/bevy/index.ts                 # triggerDiceRollAnimation(roll), diceLanded
├── engine/world/sync/rolls.ts           # animate(roll)
├── pages/world/WorldPage.tsx            # wires both
└── components/world/DiceRollerPanel/DiceRollerPanel.tsx
apps/web/e2e/rolls-dice-on-screen.spec.ts      # new
apps/demo/src/backend/handlers/dice.ts         # bindings, steps
apps/demo/e2e/rolls-across-tabs.spec.ts        # + diceLanded()
apps/engine-sandbox/src/main.ts                # dice button
docs/guides/rolls.md, docs/CONTRIBUTING.md
```

**Structure Decision**: the throw's logic goes in canvas core, beside
`door_icon.rs` and `shape.rs`, because `frame_trace.rs` records that the
engine crate cannot compile for the host. A `#[test]` there never runs. The
plugin becomes a directory, because a single file would pass 1000 lines.

## Complexity Tracking

| Addition                                     | Why it is needed                                                             | Simpler alternative rejected                                                                                                       |
| -------------------------------------------- | ---------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------- |
| CPU projection into `Mesh2d`                 | FR-006 asks for 3D dice without `bevy_pbr` or a 3D camera.                   | A second `Camera3d` with PBR adds features and bundle size, and breaks the systems that query the single `Camera2d` (research R1). |
| `canvas-core` depends on `thunderforge-dice` | The readout needs the breakdown, and canvas core is where readout tests run. | Formatting in the engine cannot be tested on the host.                                                                             |
| `steps` in the stored JSON                   | The chain cannot tell a reroll from an explosion (spec FR-002).              | Re-deriving the kinds from the formula's modifiers fails when one die both rerolls and explodes.                                   |
