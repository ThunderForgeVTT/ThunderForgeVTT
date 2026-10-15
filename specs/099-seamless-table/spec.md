# Feature Specification: A Seamless Table — Gliding Tokens and a Real Clock

**Feature Branch**: `landing-tavern-hero` (engine work the landing's walk needs)
**Created**: 2026-10-15
**Status**: Draft
**Input**: The owner, after spec 098:

> "can we fix this by providing smooth square transitions? for the engine
> snapping? i want to make the clock real and allow seamless transitioning
> the engine"
>
> "do it as a new spec"

## Overview

Two seams show on the table today.

1. **Tokens hop.** A snapped token jumps from one square's centre to the
   next. A WASD step, a teammate's move arriving from the server, a drop
   after a drag, and the landing's walker crossing the page all teleport a
   square at a time.
2. **Night is faked from outside.** The landing drives the dark with
   separate commands: an ambient shade, a day mat swapped for a night mat
   at a threshold, a grid colour flipped at the same threshold, and torch
   brightness stepped in 1/32nds by a JavaScript fade loop. Each is its own
   seam, and no table in the app has a time of day at all.

This spec makes both seamless inside the engine:

- **The glide.** A token's position stays exactly where the rules put it,
  on its square. Only the picture travels there, and it travels smoothly.
- **The clock.** The engine owns a time of day. One daylight curve gives
  the darkness and its colour. Lights marked as night lights rise and fall
  with it, and the night map fades in over the day map. Moving the clock,
  by any amount over any time, is one command, and the engine eases it.

The clock is world state: a Game Master sets it per scene, and every
client at the table sees the same time (US5). The landing drives the same
clock from its scroll.

## User Scenarios & Testing

### US1 — A token glides to its square (P1)

A token that moves one or a few squares slides there instead of
appearing there.

**Acceptance**:
1. A WASD step: the drawn token is between the two squares' centres
   partway through, and on the new centre within the glide time.
2. Rules, sight, tests and the server read the token's position. It is
   on the new square in the frame the move happened, never in between.
3. A move further than 4 squares (a scene load, a teleport) jumps.
4. Under reduced motion every move jumps, as today.
5. A token that keeps moving, such as the landing's walker crossing a
   square every few frames, moves at an even pace with no pause on each
   square.
6. The nameplate, bars and markers travel with the drawn token. So does a
   light the token carries: the pool does not run ahead of its bearer.
7. A move of less than a quarter square, such as a free drag with snapping
   off, follows at once: the glide never makes a drag lag.

### US2 — The engine keeps the time of day (P1)

**Acceptance**:
1. `set_time_of_day { hour, overSecs? }` sets the clock. Without
   `overSecs` the clock jumps (scene load). With it the clock moves there
   over that long, at an even pace.
2. The darkness and its tint follow one daylight curve over the hour: day
   holds from 07:00 to 18:00, dusk falls to 20:00, the night deepens to
   midnight, holds, and dawn breaks from 05:00 to 07:00.
3. The tint blends in a perceptual colour space: dusk passes through amber
   to night blue, and is never a muddy grey.
4. No frame changes the darkness by more than 0.02, however the clock is
   driven.
5. Hours above 24 are the next day: from 22:00 to 30:00 is through the
   night to dawn, not back through the afternoon.
6. A scene with no time of day draws as it does today: `set_ambient_light`
   still works, and an explicit `shade` still wins.
7. The engine tells the page the darkness as it changes (in steps of
   0.01), so the page's own colours can follow without guessing.

### US3 — Night lights and the night map follow the clock (P2)

**Acceptance**:
1. A light sent with `byNight: true` burns at its intensity by night, is
   out by day, and rises and falls smoothly through dusk and dawn.
2. `set_scene_background` takes an optional `nightImagePath`. The night map
   fades in over the day map as the darkness rises. There is no swap and
   no flash.

### US4 — The landing runs on the clock (P2)

**Acceptance**:
1. The landing maps scroll to an hour (day at the top, dusk after the
   tavern, night by the dungeon) and sends it with a short `overSecs`.
2. The landing's own dusk fade loop, its 1/32 torch steps, its mat swap
   and its grid flip are gone. The torches are night lights, and the mat
   is a day/night pair.
3. 098's SC-006 holds: no 100px of scroll changes the darkness by more
   than 0.08.
4. The walker glides square to square.

### US5 — A Game Master sets the table's time (P3)

**Acceptance**:
1. A scene has a time of day, stored with it. A scene made before this
   spec has none and draws as it did.
2. A Game Master sets it, either straight away or by passing time, for
   example "8 hours over 10 seconds". Every client at the table sees the
   same clock and the same transition.
3. A player who joins mid-transition lands at the same hour as everyone
   else.
   *Decided at T010*: the scene stores the hour it is going to, so a player
   who joins mid-transition lands at the end of it at once. A transition
   lasts seconds (at most 600), so the gap is that long and no longer.

## Requirements

- **FR-001**: A token's logical position (its `Transform`) never glides.
  The glide is a drawn offset only.
- **FR-002**: A glide takes as long as the time since the token's last
  hop, held between 0.08s and 0.25s. A token stepping steadily therefore
  moves at its own pace, and a single step takes about 0.15–0.25s.
- **FR-003**: There is no glide for a hop under 0.25 squares or over 4
  squares, for a new token, or under reduced motion.
- **FR-004**: The daylight curve, its blending and the clock's easing live
  in `thunderforge-canvas-core` (`daylight.rs`) as pure, deterministic,
  unit-tested functions. The engine only applies them.
- **FR-005**: The clock's easing is per second, not per frame, so the
  transition takes the same time at 30fps and 144fps.
- **FR-006**: The order of authority for darkness: explicit `shade` from
  `set_ambient_light`, then the clock, then the ambient level.
- **FR-007**: The night map is a second sprite over the day map, with its
  alpha set to the clock's night factor. It is loaded only when named.
- **FR-008** (US5): The time of day is stored on the scene, set through
  GraphQL, and broadcast to the table like other scene changes. The
  engine is told by the same `set_time_of_day` command.

## Success Criteria

- **SC-001**: Engine test: a one-square step drawn at half its glide time
  is within 0.2 squares of halfway, and the logical position is on the
  new square.
- **SC-002**: Engine test: a token stepping once every 0.15s draws moving
  at an even speed (no frame's travel more than twice the mean).
- **SC-003**: Core tests: the curve is continuous, its darkness is 0 at
  noon and at least 0.98 at midnight, and no 0.1 hour changes the
  darkness by more than 0.06.
- **SC-004**: Engine test: driving the clock 12 → 24 over 1s at 30fps and
  at 144fps gives the same darkness at 0.5s (±0.01), and no frame changes
  it by more than 0.02.
- **SC-005**: Landing e2e (desktop): the walker's drawn position changes on
  every frame while it crosses a square. 098's SC-006 still passes.
- **SC-006**: US5 e2e: a GM sets 22:00 over 2s, and a player's engine
  reaches 22:00 too.
