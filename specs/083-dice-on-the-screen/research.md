# Research: Dice on the Screen

Every decision below was checked against the code on 2026-10-07.

## R1. How the dice are drawn in a 2D engine

**Decision**: one `Mesh2d` per drawn die, built from canvas core's vertex
tables when the die spawns. It has a white `ColorMaterial`
(`bevy::sprite_render`) and a per-vertex `ATTRIBUTE_COLOR`. Each frame, the
plugin does the following:

1. Rotates the die's vertices by its current orientation, a `Quat` from
   canvas core.
2. Projects them orthographically: drop z, then scale to the die's on-screen
   size.
3. Culls the faces whose rotated normal points away, `n.z <= 0`.
4. Shades each kept face by `n · L`, against a fixed light from the upper
   left.
5. Writes positions, colours and indices into the same mesh through
   `Assets<Mesh>::get_mut`.

When the throw is despawned, its handle is dropped, so at most 20 meshes
exist, and only while a throw plays.

**Why**:

- The engine has one `Camera2d` (`plugins/camera.rs:35`), and several
  systems look it up as the only one.
- Bevy is built with `default-features = false`, and `bevy_pbr` is not
  enabled. `bevy_sprite_render`, which provides `Mesh2d` and `ColorMaterial`,
  is enabled. `plugins/darkness.rs` already draws a `Mesh2d`.
- Every die is convex, so culling by normal is a correct hidden-face test.
  It needs no depth buffer.

**Alternatives rejected**:

- A `Camera3d` with PBR, rendering to a texture. That needs new Bevy
  features and a second camera, and grows the bundle.
- Sprite sheets of pre-rendered dice. That needs assets, and FR-006 forbids
  textures.
- A new mesh asset each frame. `systems/shape.rs` documents that per-frame
  `Assets<Mesh>` inserts leak, which is why shapes use sprite chains.
  Mutating one mesh in place avoids that.

## R2. How the engine is told about a roll, and why masked rolls never play

**Decision**: `startRollSync`'s `animate` option widens from
`(dice: {finalValue}[])` to `(roll: WorldRollRecord)`. It is still called
only for `entry.__typename === "WorldRoll"` within `REPLAY_WINDOW_MS`, as
today (`apps/web/src/engine/world/sync/rolls.ts`).

`triggerDiceRollAnimation(roll)` builds one `trigger_dice_roll` command from
that record (contracts/engine-dice.md). It carries the id, roller name,
label, formula, bindings, result kind and value, and every die's sides,
rolls, steps, kept and final value. It still appends the final values to
`dicePlayed`, so FR-018 holds unchanged.

**Why**:

- A `MaskedRoll` has no `resolution`, and it cannot be built from a roll's
  row (`types_rolls.rs`, `MaskedRoll::new`). A GM only roll is never
  delivered to a player (`rolls/visibility.rs`, `event_reaches`). The
  web passes on only what it was given, and it is given nothing for those
  rolls, so a board cannot draw them.
- The sync already holds the whole `WorldRoll`, so it fetches nothing more.

**Alternatives rejected**:

- The engine subscribing to roll events itself. That puts network code in
  the engine (AGENTS.md §2).
- Sending only the roll id and letting the engine fetch it. The engine has
  no GraphQL client for rolls, and that adds a round trip.

## R3. The seeded tumble

**Decision**:

- **Seed.** The seed is FNV-1a 64 over the roll id's UTF-8 bytes. Each die
  gets `splitmix64(seed ^ index)`, a stream of `f32` in `[0, 1)`.
- **Inputs.** The stream picks:
  - an entry point on the bottom edge;
  - a resting place in a slot of the lower-third row, with a jitter of at
    most a quarter of a die;
  - a spin axis;
  - a whole number of turns, from 2 to 4;
  - a spin about z for the landed face.
- **Orientation.** The orientation at time `t` is
  `landing * Quat::from_axis_angle(axis, turns·TAU·(1 − ease(t)))`, where
  `ease` is an out-cubic with a small overshoot on the position, not the
  rotation. At `t = 1` it is exactly the landing orientation, so the landed
  face is always the server's.
- **Landing.** The landing orientation is the shortest rotation that takes
  the face's outward normal to `+z`, toward the viewer, followed by the
  seeded spin about `+z`.
- **Faces.** A d100 is two d10s, which share the die's index and take
  sub-streams 0 and 1.

`restingPlace` in the probe is reported in screen pixels, rounded to whole
pixels, relative to the throw's anchor, so it does not depend on the camera.

**Why**:

- The tumble is a pure function of the id and the index (FR-008), so every
  board plays the same throw.
- It runs on host tests in canvas core, which hold the same path twice and
  the landed face for every value of every die.
- Wasm `f32` arithmetic is IEEE and deterministic. Rounding the probe to
  pixels removes any last-bit difference between browsers.

**Alternatives rejected**:

- `rand` with a seeded RNG. It adds a dependency for 20 numbers.
- Physics with the result forced at the end. That is not deterministic
  across frame rates, and the spec rules it out.

## R4. The exploding die's `final_value`

**Decision**: out of scope. The throw draws every value in an exploded
die's chain, and from the second value on, each one is a new die of the same
kind that tumbles in beside the first (FR-011). The readout's addends are
the crate's own: each kept `DieOutcome` contributes its `final_value`, and
the total is the server's `resultValue`. Nothing in this spec changes how
the crate totals a roll.

**Why**:

- `eval.rs` sets `final_value = *rolls.last().unwrap()` after the explode
  loop (`crates/thunderforge-dice/src/eval.rs:303`). So `1d6x` rolling
  `[6, 6, 2]` totals 2 where most systems total 14.
- That may be a bug, but it changes totals, and with them stored rolls,
  verdicts and every system that uses `x`. It is a dice-crate change of its
  own, with its own tests. The existing explode tests are near `eval.rs:769`.
- The readout reports the crate's arithmetic, so the board never disagrees
  with the chat.

**Pointer**: a follow-up spec, "exploding dice total their chain", owns it.
It would sum the explosion steps into `final_value` and keep rerolls as
replacements. Step kinds (R5) are what make that change possible.

## R5. Chain step kinds and clamps

**Decision**: `DieOutcome` gains
`#[serde(default)] pub steps: Vec<ChainStep>`, where
`ChainStep { Reroll, Explode }`. It has one entry for each value in `rolls`
after the first. `eval_dice_term` pushes `Reroll` beside each push in the
`reroll_once` and `reroll_recursive` loops, and `Explode` beside each push
in the `explode_once` and `explode` loops (`eval.rs:266-301`).

A clamp is not stored. A die was clamped exactly when
`final_value != *rolls.last()`, because `apply_clamp` (`eval.rs:379`)
changes only `final_value`.

A stored die with no `steps` reads as all rerolls, as the spec's edge case
says.

**Why**:

- The two loops are distinct code paths, so the kind is known where the
  value is pushed.
- Deriving the clamp avoids a field that could disagree with the values.

**Spec correction**: FR-002 said the crate records "whether the final value
was clamped". It now says the clamp is derived. The Key Entities entry
already said so.

## R6. The readout's characters

**Decision**: the readout and every face label are ASCII.

- Separator: `Ayla: Stealth   17 + 5 = 22`, not `Ayla · Stealth`.
- Minus: `12 - 1 = 11`, not `12 − 1`.
- Fate faces: `+`, blank and `-`.
- A coin's faces read `H` and `T`. Its addend is the crate's value, `1` or
  `0`.
- A d10's 10 reads `0`. The d100 tens die reads `00` to `90`.

**Why**: the engine draws text with Bevy's `default_font`, the embedded
`FiraMono-subset.ttf`. `fc-query` reports its charset as `20-7e`, which is
printable ASCII only, so `·` and `−` would draw as missing glyphs. Shipping
a fuller font costs bundle size for two characters.

**Spec correction**: the readout examples, US4 scenario 3 and the Fate table
row now use ASCII.

## R7. The queue

**Decision**: canvas core's `ThrowQueue` holds at most one playing throw and
four waiting throws. Its operations:

- `push`: when four throws are already waiting, it removes the oldest
  waiting throw and returns it as skipped. The playing throw is never
  skipped.
- `landed()`: called by the engine when the playing throw's last die has
  landed. The next waiting throw starts then, while the previous one's
  readout holds and fades on its own (US5 scenario 1).
- `fading`: the playing throw passes to a list of fading throws, so two
  throws can be on screen at once: one fading, one tumbling.

Every skipped throw is written to the landed log with `skipped: true` and no
dice.

**Why**:

- A pure state machine tests on the host: the order, the skip rule, and
  never skipping the playing throw.
- Starting the next throw on landing, not on fade, keeps an initiative
  burst moving.

**Alternatives rejected**: playing every throw at once in a second row. That
puts dice over dice.

## R8. Screen anchoring

**Decision**: every throw entity is a child of one `DiceStage` entity. Each
frame, a system after the camera systems copies the `Camera2d`'s
translation, x and y, into the stage. It sets the stage's z to 950, above
`darkness_z` and the fog layer (`resources/canvas_layer.rs`), and its scale
to the orthographic projection's `scale`. Positions inside the stage are in
screen pixels.

The visible height comes from the camera's viewport, through `camera_state()`
(`plugins/camera.rs:160`). That decides where the lower third is.

The dice are never hit by a click. The engine has no `bevy_picking`: its
interaction systems hit-test tokens, walls and shapes by querying their own
components, and a throw entity carries none of those components.

**Why**: the throw stays fixed on screen at any pan or zoom (FR-009), and
the single camera stays.

## R9. Timings, and who owns them

**Decision**: the constants live in `dice_throw::TIMINGS`:

| Field       | Value | Meaning                          |
| ----------- | ----- | -------------------------------- |
| `tumbleMs`  | 1200  | the tumble                       |
| `stepMs`    | 500   | each reroll or explosion         |
| `holdMs`    | 2500  | how long the readout holds       |
| `fadeMs`    | 400   | the fade                         |
| `reducedMs` | 150   | the fade-in under reduced motion |

The engine exports them as `dice_timings() -> String`, in JSON.

`DiceRollerPanel` waits for `reducedMs` when the viewer prefers reduced
motion, and for `tumbleMs` otherwise. It reads both through
`engineDiceTimings()` in `apps/web/src/engine/bevy/diceThrow.ts`. When no
engine is loaded, the panel falls back to 1200 ms (FR-017).
`ANIMATION_REVEAL_MS` is deleted.

`SETTLE_DURATION_SECS` goes, replaced by `TIMINGS.tumble_ms`.

**Why**: one owner, as FR-017 asks. A wasm getter follows the existing
probe pattern (`camera_state`, `marked_tokens`).

## R10. The probe, `diceLanded()`

**Decision**: `plugins/dice/probe.rs` holds a static `OnceLock<Mutex<Vec<…>>>`
of landed and skipped throws, capped at 50 entries with the oldest dropped.
The exported `dice_landed() -> String` returns it as JSON
(contracts/engine-dice.md). The throw's systems write an entry when its last
die lands, or when the queue skips it. `installEngineProbe` exposes the
entries as `__engineProbe.diceLanded()`.

**Why**:

- This is the pattern of `sight_probe`, `marked_tokens` and `camera_state`
  (`systems/lighting_vision.rs:220-241`).
- The probe reads what the engine drew.

**Spec correction**: FR-019 said "through the event callback". The engine
reports through a wasm getter, which the probe calls. The event callback
(`sdk.rs` `set_event_callback`) carries user actions to the web, and a
probe entry is not one.

## R11. Reduced motion

**Decision**: `diceThrow.ts` exports
`watchReducedMotion(send: (on: boolean) => void): () => void`. It reads
`matchMedia("(prefers-reduced-motion: reduce)")`, sends the current value
once, and sends again on `change`. `WorldPage.tsx` starts it once the engine
is ready, and sends `{type: "set_reduced_motion", reduced}`.

Under reduced motion, a throw's dice spawn at their landing orientation and
resting place, the whole throw fades in over `reducedMs`, and the struck
values and extra dice appear at once.

**Why**: Playwright's `reducedMotion: "reduce"` drives `matchMedia`, so the
e2e proves it. The pattern already exists in `engine/lookAt.ts` and
`components/engine/BoardLoading.tsx`.

## R12. Where the engine's logic is tested

**Decision**: the shapes, landing, tumble, queue and readout go in
`crates/thunderforge-canvas-core/src/dice_throw/`, with `*_tests.rs`
siblings (the `door_icon.rs` pattern). Canvas core gains a dependency on
`thunderforge-dice`, which is pure and wasm-compatible, for `Breakdown`.

**Why**: `plugins/frame_trace.rs` records that the engine crate cannot
compile for the host, because winit has no backend under its features. A
`#[test]` in the engine never runs.

**Spec correction**: the Proof said engine tests are "run for wasm32 as the
engine lints". They are canvas core tests run on the host. The engine is
linted for wasm32.

## R13. Measuring the bundle

**Decision**: compare the brotli size of `dist/engine/engine_bg.wasm` before
and after a release build:

```sh
node -e "const z=require('zlib'),f=require('fs');console.log(z.brotliCompressSync(f.readFileSync('dist/engine/engine_bg.wasm')).length)"
```

**Why**: `pnpm bundle:check` measures the web routes, not the engine wasm.

**Spec correction**: SC-006 said the existing bundle report measures it. It
names this measurement instead.

## R14. A mesh for shapes

**Spec correction**: "What exists" said `Mesh2d` is already used for shapes.
It is used only by darkness. `systems/shape.rs` draws shapes as sprite
chains on purpose, because per-frame mesh inserts leaked. R1 explains why
this plan's meshes do not leak.
