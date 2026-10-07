# Feature Specification: Dice on the Screen

**Feature Branch**: `083-dice-on-the-screen`
**Created**: 2026-10-07
**Status**: Draft
**Input**: The owner, 2026-10-07: "can we improve the dice on the screen to an on screen animation of like a dice of that variety plus its counts rolling and then it determines the dice and then you see your bonuses?" Then, choosing between options: "3D dice in the engine". Bevy draws a real d4, d6, d8, d10, d12 or d20, one for each die rolled. Each die tumbles and lands on the server's result, and then the bonuses and the total appear. The landing is scripted to the result; it is not physics.

## Why

Since spec 081, every roll at the table plays on every board. What plays is
not dice. It is a row of flat yellow squares in the middle of the world, each
with its final number printed on it from the first frame. They wobble for a
second and stay there. They are never removed, and the next roll replaces
them.

That is not a roll anyone watches. The number is known before the squares
stop moving, so there is no moment of suspense. A d20 and a d6 look the same,
so nobody can tell what was rolled. The squares sit at the world's origin, so
on a large map they are off screen, or under a token. The bonus never shows,
so "17" on the board and "22" in the chat look like two different rolls.

A roll should look like dice thrown onto the table:

- the right dice, as many as were rolled;
- tumbling, then landing face up on the numbers the server rolled;
- then the arithmetic, `17 + 5 = 22`, so everyone can see where the total
  came from.

The server still decides every number. The engine only stages the throw.

## What exists

Counted on 2026-10-07:

- **The engine.** `crates/thunderforge-engine/src/plugins/dice_roll.rs`:
  - receives `trigger_dice_roll` with `dice: [{ finalValue }]` and nothing
    else (`sdk.rs:314`). It does not know the sides, the history, which dice
    were kept, the formula or the total;
  - spawns one 48 px yellow `Sprite` per die, with a `Text2d` of the final
    value, at world `(x, 0, 900)`, centred on the world origin;
  - wobbles them for `SETTLE_DURATION_SECS = 1.2`, then leaves them in
    place. They are despawned only when the next roll arrives.
- **The engine is 2D.** It has one `Camera2d` (`plugins/camera.rs:35`). Bevy
  runs without `bevy_pbr` or a 3D camera. `Mesh2d` is already used for shapes
  (`systems/shape.rs`) and darkness (`plugins/darkness.rs`). Several systems
  look up the camera as the single `Camera2d`. The release build is about
  4.15 MB brotli.
- **The engine already links the dice crate**
  (`crates/thunderforge-engine/Cargo.toml:49`), so it can parse a formula.
- **The web side.**
  - `triggerDiceRollAnimation(dice: { finalValue }[])`
    (`apps/web/src/engine/bevy/index.ts:1768`) forwards the values and
    appends them to `dicePlayed`.
  - `__engineProbe.dicePlayed()` returns `number[][]`. It is the e2e's
    window onto the board, read by five `rolls-*` specs and the demo's
    `rolls-across-tabs.spec.ts`.
  - `startRollSync` (`engine/world/sync/rolls.ts`) animates a whole roll
    from its event, within `REPLAY_WINDOW_MS = 4000`. A masked roll never
    animates (spec 081).
  - `DiceRollerPanel.tsx` waits `ANIMATION_REVEAL_MS = 1200` before showing
    its own result, hand-copied from the engine's constant.
- **What a roll carries.**
  - Each die is a `DieOutcome { sides, rolls, kept, final_value }`
    (`crates/thunderforge-dice/src/lib.rs:54`). `sides` is `Numeric(n)`,
    `Fate` or `Coin`.
  - `rolls` is the die's whole chain, but it does not say why each value
    after the first is there. A reroll (`r`, `rr`) and an explosion (`x`,
    `xo`) are pushed the same way (`eval.rs:274-301`).
  - `min` and `max` clamps change `final_value` without adding to `rolls`.
  - `WorldRoll` has `rollerName`, `label`, `formula` and `resolution`. It
    has no bindings. A check's formula keeps its placeholders, such as
    `1d20 + MODIFIER`. The values are stored separately, in
    `world_roll_records.bindings` (`mutations_roll_check.rs:297-313`), and
    are not exposed.

## The throw

A roll plays in three beats, the same on every board.

1. **Tumble.** One die for each die rolled enters from the bottom edge of the
   board and tumbles to a resting place in the lower third. Each die is the
   polyhedron of its kind, turning in three dimensions, with lit and shaded
   faces. While a die is moving, its faces show no numbers.
2. **Land.** Each die settles with the server's face toward the viewer, and
   that face's number appears. A die that was rerolled lands, shows its first
   number struck through, and tumbles again to its next value. A die that
   exploded lands, and a new die of the same kind tumbles in beside it. A die
   that keep/drop dropped turns dim.
3. **Readout.** Above the dice, a line appears:
   `Ayla · Stealth   17 + 5 = 22`. That is the roller, the label if there
   is one, the kept dice and bonuses as arithmetic, and the total. The
   readout and the dice hold, fade together, and are removed.

| Beat    | Duration                              |
| ------- | ------------------------------------- |
| Tumble  | 1.2 s (`SETTLE_DURATION_SECS`, kept)  |
| Reroll  | 0.5 s more per reroll or explosion    |
| Readout | appears on landing, holds 2.5 s       |
| Fade    | 0.4 s, then every entity is despawned |

The tumble's path, spin and resting place come from a seed taken from the
roll's id. The same roll therefore looks the same on every board, and a test
can predict where each die rests.

### Which die is drawn

| Die                             | Drawn as                                                        |
| ------------------------------- | --------------------------------------------------------------- |
| d4, d6, d8, d12, d20            | Tetrahedron, cube, octahedron, dodecahedron, icosahedron        |
| d10                             | Pentagonal trapezohedron, faces 1–10 (10 shown as `0`)          |
| d100                            | A percentile pair of d10s: tens (`00`–`90`) and units (`0`–`9`) |
| d3                              | A cube whose faces read 1–3 twice                               |
| d2, coin                        | A disc that flips                                               |
| Fate (`dF`)                     | A cube whose faces read `+`, `+`, blank, blank, `−`, `−`        |
| Any other size (d5, d7, d30, …) | A disc with the number, which flips like a coin                 |

A d4 shows its number on the face toward the viewer, like every other die.
The convention of reading the top vertex is not drawn.

## User Scenarios & Testing

### User Story 1 - A d20 tumbles and lands on the server's number (Priority: P1)

A player rolls `1d20 + 5`. A d20 tumbles onto every board at the table and
lands with the server's face up, say 17. Then `17 + 5 = 22` appears above it.

**Why this priority**: This is the owner's ask. Every other story is a
variation on it.

**Independent Test**: One member rolls `1d20 + 5`. The probe reports that
the engine drew one 20-sided die, that it landed on the server's value, and
that the readout reads `<value> + 5 = <total>`.

**Acceptance Scenarios**:

1. **Given** a member on the play view,
   **When** they roll `1d20 + 5`,
   **Then** the board draws one icosahedron. It lands on the server's
   `finalValue`, and the readout shows that value, `+ 5` and the server's
   total.
2. **Given** the board is panned or zoomed far from the world origin,
   **When** a roll plays,
   **Then** the dice appear in the lower third of the visible board, at the
   same size on screen at any zoom.
3. **Given** a roll has finished,
   **When** its fade ends,
   **Then** no dice or readout entities remain in the engine.
4. **Given** two boards at the table,
   **When** the same roll plays on both,
   **Then** both land each die on the same face, at the same resting place.

---

### User Story 2 - Several dice, of several kinds (Priority: P1)

A player rolls `2d6 + 1d8 + 3` for a greatsword with a flaming enchantment.
Two cubes and an octahedron tumble in. Each lands on its own number, and the
readout reads `4 + 6 + 7 + 3 = 20`.

**Why this priority**: "Of that variety plus its counts" is the ask. Damage
rolls are routinely mixed.

**Independent Test**: Roll a mixed formula. The probe lists each die's sides
and landed face, in the resolution's order, and matches the server.

**Acceptance Scenarios**:

1. **Given** `2d6 + 1d8 + 3`,
   **When** it plays,
   **Then** the probe reports three dice with sides `[6, 6, 8]` that landed
   on the server's values, and a readout of `a + b + c + 3 = total`.
2. **Given** `1d100`,
   **When** it plays,
   **Then** a tens d10 and a units d10 tumble, reading together as the
   server's value (`00` and `0` for 100). The probe reports one die of
   sides 100.
3. **Given** `4dF`,
   **When** it plays,
   **Then** four Fate cubes land on `+`, blank or `−` as the server rolled,
   and the readout reads the total.
4. **Given** `1d7`,
   **When** it plays,
   **Then** a disc flips and lands showing the server's number.

---

### User Story 3 - Advantage, rerolls and explosions show what happened (Priority: P1)

A player rolls `2d20kh1 + 4` with advantage. Two d20s land. The lower one
dims, and the readout uses only the higher one. Another player rolls
`2d6r<3` for Great Weapon Fighting. A die that landed on 1 shows the 1 struck
through, tumbles again and lands on its reroll.

**Why this priority**: Spec 084 makes advantage and rerolls routine in 5e.
A throw that hid them would make the total look wrong.

**Independent Test**: Roll formulas with keep/drop, reroll and explode
modifiers under a fixed seed, so the chains are known. The probe reports
each die's `kept`, its struck values and its final face.

**Acceptance Scenarios**:

1. **Given** `2d20kh1 + 4`,
   **When** it plays,
   **Then** both dice land, the dropped one is dimmed, the probe reports it
   `kept: false`, and the readout reads `<kept> + 4 = <total>`.
2. **Given** a die whose chain is `[1, 5]` from a reroll,
   **When** it plays,
   **Then** the die lands on 1, shows it struck through, tumbles again and
   lands on 5. The probe reports `rerolled: [1]` and `face: 5`.
3. **Given** a die whose chain is `[6, 6, 2]` from an explosion,
   **When** it plays,
   **Then** the first die lands on 6, a second d6 tumbles in and lands on 6,
   and a third lands on 2. All three show, and the readout counts them as
   the dice crate totals them.
4. **Given** `6d10cs>=8` (a success count),
   **When** it plays,
   **Then** the dice that succeed are drawn bright, the others are dimmed,
   and the readout reads `3 successes`.
5. **Given** a die clamped by `min2` from a 1,
   **When** it plays,
   **Then** it lands on 1, shows it struck through, and shows 2 without a
   second tumble.

---

### User Story 4 - Bonuses are named numbers, not placeholders (Priority: P2)

A player clicks Stealth on their sheet. The formula is `1d20 + MODIFIER`.
The readout reads `Stealth 13 + 3 = 16`. It does not read `13 + MODIFIER`,
and it does not read `16` alone.

**Why this priority**: "Then you see your bonuses" is the ask. Most rolls
at a 5e table come from the sheet, and those carry placeholders.

**Independent Test**: Roll a check from the sheet. The readout's bonus
equals the binding the server recorded.

**Acceptance Scenarios**:

1. **Given** a check rolled from the sheet,
   **When** it plays,
   **Then** each placeholder in the readout is replaced by its recorded value.
2. **Given** a formula the breakdown cannot express as a sum, such as
   `(1d6 + 2) * 2` or `{1d20, 1d20}kh1 + 4`,
   **When** it plays,
   **Then** the readout reads `formula = total`, with placeholders
   substituted, rather than wrong arithmetic.
3. **Given** a negative bonus,
   **When** it plays,
   **Then** the readout reads `12 − 1 = 11`, not `12 + -1`.

---

### User Story 5 - A busy table and a calm screen (Priority: P2)

At the start of a fight, five players roll initiative within a second. The
throws play one after another, so no throw lands on top of another. A player
who has asked their system to reduce motion sees each roll appear already
landed, with no tumble.

**Why this priority**: Initiative and area damage produce bursts. Motion
sensitivity is an accessibility requirement, not a preference.

**Independent Test**: Roll five times in quick succession, and check the
order and count of throws in the probe. Emulate `prefers-reduced-motion:
reduce`, and check that the dice land with no tumble.

**Acceptance Scenarios**:

1. **Given** a throw is playing,
   **When** another roll arrives,
   **Then** it starts when the first has landed. The first's readout fades
   as the second's dice enter.
2. **Given** four throws waiting,
   **When** a fifth arrives,
   **Then** the oldest waiting throw is skipped. It is never the one
   playing. Every skipped roll is still in the chat, and the probe reports
   it as skipped.
3. **Given** `prefers-reduced-motion: reduce`,
   **When** a roll plays,
   **Then** the dice appear landed, fade in over 150 ms, and the readout
   shows at once. Rerolls and explosions show their struck values and extra
   dice with no tumble.
4. **Given** `40d6`,
   **When** it plays,
   **Then** 20 dice tumble, a `+20 more` chip sits beside them, and the
   readout totals all 40.

### Edge Cases

- **A masked roll** (another player's GM's eyes roll) and a **GM only roll**
  for a player never play, as spec 081 already decides. Nothing in this spec
  sends their dice to a board that may not see them. A revealed roll plays
  for everyone, as spec 081's republish does.
- **The roller's own GM's eyes roll** plays on their own board and the GM's,
  and nowhere else.
- **A roll that arrives after the replay window**, or one that is only in
  the backscroll, does not play (spec 081 FR-012).
- **A scene change mid-throw** keeps the throw. It is anchored to the screen,
  not the scene. A throw is dropped only when the engine is torn down.
- **No engine on the page** (the sheet page, a board still loading): nothing
  plays and nothing waits for it. The dice roller panel still shows its
  result.
- **A roll stored before this spec** has no step kinds and no bindings. It
  plays with every extra value treated as a reroll, and a readout of
  `formula = total`.
- **Dark scenes and fog.** The dice and readout draw above darkness, fog of
  war and every board layer. They are lit by their own fixed light, not the
  scene's.
- **Clicks pass through.** The dice and readout are never the target of a
  click, drag or hover on the board.
- **Coin and Fate readouts** use the dice crate's values. A coin reads as
  its face label, and Fate sums `+1`, `0` and `−1`.

## Requirements

### Functional Requirements

**What the board is told**

- **FR-001**: `trigger_dice_roll` MUST carry the roll's id, roller name,
  label, formula, bindings, total and kind (total or success count), and,
  for each die, its sides, its whole chain, why each value after the first
  was rolled, whether it was kept, and its final value. The web side MUST
  build this from the `WorldRoll` it already fetched. It does not fetch
  anything more.
- **FR-002**: The dice crate MUST record, for each value in a die's chain
  after the first, whether it was a reroll or an explosion, and whether
  the final value was clamped. The field is additive and defaults when
  absent, so stored resolutions still read.
- **FR-003**: `WorldRoll` MUST expose the bindings the server recorded, as
  placeholder and value pairs. `MaskedRoll` MUST NOT gain them; it stays
  unable to carry a roll's content (spec 081 FR-004).

**The breakdown**

- **FR-004**: The dice crate MUST offer a pure breakdown of a resolved
  roll. It substitutes the bindings, and returns the kept dice and the
  constant terms as signed addends when the formula is a sum or difference
  of dice terms and constants. Otherwise it returns nothing. The engine
  computes the readout with it locally. The server sends no readout
  string.
- **FR-005**: A success-count roll's readout MUST read `N successes` (or
  `1 success`), and MUST mark which dice succeeded.

**The throw**

- **FR-006**: The engine MUST draw each die as the shape in "Which die is
  drawn". The meshes are built procedurally at start-up from vertex tables
  and rendered with `Mesh2d`, rotated in three dimensions and projected on
  the CPU, with each face shaded by its normal against a fixed light. The
  engine MUST NOT add `bevy_pbr`, a 3D camera, a glTF loader or textures
  for this.
- **FR-007**: Each die MUST come to rest with the face for its final value
  turned toward the viewer, decided by the engine from the face table. That
  face's number is drawn on landing, and no face shows a number while the
  die moves.
- **FR-008**: The tumble MUST be scripted. Its path, spin and resting place
  MUST be a pure function of the roll's id and the die's index, so every
  board plays the same throw.
- **FR-009**: The throw MUST be anchored to the screen. It is positioned
  each frame from the board camera's transform and scaled against its zoom,
  so it sits in the lower third of the visible board at a constant size on
  screen. The engine keeps its single `Camera2d`.
- **FR-010**: The throw MUST draw above every board layer, darkness and fog
  included, and MUST NOT be pickable.
- **FR-011**: Dropped dice MUST be dimmed. Rerolled and clamped values MUST
  show struck through before the final face. Explosions MUST add a die of
  the same kind (FR-002 decides which is which).
- **FR-012**: At most 20 dice MUST be drawn for one roll, in the
  resolution's order. Past 20, a chip MUST say `+N more`, and the readout
  MUST still count every die.
- **FR-013**: Every entity of a throw MUST be despawned when its fade ends.

**Several rolls**

- **FR-014**: Throws MUST play one at a time, in arrival order. A throw
  starts when the previous one has landed.
- **FR-015**: At most four throws MAY wait. When a fifth arrives, the oldest
  waiting throw MUST be skipped.

**Motion and timing**

- **FR-016**: The web side MUST pass the viewer's `prefers-reduced-motion`
  to the engine, and update it when it changes. Under `reduce`, dice MUST
  appear landed with no tumble (US5 scenario 3).
- **FR-017**: The engine MUST be the only owner of the throw's timings. It
  MUST export them, and `DiceRollerPanel` MUST read its reveal delay from
  that export, falling back to the current 1200 ms only when no engine is
  loaded. The hand-copied constant goes.

**The probe**

- **FR-018**: `__engineProbe.dicePlayed()` MUST keep its current shape and
  meaning, the final values per roll, so the existing `rolls-*` and demo
  specs pass unchanged.
- **FR-019**: A new `__engineProbe.diceLanded()` MUST return, for each
  throw the engine finished or skipped:
  - `{ rollId, skipped, readout, chip }`;
  - and, per die drawn, `{ sides, face, kept, rerolled, exploded,
  restingPlace }`.
    The engine reports these when they happen, through the event callback,
    so the probe reads what the engine drew rather than what the web sent.

**The demo**

- **FR-020**: The demo MUST send the same `trigger_dice_roll` payload. Its
  dice handler records step kinds and bindings as the server does. Its
  boards play the same throw.

### Key Entities

- **Throw**: one roll as the board plays it. It has the roll's id, a seed
  from it, the dice drawn, the readout and a state (waiting, tumbling,
  landed, fading, skipped). It is local to the engine and never synced.
- **Die shape**: a polyhedron's vertices, faces and the value printed on
  each face. It is built once, at start-up.
- **Breakdown**: a resolved roll as signed addends (kept dice values and
  substituted constants), or nothing when the formula is not a plain sum.
  It is derived data, computed where it is shown.
- **Chain step**: why a value in a die's chain was rolled: the original, a
  reroll or an explosion. A clamp is noted on the die.

## Success Criteria

### Measurable Outcomes

- **SC-001**: For every roll in the proof, every die the probe reports landed
  on the face the server rolled, with the sides the server rolled.
- **SC-002**: The readout's total equals the server's total for every roll in
  the proof, and its arithmetic adds up whenever a breakdown exists.
- **SC-003**: A roll lands within 1.2 s of the engine receiving it with no
  rerolls, and within 1.2 s plus 0.5 s per reroll or explosion. Under
  reduced motion it lands within 150 ms.
- **SC-004**: After a throw's fade, the engine holds zero throw entities.
  Over 50 consecutive rolls the entity count returns to its baseline.
- **SC-005**: Two boards playing the same roll report identical faces and
  resting places.
- **SC-006**: The release engine bundle grows by less than 150 KB brotli. It
  is measured by the existing bundle report, in brotli and not raw (the
  concern threshold is 100 MB compressed; this is far below it, and is
  stated so that growth is visible).
- **SC-007**: A 20-die throw keeps the board at or above 55 fps on the
  playtest machine's headless run, measured with the render probe.

### Proof

- **Dice crate tests**:
  - the chain step kinds for `r`, `rr`, `x`, `xo` and clamps;
  - reading a resolution stored without them;
  - the breakdown for sums, differences, negatives and substituted
    bindings, and nothing for products, pools and success counts.
- **Engine tests**, run for wasm32 as the engine lints (`make lint` is
  `lint-host` plus `lint-wasm`):
  - each shape's face count and face values;
  - that the landing orientation turns each value's face toward the viewer
    for every value of every die;
  - that the seed gives the same path twice;
  - the queue's order and skip rule;
  - that every throw entity is despawned.
- **Server tests**: `WorldRoll.bindings` for a check, and their absence
  from `MaskedRoll`.
- **The demo's e2e**: `rolls-across-tabs.spec.ts` passes unchanged, and
  asserts `diceLanded()` on both tabs.
- **An e2e spec, `rolls-dice-on-screen.spec.ts`**, in the `rolls` slice
  (it is owned by the `rolls-` prefix): US1 to US5 through `diceLanded()`,
  including a panned board, a mixed formula, `2d20kh1`, a seeded reroll, a
  sheet check, a burst of five, reduced motion through Playwright's
  `reducedMotion: "reduce"`, and `40d6`. The proof is `pnpm e2e:rolls`,
  green.
- **The engine sandbox** (`apps/engine-sandbox`) gains a dice button, for
  tuning the look without the stack. It is not a test.

## Assumptions

- **Look.** Dice are ivory with dark numerals, and dimmed dice are grey at
  half opacity. There are no per-player dice colours or dice skins, and no
  critical-hit effect; a game system may add one later. The tuning of
  speeds, bounce and colour is a design decision made against the sandbox,
  within FR-008's determinism.
- **No sound.** The throw is silent.
- **No off switch.** Every board plays every roll it is allowed to see.
  Reduced motion is the only setting, and it is the operating system's. A
  per-user "dice on the board" toggle is a later spec if players ask for
  one.
- **Rolls only.** Attacks and checks play the same way, because they are
  rolls (spec 081). Damage applied to a token is not animated here.
- **Spec 084** (5e roll facets) will name facets in the readout, such as
  "Lucky". This spec gives it the structure to do so: the breakdown's
  addends and the chain's step kinds.
- **Explosion totals.** The dice crate currently stores an exploded die as
  one `DieOutcome`, whose `final_value` is the chain's last value. This spec
  draws the chain as extra dice. It does not change how the crate totals a
  roll, and the readout reports the crate's total. If the crate's totals
  for explosions are wrong, that is a dice-crate fix of its own.
