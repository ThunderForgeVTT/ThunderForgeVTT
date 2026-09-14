# Research: Token Movement and Vision

Everything here was resolved by reading the code and by what the playtest of
2026-09-11 observed. No unknowns remain open.

## 1. How a door change should reach other boards

**Decision**: The server announces a **wall change** (event code 10) whenever a
wall's door state, designation, lock or secrecy changes, in addition to the
door-changed event (21) it already sends.

**Rationale**: Clients already re-read a scene's walls on event 10
(`apps/web/src/engine/world/sync/walls.ts`), and that re-read carries door
state with it. The whole defect is that a door change is announced on a channel
the wall sync does not listen to. Emitting the event the data actually changed
on fixes every client — the Game Master's, the other players', and the opener's
own — with no web change at all, and it is honest: a door *is* a wall, and it
did change.

**Alternatives considered**:

- *Make the interactives sync re-read walls on event 21.* Works, but teaches a
  second module to own wall state, and leaves the server under-reporting what
  it changed.
- *Broaden the wall sync to listen to 21 as well.* Same objection, and a client
  that misses the subscription still has stale walls.

**Note**: The opener's own board is fixed by the same change. Its optimistic
path (`handle_door_effects`) refuses to act because its wall is not a door
yet — which is itself a symptom of the missing wall event when the door was
designated.

## 2. How the engine learns which token a player may move

**Decision**: A new SDK command, `set_controlled_token`, carrying the token the
local player may move, set by the web from the same place that already sets the
viewer token (`WorldPage.tsx`). The engine tags that entity `PlayerControlled`
and clears the tag from any other.

**Rationale**: The engine's movement system is complete and drives the one
entity tagged `PlayerControlled`; today the only tagged entity is the
placeholder `setup_scene` spawns at startup. Control and point of view are
*different questions* — a Game Master sees through no token and may move any —
so reusing `set_viewer_token` would conflate them and make a Game Master's
control impossible to express.

**Alternatives considered**:

- *Derive control from the viewer token.* Smaller, but a Game Master has no
  viewer token and would therefore control nothing.
- *Tag from the token payload's owner field.* The engine would have to know who
  the local user is, which it currently does not, and that is a larger new
  concept than one command.

**The rest of the same defect**, found while building this phase because the
key still moved nothing once control was fixed:

- The engine announced a keyboard move as `update_token`, which no web module
  handles. A drag emits `upsert_token`, and that is the event the application
  listens for. The keyboard now emits the same one, through a shared helper,
  carrying the whole transform — the bridge forwards rotation and scale only
  when present, so a move that omitted them would read as a move that cleared
  them.
- The committed route's `pathCells` went with it. The web dropped that field
  anyway, and a route belongs in `moveOwnToken` once the server can judge it,
  which is phase 3 of this plan.
- The gridless branch moved the transform and returned without emitting at
  all. It emits now, like the gridded one.

**How it was found**: a probe. `movement_state()` reports what the engine
believes — the token named, whether it was found, how many are tagged, whether
the scene has a grid, how many times the movement system has run, how many
movement keys it saw, and where the controlled token actually is. It read
`runs: 223, presses: 1, x: 7.5` while every client and the server read the old
position, which turned "the keyboard does not work" into "the move happens and
is never announced" in one run. The probe stays; the same question will be
asked again — `apps/web/e2e/token-keyboard-move.spec.ts` already prints it when
the press goes nowhere, and printed `named: null, found: false, tagged: 0` when
that test was run with control deliberately switched off, which is how we know
the test would have caught the original defect rather than merely passing
beside it.

## 3. Where a move is judged against walls

**Decision**: Both sides, from one shared test.
`thunderforge-canvas-core` grows a movement-blocking crossing test beside its
existing `is_visible`; the engine calls it to stop a step or a drag before
anything is sent, and the server calls it in a new `src/server/src/movement/`
module to judge what arrives. The server is authoritative; the engine's check
exists so a player sees the stop immediately.

**Rationale**: Principle III makes the server the judge, and Principle I makes
the engine the thing that shows it. Writing the geometry once in the crate both
already depend on is what keeps the two answers the same.

**What is judged**: the path, not the endpoint — for a keyboard step and a
committed route, the cells in order; for a drag, the straight line from where
the drag began to where it was dropped. Passing exactly through the point where
two walls meet counts as crossing (spec FR-016).

**Alternatives considered**:

- *Server only.* A player would see their token jump back after a round trip,
  which reads as lag rather than as a wall.
- *Engine only.* A modified client walks through walls; Principle III forbids
  trusting it.
- *Reuse the route the engine already sends (`pathCells`).* Kept for the
  committed-route case, but it cannot be the whole answer: a drag has no route,
  and the web sync currently drops it.

## 4. How a player's move carries its path

**Decision**: `moveOwnToken` gains an optional path — the cells or points the
token passed through. When it is absent the server judges the straight line
from the token's stored position to the requested one.

**Rationale**: A path is the only way to tell walking around a wall from
teleporting through it, and the engine already computes one for a committed
route. Making it optional keeps every existing caller working and gives a drag
a sensible default: a drag *is* a straight line.

**Alternatives considered**: A separate mutation for path moves — more surface
for the same decision, and two code paths to keep honest.

## 5. Where a game system's vision comes from

**Decision**: The system pack declares, in its manifest, which of an actor's
fields carry sight in darkness and the bright and dim reach of a light the
character carries. The server resolves those into a per-token vision profile;
the web passes it to the engine with the **existing** `set_token_vision`
command.

**Rationale**: The engine already models exactly this — `VisionProfile` carries
a darkvision range, a facing and a maximum range, and `set_token_vision`
inserts it — and nothing in the product calls it; the only caller in the
repository is the engine sandbox. Genie already declares `sizeCategories` in
its manifest and resolves them from an actor's own data, so the shape is
established. This is wiring an existing capability, not building one.

**Alternatives considered**:

- *Hard-code darkvision in the server for D&D 5e.* Contradicts the owner's
  decision 2 and spec 032's pack architecture.
- *Let the web compute it.* Would make the web a second source of truth for
  simulation state (Principle I).

**Amended 2026-09-14 (owner, spec decision 6; tasks.md T065).** The carried
light half of the declaration does not travel in `set_token_vision`. As built
first, the server resolved `carriedBright`/`carriedDim` and the web dropped
them, because a vision profile has no field for a light — rightly, as it turned
out. A carried light is a light attached to its token: the web sends it as
`set_carried_light { tokenId, bright, dim }`, and the engine keeps it in the
scene's `LightSet` with `attached_token_id` set — the mechanism a Game Master's
token-attached light already used, so it gets its own shadow map row, lights
every seat's board, and follows its token without a second path. Its id is
`carried:<tokenId>`, which no server id can be, and that is what keeps it out of
the Game Master's light editing. It carries its own bright reach
(`LightSource::bright_radius`); a stored light keeps bright at half its radius
(FR-062).

*Rejected: a light inside `VisionProfile`.* It would light the dark only for
the bearer's own board, and would need its own shadowing and its own drawing in
the darkness layer — a second light system beside the first.

## 6. Where explored areas live, and how a reset reaches them

**Decision**: The engine accumulates what a player's token has seen, as a
coarse grid of cells per scene, and exposes it; the web persists it in the
IndexedDB the world cache already uses, keyed by user, world and scene,
alongside an **epoch** the server stamps on the scene. A Game Master's reset
bumps that epoch (for everyone, or for one player), and a client whose stored
epoch is older drops what it kept.

**Rationale**: The owner's decision 3: exploration is the player's own, kept in
their browser, and clearing storage loses it. The engine is the only thing that
knows what was visible (Principle I), so it accumulates; the web is the only
thing with storage, so it persists. An epoch is the smallest thing the server
can hold to make "the Game Master reset it" reach a browser that was not
watching — and it needs no per-player server state.

**Alternatives considered**:

- *Store explored areas on the server.* Contradicts decision 3 and puts a
  per-player bitmap per scene in Postgres.
- *A reset broadcast only.* A player who was offline when it happened would
  come back with their old fog.
- *Reuse the existing `fogMask` column.* That is the Game-Master-painted fog
  the spec puts out of scope; using it here would overload one field with two
  meanings.

## 7. Drawing explored areas

**Decision**: A new engine plugin, `plugins/exploration.rs`, drawing into the
existing `CanvasLayer::Fog`, which is already defined with a visibility toggle
and nothing drawing into it.

**Rationale**: Principle II wants a self-contained plugin for a new capability,
and the layer already exists in the layer stack, so nothing about z-ordering or
the darkness pass has to be re-litigated. The lighting pass already computes
what a viewer can see each frame; exploration reads that and remembers it.

**Alternatives considered**: Extending `plugins/darkness.rs` — it would couple
"what is lit now" to "what was seen once", which are different questions with
different lifetimes.

## 8. What proves each phase

**Decision**: `pnpm playtest` is the acceptance criterion. Each phase turns
named FINDINGs into passes, and the scenario's soft checks become hard once
the product supports them. e2e covers the same ground for the gating suite:
a keyboard move that persists and syncs; a crossing move refused when sent
straight to the server; a door opened by one client changing another client's
board with that client re-reading nothing.

**Rationale**: The defects were found by playing, and the same session is the
honest proof they are gone (spec FR-050, FR-051).
