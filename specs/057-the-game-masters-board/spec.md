# Feature Specification: The Game Master's Board

**Feature Branch**: `057-the-game-masters-board`

**Created**: 2026-09-15

**Status**: Draft

**Input**: Project owner, walking the play field's rails on 2026-09-15: he does
not know what **explored areas** means "other than remembering or not", and
wants **fog of war** as a separate thing one step below lights in the rail's
hierarchy. He does not know what the **Walls** rail does, and wants it to say
so — draw walls, draw doors, select doors — with **door snapping** that "makes
a lot more sense", while keeping free drawing, which he likes. He wants to
**assume a token's personality** so that he can see its line of sight, because
today he "can't really validate if these walls are really real". And he expects
**scene lighting** he can change during play, with **time of day on the right
side of the screen, not in settings** — "take a day scene and turn it to night,
and darken it with some really nice filters".

## The problem

Everything the owner asked for is one of three things: a capability the engine
already has that no control reaches, a capability the server already stores
that nothing reads, or a genuinely missing piece. Sorting them is most of this
spec's value, because three of the five items turn out not to be new features
at all.

### The Walls rail's headline control does nothing

`WallTool` (`apps/web/src/components/canvas-tools/WallTool/WallTool.tsx`) opens
with a button that reads **"Draw wall"** and, when pressed, **"Drawing walls"**
(`:86-94`). It toggles a React `useState` (`:45`, `:49-51`) and that is all it
does. Nothing dispatches it, nothing observes it, and the component's own
header says so: drawing "is implemented engine-side … toggling 'draw mode' here
only signals intent by dispatching a `set_wall_draw_mode`-shaped local UI state
today, ready to be observed by the engine bridge once that lands" (`:38-42`).
`set_wall_draw_mode` appears nowhere else in the repository.

Walls are in fact drawn by arming the rail's **Walls** tool, which puts the
engine into `AuthoringMode::Walls`
(`src/engine/src/plugins/authoring_mode.rs:47-55`, `:64-73`), after which a
drag on the canvas draws. So the button is not merely inert — it is a second,
false account of how the tool works, sitting on top of the real one.

The rest of the panel is three field names and a delete: **Blocks vision**
(`:110`), **Blocks movement** (`:121`), a **Door** select offering "Not a door
/ Door (open) / Door (closed)" (`:22-26`, `:125-143`), and **Delete wall**
(`:145-152`). None of them says what it means, and two of the wall properties
spec 030 shipped — **locked** and **secret**, both real fields on `Wall`
(`crates/thunderforge-canvas-core/src/wall.rs:36-65`) — are not in the panel at
all. Neither are the keyboard affordances the engine already gives a selected
wall: `V` toggles sight, `B` toggles movement, `O` cycles the door, Delete
removes it, Ctrl+Z undoes (`src/engine/src/systems/wall.rs:559`, `:581`,
`:603`, `:628`, `:657`).

The same fault sits in the Lights panel: its **"Place light" / "Placing
lights"** button is local React state too
(`LightingTool.tsx:230-239`). Two of the rail's panels open with a button that
does nothing.

### The walls the tool draws do not stop anybody

Every wall the wall tool creates is emitted with `"blocksVision": true,
"blocksMovement": false` — the drag path
(`src/engine/src/systems/wall.rs:448-460`), the multi-point chain (`:535-547`)
and each of the four walls of a room (`:210-221`). Only a drawn door blocks
movement, and `emit_door` explains why it is the exception (`:226-256`).

Before spec 045 that flag had no effect, so the default was invisible. Spec 045
gave it one: the server now judges every move against it (ADR-095). The result
is that a Game Master who draws a room and asks a player to walk into it
watches the player walk straight through the walls — which is exactly the
experience the owner described as not being able to tell whether the walls are
real. He is right, and it is not only that he cannot see: the walls really do
not stop anyone.

### There is fog on the server, and nothing has ever used it

Spec 045 said it in one line (`spec.md:158`): "The server stores a fog mask per
scene and the engine has a fog layer; nothing in the web uses either." That is
still true, and it is more complete than it sounds:

- **A table**, `fog_masks (fog_id, scene_id, bitmap_data, version, width,
  height, updated_by, created_at, updated_at)`
  (`src/server/src/schema.rs:290-302`), one row per scene, upserted on
  `scene_id` (`src/server/src/graphql/mutations_scenes.rs:560`).
- **A mutation**, `updateFogMask` (`mutations_scenes.rs:493-578`;
  `src/app/schema.graphql:213`), already guarded to a Game Master of the scene
  and already refused while a world's play is paused (`:508-528`). The comment
  above the guard is worth reading: reveal is the dangerous direction.
- **A query**, `fogMask(sceneId)` (`src/app/schema.graphql:1005`;
  `src/server/src/graphql/queries/scene.rs:295-330`), readable by any member of
  the world.
- **A layer**, `CanvasLayer::Fog`
  (`src/engine/src/resources/canvas_layer.rs:38`, `:115`, `:127`), which spec
  045's exploration plugin borrowed for explored areas
  (`src/engine/src/plugins/exploration.rs:22-26`, `:155`).

A grep of `apps/web/src` for `fogMask` returns nothing. So the product has a
fog of war with storage, an ownership guard, a pause gate and a render layer,
and no way for a Game Master to touch it.

It also has two gaps that matter. `updateFogMask` **does not broadcast**: it
writes and returns, with none of the `world_events` NOTIFY that every other
scene mutation carries, so a reveal today would reach nobody else's board until
they reloaded. And the stored shape is a **bitmap**, which is the one shape
spec 045's own exploration module argues against — "a bitmap of a scene is
large, has to be scaled when the grid changes, and means nothing to anything
but a shader" (`plugins/exploration.rs:15-20`).

### Explored areas are not fog of war, and were never meant to be

The owner is right that he does not know what explored areas means, because it
means something narrower than it sounds. Spec 045 decision 3 and FR-070 to
FR-078 define it as: with exploration on, the cells a **player's own token**
has seen stay faintly on **that player's** map — drawn at alpha 0.22
(`plugins/exploration.rs:34-40`), bounded to 20,000 cells (`:49-55`), kept in
that player's own browser in a database of its own
(`apps/web/src/services/exploredAreas.ts:26-28`, keyed by user, world and
scene at `:52`), and never sent to the server. Spec 045 says plainly that there
is no fog of war, and puts "fog a Game Master paints by hand" in Out of Scope.

So the owner is asking for the thing 045 named as out of scope, and he is
asking for it in the right shape: a hierarchy of lights, then fog, with
explored areas as the player's own memory underneath. That is three layers, not
one, and this spec is where the middle one gets built.

### The scene's light exists, under a name the owner did not look for

"Set the scene to dark" shipped on 2026-09-10. It is a three-button group
labelled **Scene light** — Bright, Dim, Dark
(`apps/web/src/components/canvas-tools/LightingTool/LightingTool.tsx:57-59`,
`:149-175`) — and it is live for everyone.

It is also two levels down, and on the wrong side of the screen for the way the
owner described it. It lives inside the `LightingTool` panel, which is the
content of the **Lights** tool — an icon-only torch button, labelled only by
`aria-label` and `title` — in `GmToolRail`
(`apps/web/src/components/world/GmToolRail/GmToolRail.tsx:86-92`,
`:118-131`; registered at `apps/web/src/pages/world/WorldPage.tsx:2745-2772`).
`GmToolRail` is a 48px column pinned to the **left** edge, and the tool it
opens on is `select` (`WorldPage.tsx:366`), so nothing surfaces the scene's
light on load. The exploration toggle and its reset are buried in the same
panel (`LightingTool.tsx:183-193`). The only path the product exercises to
reach any of it is the e2e clicking `gm-tool-lights`
(`apps/web/e2e/scene-lighting.spec.ts:80`).

The **right** side of the screen is a different rail entirely — `WorldDock`
(`apps/web/src/components/world/PlayDock/WorldDock.tsx:48`, `:62`, labelled
"Play panels"), holding Chat, Actors, Combat, Clocks & Timers, Asked for,
Settings and About & feedback (`WorldPage.tsx:2527`). There is no Walls or
Lights panel there. So when the owner says "in the settings I expect scene
lighting" and "time of day would be good for the right side of the screen", he
is describing the dock, and the scene's light is on the opposite rail.

**That the owner could not find a control he asked to be built is the finding.**
The answer is not a fourth place to put it — it is that the Lights panel is the
right home for the scene's own light and must say so, and that the choice of
rail is now a real question rather than an accident (Question 5).

### There is no time of day at all

Nothing in the engine, the server, the schema or the web knows what dawn, day,
dusk or night is. The nearest thing is map import, which reads a Universal VTT
file's ambient colour and chooses one of the three levels from its brightness —
under 0.35 dark, under 0.75 dim, otherwise bright
(`src/server/src/map_import/ambient.rs:13-28`). That is a one-way derivation at
import, not a property a Game Master can turn.

But the **colour channel the owner's "nice filters" needs already exists in the
engine, end to end, and nothing sets it.** `AmbientLight` carries
`level: Illumination` and `color: Option<Rgb>`
(`crates/thunderforge-canvas-core/src/vision.rs:273-278`). The engine command
`SetAmbientLight { level, color }` carries both
(`src/engine/src/payloads.rs:449-453`; `src/engine/src/sdk.rs:255`;
`src/engine/src/app.rs:735`). The darkness shader's uniform is
`ambient: vec4<f32>` whose "rgb = ambient tint, a = how dark an unlit fragment
is" (`src/engine/src/plugins/darkness.wgsl:28-31`;
`src/engine/src/plugins/darkness.rs:88-89`, `:443-446`).

What is missing is at the two ends: the server stores only a level string on
`scenes.ambient_light` (`src/server/src/schema.rs:756`;
`mutations_scenes.rs:64`) and writes `color: None`
(`src/server/src/combat/redaction.rs:88-94`), and no control offers a Game
Master anything but the three levels. Because nothing ever populates the
colour, **every dark scene in the product is already tinted the same hard-coded
moonlight blue** — the `unwrap_or` default at `darkness.rs:443-446`. Time of
day is therefore not a new render pass. It is a named position on a dial that
sets a level and a tint the engine already knows how to draw.

One constraint comes with it. The darkness layer **is not spawned at all while
the scene is Bright** — `darkness_strength` returns 0.0, 0.35 or 0.92 per level
(`darkness.rs:231-237`) and `sync_darkness` early-returns and despawns
(`:397-410`), so an unconfigured scene renders exactly as it always did. A warm
dawn or a slightly cooled day is a tint on a scene that is still bright, which
is the one case that layer currently refuses to draw. FR-043 and FR-050 are
written knowing that, and Decision 8 chooses which way it resolves.

### A Game Master cannot look through anybody's eyes

Vision is computed, correctly and in one place: `visibility_of(observer,
vision, target, lights, walls, ambient)`
(`crates/thunderforge-canvas-core/src/vision.rs:341-348`), against a
`VisionProfile` of darkvision range, facing, field of view and a hard sight
limit (`:195-215`). The server exposes `tokenVision(sceneId)`
(`src/app/schema.graphql:1048`), and a Game Master's board deliberately sees
through no token at all, so that they never lose sight of a piece (spec 045).

That last rule is what makes the owner's complaint true. A Game Master sees
everything, always, which is what they need for running the table and exactly
wrong for checking their own work. There is a lighting overlay resource,
`LightingOverlay(bool)` — "GM-facing diagnostic, off by default"
(`src/engine/src/resources/vision.rs:27-30`) — which draws light radii and
vision cones, and it is a diagnostic drawn *over* a board that still shows
everything. It is not the same thing as standing where the goblin stands.

The mechanism for standing there, though, already exists.
`ViewerToken(Option<String>)`
(`src/engine/src/systems/lighting_vision.rs:18`) is whose eyes the board is
drawn through, `None` meaning a Game Master, and it has a wasm setter,
`set_viewer_token` (`:31`), which the web calls from the signed-in person's own
token (`apps/web/src/pages/world/WorldPage.tsx:977`, `:1016`, `:1030-1031`;
bridge at `apps/web/src/engine/bevy/index.ts:1354`). What is missing is only
that a Game Master has no way to point it at a token that is not theirs — and
the product's handling of what it means to be in that state.

### Snapping exists, and no control reaches it either

The third of the three. `SnapRule`
(`crates/thunderforge-canvas-core/src/snapping.rs:59-86`) is the single place
that decides where a placed thing lands: `cell` for tokens and lights, `vertex`
for wall endpoints, honouring square and both hex orientations, and passing a
position through **untouched** when snapping is off — "free placement means
exactly where the user let go, not a gentler snap" (`:19-27`). It is already
wired into the wall drag (`src/engine/src/systems/wall.rs:279-293`), and the
wall tool already has three primitives: `WallPrimitive::Segment`, `Room` and
`Door` (`src/engine/src/resources/wall.rs:61-86`), with a drawn door emitted
already closed and already blocking movement (`systems/wall.rs:226-256`).

Spec 031 FR-024 to FR-026 asked for all of it. The engine has it, and
`set_wall_primitive("segment" | "room" | "door")` is **already exported to
wasm** (`src/engine/src/plugins/wall.rs:148`). Nothing in `apps/web` calls it,
and nothing sets `GridSnapEnabled`
(`src/engine/src/resources/token_grid.rs:43`, default on), so a Game Master
cannot choose to draw a door, cannot turn snapping off, and gets the defaults:
snapping on, segments only.

What genuinely does not exist is the other two kinds of snapping the owner
asked for. `SnapRule` snaps to the **grid** and nothing else. There is no snap
to another wall's endpoint, and no snap to a right angle.

## What exists, and what this spec adds

| The owner asked for | What exists today | What this spec does |
|---|---|---|
| Fog of war, below lights | `fog_masks` table, `updateFogMask`, `fogMask`, `CanvasLayer::Fog` — nothing in the web touches any of them; no broadcast on write | Builds the rail, the gestures and the live path; replaces the bitmap with cells (Decision 3) |
| Explored areas explained | Per-browser cell memory, spec 045 FR-070–FR-078 | Renames nothing, moves it under fog in the rail, and says what it is in a sentence |
| Scene lighting reachable in play | **Ships.** "Scene light": Bright/Dim/Dark, one click behind an icon-only torch on the **left** rail | Keeps it with the lamps, makes the panel say what it holds, and settles which rail (Question 5) |
| Time of day | **Nothing.** The tint channel exists unused through engine, command and shader; the darkness layer is despawned while a scene is bright | A named scene property that sets level and tint, on the rail |
| A walls rail that says what it does | A dead "Draw wall" button, three field names, no locked/secret | Replaces the panel with named actions and sentences |
| Draw a door | `WallPrimitive::Door` in the engine, no control | Exposes it |
| Door and wall snapping | Grid snapping only, no control to turn it off | Exposes it, adds endpoint and right-angle snapping, adds a free-draw modifier |
| Seeing as a creature does | `visibility_of`, vision profiles, `tokenVision`, and a `ViewerToken` with a wasm setter — pointed only at the viewer's own token | A mode that points it at a creature the Game Master picks, and says what that state is |

## User Scenarios & Testing *(mandatory)*

### User Story 1 - A walls rail that says what it does (Priority: P1)

A Game Master opens Walls. The panel offers, in words: draw a wall, draw a
room, draw a door, and — with something selected — what that wall does to sight
and to movement, whether it is a door, whether it is locked, and whether
players can see it is there at all. Every control has an effect, and the one
that has no effect is gone.

**Why this priority**: It is the owner's "I don't know what walls really does",
it is the smallest change, and everything else in this spec is drawn with these
tools.

**Independent Test**: A Game Master arms Draw a door, drags across a doorway,
and gets a door: closed, blocking movement, openable by a player. Nothing in
the panel toggles without changing the board.

**Acceptance Scenarios**:

1. **Given** the Walls panel, **Then** each action names what a drag will
   produce, and arming one really arms it — no control in the panel is
   decorative.
2. **Given** a Game Master who arms Draw a wall and drags, **Then** a wall
   appears where they dragged.
3. **Given** a Game Master who arms Draw a door and drags, **Then** a closed
   door appears, and a player can open it.
4. **Given** a selected wall, **Then** the panel says in a sentence what it
   currently does — whether sight passes it, whether a creature can walk
   through it, whether it is a door, whether it is locked, whether players are
   shown it — and changing any of those changes the board for everyone.
5. **Given** a wall drawn with this tool, **Then** it stops a player's token,
   unless the Game Master says it should not.
6. **Given** a player, **Then** none of this is offered to them.

---

### User Story 2 - Fog of war, painted and lifted (Priority: P1)

A Game Master covers a dungeon before the session. The table sees nothing but
the covered board. As the party enters each room, the Game Master lifts the fog
from it and the room appears — its map, its furniture and anything standing in
it — on every player's screen at once. A wrong reveal is put back.

**Why this priority**: It is the layer the owner said is missing, and it is the
only one of the five that changes what a player can see.

**Independent Test**: A Game Master covers a scene, opens two player browsers,
reveals one room, and both players see that room and nothing else, without
reloading. The Game Master re-hides it and it goes away on both.

**Acceptance Scenarios**:

1. **Given** a scene with fog, **Then** a player sees nothing of a hidden area
   — not the map under it, not the shapes on it, not a token standing in it.
2. **Given** a Game Master, **Then** they see the whole board, with what is
   hidden marked as hidden rather than hidden from them.
3. **Given** a Game Master who reveals an area, **Then** every player's board
   shows it within a second, with nobody reloading anything.
4. **Given** a Game Master who re-hides an area, **Then** it is hidden again
   everywhere, just as fast.
5. **Given** a player who joins after a reveal, **Then** they arrive with the
   fog as it stands.
6. **Given** a player, **Then** no gesture of theirs changes the fog.
7. **Given** a scene whose fog a Game Master has never touched, **Then**
   nothing is hidden — fog starts off and is turned on deliberately.

---

### User Story 3 - Three layers, and a rail that shows the order (Priority: P1)

The rail reads the way the board is built: **Lights** on top — the scene's own
light, the time of day and the lamps in it; **Fog** below it — what the Game
Master has covered and uncovered; and under that, the note that each player's
own map remembers where they have been. A Game Master can tell, from the rail,
which of the three is hiding something.

**Why this priority**: The owner asked for the hierarchy by name. It is also
what stops the next person asking what explored areas means.

**Independent Test**: A Game Master darkens a scene, fogs half of it, and turns
exploration on. Three controls in three places say which is doing what, and
turning each off in turn changes exactly one thing on a player's screen.

**Acceptance Scenarios**:

1. **Given** the rail, **Then** the scene's own light and the time of day are
   in the same place as the lamps, and that place says it holds them.
2. **Given** the rail, **Then** fog is its own entry, below lights.
3. **Given** the fog entry, **Then** it explains in a sentence that fog is what
   the Game Master hides from the table, and that a player's own map separately
   remembers where they have been.
4. **Given** a dark scene, a fogged area and an explored area, **Then** a Game
   Master can say which of the three is responsible for any part of a player's
   board being hidden.

---

### User Story 4 - Standing where a creature stands (Priority: P1)

A Game Master picks a creature and the board becomes what that creature sees:
its line of sight, its darkvision, the dark where it has no light, and the fog
as that creature's player would meet it. They walk it into the corridor, watch
the wall cut the sight off, and come back out. The board is unmistakably not
theirs while they are in it, and one key returns them.

**Why this priority**: It is what the owner actually asked for — a way to
validate the walls — and it is the only honest test of every other item in this
spec.

**Independent Test**: A Game Master enters a goblin's view in a walled room. A
token behind the wall is not drawn. They open the door and it appears. They
leave the view and see the whole board again.

**Acceptance Scenarios**:

1. **Given** a Game Master in a creature's view, **Then** the board shows what
   that creature sees and hides what it does not — through walls, through
   closed doors, through darkness beyond its darkvision.
2. **Given** that view, **Then** the fog hides from them what it hides from
   that creature's player.
3. **Given** that view, **Then** the screen says whose eyes these are, without
   the Game Master having to remember.
4. **Given** that view, **Then** one keystroke leaves it, and leaving restores
   the whole board.
5. **Given** that view, **Then** the Game Master may move that creature, and
   the move is an ordinary move by its controller — it is not a player's
   session and it is not judged as one.
6. **Given** a Game Master who enters the view and reloads, **Then** they come
   back to their own board, not to the creature's.
7. **Given** a player, **Then** no such control is offered to them.

---

### User Story 5 - Turning a day scene into night (Priority: P2)

A Game Master reaches the rail during play and moves the scene from day to
dusk, and then to night. The board warms, then cools and darkens; the lamps in
the street start to matter; a character with darkvision starts to see further
than one without. Nobody reloads, and the tokens are as readable at midnight as
at noon.

**Why this priority**: It is the item the owner most wanted on the right of the
screen, and it is the one with the most new surface — but it changes
presentation and one existing property, not what anybody may do.

**Independent Test**: A Game Master sets a bright outdoor scene to night. Every
board darkens and takes the night's colour within a second, a torch that did
nothing at noon now lights a pool, and a token's portrait is still identifiable.

**Acceptance Scenarios**:

1. **Given** a scene, **Then** its time of day is set from the rail during
   play, not from a settings page.
2. **Given** a change of time, **Then** every board at the table follows within
   a second without reloading.
3. **Given** night, **Then** the scene's own light is dark, carried and placed
   lights do what they do in the dark, and a creature's darkvision applies.
4. **Given** any time of day, **Then** a token's own artwork and any text on
   the board stay readable.
5. **Given** a Game Master who wants a light level the times do not offer,
   **Then** they can still set the scene's light directly, and doing so does
   not silently claim a time of day it is not.
6. **Given** a person who has asked their system for reduced motion, **Then**
   the change of time is instant, with no animated transition.
7. **Given** a scene whose time of day has never been set, **Then** it behaves
   exactly as it does today.

---

### User Story 6 - Snapping that makes sense, and drawing freely (Priority: P2)

A Game Master draws a wall and its ends land on the grid. The next wall's start
jumps to the end of the last one, so a room closes instead of leaking light
through a hair-wide gap. A held key turns all of it off and the wall goes
exactly where they let go — which is how the tool behaves today and what the
owner asked to keep.

**Why this priority**: The owner named door snapping specifically, and a wall
that misses its neighbour by a pixel is a light leak nobody can find. It is
below the rail and the fog because the current behaviour, while unreachable, is
not wrong.

**Independent Test**: On a gridded scene, a Game Master draws four walls around
a room; every corner meets. They hold the free-draw key and draw a fifth at an
angle, and it stays where they drew it.

**Acceptance Scenarios**:

1. **Given** a gridded scene, **Then** a wall's ends land on the grid's corners
   as they do today.
2. **Given** an endpoint dragged near the end of an existing wall, **Then** it
   lands exactly on it, and the two walls share a point.
3. **Given** a wall being drawn from an existing endpoint, **Then** it can be
   held to a right angle, and the Game Master can see that it is being held.
4. **Given** a held free-draw key, **Then** nothing snaps and the wall is
   exactly where it was drawn.
5. **Given** a hex scene, **Then** snapping lands on hex corners, in that
   scene's orientation, rather than on an invented square lattice.
6. **Given** a gridless scene, **Then** grid snapping does nothing and says so
   rather than offering a lattice that is not there; endpoint and right-angle
   snapping still work.
7. **Given** a door drawn onto an existing wall, **Then** it snaps along that
   wall rather than crossing it at an angle.

---

### User Story 7 - A dark board is still a legible board (Priority: P3)

A player with low vision plays a night scene. The map is dark, and the token
names, the numbers on the status displays, the selection outlines and the
tokens' own portraits are all still readable. Nothing about the darkening is
carried by colour alone.

**Why this priority**: It is a constraint on everything above rather than a
feature, and it is the one that quietly fails if nobody writes it down.

**Independent Test**: An automated audit of a night scene with fog and a
creature view active reports no contrast failure for any text or control on the
board.

**Acceptance Scenarios**:

1. **Given** any time of day, **Then** text drawn on the board meets the
   contrast bar the rest of the product is held to.
2. **Given** fog, **Then** the fact that an area is hidden is not conveyed by
   colour alone.
3. **Given** a creature view, **Then** the fact that the board is somebody
   else's is stated in words, not only by a tint at the edge.
4. **Given** reduced motion, **Then** no transition in this feature animates.

---

### Edge Cases

- **Fog is turned off after a session.** What was hidden is remembered, not
  discarded: turning it on again restores the same shape. Only an explicit
  reset clears it, the way spec 045 FR-074 treats exploration.
- **A player is standing in an area that gets re-hidden.** They stop seeing the
  room. They keep seeing their own token, because a player who cannot see their
  own piece cannot play.
- **Fog over an area a player has explored.** Fog wins. A Game Master hiding
  something is a deliberate act; a player's memory of the corridor is not a
  right to see through it.
- **The map is replaced under existing fog.** The fog is in the scene's own
  cells, not in the image, so it survives a change of map art. A change of the
  scene's **grid** does not resize it either — the cells are what they are, and
  the Game Master is told the fog may no longer line up.
- **A scene with no grid.** Fog still works: the cells come from the scene's
  own spacing, the same way exploration handles a gridless scene today.
- **A creature view of a token that is deleted, or moved to another scene.**
  The view ends and the Game Master is returned to their own board, told why.
- **A creature view while the world's play is paused.** Looking is allowed;
  moving is not, because no move is.
- **A Game Master in a creature's view when a player reveals something.** The
  view follows the board. It is a way of drawing the board, not a snapshot.
- **Two Game Masters.** Fog is one scene's fact, so the second sees the first's
  reveals. Each one's creature view is their own.
- **A time of day set on a scene whose map was imported as dark.** The import's
  guess is a starting value, not a lock; setting a time replaces it.
- **A very large fog set.** It is bounded, like exploration's 20,000 cells, and
  the Game Master is told when the bound is reached rather than the board
  quietly stopping.
- **An offline Game Master.** A reveal made while disconnected reaches the
  table when they reconnect, or is refused plainly. It is never applied
  locally and silently lost.

## Requirements *(mandatory)*

### Functional Requirements

**The walls rail**

- **FR-001**: No panel on the tool rail MUST contain a control that has no
  effect. The "Draw wall" toggle and the Lights panel's "Place light" toggle,
  both of which set only local React state, MUST be removed or connected.
- **FR-002**: The panel MUST offer, as named actions, drawing a wall, drawing a
  room, and drawing a door, and arming one MUST change what the next drag on
  the board produces. Spec 056 FR-004 offers *start a wall here* from the
  board's own menu without arming anything; the two MUST be two ways into one
  drawing behaviour, not two drawing behaviours.
- **FR-003**: Arming a drawing action MUST be visible on the panel, and exactly
  one MUST be armed at a time.
- **FR-004**: For the selected wall, the panel MUST say in a sentence what the
  wall currently does, covering sight, movement, whether it is a door, whether
  it is locked and whether players are shown it — not field names alone.
- **FR-005**: The panel MUST expose **locked** and **secret**, which spec 030
  defined, which are real fields on a wall, and which no panel offers today.
  Spec 056 FR-014 puts the same two on the wall's right-click menu; the panel
  and the menu MUST read and write the one state, and neither MUST become a
  second account of what a wall is.
- **FR-005a**: Every action the engine already offers on a selected wall by
  keyboard — toggling sight, toggling movement, cycling the door, deleting,
  undoing — MUST have a visible equivalent, in this panel or in the wall's own
  menu (spec 056 FR-014), so that the keyboard is a shortcut rather than the
  only way.
- **FR-005b**: This spec says what the Walls panel *means*. Spec 056 says how a
  wall is *reached* — selected by clicking it, and acted on by a menu opened on
  it. Neither MUST reimplement the other: the panel MUST NOT grow its own way
  of picking a wall off the board, and the menu MUST NOT grow its own copy of
  the panel's sentences.
- **FR-006**: A wall drawn by this tool MUST block movement by default. A Game
  Master MUST be able to draw one that does not, and the panel MUST say which
  it is drawing.
- **FR-007**: A door drawn by this tool MUST be the same kind of door spec 030
  defines — a wall with a door state — and MUST be openable by a player under
  spec 030 FR-011.
- **FR-008**: None of this MUST be offered to a player, consistent with the
  authoring-tool grants already in place.

**Snapping**

- **FR-010**: Snapping MUST be a control a Game Master can see and change while
  drawing, not a default they cannot reach.
- **FR-011**: A wall endpoint MUST snap to the scene's grid as it does today —
  to cell corners, honouring square and both hex orientations.
- **FR-012**: A wall endpoint MUST snap to the endpoint of an existing wall
  when drawn near one, producing a shared point rather than two points close
  together.
- **FR-013**: A wall being drawn MUST be able to be held to a right angle
  relative to its own start, and the board MUST show while it is being held.
- **FR-014**: A modifier held during a drag MUST suspend every kind of snapping
  for that drag, placing the wall exactly where it was drawn. Releasing it MUST
  restore the Game Master's setting, not overwrite it.
- **FR-015**: On a gridless scene, grid snapping MUST do nothing and MUST say
  so, while endpoint and right-angle snapping still apply.
- **FR-016**: A door drawn along an existing wall MUST snap to that wall's line.
- **FR-017**: Snapping MUST be decided in one place, for every kind of content,
  as it is today. A second implementation for walls MUST NOT be introduced.
- **FR-018**: The snapping rule this spec defines MUST be the rule spec 056
  FR-003 defers to when it says everything placed from the board's menu lands
  at the point it was opened on "subject to the scene's snapping rule". A thing
  placed from that menu and a thing drawn from the rail MUST land by the same
  rule.

**Fog of war**

- **FR-020**: Fog MUST be a thing of its own, below lights in the rail, and
  MUST NOT be presented as a mode of lighting or of exploration.
- **FR-021**: Fog MUST hide **everything in a hidden area** from a player: the
  map art, shapes, images, and any token standing in it. It is not a dimming.
- **FR-022**: Fog MUST be authored by a Game Master only. No gesture available
  to a player MUST change it.
- **FR-023**: A Game Master MUST be able to cover the whole scene, uncover the
  whole scene, and cover and uncover an area they indicate.
- **FR-024**: A Game Master's own board MUST show the whole scene, with hidden
  areas marked as hidden rather than hidden from them.
- **FR-025**: Fog MUST be one fact per scene, shared by the table, and MUST be
  authoritative on the server (Decision 2, Question 1).
- **FR-026**: A change to the fog MUST reach every board in the scene within
  one second, with no client re-reading anything of its own — the failure spec
  045 found in doors.
- **FR-027**: A client arriving in a scene MUST receive the fog as it stands.
- **FR-028**: Fog MUST be stored as the scene's own cells, in the vocabulary
  walls, movement and exploration already use, and MUST NOT be stored as a
  bitmap (Decision 3). The existing `fog_masks` bitmap MUST be retired rather
  than left as a second, disagreeing record.
- **FR-029**: Fog MUST be off for every existing scene and for every new one
  until a Game Master turns it on.
- **FR-030**: Turning fog off MUST keep what was hidden, so that turning it on
  restores it. Only an explicit reset MUST clear it.
- **FR-031**: Fog MUST be refused while a world's play is paused, as
  `updateFogMask` already is.
- **FR-032**: Fog MUST take precedence over a player's explored area: an area
  the Game Master has hidden MUST be hidden even where that player has been.
- **FR-033**: Explored areas MUST remain exactly as spec 045 FR-070 to FR-078
  define them — per player, in that player's own browser, never on the server.
  This spec MUST NOT move them to the server.
- **FR-034**: The rail MUST say, in a sentence a Game Master reads, what fog is
  and what explored areas are, so that the two are not confused.
- **FR-035**: Fog MUST NOT be offered as the answer to hiding one thing. Spec
  056 FR-050 asks for an object a Game Master can hide from players, and an
  object has no actor and so no visibility field today. Fog hides an **area**
  and everything standing in it; hiding one object wherever it stands is spec
  056's, and this spec MUST NOT build a second mechanism for it.

**The scene's light, and time of day**

- **FR-040**: The scene's own light — Bright, Dim, Dark — MUST stay reachable
  during play from the rail, where it is today, and the panel that holds it
  MUST say that it holds the scene's light as well as the lamps in it
  (Decision 4).
- **FR-041**: A scene MUST carry a **time of day** as a property of its own,
  settable while playing, from the board's own rails and not from a settings
  page. Which rail is Question 5; that it is not a settings page is not.
- **FR-042**: Time of day MUST be a small set of named times. Dawn, day, dusk
  and night MUST be the set the product ships (Decision 5, Question 2).
- **FR-043**: Each named time MUST determine the scene's own light level and a
  colour the board is tinted with. Both MUST be stated per time, not left to a
  renderer to guess. The colour MUST be carried on the scene, beside the level,
  because the server stores only the level today and the engine's ambient
  colour is consequently never set.
- **FR-043a**: A time of day whose light level is bright MUST still be able to
  tint the board, which the darkness layer currently cannot do because it is
  not drawn at all while a scene is bright. A bright scene with no time of day
  set MUST continue not to draw it (FR-050, Decision 8).
- **FR-044**: Setting a time of day MUST reach every board in the scene within
  one second without a reload.
- **FR-045**: A Game Master MUST still be able to set the scene's light level
  directly. Doing so MUST leave the time of day unset rather than claim a time
  the Game Master did not choose.
- **FR-046**: Time of day MUST interact with vision only through the scene's
  light level. A creature's darkvision, a carried light and a placed light MUST
  behave at night exactly as they behave in a scene a Game Master set to dark.
  Time of day MUST NOT be a second vision rule.
- **FR-047**: The tint MUST be presentation. It MUST NOT change what
  `visibility_of` returns, what the server judges, or who can see whom.
- **FR-048**: The tint MUST NOT be applied to a token's own artwork, to
  nameplates, to status displays or to selection marks. A darkened board MUST
  leave the pieces on it readable (FR-081).
- **FR-049**: A game system's pack MUST be able to declare its own names for
  the times, with the same count and the same effects. A pack that declares
  none MUST get the product's four.
- **FR-050**: A scene that has never had a time of day set MUST render exactly
  as it does today.
- **FR-051**: The map importer's derived ambient level MUST remain a starting
  value only; setting a time of day MUST replace it without complaint.

**Seeing as a creature does**

- **FR-060**: A Game Master MUST be able to enter a **creature view**: pick a
  creature on the scene and have the board drawn as that creature sees it. It
  MUST be the engine's existing viewer-token mechanism pointed at a token the
  Game Master chose, not a second way of deciding whose eyes a board uses.
- **FR-061**: A creature view MUST apply that creature's line of sight through
  walls and doors, its darkvision, its facing and field of view, and its sight
  limit — the same `VisionProfile` its player's board uses. It MUST NOT be an
  overlay drawn on top of a board that still shows everything.
- **FR-062**: A creature view MUST apply the fog as that creature's controller
  would meet it.
- **FR-063**: A creature view MUST NOT apply that creature's controller's
  explored areas, which live in that person's browser and are not the Game
  Master's to read (FR-033).
- **FR-064**: A creature view MUST say, on screen and in words, whose eyes are
  being used.
- **FR-065**: A creature view MUST be left by a single keystroke, and leaving
  it MUST restore the Game Master's own board.
- **FR-066**: A creature view MUST be a way of drawing the board, not a copy of
  it: anything that changes on the scene while the view is held MUST be
  reflected in it.
- **FR-067**: A creature view MUST NOT survive a reload. A Game Master returns
  to their own board (Decision 7).
- **FR-068**: A Game Master in a creature view MUST be able to move that
  creature, and the move MUST be judged as a move by the creature's controller
  — a Game Master — and not as a player's (Decision 6). Walls MUST NOT stop
  them, per spec 045 decision 1.
- **FR-069**: A creature view MUST NOT be offered to a player, and MUST NOT
  give a Game Master any information they could not already obtain.
- **FR-070**: A creature view MUST reuse the gestures already being built for
  moving a token with the keyboard and for looking at a creature. It MUST NOT
  introduce a second way to do either.

**Performance**

- **FR-075**: Fog MUST cost no more per frame than the engine's measured
  headroom allows: a scene at the lighting sweep's ordinary level — 4
  shadow-casting lights and 200 walls on an imported map — MUST stay in the
  same frame-rate band with fog on as with fog off.
- **FR-076**: A fully fogged scene at the sweep's largest measured level — 32
  lights and 1,600 walls — MUST remain interactive, at or above the 30fps floor
  the capacity sweeps use.
- **FR-077**: A time-of-day tint MUST add no measurable per-frame cost beyond
  what the darkness layer already pays, because it is the ambient colour that
  layer's shader already reads.
- **FR-078**: Fog MUST be bounded in size, as exploration is at 20,000 cells,
  and MUST tell the Game Master when the bound is reached.
- **FR-079**: A creature view MUST cost no more than a player's board of the
  same scene, because it is the same computation with a different viewer.
- **FR-080**: The measured numbers for fog and time of day MUST be emitted as
  generated JSON under `marketing/`, beside the existing capacity and lighting
  figures, rather than transcribed into prose.

**Accessibility**

- **FR-081**: At every time of day and under fog, text on the board — token
  names, status displays, dice results — MUST meet WCAG 2.2 AA contrast.
- **FR-082**: Fog, a creature view and the time of day MUST each be
  distinguishable without relying on colour alone.
- **FR-083**: A person who has asked their system for reduced motion MUST get
  instant changes, with no animated transition between times of day, into or
  out of a creature view, or as fog is revealed.
- **FR-084**: Every control this spec adds MUST be operable by keyboard and
  MUST be reachable with a screen reader.

**Proof**

- **FR-090**: The e2e suite MUST cover: a wall drawn with the tool stopping a
  player's token; a door drawn with the tool being opened by a player; a fog
  reveal reaching a second client within a second with that client re-reading
  nothing; fog hiding a token from a player and not from a Game Master; a time
  of day changing every board; and a creature view hiding a token behind a wall
  and showing it when the door opens.
- **FR-091**: The lighting sweep MUST be extended to measure fog and a
  time-of-day tint, and MUST fail if either moves an ordinary map out of its
  frame-rate band.
- **FR-092**: The manual playtest suite MUST exercise User Stories 1, 2, 4 and
  5 in a Genie world and a D&D 5e world, recording every seat's view.

### Key Entities

- **Fog**: for one scene, the areas a Game Master has hidden from the table.
  One fact per scene, on the server, expressed in the scene's cells. Off until
  turned on.
- **Explored area**: unchanged from spec 045 — for one player in one scene,
  the cells their own token has seen, kept in that player's browser.
- **Time of day**: a named position on a scene — dawn, day, dusk or night, or a
  pack's own names for the same four — which sets the scene's light level and
  the colour the board is tinted with. Unset on every scene that has not had
  one chosen.
- **Scene light**: unchanged — Bright, Dim or Dark, the illumination of a point
  no light reaches.
- **Wall primitive**: what a drag with the wall tool draws — a segment, a room
  or a door. It exists in the engine and gets a control.
- **Snap rule**: where a drawn point lands. Today, the grid. This spec adds an
  existing wall's endpoint and a right angle, and a modifier that suspends all
  three.
- **Creature view**: a Game Master's board drawn through one creature's eyes,
  held for as long as they hold it and never longer than the page.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: **0** controls in the Walls panel have no effect on the board.
- **SC-002**: A Game Master can draw a door in **one** step from the rail,
  without first drawing a wall and converting it.
- **SC-003**: A wall drawn with the wall tool stops a player's token in
  **100%** of runs, unless the Game Master drew one that should not.
- **SC-004**: Four walls drawn around a room with snapping on share **4**
  corner points exactly, and the room leaks **0** light.
- **SC-005**: With the free-draw modifier held, a wall's endpoints are within
  **0** units of where the pointer was released.
- **SC-006**: A fog reveal is visible on a second client within **one second**,
  with that client issuing **0** re-reads of its own — measured the way spec
  045 measures a door.
- **SC-007**: A token standing in a hidden area is drawn on a player's board
  **0** times, and on the Game Master's **every** time.
- **SC-008**: **0** bytes of a player's explored area reach the server, verified
  by inspecting the requests a playing client makes.
- **SC-009**: A Game Master can name, for any hidden part of a player's board,
  which of light, fog or exploration is hiding it, in **one** look at the rail.
- **SC-010**: A change of time of day reaches every board in the scene within
  **one second** in **100%** of runs.
- **SC-011**: At every time of day, **100%** of text drawn on the board passes
  an automated WCAG 2.2 AA contrast check.
- **SC-012**: With reduced motion requested, **0** transitions in this feature
  animate.
- **SC-013**: A creature view hides a token behind a vision-blocking wall in
  **100%** of runs and shows it within **one second** of the door opening.
- **SC-014**: A creature view is left in **one** keystroke, and **0** creature
  views survive a reload.
- **SC-015**: With fog on, an ordinary map — 4 lights, 200 walls — stays in the
  same frame-rate band as with fog off, across **all** measured runs.
- **SC-016**: A fully fogged scene at 32 lights and 1,600 walls holds at or
  above **30fps**.
- **SC-017**: The fog and time-of-day figures are emitted as generated
  `marketing/*.json`, with **0** numbers transcribed by hand.
- **SC-018**: Every control added by this spec is operable by keyboard, with
  **0** axe violations on the play view.
- **SC-019**: A scene that has had no time of day set renders identically to
  today, verified by comparison in **100%** of runs.

## Assumptions

- **The engine draws; React asks.** Fog, vision, the tint and every snap
  decision are the engine's, under constitution Principle I. React offers the
  controls and shows what the engine reports, and becomes a second source of
  truth for none of it.
- **Fog is authoritative, exploration is not, and that is the point of the
  hierarchy.** What a Game Master hides is a rule about what the table may see,
  so it lives on the server and is enforced there. What a player remembers is a
  convenience for that player, so it lives in their browser and clearing it
  costs them nothing but their own map. Spec 045 decided the second and left
  the first out; this spec does the first, and does not revisit the second.
- **Hidden still means not drawn.** Spec 045's assumption holds: the server
  sends every token to every member and the client hides what it must.
  Withholding a fogged token server-side is the same question spec 045
  deferred, and this spec defers it the same way — with the note that fog makes
  it more tempting, because a fogged room is a secret in a way a dark corridor
  is not.
- **The fog table is storage, not a design.** `fog_masks` was written for a
  bitmap by an earlier phase and nothing ever read it. Adopting its shape
  because it is there would be adopting the one shape spec 045's exploration
  module documents as wrong for this data.
- **Time of day is a name for a light and a colour**, not a simulation. There
  is no sun, no clock, no shadow angle, and nothing advances on its own.
- **An ADR records what fog is and where it lives.** Fog becoming
  server-authoritative shared scene state, and the retirement of the stored
  bitmap in favour of cells, is a decision about an ownership boundary
  (Principle IV) and is recorded as an ADR at planning time.
- **A second ADR may be needed for the creature view**, because a Game Master
  acting as a creature touches the movement-authority boundary ADR-095 drew.
  The planning step decides whether ADR-095 already answers it; this spec's
  position is that it does, because the actor is still a Game Master.
- **The measured numbers are the budget.** 1,600 tokens hold 60fps and 3,200
  hold 30 (`marketing/engine-status-capacity.json`, 2026-08-30). The lighting
  sweep runs (2, 50), (4, 200), (8, 400), (16, 800) and (32, 1,600) light-wall
  pairs against an imported map and requires the ordinary level to stay above
  20fps (`apps/web/e2e/engine-lighting-limits.spec.ts`). The darkness shader
  carries at most 128 lights and 512 shadow directions, a real ceiling under
  WebGL2 (`darkness.wgsl:23-24`). Fog must fit inside what is left, not ask for
  a new budget.

## Out of Scope

- **Keyboard movement and the "look at this creature" control**, which are
  being built alongside this spec. FR-070 builds on them and does not
  respecify them.
- **Reaching and placing things on the board**, which is spec 056: the
  right-click menu, selecting a wall by clicking it, the one "place here" menu,
  interaction points, and what an object is. This spec says what the Walls
  panel *means* and how a drawn point *lands*; 056 says how a thing is reached
  and placed. FR-002, FR-005, FR-005b and FR-018 mark the seam.
- **Making a wall a door after the fact**, which is spec 056 FR-014. This spec
  adds drawing one as a door in the first place (FR-002, FR-007).
- **A badge on a designated door.** Spec 056 FR-037 found that a door carries
  no visible sign that it responds, because badges are drawn only over token
  subjects. That is 056's to fix, and the secret flag this spec exposes
  (FR-005) is a different question — whether players are drawn the wall at all.
- **Giving an object hit points, and hiding one object from players.** Spec 056
  FR-045 and FR-050. Fog hides areas (FR-035).
- **Withholding fogged tokens server-side.** See Assumptions.
- **A clock that advances.** No sunrise over a session, no timed events, no sun
  angle, no directional shadows.
- **Per-player fog.** Fog is one scene's fact (Decision 2). If the owner wants
  fog that differs per player, Question 1 says what that costs.
- **A weather layer, rain, fog-as-atmosphere, or any effect that is not about
  what can be seen.**
- **Changing what a wall, a door, a light or a vision profile means.** Specs
  030 and 045 own those; this spec gives them controls.
- **Moving explored areas to the server**, or carrying them between a player's
  browsers. Spec 045 decided this and it stands.
- **Redrawing the play view's layout.** The rail gains an entry and its panels
  gain honest labels; the dock, the sheet and the map are untouched.
- **Automatic reveal.** Fog is lifted by a Game Master's hand. Fog that lifts
  itself as a party walks is what explored areas already are, from the other
  direction.

## Dependencies

- **Spec 001**: the wall tool and `CanvasLayer::Fog`, which has waited since.
- **Spec 003**: the wall-passability flag.
- **Spec 030**: doors — open, closed, locked and secret — which FR-005 and
  FR-007 finally expose.
- **Spec 031 FR-024 to FR-026**: grid snapping and the room and door
  primitives, built in the engine and never given a control.
- **Spec 032**: the pack architecture, through which a game system declares its
  names for the times of day (FR-049).
- **Spec 036** and **spec 005**: live sync, which fog and time of day both need
  and which `updateFogMask` does not use today.
- **Spec 045**: movement, vision, the scene's ambient light and explored areas —
  the spec this one sits on top of, and whose no-fog-of-war decision it
  supersedes.
- **Spec 051**: the pause gate, which fog already respects.
- **Spec 056**: reaching and placing the things on a map. It keeps the rail
  panel working (its FR-006) and defers where a placed thing lands to "the
  scene's snapping rule" (its FR-003) — which is FR-010 to FR-018 here. Its
  wall menu and this spec's wall panel act on one wall, through one state.
- **ADR-095**: server-side movement adjudication, which decides how a move made
  from a creature view is judged.

## Decisions (owner, 2026-09-15)

1. **Fog of war is its own layer, below lights.** The owner asked for the
   hierarchy in those words: "we have a hierarchy: lights, we need one below
   it, fog". Explored areas stay what they are and stop being mistaken for fog
   (FR-020, FR-034). This **supersedes spec 045's decision that there is no fog
   of war** — 045 built the player's memory and deliberately left the Game
   Master's cover out; this spec builds the cover.

2. **Fog is shared, not per player.** One scene, one fog, on the server. It is
   the shape the existing table already has, it is what a Game Master means
   when they uncover a room, and the per-player layer the owner might otherwise
   want already exists as explored areas. Recorded as a decision rather than a
   default because it is the one genuinely reversible choice here — Question 1
   states what reversing it costs.

3. **Fog is cells, not a bitmap.** The stored `fog_masks` bitmap is retired.
   Cells are what walls, movement and exploration already speak, they survive a
   change of zoom and of map art, they are small enough to send on every
   change, and the exploration module already wrote down why a bitmap is the
   wrong shape for exactly this data.

4. **The scene's light stays where it is, and the panel starts saying so.** The
   owner expected to find scene lighting and did not, but the answer is not a
   fourth place to put it. Lights is the right home for the scene's own light,
   the time of day and the lamps; the panel's job is to say that it holds all
   three.

5. **Time of day is on the rail, and it is named steps.** "Time of day would be
   a really good thing for the right side of the screen, not necessarily in
   settings." Dawn, day, dusk and night are what a table says out loud;
   a continuum would be a slider whose middle nobody can name. Question 2
   offers the continuum if the owner wants it.

6. **A creature view lets a Game Master act, not only look.** Moving an NPC is
   already a Game Master's right and walls already do not stop them (spec 045
   decision 1), so a view that refused the move would be a strictly worse way
   to do something they can already do — and the Game Master would leave the
   view to move the creature, which is precisely the moment they most need to
   be inside it. Validating a wall means walking into it.

7. **A creature view does not survive a reload.** It is a lens, not a state. A
   Game Master who reloads and comes back blind to half their own board, with
   no memory of why, is a support question; coming back to their own board
   costs one keystroke to re-enter.

8. **A scene with no time of day set renders exactly as it does today.** The
   darkness layer is not drawn while a scene is bright, and an unconfigured
   scene must keep that. So the tint is drawn because a Game Master chose a
   time, not because the product decided every board now has one. "Day" is a
   time somebody picked; "no time set" is the absence of the feature, and the
   two must look different in the code even where they may look the same on
   screen (FR-043a, FR-050).

## Questions for the owner

1. **Q1 — Is fog one thing for the table, or one per player?**
   *(recommendation: A)*

   | Option | Answer | Implications |
   |--------|--------|--------------|
   | **A** | **One fog per scene, shared** | What the server already stores, and what "uncover the room" means at a table. Explored areas already give each player their own layer, so the hierarchy the owner asked for is complete without this. Cheapest to build and to reason about. |
   | B | One fog per player, authored per player | Lets a Game Master show the scout what the party cannot see. Multiplies the storage and the traffic by the size of the table, needs a per-player authoring gesture nobody has asked for, and makes "reveal this room" ambiguous. |
   | C | Shared now, per player later behind a scene switch | Keeps A's cost today and leaves the door open. Risks a second fog model nobody finishes. |

2. **Q2 — Named times, or a continuum?** *(recommendation: A)*

   | Option | Answer | Implications |
   |--------|--------|--------------|
   | **A** | **Four named times: dawn, day, dusk, night** | What a table says. Four values to store, four tints to design, a pack can rename them. Each maps cleanly onto the three light levels the engine already draws. |
   | B | A continuous dial from midnight to midnight | "Darken it with nice filters" as far as it goes. Every position needs a light level and a tint, nobody can say what 14:20 looks like, and every scene gains a number that is harder to reason about than a name. |
   | C | Named times, with a hidden continuum behind them | Both costs. |

3. **Q3 — Does the time of day belong to the scene, or to the world?**
   *(recommendation: A)*

   | Option | Answer | Implications |
   |--------|--------|--------------|
   | **A** | **The scene** | A dungeon is dark at noon. Matches the scene's own light, which is already per scene, and needs no new object. A Game Master sets each scene as they open it. |
   | B | A world clock every outdoor scene reads | "It is night in the world" is one act instead of many, and it is the fiction a table actually keeps. Needs a world clock, a per-scene indoors/outdoors flag, and an override, none of which exists. |
   | C | The scene, with a world clock in a later spec | A is built now and B stays possible, at the cost of a migration later. |

4. **Q4 — Does turning fog off keep what was hidden?** *(recommendation: A)*

   | Option | Answer | Implications |
   |--------|--------|--------------|
   | **A** | **Keep it; only a reset clears it** | A Game Master who flicks fog off to check something does not lose an evening's work. Matches how exploration treats its reset (spec 045 FR-074). |
   | B | Turning it off clears it | Simpler state, one fewer thing to explain, and one accidental click that costs a prepared dungeon. |

5. **Q5 — Which rail holds the scene's light and the time of day?**
   *(recommendation: A, and this one contradicts the owner's own words, so it
   is asked rather than assumed)*

   The owner said "time of day would be a really good thing for the right side
   of the screen". The right side is `WorldDock` — Chat, Actors, Combat,
   Clocks, Settings. The scene's light, the lamps, the walls and the fog are
   all on the **left** rail, `GmToolRail`.

   | Option | Answer | Implications |
   |--------|--------|--------------|
   | **A** | **Left rail, with the lights** | The scene's light and the time of day are the same dial read two ways; splitting them across two rails is how the owner failed to find the first one. The fix for "I could not find it" is a panel that says what it holds, not a second location. |
   | B | Right dock, as the owner said | It is where he looked, and the dock is the panel side rather than the tool side — time of day is not a drawing tool. Costs a scene-light control in two places, or a scene light that lives away from the lamps it interacts with. |
   | C | Left rail, and a shortcut in the dock's Settings panel that opens it | Both discoverable and single-sourced. One more thing to keep honest. |
