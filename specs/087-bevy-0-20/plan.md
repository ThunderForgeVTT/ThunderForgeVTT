# Implementation Plan: Bevy 0.20

**Branch**: `087-bevy-0-20` | **Date**: 2026-10-08 | **Spec**: [spec.md](spec.md)

## Summary

The engine moves from Bevy 0.19.1 to 0.20.0, and canvas-core moves from
glam 0.32 to 0.33 in the same commit. The work is a dependency bump plus a
short list of code changes:

- port the one custom shader, `darkness.wgsl`, to WESL;
- move the render probe onto the Mesh2d sprite pipeline;
- possibly add `#[extract_app(RenderApp)]` to one `ExtractResource`.

Three changes need no code but can still change behaviour: camera
tonemapping, the draw order of sprites at the same z, and float results in
glam. Each one is checked against a baseline taken on 0.19.1 first.

The work runs in its own worktree on branch `087-bevy-0-20`, in six phases
after the baseline:

1. **Host compiles.**
2. **wasm32 and `make lint`.**
3. **Behaviour.**
4. **Visuals.**
5. **Perf and slices.**
6. **Docs.**

It reaches main as a fast-forward. To roll back, revert the merge.

## Technical Context

**Language/Version**: Rust 1.99 (Bevy 0.20 MSRV 1.97.1), edition as in the
workspace. TypeScript is untouched, apart from any e2e reading of
render-probe lines.
**Primary Dependencies**: bevy 0.19.1 → 0.20.0, glam 0.32 → 0.33,
wgpu 29 → 30 (transitive), naga_oil 0.22 → wesl 0.6 (transitive).
**Storage**: none. No migration, no GraphQL change.
**Testing**: `cargo test` (canvas-core, engine, server, combat), the web
vitest suite, `make lint`, and Playwright slices (spec.md, Proof).
**Target Platform**: wasm32 in the browser on WebGL2; x86_64 for the
host tests and the server.
**Project Type**: an engine library inside a Cargo and pnpm monorepo.
**Performance Goals**: engine-limits median fps at least 95% of the
baseline at 3200–6400 tokens (SC-003). Brotli growth at most 10%
(SC-004).
**Constraints**:
- Bevy and glam move in one commit (FR-002).
- No second glam or wgpu in the tree.
- No change to `EngineStats` fields.
- Slices only, never the full suite.
- The worktree must copy its wasm build over main's `dist/engine` before
  any e2e run.

**Scale/Scope**: about 4 engine files edited (`Cargo.toml`, `darkness.rs`,
`darkness.wesl`, `render_probe.rs`), 1 canvas-core Cargo file, possibly
`canvas_layer.rs`/`token.rs` (FR-009) and the cameras (FR-008), and the
docs.

## Constitution Check

| Principle                                          | How this plan meets it                                                                                                                                                               |
| -------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| I. ECS owns simulation, React owns chrome          | Nothing moves across the line. The upgrade is inside the engine, and the web reads the same `EngineStats`.                                                                             |
| II. Plugin-modular engine                          | The changes stay inside their plugins: `DarknessPlugin` (shader), `RenderProbePlugin` (probe). No plugin gains a dependency on another.                                               |
| III. Ownership and authorization at the data boundary | Not touched. The server's reach, budget and movement only see a glam bump, which `cargo test -p thunderforge-server` and the combat slice prove unchanged.                           |
| IV. Specs before divergent implementation          | This spec. The only decisions it leaves open are the owner's (spec.md, Open items).                                                                                                    |
| V. Verify before claiming done                     | The baseline comes before any change (FR-011), and each SC is a measured threshold against it.                                                                                        |
| VI. Every feature is proven by its own slice       | The upgrade has no slice of its own. It is proven by the slices that own the engine's paths. Cargo.lock asks for the full suite, and the slices stand in for it (Open item 1).         |
| VII. Telemetry is on, anonymous, redirectable      | Not touched. Merge order with 086 US7 keeps `engine.frames` from showing the upgrade as a step (Open item 2).                                                                          |

There are no violations. VI is satisfied by the named slices, and the
substitution is recorded as Open item 1.

## Worktree, branch and merge

- `git worktree add ../ThunderForgeVTT-087 -b 087-bevy-0-20 main`. The
  baseline commit is recorded in research.md (R18) when Phase 0 runs.
- The worktree resolves `@thunderforge/engine` through **main's**
  `dist/engine`. Before each e2e run, run
  `ENGINE_PROFILE=<dev|release> node scripts/build.mjs --only-wasm` in the
  worktree and copy its `dist/engine` over main's. After the work is done,
  rebuild main's own engine. Spec 086 is being built in main and must not
  run e2e against a 0.20 engine by accident, so these runs are coordinated
  through the e2e lock: wait on it, and never bypass it.
- Commits are signed and go through `mcp__gitops__commit` with explicit
  files.
- **Merge order.** 087 merges before 086 US7 (T074–T077). Both specs
  change `Cargo.lock`, and the merge is fast-forward only. If main moves
  first:
  - `merge_ff_only` refuses;
  - the owner decides whether to rebase;
  - after a rebase, Phase 5 runs again, along with the baseline if 086's
    dependencies changed the engine's build.
- **Rollback.** Reverting the single merge range restores 0.19.1. No data
  or schema depends on the version.

## Project Structure

### Documentation (this feature)

```text
specs/087-bevy-0-20/
├── spec.md
├── plan.md
├── research.md         # audit counts, baseline method, then the baseline and 0.20 numbers
├── data-model.md       # EngineStats and the probe line: what may not change
├── contracts/
│   └── render-probe.md # the probe's console line and its 0.20 sources
├── quickstart.md
└── tasks.md
```

### Source Code (repository root)

```text
crates/thunderforge-engine/
├── Cargo.toml                       # bevy 0.20.0, both blocks; comments
└── src/
    ├── plugins/darkness.rs          # embedded_asset! and fragment_shader() → .wesl
    ├── plugins/darkness.wgsl → darkness.wesl
    ├── plugins/render_probe.rs      # Mesh2d sources, extract_app, PhaseSort anchor
    ├── resources/canvas_layer.rs    # only if FR-009 needs a tie-break
    ├── systems/token.rs             # only if FR-009 needs a tie-break
    ├── systems/camera.rs (+ other Camera2d spawns)  # only if FR-008 fails
    └── startup.rs                   # PanicHandlerPlugin / RenderDebugOverlay check
crates/thunderforge-canvas-core/
├── Cargo.toml                       # glam 0.33; comment
└── src/*_tests.rs                   # only float comparisons FR-006 allows
Cargo.lock
apps/engine-sandbox/README.md        # version reference
docs/CONTRIBUTING.md                 # "Upgrading Bevy"
```

## Complexity Tracking

None. The plan adds no crate, no abstraction and no feature flag. A
runtime flag would mean shipping two Bevy versions, which the bundle and
the build cannot carry. The rollback is the revert.
