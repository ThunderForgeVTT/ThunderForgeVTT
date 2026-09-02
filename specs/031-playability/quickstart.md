# Quickstart: Validating Playability 001

How to prove this feature works. Written for a person at a keyboard as much as
for a test run — this feature came out of a playtest, and several of its
defects were invisible to the automated suite.

---

## Prerequisites

```bash
make services-up          # Postgres + RustFS
make migrate              # apply migrations
```

Start the stack. The engine builds `--dev` by default (seconds); use
`ENGINE_PROFILE=release` when measuring load behaviour.

```bash
node ./scripts/dev.mjs                       # http://localhost:5173
```

For a run that registers many throwaway users, add the auth rate-limit bypass —
otherwise the limiter (15/min for login and register, per IP) will replace the
app with "ThunderForge could not load the current instance state", which looks
nothing like its cause:

```bash
THUNDERFORGE_DISABLE_AUTH_RATE_LIMIT=1 node ./scripts/dev.mjs
```

---

## Per-crate checks (Constitution V)

Each surface has one correct check. The engine crate targets wasm32 — a native
`cargo check` will always fail there and is not a signal.

```bash
cargo check --target wasm32-unknown-unknown -p thunderforge_engine
cargo check -p thunderforge                       # server
cargo test -p thunderforge_canvas_core            # snapping, retention rules
pnpm -F @thunderforge/web exec tsc --noEmit --ignoreDeprecations 6.0
pnpm -F @thunderforge/web test                    # vitest
```

`thunderforge_canvas_core` is where snapping and scene-retention rules are
tested, because the engine crate's own `#[cfg(test)]` modules compile and never
execute.

---

## End-to-end

```bash
node scripts/e2e-parallel.mjs --shards=4          # whole suite, sharded
node scripts/e2e-parallel.mjs --shards=1 --only=<substring> --all   # one file
```

The harness gives each shard its own database and bucket. It refuses to run on
top of itself; a stale lock from an interrupted run is reclaimed automatically.

---

## Manual validation, by user story

Automated coverage is listed per story; the hand-check is what the story is
actually for.

### US1 — a GM runs a scene without leaving the table

1. Open a world, launch a scene, connect a second browser as a player.
2. From the actors pane, **View** an actor → opens in a **new tab**; the play
   screen is still there and still connected.
3. **Place** an actor → the token follows the cursor; left-click drops it;
   it snaps like a dragged token.
4. Start a placement and cancel → nothing is created.
5. Restrict Select to tokens → clicking a wall selects nothing.
6. Collapse the filter menu, reload → still collapsed, choices intact.

**Watch for**: the connection indicator staying silent throughout. A stuck
"Connecting…" over a healthy socket is a regression of a bug fixed in this
codebase before.

### US2 — a player uses their character

1. As the connected player, **View** your own character.
2. It opens **in the pane** — the map stays live behind it.
3. Trigger a stat roll and an ability roll; both reach the table.
4. Dismiss; the pane returns to its previous content.

**SC-002**: from play screen to a completed roll in under five seconds.

### US3 — things on the map

1. As GM, place a lore marker and an item.
2. As player, click the lore → opens in a new tab.
3. Click the item → offered **Pickup** and **View**.
4. Pickup → the token disappears for *both* browsers and appears in inventory.
5. **The race**: two players click Pickup at the same moment → exactly one gets
   it, the other is told it is gone. Worth doing by hand at least once; SC-006
   demands 100%.

### US4 — moving the party

1. With player tokens placed, change scene **bringing the party** → old walls,
   lights and non-party tokens gone; new scene present; party tokens there.
2. Change again **without** the party → no tokens carried.
3. Bring the party to a scene where a character already has a token → no
   duplicate.

### US5 — preparing without revealing

1. **Preload** a scene → GM stays on the list, and the connected player's view
   **does not change at all**. This is the assumption most likely to be wrong
   (research R1); check it deliberately.
2. **Launch** → both change.
3. The list shows each scene's description and render, and explains the
   difference between the two actions.

### US6 — authoring quickly

1. Draw walls with snapping on → they follow the grid.
2. Repeat on a **hex** scene → they follow hex lines, not squares.
3. Turn snapping off → free drawing.
4. Room helper → a closed room in one gesture. **SC-005**: room with a door in
   under thirty seconds.
5. The door opens, closes and locks.

### US7 — combat

*Partly blocked on spec 032.* FR-030 is testable now:

1. Select three tokens → the combat panel offers exactly those three.
2. Start combat → for a system with rounds, the round and current participant
   show and advance.
3. FR-031 (a system without rounds shows no round counter) awaits pack-supplied
   surfaces.

### US8 — content management

1. Twenty players → find one by search; see and change their bound character.
2. Confirm the change is reflected on the actor page too (three surfaces write
   this relation).
3. Create an NPC → full page, explicit save, no inline form on the list.
4. Give an actor a portrait **and** a token image; both appear in their places.
5. Set an item price; organise lore into a tree and tag it.

### US9 — the defects

1. **FR-040 / FR-040a**: switch between every *ordered* pair of tools → nothing
   is ever placed. Include text, and include switching *from* text — text is the
   only DOM-handled tool and the only one that does not currently misfire, so it
   is the control case.
   Also: begin a drag or a placement, then switch tools mid-gesture → the
   gesture must not complete under the new tool's rules.
2. **FR-041**: load the play view watching closely → exactly one loading
   indicator at any moment.
3. **FR-042**: open a world in **each supported browser**. Either content is
   served from the device and reported, or the user is told this browser cannot
   keep content — never a silent zero.

   The supported set is **decided: Chromium-based browsers only for now, with
   Firefox a later target** (constitution, and MVP.md "Supported browsers").
   So this check is Chromium for the served-and-reported path. Firefox remains
   the known unsupported case from research R7, and the honest thing to verify
   there is the *other* branch — that it says plainly it cannot keep content.
   That branch is now covered twice: a capability probe before anything is
   attempted, and the engine's own degradation reason afterwards.
4. **FR-043**: with the console open, browse actors and scenes → no repeated
   failed request for an absent identifier.

---

## Suggested regression tests

These encode properties whose absence let the current defects ship:

- **Exactly one loading indicator is visible at any moment.** The existing
  `engine-loading.spec.ts` asserts that *a* loader appears; nothing asserts
  uniqueness, which is why two shipped.
- **Switching tools places nothing** — for every ordered pair of tools, not just
  the pair that was noticed. Cheap to write once the mode is a single engine
  state rather than an ambient flag.
- **Concurrent pickup yields exactly one inventory entry.**
- **Snapping is correct on hex**, as a native `canvas-core` test rather than a
  browser one.

---

## A note on shared fixtures

Moving NPC and item creation to dedicated pages breaks three e2e specs that
create content *incidentally* through the inline forms
(`world-compendium.spec.ts`, `players-section.spec.ts`, `actor-claim.spec.ts`).
Two of them are not about the compendium at all.

Add a shared `createNpc` fixture and point them at it **before** changing the
UI. That turns this change, and the next one, into a one-line fixture edit
rather than a three-spec edit.
