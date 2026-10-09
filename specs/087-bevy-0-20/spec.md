# Feature Specification: Bevy 0.20

**Feature Branch**: `087-bevy-0-20`
**Created**: 2026-10-08
**Status**: Planned (plan.md, tasks.md)
**Input**: "Upgrade the engine from Bevy 0.19.1 to 0.20.0. The board looks
and behaves the same, the frame rate is no worse, and the engine bundle
stays within budget."

## Why

Bevy 0.20.0 was published on 2026-10-08. It brings sprite materials, a
single sprite backend built on `Mesh2d`, `despawn_all`, weak ordering with
`chain_weak`, and per-column change ticks. The owner wants those features
within reach. Staying on 0.19 means each later upgrade spans more
releases and costs more.

The upgrade has no feature of its own. A player should see no
difference, apart from a frame rate that may improve. This spec only
moves the engine onto 0.20 and proves it. Adopting the new features is
listed under **Later**.

## What exists

Counted on 2026-10-08 against `main` at `3f4cb3b2`. research.md has every
count, with files.

- `crates/thunderforge-engine/Cargo.toml:50` and `:159` pin
  `bevy = "0.19.1"`, with default features off. The host list and the
  wasm32 list add `bevy_winit` and `webgl2`.
- `crates/thunderforge-canvas-core/Cargo.toml:19` pins `glam = "0.32"` to
  match Bevy. Its comment records that a mismatch once broke the build in
  44 places across 14 files.
- canvas-core re-exports `glam::Vec2` (`lib.rs:63`), so glam is not just
  an engine dependency. It also reaches:
  - the server's movement, combat budget, reach and redaction, token
    mutations and level travel;
  - thunderforge-combat (its wasm too), content, pdf and pack-system-spec;
  - every pack's server crate.
- The lockfile holds bevy 0.19.1, glam 0.32.1, wgpu 29.0.4, naga 29.0.4 and
  naga_oil 0.22.0. Bevy 0.20 needs glam ^0.33.2, wgpu ^30 and wesl ^0.6
  (wesl replaces naga_oil), and its MSRV is 1.97.1. Local rustc is 1.99.
- One custom shader, `plugins/darkness.wgsl`, uses naga_oil syntax:
  `#import bevy_sprite::mesh2d_vertex_output::VertexOutput` and
  `@group(#{MATERIAL_BIND_GROUP})`.
- `plugins/render_probe.rs` reads several internals of the 0.19 sprite
  pipeline:
  - `ExtractedSprites`;
  - `RenderVisibleEntities::get::<Sprite>()`;
  - `Transparent2d` items, after `RenderSystems::PhaseSort`;
  - an `ExtractResource`, `RenderProbeEnabled`.
- 16 `Camera2d` spawns in 11 files, none of which sets `Tonemapping`.
- 75 `Sprite` uses in 24 files. All five `Sprite { .. }` literals end in
  `..default()`.
- Tokens share one z per layer (`resources/canvas_layer.rs`, layer index ×
  10). Token handles sit at +2.0 (`systems/token.rs:763`).
- The release engine is about 4.15 MB brotli (24.7 MB unstripped). Any
  growth is worth reporting, but the level of concern is 100 MB brotli.
- `node scripts/e2e-slice.mjs which Cargo.lock` reports **FULL SUITE**,
  because Cargo.lock counts as a cross-cutting path.
- Spec 086 is being built in the main tree. Its US7 (T074–T077) adds
  `engine.load` spans and an `engine.frames` summary that reads
  `apps/web/src/engine/bevy/stats.ts`. It also changes `Cargo.lock`.

## User Scenarios & Testing

### User Story 1 - The board looks the same (Priority: P1)

A GM opens a scene with a map, tokens, walls, lights, darkness, fog and
shapes. Everything is drawn as it was on 0.19.1: the same colours, the
same darkness, the same stacking order.

**Why this priority**: an upgrade that changes how the board looks is a
regression that players see at once. The defaults that changed in 0.20
hit exactly this: tonemapping, the sprite backend, same-z order and the
shader dialect.

**Independent Test**: take render captures of the same scenes on 0.19.1
and on 0.20, and compare them. Run the lighting, canvas, tokens and scenes
slices.

**Acceptance Scenarios**:

1. **Given** a lit scene with darkness and walls, **When** it is opened on
   0.20, **Then** the darkness, light falloff and wall shadows match the
   0.19.1 capture.
2. **Given** a sprite of a known colour, **When** it is drawn on 0.20,
   **Then** its pixel colour matches 0.19.1 within FR-008's tolerance.
3. **Given** three tokens stacked on one square, **When** the board is
   drawn, **Then** they stack in the same order as on 0.19.1, and the top
   one is the one a click picks.
4. **Given** a selected token, **When** it is drawn, **Then** its resize
   and rotate handles sit above it.
5. **Given** dice thrown on the board, nameplates and condition markers,
   **When** they are drawn, **Then** their text size and placement match
   0.19.1.

---

### User Story 2 - The board behaves the same (Priority: P1)

Every gesture a GM or player makes on the board does what it did before.
That covers moving, selecting, box select, walls, doors, lights, fog,
vision, dice, scene switching and levels.

**Why this priority**: changes to schedules and system order can break
behaviour silently.

**Independent Test**: the e2e slices named in **Proof**, and the engine
and canvas-core unit tests.

**Acceptance Scenarios**:

1. **Given** the engine and canvas-core unit tests, **When** they run on
   0.20, **Then** all pass with no test deleted or loosened, apart from
   the float comparisons that FR-006 allows.
2. **Given** a token moved into a wall, **When** the server adjudicates
   the move, **Then** the result is the same as on 0.19.1. The glam bump
   reaches the server through canvas-core.
3. **Given** a scene switch, **When** the new scene loads, **Then** the
   previous scene's entities are gone and the new ones are drawn.

---

### User Story 3 - The frame rate is no worse (Priority: P1)

A large battle map runs at least as fast on 0.20 as it did on 0.19.1.

**Why this priority**: 0.20 replaces the sprite backend. Bevy reports
"improved performance in many cases". Our case is thousands of tokens on
WebGL2, which Bevy's benchmarks may not cover.

**Independent Test**: run `pnpm e2e:engine-limits` against a **release**
engine before and after, on the same machine, and compare the per-level
fps and frame times.

**Acceptance Scenarios**:

1. **Given** the 0.19.1 baseline, **When** the sweep runs on 0.20, **Then**
   each level's fps is within SC-003's tolerance of the baseline, or better.
2. **Given** the render probe is switched on, **When** a scene is drawn,
   **Then** it still logs its counts, and the counts it still reports agree
   with the baseline.

---

### User Story 4 - The engine bundle stays within budget (Priority: P2)

The engine a browser downloads grows by no more than SC-004 allows, and
the growth is reported.

**Why this priority**: two dependencies change at once, wgpu 30 and wesl
in place of naga_oil, and either could add size. The budget leaves plenty
of room, but the growth still has to be seen.

**Independent Test**: measure the release `.wasm` with brotli before and
after.

**Acceptance Scenarios**:

1. **Given** the release engine, **When** it is built on 0.20, **Then**
   its brotli size is recorded next to the baseline, along with the
   difference.
2. **Given** `pnpm e2e:resumable-downloads` on a release build, **When**
   it runs, **Then** the engine still downloads, resumes and caches.

---

### User Story 5 - The docs name the version we run (Priority: P3)

A contributor reading the Cargo comments or the sandbox README finds the
version we actually run, and a note on what 0.20 changed for us.

**Why this priority**: stale version notes cost the next upgrade time.
The canvas-core comment that names Bevy's glam version is the one that
breaks builds.

**Independent Test**: `git grep -n "0\.19"` across the engine,
canvas-core, the sandbox README and `docs/CONTRIBUTING.md` finds no
reference that is stale.

**Acceptance Scenarios**:

1. **Given** the canvas-core Cargo comment, **When** it is read, **Then**
   it names Bevy 0.20 and glam 0.33.
2. **Given** `docs/CONTRIBUTING.md`, **When** a contributor looks for how
   to upgrade Bevy, **Then** a short section names the pins that must move
   together, the shader dialect, and the baseline steps.

### Edge Cases

- **A same-z tie.** The new backend may break ties between sprites at the
  same z in a different order. Hit-testing (`token_stack.rs`) decides
  which token a click picks, independently of drawing. So the drawn order
  and the picked token could disagree.
- **Text on the old backend.** `Text2d` stays on the old sprite pipeline
  in 0.20. Nameplates and dice readouts then share a pass with sprites
  that have moved to Mesh2d. Their order relative to sprites is in the
  visual check.
- **WebGL2.** The darkness material uses fixed uniform arrays: MAX_LIGHTS
  128, SHADOW_BINS 512 and SIGHT_ROW 128. WESL has to produce the same
  layout, or the WebGL2 shader fails to link and the board goes blank.
- **A blank canvas with no error.** A missing `*_render` feature once
  cost a day to a blank canvas. If the feature split (bevy_extract,
  bevy_curve, bevy_shape) leaves a render half out, the result looks the
  same.
- **Float drift at the server.** glam 0.33.8 makes FMA the default where
  the target supports it, and 0.33.11 changes `lerp`. The server
  (native, x86_64) and the combat wasm (wasm32) could then disagree by an
  ulp on a reach or budget edge.
- **Spec 086 landing first.** If 086 US7 is merged before 087, its
  `engine.frames` baseline is taken on 0.19.1, and 087 shows up as a step
  in it.

## Requirements

### Functional Requirements

**The bump**

- **FR-001**: Bevy MUST be 0.20.0 in both of the engine's dependency
  blocks (host and wasm32), with the same features. A feature is added
  only if the build or a check shows it is missing.
- **FR-002**: glam in canvas-core MUST be the same minor version as
  Bevy's glam (0.33), so that the workspace resolves a single glam. The
  bevy, glam and canvas-core changes MUST land in one commit.
- **FR-003**: The workspace MUST resolve one `wgpu` (30) and one `glam`
  (0.33). `cargo tree -d` MUST show no second copy of either.
- **FR-004**: No crate outside the engine and canvas-core MAY change
  version in this spec, apart from what Bevy 0.20 itself pulls in.

**The code**

- **FR-005**: `darkness.wgsl` MUST be ported to WESL as `darkness.wesl`,
  with `import ...;` for imports and `constants::MATERIAL_BIND_GROUP`.
  It MUST keep its uniform layout, so that WebGL2 links it.
  `fragment_shader()` and `embedded_asset!` MUST name the new file.
- **FR-006**: Tests that compare floats exactly MAY move to an
  approximate comparison only where glam 0.33 changed the result, and
  only with a tolerance of at most `1e-5`. Each such test MUST be named in
  tasks.md. No test MAY be deleted.
- **FR-007**: The render probe MUST still log on the 0.20 render
  pipeline, with its counts read from the 0.20 equivalents: sprites
  drawn, visible sprites and items in the transparent 2D phase.
  `EngineStats` MUST keep every field it has today, because e2e specs and
  `stats.ts` read them.
- **FR-008**: Every `Camera2d` MUST draw colours as 0.19.1 did. If 0.20's
  default tonemapping (`Linear`) changes a sprite's pixel colour by more
  than 1/255 on any channel, the cameras MUST set the tonemapping that
  matches 0.19.1.
- **FR-009**: Tokens that share a square MUST draw in the order
  hit-testing picks them. If 0.20 breaks the tie differently, the engine
  MUST give each token a stable z offset within its layer, below the +2.0
  of the handles.
- **FR-010**: No system MAY rely on an ordering that 0.20 made weak.
  `render_probe.rs`'s `.after(RenderSystems::PhaseSort)` MUST still hold
  or MUST be re-anchored.

**The proof**

- **FR-011**: A baseline MUST be taken on 0.19.1 before any version
  changes, and recorded in research.md. It covers:
  - engine-limits fps and frame times on a release engine;
  - the release engine's brotli size;
  - render-probe counts on a fixed scene.
- **FR-012**: `make lint` (lint-host and lint-wasm) MUST pass.
- **FR-013**: The canvas-core, engine, server, combat and web unit tests
  MUST pass.
- **FR-014**: The e2e slices named in **Proof** MUST pass, along with
  every slice that `pnpm e2e:which --diff` names other than the full
  suite (Open item 1).

**The docs**

- **FR-015**: Stale version references MUST name 0.20. That covers the
  canvas-core Cargo comment, the engine Cargo comments, the sandbox
  README and code comments that name a Bevy version as current.
  `docs/CONTRIBUTING.md` MUST gain a short **Upgrading Bevy** section.

### Key Entities

- **Baseline**: the 0.19.1 measurements of FR-011, with the commit they
  were taken at.
- **EngineStats**: the engine's frame counters. Its fields do not change
  (data-model.md).
- **Render probe trace**: the per-60-frame console line. Its sources
  change, but its meaning does not (contracts/render-probe.md).

## Success Criteria

### Measurable Outcomes

- **SC-001**: `cargo build`, `make lint` and the unit tests of FR-013 are
  green on 0.20, with one glam and one wgpu in `cargo tree -d`.
- **SC-002**: The visual check (quickstart.md) finds no difference from
  the 0.19.1 captures:
  - sprite pixel colours within 1/255 per channel;
  - darkness and shadows the same by eye;
  - the stacking order of three stacked tokens the same.
- **SC-003**: For every engine-limits level from 3200 to 6400 tokens, the
  0.20 median fps is **at least 95% of the baseline's**, and the median
  frame time is no more than 105% of it. Both are measured on a release
  engine on the same machine. The existing gate (fps > 20 at the first
  level) holds.
- **SC-004**: The release engine's brotli size grows by **no more than
  10%** (about 0.4 MB) over the baseline, and the difference is reported.
  It stays far under the 100 MB level of concern.
- **SC-005**: Every slice in **Proof** passes in a single run, with
  `THUNDERFORGE_DISABLE_AUTH_RATE_LIMIT=1` and `--workers=1` where the
  external stack is used.
- **SC-006**: On the same scene, the render probe's sprite and token
  counts on 0.20 equal the baseline's.

### Proof

On the `087-bevy-0-20` worktree, after Phase 5:

- `make lint`.
- `cargo test -p thunderforge-canvas-core`, `cargo test -p thunderforge-engine`,
  `cargo test -p thunderforge-server` and `cargo test -p thunderforge-combat`.
- `pnpm -F @thunderforge/web test`.
- The slices:
  - `pnpm e2e:engine-limits` (release engine);
  - `pnpm e2e:engine-other`;
  - `pnpm e2e:canvas`;
  - `pnpm e2e:tokens`;
  - `pnpm e2e:lighting`, which holds vision and darkvision;
  - `pnpm e2e:scenes`, which holds fog (`scene-exploration.spec.ts`) and
    scene switching;
  - `pnpm e2e:combat`, which holds dice and the server's reach and budget;
  - `pnpm e2e:rolls`;
  - `pnpm e2e:resumable-downloads` (release engine);
  - `pnpm e2e:worlds`.
- `pnpm e2e:which --diff`, with each slice it names run, except the full
  suite (Open item 1).

The full suite (`e2e-parallel`) is not run.

## Assumptions

- Bevy 0.20.0 is the release on crates.io, published 2026-10-08. A
  0.20.1 that comes out during the work is taken if it only fixes bugs.
- Every engine feature we use still exists in 0.20 (research.md R3). The
  features 0.20 split out are pulled in by the ones we already enable.
- No persisted data, GraphQL contract or server behaviour changes. The
  glam bump can only shift float results in the last bit.
- The baseline and the 0.20 run use the same machine and browser, with no
  other e2e run in between.

## Open items

These are the owner's decisions. Each has a default that the work follows
unless the owner says otherwise.

1. **The slices stand in for the full suite.** `e2e:which` marks
   Cargo.lock as cross-cutting and asks for the full suite. Default: the
   slices listed in **Proof** replace it, as with every other dependency
   bump.
2. **Merge order with spec 086.** Default: 087 merges before 086 US7
   (T074–T077), so that `engine.frames` and `engine.load` start on 0.20.
   Both specs change Cargo.lock, and merges are fast-forward only. If 086
   lands first, the owner decides whether 087 is rebased. A rebase means
   re-running the Proof slices and taking a new baseline.
3. **The perf tolerance.** Default: 95% of baseline fps (SC-003). The
   owner may want the bar at 100%, or may want a gain recorded as a
   target.
4. **The bundle tolerance.** Default: 10% brotli growth (SC-004). Growth
   beyond that is reported and the work pauses for the owner, but it is
   not a failure by itself, given the 100 MB level of concern.
5. **The render probe's counters.** Default: keep the field names and
   read them from the Mesh2d pipeline. The alternative is to rename them
   to say what 0.20 counts. That changes e2e specs, so it would be a
   separate change.
6. **A z tie-break for stacked tokens.** Default: added only if Phase 4
   shows the order changed (FR-009).
7. **Tonemapping.** Default: set it explicitly only if FR-008's check
   fails. The other choice is to set it on every camera anyway, as a
   guard against the next default change.

## Later

These are candidates the upgrade makes possible. None is adopted here.

- **Sprite materials.** `MaterialExtension2d` and `SpriteMaterial` could
  draw darkness, fog or token rings as materials on sprites rather than
  as separate meshes.
- **`despawn_all`.** On a scene switch (`scene_transition.rs:203`,
  `unload_previous_scene`), it could replace the per-entity despawn loop.
  Bevy measures 1.3 to 1.7 times faster from 100 to 100k entities.
- **`chain_weak`.** For our own system chains where the order is a
  preference and not a requirement.
- **Per-column change ticks.** `#[component(summary_tick)]` on components
  that change rarely across many entities, such as token position, so
  that change detection skips whole columns.
