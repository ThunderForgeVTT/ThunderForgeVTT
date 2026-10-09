# Research: Bevy 0.20

Read on 2026-10-08 against `main` at `3f4cb3b2`. Sources:

- the Bevy 0.19 → 0.20 migration guide and the 0.20 release notes;
- the crates.io records for `bevy` 0.20.0, `bevy_internal` 0.20.0 and
  `glam` 0.33.x;
- the glam changelog for 0.33.0 to 0.33.12.

Counts come from `git grep` over `crates/thunderforge-engine/src`, which
holds 32,548 lines of Rust, unless a line says otherwise. **Hit** means our
code has to change. **Implicit** means our code does not change but its
behaviour can. **0** means we do not use the item.

## Summary table

| #   | 0.20 change                                               | Our count                                   | Kind     |
| --- | --------------------------------------------------------- | ------------------------------------------- | -------- |
| R4  | WESL shaders replace naga_oil `#import`/`#{...}`          | 1 file (`darkness.wgsl`), 2 directives      | Hit      |
| R5  | Sprites drawn as Mesh2d + `SpriteMaterial`                | `render_probe.rs` (4 internals); 75 `Sprite` in 24 files | Hit + implicit |
| R6  | `ExtractResource`/`ExtractComponent` need `#[extract_app]` | 1 (`RenderProbeEnabled`)                    | Hit (maybe) |
| R7  | Weak ordering of built-in sets                            | 1 (`render_probe.rs:84`)                     | Check    |
| R8  | `Camera2d` defaults to `Tonemapping::Linear`              | 16 spawns in 11 files                       | Implicit |
| R9  | Same-z sprite order under the new backend                 | every token on a shared square              | Implicit |
| R10 | `Sprite.alpha_mode` field added                           | 5 literals, all `..default()`               | 0        |
| R11 | `TextFont` default size is `Rem(1.)`                      | 9 in 4 files, all set `FontSize::Px`        | 0        |
| R12 | `NextState::set_if_neq` → `set_if_different`              | 0 (9 `set_if_neq` are `DetectChangesMut`)   | 0        |
| R13 | Observers `On<Add, A>` → `On<Add<A>>`, flat pointer events | 0                                          | 0        |
| R14 | Smaller renames and removals                              | 0 each                                      | 0        |
| R15 | glam 0.32 → 0.33                                          | canvas-core and all its dependants          | Implicit |
| R16 | wgpu 29 → 30, wesl replaces naga_oil                      | lockfile                                    | Implicit |

## R1. Versions and toolchain

**Decision**: move to bevy 0.20.0, glam 0.33 (0.33.12 is the latest) and
wgpu 30. No toolchain change.

**Verified**:

- `bevy` 0.20.0 was published 2026-10-08T22:31Z, with `rust-version`
  1.97.1. Local `rustc` is 1.99.
- It requires glam `^0.33.2`, wgpu `^30` and wesl `^0.6`. naga_oil is
  gone from the tree.
- Lockfile today: bevy 0.19.1, glam 0.32.1, wgpu 29.0.4, naga 29.0.4,
  naga_oil 0.22.0.

**Alternatives rejected**: waiting for 0.20.1. The owner wants the
features now, and a patch release will be a lockfile bump.

## R2. canvas-core is shared with the server

**Decision**: bump glam in canvas-core in the same commit as bevy
(FR-002). Prove the server's reach, budget and movement through
`cargo test -p thunderforge-server` and the combat slice.

**Verified**:

- `crates/thunderforge-canvas-core/Cargo.toml:19` pins `glam = "0.32"`,
  and `lib.rs:63` re-exports `glam::Vec2`.
- These depend on canvas-core:
  - `thunderforge-server`: `movement/mod.rs`, `combat/{budget,reach,redaction}.rs`,
    `graphql/mutations_tokens.rs` and `level_travel.rs`;
  - `thunderforge-combat`: budget, reach and its wasm build;
  - `thunderforge-content`, `thunderforge-pdf` and
    `thunderforge-pack-system-spec`;
  - `apps/thunderforge`, and every pack's server crate.
- If two glams are in the tree, `Vec2` from canvas-core is not Bevy's
  `Vec2`. The Cargo comment records that this broke the build in 44 places
  across 14 files.

**Why**: glam is the one dependency of this upgrade that reaches server
adjudication.

## R3. Features

**Decision**: keep the feature lists unchanged. Add a feature only if the
build fails or a slice shows a blank canvas.

**Verified** against `bevy` 0.20.0's feature table: every feature we use
still exists. That is bevy_asset, bevy_log, bevy_core_pipeline,
bevy_state, bevy_gizmos, bevy_render, bevy_sprite, bevy_sprite_render,
bevy_ui_render, bevy_gizmos_render, bevy_text, bevy_ui, bevy_window,
default_font, png, webp, bevy_winit, webgl2 (still mapped to
`bevy_internal/webgl`) and debug (our `debug-names`).

- **Removed:** `shader_format_glsl` and `shader_format_wesl`. We use
  neither, and WESL is now always on.
- **New:** bevy_curve, bevy_extract, complex_script_segmentation,
  compressed_image_saver_universal, pan_orbit_camera, pan_orbit_gizmo and
  render_dev_tools.
- `bevy_render` enables `bevy_extract`, and `bevy_gizmos` enables
  `bevy_curve`. `bevy_shape` is a required dependency, not a feature.

**Open**: confirm the split crates appear with `cargo tree -e features -i
bevy_extract` and `-i bevy_curve` at T010. The 0.18 lesson was that a
missing render half gives a blank canvas with no error.

**Result (T011)**: confirmed on the bumped lockfile. `cargo tree
--workspace --target all -d` lists no second glam or wgpu: glam 0.33.12
and wgpu 30.0.1 only. `bevy_extract`, `bevy_curve` and `bevy_shape` are
in the engine's wasm32 tree, naga_oil is gone and wesl 0.6.0 is in. The
workspace has `default-members`, so a plain `cargo tree -i` finds
nothing; it needs `--workspace --target all`, or `-p thunderforge-engine
--target wasm32-unknown-unknown`.

## R4. WESL shaders

**Decision**: port `plugins/darkness.wgsl` to `plugins/darkness.wesl`.

**Verified**:

- It is our only shader: `git ls-files '*.wgsl' '*.wesl'` in the engine
  finds 1.
- It has 2 naga_oil directives:
  - line 23: `#import bevy_sprite::mesh2d_vertex_output::VertexOutput`;
  - the binding group: `@group(#{MATERIAL_BIND_GROUP}) @binding(0..2)` for
    the `darkness` uniform, the `shadow_map` texture and its sampler.
- `darkness.rs:244` has `embedded_asset!(app, "darkness.wgsl")`, and
  `darkness.rs:158` returns
  `"embedded://thunderforge_engine/plugins/darkness.wgsl"`.
- The guide gives the new forms:
  - `#import a::b` becomes `import a::b;`;
  - `#{MATERIAL_BIND_GROUP}` becomes `constants::MATERIAL_BIND_GROUP`;
  - module paths follow crate paths;
  - a shader with directives must be `.wesl`, while plain WGSL with no
    directives still loads as `.wgsl`.

**Open**: the 0.20 module path of `VertexOutput` for Mesh2d. It is likely
under `bevy_sprite_render`. T020 finds it in the 0.20 source
(`~/.cargo/registry/src/*/bevy_sprite_render-0.20.0/src/**/*.wesl`).

**Risk**: on WebGL2, uniform arrays must have a fixed size. MAX_LIGHTS is
128, SHADOW_BINS 512 and SIGHT_ROW 128. If WESL lowers them differently,
the shader fails to link, and the only sign is a console error. The
lighting slice and the visual check cover this.

**Result (T020–T023)**: `VertexOutput` is at
`bevy_sprite_render::mesh2d::vertex_output::VertexOutput` in 0.20, so line
23 is now `import bevy_sprite_render::mesh2d::vertex_output::VertexOutput;`.
The three bindings use `@group(constants::MATERIAL_BIND_GROUP)`;
`constants` needs no import. The file moved to `darkness.wesl`, and
`darkness.rs`, `darkness_probe.rs`, the `shadow_map.rs` comment and both
slices that named the old path (`engine-other`, `lighting` in
`scripts/e2e/slices.json`) follow it. The uniform layout and the three
array sizes are unchanged, and WebGL2 links it: the dev engine draws the
lit scene in the sandbox with no shader error, and the darkness capture
matches the baseline to the pixel (T042). The smoke test ran in the
engine sandbox rather than over main's `dist/engine`, because copying an
engine there would have put 0.20 under spec 086's runs. `make lint`
(lint-host and lint-wasm) is green with no new warning and no `allow`.

## R5. Sprites drawn as Mesh2d

**Decision**: keep `Sprite` everywhere, and move the render probe onto the
0.20 pipeline (FR-007, contracts/render-probe.md).

**Verified**:

- 75 `Sprite` hits in 24 files. None reaches into the sprite pipeline
  except `plugins/render_probe.rs`, which imports:
  - `bevy::sprite_render::ExtractedSprites`, read for the count of
    extracted sprites;
  - `bevy::render::view::RenderVisibleEntities`, through
    `.get::<Sprite>().entities_cpu_culling.len()`;
  - `bevy::core_pipeline::core_2d::Transparent2d` with
    `ViewSortedRenderPhases`, for the item count;
  - comments that name `extract_sprites` and `queue_sprites`.
- In 0.20, sprites become Mesh2d entities with a `SpriteMaterial`
  (`SpriteMeshMaterial` after the rename). `Text2d` stays on the old
  backend, so `ExtractedSprites` may still exist but would then count text
  only.
- `EngineStats.sprites` is counted in the main world
  (`Query<(), With<Sprite>>`), so it does not change.

**Risk**: the sprite backend drives frame rate at 3200–6400 tokens. Bevy
says the change "improved performance in many cases". WebGL2 with
thousands of instanced sprites is the case we measure (SC-003).

**Result (T030)**: in 0.20 a token's quad is a `Mesh2d` with a
`SpriteMeshMaterial`, in the `Mesh2d` visibility class; `Text2d` glyphs
stay in the `Sprite` class and in `ExtractedSprites`. Left alone, the
probe's two render-world counts fell from 748 to 474, the glyphs only,
while the board drew every token. The probe now tells a sprite quad apart
in the render world by its material (`RenderMaterial2dInstances`, the
asset type `SpriteMeshMaterial`) and adds those to the glyphs, so each
line keeps its label and its 0.19.1 meaning. It also logs the split, as
a new line: `render: extracted text=… sprite meshes=…`. On the
3200-token level, a dev 0.20 engine gives 3202 / 274 / 750 / 748 / 748,
the baseline's counts exactly, with 474 text and 274 sprite meshes.
`EngineStats` is unchanged.

## R6. Generic extraction

**Decision**: add `#[extract_app(RenderApp)]` to `RenderProbeEnabled` if
the build asks for it.

**Verified**:

- One `ExtractResource`: `render_probe.rs:43/55/68`,
  `#[derive(Resource, Default, Debug, Clone, ExtractResource)] pub struct
  RenderProbeEnabled(pub bool);`.
- `ExtractComponent` and `SyncComponent`: 0.
- The guide says extraction is generic over the target app in the new
  `bevy_extract` crate, and that the derives need `#[extract_app(...)]`.
  It names `ExtractComponent` and `SyncComponent` explicitly. It is not
  clear whether `ExtractResource` is included.

**Result (T013)**: `ExtractResource` is included. The host build failed
on `RenderProbeEnabled` until it had `#[extract_app(RenderApp)]`. That
was the only compile error in the engine.

## R7. Weak ordering

**Decision**: check the one ordering against a built-in render set.

**Verified**:

- `render_probe.rs:84` has
  `render_app.add_systems(Render, trace_render_phases.after(RenderSystems::PhaseSort))`.
- `PostUpdate` has 5 sites, none in `UiSystems`. `dice/mod.rs:59-60`
  orders `.before(TransformSystems::Propagate).before(VisibilitySystems::VisibilityPropagate)`.
  Neither set is on the guide's weak list (Render, RenderGraph, Core2d,
  Core3d, ExtractSchedule, PostUpdate `UiSystems`).
- `.chain()`: 18 hits in 15 files, all strong chains on our own systems,
  so they are unaffected.

**Open**: whether `RenderSystems::PhaseSort` keeps its name in 0.20.
`.after` a weakly ordered set still orders, because weak only means the
built-in sets no longer force an order among themselves.

**Result (T013)**: `RenderSystems::PhaseSort` keeps its name, and the
probe's `.after` compiles unchanged. The probe's lines on 0.20 (T030)
show it still runs after the sort.

## R8. Camera tonemapping

**Decision**: measure, then set it explicitly only if colours shift
(FR-008).

**Verified**:

- `Camera2d` appears 16 times in 11 files. `Tonemapping`, `DebandDither`,
  `Hdr` and `ColorGrading` appear 0 times.
- 0.20 makes `Tonemapping::None` a true passthrough, and gives `Camera2d`
  a default of `Tonemapping::Linear`. On an LDR camera, Linear should be
  the identity, so the expected effect is none. The check proves it.

## R9. Same-z draw order

**Decision**: check it in Phase 4, and add a tie-break only if the order
changed (FR-009).

**Verified**:

- `resources/canvas_layer.rs` gives each layer one z (index × 10). Every
  token in a layer shares it.
- Handles sit at +2.0 (`systems/token.rs:763`).
- canvas-core `token_stack.rs` decides which token a click picks
  ("higher is nearer the viewer"), without regard to draw order.
- The guide warns that sprites with the same z may sort differently under
  the new backend.

## R10–R14. Changes that do not hit us

| Item                                                                                  | Count | Note                                                                                                                         |
| ------------------------------------------------------------------------------------- | ----- | ---------------------------------------------------------------------------------------------------------------------------- |
| `Sprite.alpha_mode` (defaults to Blend)                                               | 0     | 5 `Sprite {` literals: `app.rs:334`, `condition_markers.rs:282/293`, `background.rs:73/189`, all with `..default()`          |
| `TextFont` default `Rem(1.)`                                                          | 0     | 9 uses: `dice/readout.rs:67/97/118/143`, `nameplate.rs:176/242`, `shape.rs:723`, `token_move.rs:466`. All set `FontSize::Px` |
| `NextState::set_if_neq` → `set_if_different`                                          | 0     | 9 `set_if_neq` calls (`camera.rs:110/125/128`, `exploration.rs:296`, `nameplate.rs:239`, `lighting_vision.rs:49/95/143`, `token_move.rs:370`) are `DetectChangesMut`. `NextState` uses `.set` |
| Observers `On<Add<A>>`, flat pointer events                                           | 0     | No `On<`, `add_observer` or `.observe(`. `touch.rs:54` defines our own `Pointer` `SystemParam`, a possible prelude clash      |
| `iter_many` yields `Result`                                                            | 0     |                                                                                                                              |
| `Font::from_bytes` without a family name, `FontSource`                                 | 0     |                                                                                                                              |
| `Interaction`/`Button` deprecated                                                      | 0     | `DispatchInteraction` is ours, and `MouseButton` is input                                                                    |
| `Entity::PLACEHOLDER` → `Option<Entity>` in `UiCameraMapper`, `RetainedViewEntity`     | 0     | The constant itself remains. The guide only replaces some of its uses                                                        |
| `AssetId::invalid` deprecated, `ComponentInfo::id` removed, `Name` from `&'static str` | 0     |                                                                                                                              |
| `SpriteMaterial` → `SpriteMeshMaterial`, `MeshTag`, `ShaderBuffer`, `WgpuWrapper`      | 0     |                                                                                                                              |
| `BorderRadius`, `Val::Em/Rem`, retained UI rendering, `Node`                           | 0     | bevy_ui is compiled but unused                                                                                               |
| `RenderDebugOverlay` added to `DefaultPlugins`                                         | check | `startup.rs:83` builds on `DefaultPlugins`. Confirm the overlay is off by default and adds nothing to the wasm, or `.disable::<>()` it |
| Math primitives, curves, bounding volumes moved to bevy_shape/bevy_curve               | 0     |                                                                                                                              |
| `ScheduleBuildSettings` shuffle_seed, OIT, `CompressedImageSaver`                      | 0     |                                                                                                                              |
| Panics routed to the fallback error handler                                           | check | `startup.rs:90` disables `PanicHandlerPlugin` so that spec 070's `install_panic_hook` tells the page about a panic. Confirm the plugin still exists under that name and that the hook still fires (`canvas-engine-stopped.spec.ts`, canvas slice) |

`Material2d` and `MeshMaterial2d` appear 6 times in 2 files:
`darkness.rs`, and `dice/throw.rs:133-141, 280-281`, which uses
`ColorMaterial` with `Mesh2d`. The guide does not change `Material2d` for
our use. `RenderDevice` is used at `cached_assets.rs:768`, where only the
wgpu 30 signature can matter.

**Result (T013)**: none of these needed a change. `PanicHandlerPlugin`
keeps its path (`startup.rs`), our `Pointer` in `touch.rs` does not clash
with the 0.20 prelude, and `RenderDevice` at `cached_assets.rs:768`
compiles as it is. canvas-core needed nothing for glam 0.33 (T012), and
no other crate changed (T014).

## R15. glam 0.33

**Decision**: take it, and treat any test that changes as a finding.

**Verified** in the changelog:

- 0.33.0: types other than f32 are optional but on by default, and the
  `as_dmat*` methods are `#[must_use]`.
- 0.33.7: SIMD rounding for Vec3A and Vec4 now goes half away from zero.
- 0.33.8: `fast-math` is a no-op and FMA is used where the target
  supports it. Mat3's determinant and inverse now follow Mat4's.
- 0.33.11: `FloatExt::lerp` is computed as `self*(1-s)+rhs*s`, and
  `lerp_monotonic` was added.
- 0.33.12: `try_inverse` returns only finite results.

canvas-core has 735 `assert_eq!` in 39 files. Most compare integers and
enums, but some compare floats exactly:

- `grid.rs:611`;
- `door_icon_tests.rs:204`, `distance_to_segment` to 3.0, 5.0 and 5.0;
- `camera.rs:231`.

17 files already use an approximate comparison.

**Risk**: FMA on x86_64 (the server) but not on wasm32 (combat in the
demo) could make the two disagree by an ulp at a reach or budget boundary.
The combat slice and `cargo test -p thunderforge-combat` cover this. A
disagreement would be a finding for the owner, not something to tune
away.

**Result (T031–T034)**: no test changed.

- canvas-core: 567 passed, 0 failed.
- engine: 365 passed, 0 failed.
- server (`RUST_MIN_STACK=16777216`): 1992 passed, 0 failed, 6 ignored.
  Movement, reach, budget and redaction come out as before. In the run
  beside another build, the doc-test step failed to find
  `thunderforge_canvas_core` (E0463); `cargo test -p thunderforge-server
  --doc` alone passes (the crate has no doc tests), so that was the
  shared target directory, not the upgrade.
- combat: all pass.
- No fixture compared the host and wasm32 builds, so T034 adds one:
  `crates/thunderforge-combat/tests/float_parity.json`, five reach cases
  and seven move-cost cases on square, hex and gridless grids, at
  awkward coordinates, with a wall and a large creature. The host test
  (`tests/float_parity.rs`) and the demo's
  `src/backend/handlers/floatParity.test.ts`, which loads
  `dist/combat/combat_bg.wasm`, both assert each result bit for bit,
  `-0.0` included. Both pass on glam 0.33, so x86_64 and wasm32 agree.
- web: 906 tests in 111 files pass, and `typecheck` is clean. demo: 161
  in 20 files (its `combat.test.ts` needs `pnpm -F @thunderforge/demo run
  maps` first in a fresh worktree).
- Scene switch (T036): `unload_previous_scene` despawns each token with
  `commands.entity(e).despawn()`, which in 0.20 still despawns
  `Children` recursively. No change; the scenes slice runs it.

## R16. wgpu 30 and wesl

**Decision**: accept them, and measure the bundle (SC-004).

**Verified**:

- wgpu goes from 29.0.4 to 30. Our only direct wgpu-level use is
  `RenderDevice` at `cached_assets.rs:768`.
- naga_oil 0.22 is replaced by wesl 0.6, which runs inside the wasm
  engine when it compiles the shader. Its effect on size is unknown, and
  the baseline answers it.

## R17. Proof and the full-suite rule

**Decision**: prove with slices (Open item 1).

**Verified** with `node scripts/e2e-slice.mjs which <path>`:

| Path                                  | Slices                       |
| ------------------------------------- | ---------------------------- |
| `Cargo.lock`                          | FULL SUITE (cross-cutting)   |
| `crates/thunderforge-engine/Cargo.toml`      | engine-limits, engine-other  |
| `crates/thunderforge-canvas-core/Cargo.toml` | engine-other                 |
| `plugins/darkness.wgsl`               | engine-other, lighting       |
| `plugins/render_probe.rs`             | canvas                       |
| `plugins/dice/readout.rs`             | combat, rolls                |
| `systems/nameplate.rs`                | engine-limits, tokens        |
| `systems/camera.rs`                   | canvas                       |

Fog (`plugins/exploration.rs`, `scene-exploration.spec.ts`) belongs to
the **scenes** slice. Vision and darkvision belong to **lighting**.
`resumable-downloads` and `rolls` each have a standalone part and an
integration part.

The memory rule is that the full suite (`e2e-parallel`, about 4 h) never
gates a change, even a schema change.

## R18. Baseline method

**Decision**: take the baseline on the 0.19.1 tree, in the worktree,
before any version change (T003–T007).

- **Frame rate.** Build a release engine with
  `ENGINE_PROFILE=release node scripts/build.mjs --only-wasm`, then copy it
  over main's `dist/engine`. The worktree resolves `@thunderforge/engine`
  through main's `dist/engine`, so a worktree build is not seen otherwise.
  Then run `pnpm e2e:engine-limits` and record each `[engine] tokens=…
  fps=… frameTime=…` line for 3200, 4000, 4800, 5600 and 6400. Run it 3
  times and take the median per level.
- **Size.** Brotli the release `.wasm` with
  `node -e "zlib.brotliCompressSync"` at quality 11. That is the same
  measure `scripts/check-bundle-budget.mjs` uses for the web. Record raw
  and brotli bytes.
- **Render probe.** On a fixed scene (the 3200-token level, at the
  default camera), call `set_render_probe` and record three of its
  60-frame lines: sprites, visible, extracted and `Transparent2d` items.
- **Pictures.** In the sandbox or the web app, take one capture each of:
  - a lit scene with darkness and walls;
  - three stacked tokens with one selected;
  - nameplates and a dice readout;
  - a plain sprite of a known colour.

## R19. Spec 086 overlap

**Decision**: 087 lands before 086 US7 (Open item 2).

**Verified**:

- 086 US7 is:
  - T074 `loadTelemetry.ts`, with hooks at `index.ts:522`;
  - T075 `framesSummary.ts`, which reads `stats.ts`;
  - T076 traceparent;
  - T077, the telemetry-engine e2e.
- 086 also changes `Cargo.lock` (opentelemetry 0.33), the workspace
  members and the Makefile.
- 087 changes `Cargo.lock` and both Cargo files. It may change
  `render_probe.rs`, but not `stats.ts` or `EngineStats`' fields.
- The textual overlap is Cargo.lock only. The semantic one is that
  `engine.frames` measured before and after 087 would show a step.

## Baseline

Taken at `caafdd6d` (T001), on bevy 0.19.1, glam 0.32.1 and wgpu 29.0.4,
in the worktree `../ThunderForgeVTT-087`.

**Toolchain (T002).** `rustc` 1.99.0. `cargo search bevy` shows 0.20.0 as
the latest; there is no 0.20.1, so R1 stands.

**Engine (T003).** A release build (`ENGINE_PROFILE=release node
scripts/build.mjs --only-wasm`). The worktree resolves
`@thunderforge/engine` through its own `dist/engine`, so nothing was
copied over main's: that would have put this engine under spec 086's runs.

**Frame rate (T004).** `pnpm e2e:engine-limits`, three runs, each started
only when no e2e run, no build from another tree, and a 1-minute load
under 4. All 4 tests passed in each run.

| Run | Load at start (1/5/15 min) | 3200 | 4000 | 4800 | 5600 | 6400 |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | 2.18 / 3.08 / 4.99 | 61 fps, 16.5 ms | 60, 16.7 | 58, 17.2 | 61, 16.5 | 60, 16.6 |
| 2 | 2.41 / 4.54 / 5.32 | 61, 16.3 | 60, 16.7 | 60, 16.7 | 60, 16.7 | 60, 16.6 |
| 3 | 3.82 / 4.16 / 4.73 | 60, 16.7 | 60, 16.7 | 60, 16.8 | 60, 16.8 | 60, 16.6 |
| **Median** | | **61, 16.5** | **60, 16.7** | **60, 16.8** | **60, 16.7** | **60, 16.6** |

Every level sits at the display's 60 Hz, so the slice measures whether a
frame is dropped, not how much headroom there is. Run 1's report says
another tree's `cargo`/`rustc` was active for 7 samples; its numbers match
runs 2 and 3, so it is kept.

**Size (T005).** Release `engine_bg.wasm`: 30,424,557 bytes raw,
5,069,241 bytes brotli at quality 11.

**Render probe (T006).** The 3200-token level at the default camera, in
the engine sandbox, release engine (load about 12.6 / 9.3 / 6.8, so these
are counts, not timings):

```
main: sprites total=3202 view_visible=274
render: Transparent2d view items=750
render: views matching (RenderVisibleEntities, ExtractedView, Msaa)=1 · views missing Msaa=1
render: ExtractedSprites=748
render: view RenderVisibleEntities<Sprite>=748
```

`engineStats` read fps 65.4, frame 15.3 ms and `tokens_culled` 2020 there.

**Captures (T007).** In the engine sandbox, Chromium with a GPU, 1280×900
canvas. The four pictures are in `baseline/` (`sprite.png`, `stack.png`,
`text.png`, `darkness.png`).

- Known sprite: the red token at world (-170, 10) reads RGBA
  `[203, 67, 75, 255]`; clear floor reads `[34, 40, 49, 255]`.
- Darkness: torch `[255, 200, 128]`, lit floor at (-60, 0)
  `[99, 103, 111]`, wall shadow at (60, 0) `[100, 105, 123]`, far dark
  `[39, 47, 78]`.
- Stack: a click on three tokens on one square picks `stack-a`, the
  lowest id, and the box-select returns all three.
- `stack.png` is not stable from run to run, even on 0.19.1: two captures
  of the same build differ inside the stack's box (1,924 pixels, up to 222
  per channel). The three photo tokens share one z, and which is drawn on
  top most likely follows when each one's art arrives. So the stack is compared by a
  second, deterministic check instead: three plain tokens of the three
  kind colours (character blue, npc red, vehicle amber, ids `kind-a`,
  `kind-b`, `kind-c`) on one square, added in two orders, reading the top
  pixel before a click.

  | Added | Drawn on top | Click picks |
  | --- | --- | --- |
  | a, b, c | `kind-c` (amber `[229, 163, 64]`) | `kind-a` |
  | c, b, a | `kind-a` (blue `[106, 137, 237]`) | `kind-a` |

  The last token added is drawn on top, and a click picks the lowest id
  (`token_stack.rs`), whatever is drawn. When those disagree, the click
  takes a token the player cannot see. That is true on 0.19.1 already.

## Result

Measured on the release 0.20 engine unless a line says otherwise.

### Visuals (T040–T045)

The four captures are in `baseline/` with the suffix `-020`, taken by
the same script in the same sandbox as the baseline (load 2.31 / 6.86 /
9.97 at the start, no e2e running).

- **Colour (T041).** `sprite-020.png` equals `sprite.png` to the pixel:
  the red token reads `[203, 67, 75, 255]` on both. `Camera2d`'s new
  `Tonemapping::Linear` default is the identity on our LDR cameras, so no
  tonemapping is set.
- **Darkness (T042).** `darkness-020.png` equals `darkness.png` to the
  pixel, and so does every sampled value (torch, lit floor, wall shadow,
  far dark). The WESL port draws the same falloff, shadows and edge of
  sight.
- **Text (T043).** `text-020.png` equals `text.png` to the pixel:
  nameplates and the dice readout keep their size, position and order
  against the sprites.
- **Stacking (T040).** Same-z order did not change. The kind-colour check
  gives the same top pixel and the same pick on both versions: the last
  token added is drawn on top, and a click picks the lowest id. No
  tie-break was added. `stack-020.png` differs from `stack.png` inside
  the stack's box (3,694 pixels), as two 0.19.1 captures differ from each
  other (1,924); that picture is not stable from run to run on either
  version.
  - Found on the way, and present on 0.19.1: when the token drawn on top
    is not the lowest id, a click picks a token the player cannot see.
    That is not a 0.20 change, so it is left for its own fix.
- **RenderDebugOverlay (T044).** `DefaultPlugins` adds
  `RenderDebugOverlayPlugin` only with bevy's `render_dev_tools` feature,
  which we do not enable, and `bevy_dev_tools` is not in the engine's
  wasm32 tree. It is absent, so nothing is disabled.
- 0.20 logs one new line at startup on WebGL2: "Sparse buffer updates
  disabled. RenderDevice lacks support: max_storage_buffers_per_shader_stage
  (0) < 3". It is `info`, and WebGL2 has no storage buffers; nothing
  draws differently.
