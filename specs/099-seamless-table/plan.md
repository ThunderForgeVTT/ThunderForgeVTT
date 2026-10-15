# Implementation Plan: A Seamless Table

**Branch**: `landing-tavern-hero` | **Date**: 2026-10-15 | **Spec**: [spec.md](./spec.md)

## Core (`thunderforge-canvas-core`)

- `glide.rs`: the glide's arithmetic.
  - `Glide { offset, speed, since_hop }`.
  - `Glide::hop(delta, cell, reduced)` starts or extends a glide.
  - `Glide::step(dt)` shrinks the offset at `speed`.
  - Every FR-002/FR-003 rule lives here, unit-tested.
- `daylight.rs`: the clock's arithmetic.
  - `sky_at(hour) -> Sky { darkness, tint }` from keyframes. Darkness is
    smoothstepped between keys; tint is blended in OKLab.
  - `night_glow(hour)`: 0 by day, 1 by night, for night lights and the
    night map.
  - `Clock { hour, goal, rate }` with `set(goal, over_secs)` and
    `step(dt)`. Even pace, and the frame cap applies to darkness.

## Engine (`thunderforge-engine`)

- **Glide** (`systems/token_glide.rs`):
  - `GlideSet: Local<HashMap<Entity, (Vec2 last, Glide)>>`.
  - `track_token_hops` runs in PostUpdate after snapping and before
    transform propagation. It compares each token's `Transform` with the
    last one and calls `hop`.
  - `draw_glides` runs after `TransformSystems::Propagate`. It rewrites
    the `GlobalTransform` of a gliding token and its descendants from
    their `Transform` plus the offset. Nothing accumulates, because each
    write is computed from scratch. It writes once more when the offset
    reaches zero.
  - Carried lights take their position from the drawn token
    (`GlobalTransform`), not the logical one.
  - `token_footprints` adds `drawnX`/`drawnY`.
  - Reduced motion: `SetReducedMotion` also sets a `ReducedMotion`
    resource, which the glide reads.
- **Clock** (`plugins/daylight.rs`):
  - `TimeOfDay(Option<Clock>)` resource and `set_time_of_day` command.
  - `step_clock` advances it by `Time::delta_secs`.
  - `sync_darkness` reads `sky_at(clock.hour)` when no explicit shade is
    set (FR-006).
  - `NightLights(HashSet<String>)`, filled from `upsert_light`'s
    `byNight`, scales intensity by `night_glow`.
  - `emit_daylight` sends `{type:"time_of_day", hour, darkness}` when
    darkness moves 0.01.
  - `SceneBackground.night_path`: a `NightBackgroundSprite` over the day
    sprite, with alpha = `night_glow`.

## Landing (US4)

- `journey.ts`: `hourAt(scroll)` replaces `duskAt`. 12:00 until
  `dayEnds`, 18:00 → 20:00 over `FALL_PX`, 20:00 → 24:00 by `nightAt`.
  The slope cap is kept by bounding hours per pixel against the curve's
  steepest darkness per hour (core test).
- `PageTable.tsx` sends `set_time_of_day {hour, overSecs: 0.25}` and
  reads `time_of_day` events for `html.dusk` and `data-dusk`. The fade
  loop, the `torch(i, glow)` steps, and the mat/grid swaps are removed.
  The torches and fire become `byNight`, and the mat sends
  `nightImagePath`.

## Server and app (US5)

- Migration: `scenes.time_of_day REAL NULL`.
- GraphQL: `setSceneTimeOfDay(sceneId, hour, overSecs)`, Game Master only.
  It broadcasts on the scene's change channel.
- `apps/web`: the world page sends `set_time_of_day` from the scene and
  from the broadcast. The GM gets a control in the scene tools.

## Proof

- Core: `cargo test -p thunderforge-canvas-core glide daylight`.
- Engine: `cargo test -p thunderforge-engine token_glide daylight`, and
  wasm clippy.
- Landing: `walk-along`, `a11y`, `page-mat`, and the phone/tablet set.
- US5: the web e2e slice for scenes.
