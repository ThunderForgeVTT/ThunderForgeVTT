# Tasks: Bevy 0.20

**Input**: Design documents from `specs/087-bevy-0-20/`
**Prerequisites**: plan.md, spec.md, research.md, data-model.md,
contracts/render-probe.md, quickstart.md

**Merge order**: 087 merges before spec 086's US7 (T074–T077), which adds
`engine.load` and `engine.frames`. Both specs change `Cargo.lock`, and
merges are fast-forward only. If main moves first, stop at T064 and ask
the owner (spec.md Open item 2).

**Tests**: this is an upgrade, so most of the proof is the existing tests
plus a baseline. A new test is added only where a behaviour has no test
today (T034, T040).

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel. It touches a different file, and depends
  on no unfinished task.
- **[Story]**: one of:
  - US1, the board looks the same;
  - US2, it behaves the same;
  - US3, the frame rate is no worse;
  - US4, the bundle stays within budget;
  - US5, the docs name the version we run.

---

## Phase 0: Setup and baseline (on 0.19.1)

**Purpose**: the numbers everything after is measured against. No version
changes in this phase.

- [x] T001 Create the worktree: `git worktree add ../ThunderForgeVTT-087 -b 087-bevy-0-20 main`. Record the base commit in research.md as `## Baseline` → "Taken at `<hash>`".
- [x] T002 [P] Check the toolchain: `rustc --version` is at least 1.97.1, and `cargo search bevy --limit 1` shows 0.20.x. If 0.20.1 or later exists and is bug-fix only, use it, and record that in research.md R1.
- [x] T003 [US3] Build a release engine (`ENGINE_PROFILE=release node scripts/build.mjs --only-wasm`) and copy `dist/engine` over main's (quickstart.md). Wait for the e2e lock if a run holds it.
- [x] T004 [US3] Run `THUNDERFORGE_DISABLE_AUTH_RATE_LIMIT=1 pnpm e2e:engine-limits` three times. Record the median fps and frame time for each level (3200, 4000, 4800, 5600, 6400) in research.md under `## Baseline`.
- [x] T005 [P] [US4] Record the release `.wasm` size, raw and brotli at quality 11, in research.md under `## Baseline` (quickstart.md step 2).
- [x] T006 [P] [US3] Record three render-probe lines on the 3200-token scene in research.md under `## Baseline` (contracts/render-probe.md).
- [x] T007 [P] [US1] Take the four baseline captures (quickstart.md step 4). Keep them in `specs/087-bevy-0-20/baseline/` (`darkness.png`, `stack.png`, `text.png`, `sprite.png`), and record the known sprite's pixel value.
- [x] T008 Commit the baseline (research.md and `baseline/`) on `087-bevy-0-20` as "Spec 087: the 0.19.1 baseline".

**Checkpoint**: the baseline is recorded and committed. Nothing else has
changed.

---

## Phase 1: Host build compiles

**Goal**: `cargo build` and `cargo check --all-targets` on the host, with
one glam and one wgpu.

- [x] T010 Bump the versions together, in one commit (FR-002):
  - `bevy = "0.20.0"` in both blocks of `crates/thunderforge-engine/Cargo.toml` (lines 50 and 159), with the feature lists unchanged;
  - `glam = "0.33"` in `crates/thunderforge-canvas-core/Cargo.toml`;
  - then `cargo update -p bevy -p glam`.
- [x] T011 `cargo tree -d | grep -E "^(glam|wgpu) "` prints nothing (FR-003). Then:
  - confirm `bevy_extract`, `bevy_curve` and `bevy_shape` are in the tree (`cargo tree -i bevy_extract`, `-i bevy_curve`) and record the result in research.md R3;
  - confirm `naga_oil` is gone and `wesl` is present.
- [x] T012 `cargo check -p thunderforge-canvas-core --all-targets`, and fix what glam 0.33 breaks. The changelog points at `#[must_use]` on `as_dmat*` and at features that are now optional.
- [x] T013 `cargo check -p thunderforge-engine --all-targets`. Fix compile errors only, keeping behaviour the same. Each fix is recorded in research.md under the R entry it belongs to. The expected sites are:
  - `render_probe.rs` imports (R5);
  - `RenderProbeEnabled` `#[extract_app(RenderApp)]` (R6);
  - `RenderSystems::PhaseSort` (R7);
  - the `PanicHandlerPlugin` name in `startup.rs:90`;
  - `touch.rs:54` `Pointer` against the prelude (R10–R14);
  - `cached_assets.rs:768` `RenderDevice` (R16).
- [x] T014 `cargo check --workspace --all-targets` for every other crate that depends on canvas-core (server, combat, content, pdf, pack-system-spec, apps/thunderforge, the pack crates). Expect no source change (FR-004).

**Checkpoint**: the host compiles, with a single glam and a single wgpu.

---

## Phase 2: wasm32 build and `make lint`

- [x] T020 [US1] Port the shader (R4, FR-005):
  - Move `crates/thunderforge-engine/src/plugins/darkness.wgsl` to `darkness.wesl` (`git mv`).
  - Change `#import bevy_sprite::mesh2d_vertex_output::VertexOutput` to the 0.20 WESL `import …;`, with the module path read from `bevy_sprite_render-0.20.0`'s shaders.
  - Change `#{MATERIAL_BIND_GROUP}` to `constants::MATERIAL_BIND_GROUP` at bindings 0–2.
  - Keep `MAX_LIGHTS`, `SHADOW_BINS`, `SIGHT_ROW` and the uniform layout as they are.
  - Point `darkness.rs:244` `embedded_asset!` and `darkness.rs:158` `fragment_shader()` at `darkness.wesl`.
- [x] T021 `cargo check -p thunderforge-engine --target wasm32-unknown-unknown`, then `ENGINE_PROFILE=dev node scripts/build.mjs --only-wasm`. Both are green.
- [x] T022 `make lint`, both `lint-host` and `lint-wasm`, which also covers cache-browser, opfs, combat `--features wasm` and dice wasm. Fix every new clippy warning without `allow`, unless research.md records why one is needed.
- [x] T023 [US1] Smoke test: copy the dev engine over main's `dist/engine`, open a lit scene with `make dev`, and check that the canvas is not blank and the console shows no shader error. A blank canvas is the 0.18 failure (missing render half). If it happens, go back to T011.

**Checkpoint**: wasm builds, lint is green, and the board draws.

---

## Phase 3: Behaviour (US2)

**Goal**: every unit test passes, and the server's adjudication is
unchanged.

- [x] T030 [US3] Move the render probe to 0.20 (FR-007, contracts/render-probe.md):
  - In `plugins/render_probe.rs`, read the visible, extracted and phase counts from the Mesh2d sprite pipeline.
  - Log `ExtractedSprites` as text if it still exists.
  - Keep the line's labels, and replace the comments that name `extract_sprites`/`queue_sprites`.
  - Leave `EngineStats` unchanged (data-model.md).
- [x] T031 [US2] `cargo test -p thunderforge-canvas-core`. Each failure is examined first:
  - **A float-only difference from glam 0.33** (R15), for example `door_icon_tests.rs:204`, `grid.rs:611` or `camera.rs:231`: it may move to an approximate comparison with a tolerance of at most `1e-5` (FR-006). List each such test here, with the old and new values.
  - **Anything else** is a regression and is fixed in code.
- [x] T032 [US2] `cargo test -p thunderforge-engine`, under the same rule as T031.
- [x] T033 [US2] `RUST_MIN_STACK=16777216 cargo test -p thunderforge-server` and `cargo test -p thunderforge-combat`. The server's movement and combat (reach, budget, redaction) tests pass unchanged. A changed result in them is a finding for the owner, not something to tune (R15).
- [x] T034 [P] [US2] Add a test in `crates/thunderforge-combat` that runs the same reach and budget fixtures as the server's tests and asserts the results bit for bit. Run it with `cargo test` (host) and through the combat wasm build. This catches FMA disagreement between x86_64 and wasm32 (R15). If such a fixture already exists, point to it here instead.
- [x] T035 [P] [US2] `pnpm -F @thunderforge/web test` and `pnpm -F @thunderforge/web typecheck`.
- [x] T036 [US2] Scene switch: confirm `scene_transition.rs:203` `unload_previous_scene` still empties the old scene under 0.20's despawn. Expect no change. `despawn_all` is listed under Later.

**Checkpoint**: all unit tests are green. Any test that was changed is
listed in T031 and T032.

---

## Phase 4: Visuals (US1)

- [x] T040 [US1] Stacked tokens (R9, FR-009):
  - Capture three stacked tokens with one selected, and compare with `baseline/stack.png`.
  - Click the stack, and check that the token picked is the one drawn on top.
  - If the order changed, give each token a stable z offset within `CanvasLayer::Tokens` (below the handles' +2.0), derived from the same key as `token_stack.rs`. Add a test in the engine for the offset, and recheck.
- [x] T041 [US1] Colour (R8, FR-008):
  - Capture the known sprite and compare it per channel with `baseline/sprite.png`.
  - If it is out by more than 1/255, set the 0.19.1-equivalent `Tonemapping` on every `Camera2d` spawn (16 in 11 files, research.md R8), and recheck.
- [x] T042 [P] [US1] Darkness: capture the lit scene and compare it with `baseline/darkness.png`. Falloff, wall shadows and the edge of sight should be the same by eye.
- [x] T043 [P] [US1] Text: capture nameplates and a dice readout, and compare with `baseline/text.png`. Check the size, position and order against the sprites (`Text2d` is still on the old backend).
- [x] T044 [P] [US1] `RenderDebugOverlay`: confirm that it is off by default under `DefaultPlugins` (`startup.rs:83`) and draws nothing. If it is on, or adds weight, `.disable::<>()` it.
- [x] T045 Save the four 0.20 captures beside the baseline in `specs/087-bevy-0-20/baseline/` with the suffix `-020`, and record the result of each comparison in research.md under `## Result`.

**Checkpoint**: SC-002 holds.

---

## Phase 5: Perf and slices (US3, US4, proof)

- [x] T050 [US3] Build a release engine, copy it over main's, and run `THUNDERFORGE_DISABLE_AUTH_RATE_LIMIT=1 pnpm e2e:engine-limits` three times. Record the medians next to the baseline in research.md under `## Result`. SC-003 requires at least 95% of baseline fps and at most 105% of baseline frame time at every level. If it fails, stop and report the levels to the owner (Open item 3).
- [x] T051 [P] [US4] Measure the release `.wasm` raw and brotli. Record it next to the baseline with the difference. SC-004 allows at most 10% growth. If it is above that, report it and pause for the owner (Open item 4).
- [x] T052 [P] [US3] Render-probe lines on the same scene. Compare with the baseline per contracts/render-probe.md (SC-006).
- [x] T053 Run the release-engine slices: `pnpm e2e:engine-limits` (from T050) and `pnpm e2e:resumable-downloads`, both the standalone and the integration part.
- [x] T054 Copy a dev engine across, then run each of these slices, all green:
  - `pnpm e2e:engine-other`, which holds the darkness shader;
  - `pnpm e2e:canvas`, which holds the render probe, the camera and `canvas-engine-stopped.spec.ts` (the panic hook);
  - `pnpm e2e:tokens`;
  - `pnpm e2e:lighting`, which holds vision and darkvision;
  - `pnpm e2e:scenes`, which holds fog in `scene-exploration.spec.ts`, and the scene switch;
  - `pnpm e2e:combat`;
  - `pnpm e2e:rolls`, both parts;
  - `pnpm e2e:worlds`.

  Use `THUNDERFORGE_DISABLE_AUTH_RATE_LIMIT=1` and `--workers=1` on the external stack.

  **Done** (2026-10-09, quiet machine): every slice above is green. The dev engine keeps line tables only, the dice frame-time check runs on a release engine, and `look-at-and-follow` waits for the engine probe (160d13fd). `scene-live-launch` fails only from its test, which main fixed in 0add03cc; with that version it passes on 0.20. See research.md, "Final dev-engine proof".

- [x] T055 `pnpm e2e:which --diff` against `main`. Run each slice it names that T053 and T054 did not run. Record in research.md that it asks for the full suite because of `Cargo.lock`, and that the slices stand in for it (Open item 1).
- [x] T056 `make lint` and the units of T031–T035, again, on the final tree.

**Checkpoint**: SC-001 to SC-006 are recorded in research.md under
`## Result`.

---

## Phase 6: Docs and version references (US5)

- [x] T060 [P] [US5] Update the version references (FR-015):
  - `crates/thunderforge-canvas-core/Cargo.toml:19` becomes "bevy 0.20 uses glam 0.33";
  - in `crates/thunderforge-engine/Cargo.toml`, the comments that name 0.18 (line 81) and the size note (the new brotli figure from T051);
  - `apps/engine-sandbox/README.md:119`;
  - the code comments that name a Bevy version as current: `cached_assets.rs:23`, `render_probe.rs:26` and `:241`, `startup.rs:68`. Keep a comment's version where it records history, such as "since 0.18".
- [x] T061 [P] [US5] Add a short **Upgrading Bevy** section to `docs/CONTRIBUTING.md`:
  - bevy and canvas-core's glam move together;
  - `cargo tree -d` must show one glam and one wgpu;
  - shaders are WESL (`.wesl`, `import …;`);
  - the render halves (`*_render` features) and the blank-canvas symptom;
  - take a baseline first (quickstart.md);
  - copy the worktree engine over main's `dist/engine`.
- [x] T062 Confirm `git grep -n "0\.19" -- crates/thunderforge-engine crates/thunderforge-canvas-core apps/engine-sandbox docs/CONTRIBUTING.md` shows no stale reference.
- [x] T063 Set spec.md **Status** to "Implemented". Record the final numbers in spec.md under **What exists** for the next upgrade.
- [ ] T064 Merge: `mcp__gitops__merge_ff_only` from main onto `087-bevy-0-20`. If it refuses because 086 has landed, stop and report (Open item 2). Do not rebase without the owner. Afterwards, rebuild main's own engine (`node scripts/build.mjs --only-wasm`) and remove the worktree.

---

## Dependencies & Execution Order

- **Phase 0** comes before everything else. The baseline is meaningless
  once a version has moved.
- **Phase 1 → 2 → 3 → 4 → 5 → 6**, in order.
  - In Phase 3, T030 comes before T032, and T031–T035 can run in any order.
  - Phase 4 needs the T023 smoke test.
  - Phase 5 needs Phases 3 and 4 to be green.
- **e2e runs** (T004, T050, T053–T055) take the e2e lock, which they
  share with spec 086 in main. Each copy over main's `dist/engine` waits
  for any 086 run to finish, and main's engine is rebuilt at T064.

## Parallel Opportunities

- T005, T006 and T007 after T003.
- T034 and T035 in Phase 3.
- T042, T043 and T044 in Phase 4.
- T051 and T052 in Phase 5.
- T060 and T061 in Phase 6.

## Implementation Strategy

1. **Baseline first, and committed.**
2. **Compile, host then wasm, with lint green.** Most of the real work is
   the shader port (T020) and the probe (T030).
3. **Prove behaviour with unit tests, then visuals with captures, then
   perf and slices against the baseline.**
4. **Docs last, then the fast-forward.** To roll back, revert the merge.
