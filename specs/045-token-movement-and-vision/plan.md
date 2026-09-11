# Implementation Plan: Token Movement and Vision

**Branch**: `045-token-movement-and-vision` | **Date**: 2026-09-11 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/045-token-movement-and-vision/spec.md`

## Summary

Three defects and three capabilities, in the order they can be proved.

The defects were found by playing: a player's keyboard moves nothing, a wall
that blocks movement blocks nothing, and a door opened by one person stays shut
on every other board. The capabilities the spec adds are the rules those
defects hide — what a player sees, what a game system says about seeing in the
dark, and a map that remembers where a hero has been.

The work is sequenced so each phase is provable on its own by
`pnpm playtest`, whose findings are the acceptance criteria: each phase turns
one or more FINDINGs into passes, and no phase needs the next one to be worth
shipping.

| Phase | What lands | The finding it clears |
|---|---|---|
| 1 | A door change reaches every board | "the door Aria opened should be open on her own board" ×3 |
| 2 | A player's keyboard moves their own token | "D (east) should walk Aria's own token one cell" |
| 3 | The server refuses a move through a wall; the engine stops it first | "a wall that blocks movement should stop Aria at it" ×2 |
| 4 | The vision and light rules written down as behaviour | (protects what already works) |
| 5 | A game system declares sight in darkness and carried-light reach | (new capability, decision 2) |
| 6 | A player's map remembers where they have been | (new capability, decision 3) |

## Technical Context

**Language/Version**: Rust (server, native; engine, `wasm32-unknown-unknown`),
TypeScript 5 with React 19 (web), all as the repository already builds them.

**Primary Dependencies**: Bevy 0.19 (engine), `thunderforge-canvas-core` (the
shared geometry and vision crate), async-graphql + Diesel + Postgres (server),
Playwright (e2e and playtest).

**Storage**: Postgres for scenes, walls and tokens. **Explored areas are not
stored on the server** — they live in the player's own browser, in the
IndexedDB the world cache already uses (`src/services/worldCacheStorage.ts`),
per the owner's decision 3.

**Testing**: `cargo test` for the server and `thunderforge-canvas-core`;
engine tests under `wasm32`; `vitest` for web units; Playwright for e2e; and
`pnpm playtest` as the acceptance proof for every phase.

**Target Platform**: A browser against a ThunderForge instance; the engine is
WASM in that browser.

**Project Type**: Existing multi-part repository — engine, server, web app and
system packs — not a new project.

**Performance Goals**: Judging a move adds no delay a player notices for
scenes up to 2,000 wall segments. A door change, a move and a vision change
reach every client within one second (spec SC-004, FR-034).

**Constraints**: The engine is the only thing that draws or spatially reasons
(Principle I). The server is authoritative for anything persisted
(Principle III). Explored areas must survive a reload in the same browser and
must be droppable on a Game Master's reset (FR-073, FR-078).

**Scale/Scope**: Six phases; touches the engine (movement, vision, a fog
layer), the server (move adjudication, door events, a scene setting), the web
(sync, the play dock, browser storage), and the system packs (a vision
declaration). No new service, no new dependency.

## Constitution Check

| Principle | How this plan satisfies it |
|---|---|
| **I. ECS owns simulation** | The stop at a wall is shown by the engine, not by React; explored areas are drawn by an engine system into the existing `CanvasLayer::Fog`; the web keeps no second copy of canvas state, only the bytes it persists on the engine's behalf. |
| **II. Plugin-modular engine** | Movement rules extend `systems/token_move.rs` within the existing token plugin; exploration ships as its own plugin (`plugins/exploration.rs`) with its own resources, added to the `App` builder independently and talking to lighting through resources, not private calls. |
| **III. Ownership at the data boundary** | `moveOwnToken` judges a player's move server-side against the scene's walls; a Game Master is never judged (decision 1); door state stays a GM-only mutation; exploration is per-browser and carries no server authority to get wrong. |
| **IV. ADRs before divergent implementation** | Phase 3 introduces server-side movement adjudication — a new ownership boundary — and lands with an ADR. Phase 5 changes the pack manifest contract and amends ADR-027's manifest contract the way spec 016 did. |
| **V. Verify before claiming done** | Per phase: `cargo check` (server), `cargo check --target wasm32-unknown-unknown` (engine), `pnpm -F @thunderforge/web exec tsc --noEmit` (web — `pnpm verify` does not type-check it), then `pnpm playtest` for the finding this phase clears. |

**Gate result**: PASS. No violations to justify; the one boundary change
(Phase 3) is recorded as an ADR rather than waived.

## Project Structure

### Documentation (this feature)

```text
specs/045-token-movement-and-vision/
├── spec.md              # the feature specification
├── plan.md              # this file
├── research.md          # Phase 0: the decisions and why
├── data-model.md        # Phase 1: entities and state
├── contracts/
│   └── movement-and-vision.md   # GraphQL, engine SDK and pack contracts
├── quickstart.md        # Phase 1: how to prove each phase
└── checklists/
    └── requirements.md  # written by /speckit-specify
```

### Source Code (repository root)

```text
src/engine/src/
├── systems/token_move.rs        # phase 2, 3: control, gridless emit, wall stop
├── systems/lighting.rs          # phase 4, 5: vision profiles from the pack
├── plugins/exploration.rs       # phase 6: new plugin, draws into CanvasLayer::Fog
├── sdk.rs, payloads.rs          # set_controlled_token, exploration commands
crates/thunderforge-canvas-core/
├── src/wall.rs, vision.rs       # phase 3: crossing tests reused by both sides
src/server/src/
├── graphql/mutations_tokens.rs  # phase 3: judge a player's move
├── graphql/mutations_interactives_support.rs  # phase 1: announce a wall change
├── graphql/queries/scene.rs     # phase 6: exploration flag and epoch
├── movement/                    # phase 3: the adjudicator, with its own tests
apps/web/src/
├── engine/world/sync/walls.ts   # phase 1: re-read on a door change
├── engine/world/sync/tokens.ts  # phase 2, 3: carry the path, apply a refusal
├── engine/bevy/index.ts         # phase 2, 5, 6: the new commands
├── services/worldCacheStorage.ts# phase 6: explored areas per scene
packs/systems/{dnd5e,genie}/
└── system.json                  # phase 5: the vision declaration
apps/web/playtest/               # the acceptance proof for every phase
```

**Structure Decision**: The repository's existing layout. This feature adds one
engine plugin and one server module; everything else extends a file that
already owns that concern.

## Complexity Tracking

No constitution violations to justify.

One risk worth naming rather than tracking as complexity: **Phase 3 duplicates
a geometry decision on two sides** — the engine must stop a move before it is
sent, and the server must refuse one that arrives anyway. Both use the same
crossing test from `thunderforge-canvas-core`, which is the one place it is
written and tested; neither side reimplements it.
