# Feature Specification: Token Movement and Vision

**Feature Branch**: `045-token-movement-and-vision`

**Created**: 2026-09-11

**Status**: Built, 2026-09-11 to 2026-09-12, with one gap and two claims not
yet proven at the table (checked 2026-09-14; phases as numbered in
[tasks.md](./tasks.md)). The owner's decisions of 2026-09-11 are recorded under
[Decisions](#decisions-owner-2026-09-11); nothing is open.

- **Phase 2 — one crossing test for both sides.** Shipped: `movement_blocked_by`
  and `path_blocked_by` in `thunderforge-canvas-core`, and ADR-095 (PROPOSED).
- **Phase 3 (US3) — a door reaches every board.** Shipped and table-proven:
  the dungeon crawl checks the opened door on all three boards, hard.
- **Phase 4 (US1) — a player's keyboard walks their token.** Shipped and
  table-proven on a gridded scene, by the crawl and
  `e2e/token-keyboard-move.spec.ts`. The gridless step is built and not
  exercised by either.
- **Phase 5 (US2) — a wall stops a hero.** Shipped and table-proven: the
  engine refuses first and says so, the server refuses a crossing move sent
  straight to it (`e2e/token-movement-walls.spec.ts`), and the crawl's aimed
  drag and twelve-round wander cross no wall in either game system.
- **Phase 6 (US3, US4) — the rules of sight and light, held.** Shipped and
  table-proven: darkness alone hides a token, a player with no token sees the
  lit board, a carried light travels, and the Game Master is shown what the
  party cannot see.
- **Phase 7 (US6) — a game system says how a hero sees.** Darkvision shipped:
  D&D 5e declares it, and `combat-5e.playtest.ts` proves a sheet's sixty feet
  reach the engine as twelve cells, and a sheet edit reaches the board with no
  reload. **Not proven:** SC-007 as written — a token at 50 feet shown dimly,
  one at 70 feet hidden — is unit-tested in `thunderforge-canvas-core` and has
  not been watched on a board. **Not met:** a carried light's reach from the
  character's data is resolved by the server and never reaches the engine
  (FR-061, FR-064; tasks.md T065).
- **Phase 8 (US7) — a map that remembers.** Shipped and proven in a real
  browser by `e2e/scene-exploration.spec.ts`: the map survives a reload and a
  Game Master's reset reaches the player's storage. Not played in a playtest.
- **Phase 9 — polish.** The contract was checked against the code and amended
  where the shape changed (contracts §5). The full e2e re-measure (T063) is
  left to the next full run on `main`.

**Input**: Project owner: "file a spec for the WASD movement bug and shading
and how the token interacts with the environment"

## Context

This spec is what a table needs from its board: a hero walks where the player
sends them, stops where a wall stands, and sees what a person standing there
could see. Two of those three are broken today, and the third works without a
spec saying what it should do. All three were found by the first run of the
manual playtest suite (`pnpm playtest`, `apps/web/playtest/`), in which a Game
Master and two players run a short dungeon crawl in a Genie world and a D&D 5e
world.

### A player cannot walk their own token with the keyboard

The engine's keyboard movement (`src/engine/src/systems/token_move.rs`) is
complete on paper: a movement key steps one cell, Shift plus a movement key
plans a route drawn on the canvas, Space commits it and Escape discards it.

It drives exactly one entity: the one tagged `PlayerControlled`. The only
entity ever given that tag is the placeholder `setup_scene` spawns when the
engine starts (`src/engine/src/app.rs`). No token loaded from the server is
ever tagged, and the web tells the engine which token is a player's eyes
(`set_viewer_token`) but never which one is theirs to move. In the playtest a
player pressed D with the scene loaded and their own token on screen; the
token stayed where it was on every client and on the server. Route planning
is unreachable for the same reason.

Two more gaps sit behind that one, and neither was visible until it was fixed
(found while building phase 2, 2026-09-11):

- **The move was announced to nobody.** The keyboard emitted `update_token`,
  a shape nothing in the web has ever handled; a drag emits `upsert_token`.
  So with control fixed, a keypress moved the token on its own canvas — the
  engine reported it at the next cell's centre — and the store, the table and
  the server all still read the old position.
- **On a gridless scene the step sent nothing at all**, so even a handled
  event would not have left that canvas.

### A wall that blocks movement blocks nothing

Every wall carries a "blocks movement" flag, independent of "blocks vision".
It is authored (hand-drawn walls, map import, the Game Master's passability
toggle from spec 003 FR-001) and stored, and **nothing reads it when a token
moves**:

- the server accepts a moved position as given, from a player's move and from
  a Game Master's update alike;
- the route the engine attaches to a committed move — sent, its comment says,
  "so a server that cares can validate the path" — is dropped by the web sync,
  which forwards the position only;
- a drag checks nothing.

Spec 004 set this aside deliberately ("consistent with existing
wall-passability behavior — this spec does not change it"), and no spec since
has taken it up. The playtest records a player dragging their hero through a
solid wall, and heroes wandering through walls in its seeded random walk.

### A door opened by one person stays shut for everyone else

When a door opens, the server announces it on the world's event channel as a
door change (event 21) and records nothing about walls. A client re-reads a
scene's walls only when told a *wall* changed (event 10); on a door change it
re-reads the interactive markers and nothing else. The engine changes a
door's state only when that same client opened it, or when it reloads every
wall.

So the player who clicks a door sees it open at once, and every other board —
the Game Master's included — keeps it shut until that person reloads. It
stays shut for sight and for light alike, since both read the same walls.
In the 2026-09-11 playtest the door was opened through the server directly,
so no board opened it: with the scene then set to daylight, the goblin beyond
the open door stayed hidden from the player for the full 15 seconds the
playtest waited.

It is not only the opening. A wall *becoming* a door is announced the same
way, so it too never reaches another board: in the 2026-09-11 playtest, after
the Game Master designated the wall a door, the player's own board still held
it as a plain wall with no door state at all — which is also why her client
could not apply her own opening, having no door there to open.

The doors e2e (`interactive-doors.spec.ts`, "it reaches an open page without
a reload") did not catch this because it re-reads the walls itself before
checking — doing the one step the product never does.

### Sight and light work, and no spec says what they should do

Playtest 2026-09-10 P9 (`specs/031-playability/playtest-2026-09-10.md`) found
that walls did not shade, and the fix landed on 2026-09-10 and 2026-09-11:

- a scene has an ambient light, bright, dim or dark, set by its Game Master,
  and an imported map takes the level its file records;
- each light is stopped only by the walls between it and the point it lights,
  so a room lit from both sides stays lit behind each wall;
- a light carried by a token is shaded where the token is;
- the darkness covers the map — art, grid, shapes, images — and sits under
  tokens; a Game Master's walls and light markers stay above it as authoring
  aids, and players are not drawn light markers at all;
- a player sees through their own token, so walls, and darkness in a dim or
  dark scene, hide the tokens it cannot see; a Game Master sees through none
  and never loses a token.

That is behaviour a table relies on, and it is specified only in a defect
write-up. It also leaves the rules a table argues about unstated: what closed
and open doors do to sight, what a player with no token sees, whether party
members share sight, and whether a game system changes vision (D&D 5e's
darkvision and its bright and dim light; Genie's own rules).

There is no fog of war. The server stores a fog mask per scene and the engine
has a fog layer; nothing in the web uses either.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - A player walks their hero with the keyboard (Priority: P1)

A player at the table presses a movement key and their hero takes one step.
Everyone at the table sees it, and it is still there after a reload.

**Why this priority**: It is the most basic thing a player does on a board,
and it does not work. The code for it exists; only the connection to the
player's own token is missing.

**Independent Test**: A player who owns one token on a gridded scene presses
D. Their token is one cell east on their own screen, on the Game Master's, on
another player's, and on the server, and it is still there after the player
reloads.

**Acceptance Scenarios**:

1. **Given** a player who owns a token on a gridded scene, **When** they press
   a movement key, **Then** the token moves one cell in that direction and
   every client showing the scene shows it there.
2. **Given** a player who owns several tokens on the scene, **When** they press
   a movement key, **Then** their primary token moves, and no other.
3. **Given** a player planning a route with Shift and the movement keys,
   **When** they press Space, **Then** the token moves to the end of the route
   and every client shows it there; **When** they press Escape instead,
   **Then** the route disappears and the token does not move.
4. **Given** a gridless scene, **When** a player presses a movement key,
   **Then** the token moves one nominal cell and every client shows it there.
5. **Given** a player typing in the chat or any text field, **When** they type
   a movement letter, **Then** no token moves.
6. **Given** a player who owns no token on the scene, **When** they press a
   movement key, **Then** nothing moves and nothing is sent.

---

### User Story 2 - A wall stops a hero (Priority: P1)

A player walks their hero into a wall and the hero stops at it, however the
move was made. A door stops them while it is closed and lets them through
once it is open.

**Why this priority**: A map's walls are its rules. A wall a hero can walk
through turns every dungeon into an open field and makes a Game Master police
the board by hand.

**Independent Test**: On a scene with one wall that blocks movement, a player
tries to cross it by keyboard, by planned route and by drag, and a script
signed in as that player sends the crossing move straight to the server. The
token ends every attempt on its own side, on every client.

**Acceptance Scenarios**:

1. **Given** a player's token next to a wall that blocks movement, **When**
   they press the key that would step through it, **Then** the token stays
   where it is and the player is shown that a wall is in the way.
2. **Given** a player planning a route, **When** the route would pass through
   a wall that blocks movement, **Then** the route does not extend past the
   wall.
3. **Given** a player dragging their token, **When** they drop it on the far
   side of a wall that blocks movement, **Then** the token returns to where the
   drag began and the player is shown that a wall is in the way.
4. **Given** any client, including one that has been modified, **When** it
   asks the server to move a player's token across a wall that blocks movement,
   **Then** the server refuses, and the token stays where it was on every
   client.
5. **Given** a closed door, **When** a player moves into it, **Then** it stops
   them as a wall does; **Given** the same door open, **Then** they pass.
6. **Given** a wall that blocks vision but not movement (a window, a curtain),
   **When** a player moves through it, **Then** they pass.
7. **Given** a Game Master who toggles a wall's passability or opens a door,
   **When** a player moves next, **Then** the change applies, with no reload.
8. **Given** a Game Master, **When** they move any token across a wall or a
   closed door, by any means, **Then** it goes where they put it.

---

### User Story 3 - A player sees what their hero could see (Priority: P1)

A player's board shows what their hero could see from where they stand:
walls and closed doors hide what is behind them, and in a dim or dark scene,
what no light reaches is hidden too. The Game Master sees everything.

**Why this priority**: The first two stories give a hero a place on the board;
this one makes that place matter. It works today, and this spec makes it the
promise the rest is tested against.

**Independent Test**: A player's token stands on one side of a wall, and an
NPC stands on the other. The NPC is not drawn for the player and is drawn for
the Game Master. The Game Master opens a door in the wall, and in a bright
scene the player sees the NPC through it within a second.

**Acceptance Scenarios**:

1. **Given** a wall that blocks vision between a player's token and another
   token, **When** the player looks at the board, **Then** the other token is
   not drawn for them.
2. **Given** a closed door between them, **Then** the other token is hidden;
   **When** the door opens, **Then** it is shown, subject to light.
3. **Given** a dim or dark scene, **When** another token stands where no light
   reaches it, **Then** it is hidden from the player even with nothing in the
   way; **When** a light reaches it, **Then** it is shown.
4. **Given** any scene, **Then** a player always sees their own tokens.
5. **Given** a Game Master, **Then** every token is drawn for them, and one
   that at least one player cannot see is marked as such.
6. **Given** a Game Master who changes a wall, a door, a light or the scene's
   light, or a token that moves, **Then** every player's board reflects it
   within a second, without a reload.

---

### User Story 4 - Light shades the room as a table expects (Priority: P2)

In a dim or dark scene, the map is dark except where light falls; each light
is stopped by the walls around it; a torch a hero carries goes with them.

**Why this priority**: It shipped on 2026-09-11 and is correct as far as
anyone has checked. It is here so its rules are written down and protected,
not because it is broken.

**Independent Test**: In a dark scene with two lights either side of one wall,
each side is lit and the point behind the wall from each light is lit by the
other. A hero carrying a light walks three cells, and the light's pool follows
them.

**Acceptance Scenarios**:

1. **Given** a dim or dark scene, **Then** the map is darkened wherever no
   light reaches, and tokens are drawn above that darkness (subject to Story 3).
2. **Given** a light and a wall that blocks vision, **Then** the light does not
   reach the far side of the wall, and does reach anywhere another light lights
   that the wall does not shadow from it.
3. **Given** a closed door, **Then** it stops light as a wall does; open, it
   does not.
4. **Given** a light carried by a token, **When** the token moves by any means,
   **Then** the lit area moves with it.
5. **Given** a player, **Then** they are not drawn light markers or wall
   handles; **Given** a Game Master, **Then** they are, above the darkness.

---

### User Story 5 - The playtest shows it working (Priority: P2)

The manual playtest suite runs a whole session against these rules in Genie
and in D&D 5e, records what every seat saw, and passes.

**Why this priority**: The defects in this spec were found by watching a
session, and the only honest proof they are gone is the same session passing.

**Independent Test**: `pnpm playtest` passes, including every check it
currently records as a FINDING.

**Acceptance Scenarios**:

1. **Given** the dungeon-crawl playtest, **When** it runs, **Then** its
   keyboard step, its attempt to walk through a wall and its seeded wander all
   pass, in both game systems.

---

### User Story 6 - A game system decides how a hero sees in the dark (Priority: P2)

A D&D 5e dwarf sees in the dark that blinds a human, and a torch lights
clearly close by and dimly further out. The game system says so, not the Game
Master by hand, and Genie says what its own heroes can do.

**Why this priority**: Sight and darkness only feel right when they follow the
rules the table is playing. The engine can already see in darkness within a
range and tell bright light from dim; nothing tells it which token can.

**Independent Test**: In a dark D&D 5e scene, a character with 60 feet of
darkvision and one without stand side by side. A token 50 feet away in the
dark is shown dimly to the first and not at all to the second, and a token 70
feet away is shown to neither.

**Acceptance Scenarios**:

1. **Given** a D&D 5e character whose sheet gives them darkvision, **When** a
   token stands in darkness within that range, **Then** it is shown to them
   dimly; **When** it stands beyond it, **Then** it is hidden.
2. **Given** darkvision, **Then** it never sees through a wall that blocks
   vision or a closed door.
3. **Given** a light with a bright reach and a dim reach — a torch, bright to
   20 feet and dim for 20 feet beyond — **Then** a token in the bright part is
   shown clearly and one in the dim part dimly.
4. **Given** a Genie world, **Then** its tokens see by the rules Genie
   declares, and until it declares any, by the default rules, with no sight in
   darkness.
5. **Given** a character whose darkvision changes on their sheet, **When** the
   sheet is saved, **Then** every board applies the new range within a second.

---

### User Story 7 - A player's map remembers where they have been (Priority: P3)

In a scene where the Game Master has turned exploration on, what a player's
hero has seen stays on that player's map, faded, after the hero moves on.
What the hero has never seen stays dark.

**Why this priority**: It is the dungeon crawl's own map, drawn as the party
goes, and it makes walls and darkness matter beyond the moment. It builds on
every story above, so it comes last.

**Independent Test**: With exploration on, a player walks their hero through
two of three rooms and reloads. The map of both rooms is shown to them faded,
with no tokens in it that the hero cannot currently see, and nothing of the
third room is shown.

**Acceptance Scenarios**:

1. **Given** exploration on, **When** a player's token sees an area, **Then**
   that area stays on the player's map after the token moves away, dimmed, with
   no tokens drawn in it that the token cannot currently see.
2. **Given** an area the token has never seen, **Then** nothing of it is shown
   to the player.
3. **Given** a reload or a later session in the same browser, **Then** the
   player's explored area is as they left it; **Given** a browser whose
   storage has been cleared, **Then** it starts empty again.
4. **Given** a Game Master, **Then** they see the whole scene, and can reset
   its exploration for one player or for everyone at the table, **Then** those
   players' boards forget it without their doing anything.
5. **Given** two players, **Then** each has their own explored area, and what
   one explores is not shown to the other.
6. **Given** exploration off, the default, **Then** nothing is remembered, and
   the scene behaves as Stories 3 and 4 describe.

---

### Edge Cases

- **Corners.** Two wall segments that meet at a point leave no gap: a move
  that passes exactly through the meeting point is a crossing.
- **Along a wall.** A move that runs along a wall without crossing it is
  allowed.
- **Standing on a wall.** A wall drawn through a token's current position does
  not move the token; the token may then move away to either side.
- **A door closing on a hero.** A door closed while a token stands in the
  doorway leaves the token where it is; its next move is judged from there.
- **Large tokens.** A token bigger than a cell is judged by the path of its
  centre (see Assumptions).
- **Gridless scenes.** A keyboard step is one nominal cell, judged along the
  straight line it travels.
- **Hex scenes.** A keyboard step moves to the neighbouring hex in the key's
  direction, as the engine already does, and is judged the same way.
- **Simultaneous moves.** Two players moving at once are judged separately; a
  wall edited while a move is in flight is judged as the server holds it when
  the move arrives.
- **Moves made offline.** A move queued while a client was offline is judged
  when it reaches the server, against the walls as they are then; a refused
  queued move returns the token to its last accepted position.
- **Secret doors.** A player is not drawn a secret door, and a closed one
  stops them like any wall. The feedback does not reveal it is a door.
- **The startup placeholder.** Nothing a player does moves the engine's
  startup placeholder or sends a move for it.
- **Darkvision and a carried light together.** Both apply, and whichever shows
  a token better decides how it is shown.
- **Explored through a door.** An area seen through an open door stays
  explored after the door closes.
- **Walls edited after exploring.** Editing walls explores or un-explores
  nothing. A Game Master who reshapes a room resets its exploration.
- **Exploration turned off and on.** Turning a scene's exploration off keeps
  what players had explored; turning it on again shows it. Only a reset clears
  it.

## Requirements *(mandatory)*

### Functional Requirements

**Keyboard movement**

- **FR-001**: A player MUST be able to move their own token one cell per
  movement-key press on a gridded scene, and one nominal cell on a gridless
  scene. "Their own token" is their primary token in the scene, else the only
  token they own there.
- **FR-002**: A keyboard move MUST be persisted and shown on every client of
  the scene, exactly like a move made by dragging.
- **FR-003**: Shift with a movement key MUST extend a planned route shown on
  the canvas; Space MUST commit it; Escape MUST discard it; an unmodified
  movement key MUST discard any plan and step instead.
- **FR-004**: Movement keys MUST NOT move anything while the user is typing in
  a text field.
- **FR-005**: A player with no token of their own in the scene MUST NOT move
  anything, and nothing MUST be sent.
- **FR-006**: A Game Master with a token selected MUST be able to move that
  token with the same keys.
- **FR-007**: No input MUST move, or send a move for, anything that is not a
  token in the scene.

**Walls and doors**

- **FR-010**: A player's move MUST NOT carry a token across a wall segment that
  blocks movement, or across a closed door.
- **FR-011**: Walls that do not block movement, and open doors, MUST NOT stop a
  move.
- **FR-012**: The server MUST refuse any player's move whose path crosses a
  wall that blocks movement or a closed door, whichever client sent it. A
  refused move MUST leave the token at its last accepted position on every
  client, and the moving player MUST be told that a wall is in the way.
- **FR-013**: The engine MUST stop a keyboard step into such a wall before
  anything is sent, and show the player why.
- **FR-014**: A planned route MUST NOT extend through such a wall.
- **FR-015**: A drag dropped across such a wall MUST return the token to where
  the drag began, with the same feedback.
- **FR-016**: A move MUST be judged along the path it takes: for a step or a
  committed route, the cells it passes through in order; for a drag, the
  straight line from where it began to where it was dropped. Passing through
  the point where two walls meet MUST count as crossing.
- **FR-017**: Walls and doors MUST NOT stop a Game Master's moves, by any
  means, and the server MUST NOT refuse a Game Master's move for crossing one
  (decision 1).
- **FR-018**: A change to a wall's passability, or a door opening or closing,
  MUST apply to the next move on every client without a reload.
- **FR-019**: A closed secret door MUST stop a player's move, and the feedback
  MUST NOT reveal that it is a door.
- **FR-020**: A door opening or closing MUST reach every client's board —
  what the door hides, what light it stops and whether it can be passed —
  within one second and without a reload, whoever opened or closed it and
  however they did.

**Vision**

- **FR-030**: A player's board MUST show the scene from their own token (as
  defined in FR-001): a token that the player's token cannot see, because a
  wall that blocks vision or a closed door is in the way, MUST NOT be drawn for
  that player.
- **FR-031**: In a dim or dark scene, a token MUST NOT be drawn for a player
  unless a light reaches it, unshadowed from that light — including a light
  the player's own token carries. In a bright scene, only FR-030 applies.
- **FR-032**: A player MUST always see their own tokens.
- **FR-033**: A Game Master MUST see every token; a token at least one player
  cannot currently see MUST be marked for the Game Master.
  - **Consequence, to watch in play**: "at least one" means a split party marks
    nearly everything — with a scout through a door, each half of the party is
    a token the other half cannot see, and so is every monster near either.
    Built as written, and the engine tests state the case plainly. If a Game
    Master finds the marks too noisy to read, the alternative is to mark only
    what *no* player can see; that is a change to this requirement, not a bug.
- **FR-034**: A change to walls, doors, lights, the scene's light or token
  positions MUST be reflected on every client's board within one second, with
  no reload.
- **FR-035**: A player with no token in the scene MUST see the board as it is
  lit, with no line of sight and no exploration applied (today's behaviour;
  decision 4).

**Light**

- **FR-040**: In a dim or dark scene, the map — its art, grid, shapes and
  pasted images — MUST be darkened wherever no light reaches, and tokens MUST be
  drawn above that darkness.
- **FR-041**: Each light MUST light up to its radius, stopped only by walls
  that block vision and closed doors between it and the point lit.
- **FR-042**: A light carried by a token MUST light from wherever the token is,
  following every move.
- **FR-043**: Players MUST NOT be drawn light markers or wall handles; a Game
  Master MUST be, above the darkness.
- **FR-044**: A scene's light MUST stay the Game Master's to set, bright, dim or
  dark, and an imported map MUST take the level its file records.

**Game-system vision** (decision 2)

- **FR-060**: A game system MUST be able to declare, from an actor's system
  data, the range within which that actor's token sees darkness as dim light.
- **FR-061**: A light MUST have a bright reach and a dim reach. A Game Master
  MUST be able to set both on a placed light, and a game system MUST be able to
  set both for a light a character carries, from that character's data.
- **FR-062**: A light saved before this spec, with a single radius, MUST keep
  its look: bright to half its radius and dim to the whole of it.
- **FR-063**: Sight in darkness MUST show darkness as dim, only within its
  range, and never through a wall that blocks vision or a closed door.
- **FR-064**: D&D 5e MUST declare its characters' darkvision and the reach of
  the lights they carry.
- **FR-065**: Genie MUST declare its own vision rules the same way. Until it
  declares any, a Genie token MUST see by the default rules, with no sight in
  darkness.
- **FR-066**: A token whose game system declares nothing MUST see by the
  default rules.
- **FR-067**: A change to an actor's vision on its sheet MUST reach every board
  within one second.

**Explored areas** (decision 3)

- **FR-070**: A Game Master MUST be able to turn exploration on or off for a
  scene. It MUST be off for every existing scene, and for a new one, until
  turned on.
- **FR-071**: With exploration on, everything a player's own token has seen in
  the scene MUST be remembered for that player.
- **FR-072**: A player's board MUST show an explored area they cannot currently
  see with its map dimmed and no tokens in it, and MUST show nothing of an area
  never seen.
- **FR-073**: A player's explored area MUST persist across reloads and later
  sessions **in that player's own browser**. It is kept on the player's
  machine, not on the server: a player who clears their browser's storage
  starts that scene unexplored again, and that is theirs to lose (decision 3).
- **FR-074**: A Game Master MUST be able to reset a scene's exploration from
  their own tools, for one player or for everyone at the table.
- **FR-078**: A reset MUST reach the browsers it applies to within a second and
  clear what they kept, without those players doing anything. A player who was
  not looking at the scene when it happened MUST NOT get the cleared
  exploration back when they next open it.
- **FR-075**: A Game Master's board MUST show the whole scene whatever has been
  explored.
- **FR-076**: A player's explored area MUST be theirs alone; nothing one player
  explores MUST be shown to another.
- **FR-077**: A player with no token in the scene explores nothing, and sees as
  FR-035 says.

**Proof**

- **FR-050**: The manual playtest suite MUST exercise User Stories 1–4, 6 and 7
  in a
  Genie world and a D&D 5e world, recording every seat's view, and its checks
  currently marked FINDING MUST pass.
- **FR-051**: The e2e suite MUST cover a keyboard move that persists and
  reaches another client; a crossing move sent straight to the server by a
  player's session being refused; and a door opened and closed by one client
  changing both passage and sight live on another client's board, with that
  other client re-reading nothing itself.

### Key Entities

- **Token**: a piece on a scene. Has a position, an owner, whether it is its
  owner's primary token, and optionally a light it carries.
- **Wall segment**: a line on a scene. Has whether it blocks movement, whether
  it blocks vision, and optionally a door state (closed or open) and a secret
  flag.
- **Light**: a point on a scene, or carried by a token. Has a bright reach, a
  dim reach, and whether it casts shadows.
- **Scene**: has a light level (bright, dim, dark), a grid (square, hex or
  none, and a cell size), and whether exploration is on.
- **Move**: a request to put a token somewhere. Has the token, who asked, where
  it starts, where it ends, and the path between; it is accepted or refused.
- **Viewer**: whose eyes a board is drawn through — a player's own token, or,
  for a Game Master, none.
- **Vision profile**: how a token sees — the range within which it sees
  darkness as dim — declared by its game system from its actor's data. A light
  the character carries is declared beside it, and is a light, not part of the
  profile (decision 6).
- **Explored area**: for one player in one scene, everything their token has
  seen while exploration was on. Kept in that player's own browser, until a
  Game Master resets it or the player clears their storage.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: In the dungeon-crawl playtest, every movement-key press by a
  player moves their token one cell, and every seated client shows the new
  position within one second, in both game systems.
- **SC-002**: No hero ends a move on the far side of a wall that blocks
  movement: not when one is aimed straight through a wall, and not across the
  playtest's seeded wander of twelve rounds in each of two game systems. Today
  the aimed move goes through, in both systems.
- **SC-003**: Every crossing move sent straight to the server by a player's
  session is refused, and in every case the token stays where it was on every
  client.
- **SC-004**: After a Game Master opens or closes a door, moves a light or
  changes the scene's light, every player's board shows the result within one
  second.
- **SC-005**: Across a whole playtest session, the Game Master's board never
  hides a token.
- **SC-006**: A player whose move is stopped by a wall is shown why at the
  moment it happens, and can tell a wall stopped them rather than the game
  failing.
- **SC-007**: In a dark D&D 5e scene, a character with 60 feet of darkvision is
  shown a token 50 feet away in darkness, dimly, and is not shown one 70 feet
  away; a character without darkvision is shown neither.
- **SC-008**: With exploration on, after a player walks through two of three
  rooms and reloads in the same browser, their board shows the map of both
  rooms and nothing of the third; and after the Game Master resets the scene's
  exploration, it shows nothing of any of them, within a second and without
  the player doing anything.

## Assumptions

- **A move is judged by its token's centre.** A large token's footprint
  colliding with walls is not modelled; its centre's path is. Footprint
  collision can follow if play shows it matters.
- **Players see only through their own token.** Party members do not share
  sight or explored areas (decision 5).
- **Hidden means not drawn, not withheld.** The server still sends every token
  to every member of the scene, and a player's client hides what they cannot
  see. A determined player can read hidden positions from their own network
  traffic. Withholding hidden tokens server-side is out of scope here.
- **Distance and turns are not enforced.** How far a hero may move, whose turn
  it is and difficult terrain stay with the Game Master and the game system.
- **A system's distances convert through the grid.** A game system states
  ranges in its own units — D&D 5e in feet, five to a square — and one cell of
  the scene's grid is one of that system's squares.
- **Judging a move is fast.** For scenes of up to 2,000 wall segments, the
  server's check adds no delay a player can notice.
- **An ADR records movement authority.** The server judging token movement is
  a new decision about an ownership boundary (Principle IV) and is recorded as
  an ADR at planning time.

## Out of Scope

- Movement budgets, turn order, difficult terrain, elevation, and tokens
  blocking each other.
- Server-side withholding of tokens a player cannot see (see Assumptions).
- Sound and hearing.
- Fog a Game Master paints by hand, using the per-scene fog mask the server
  already stores. Explored areas are what this spec builds.
- Keeping explored areas on the server, or carrying them from one of a
  player's browsers to another. What a browser forgets is forgotten.
- Shared sight and shared exploration between party members.
- Vision rules beyond sight in darkness and light's reach — blindsight,
  truesight, magical darkness — which a game system may add later the same
  way.

## Dependencies

- **Spec 002**: walls as authored segments.
- **Spec 003 FR-001**: the Game Master's wall-passability toggle, which this
  spec finally gives an effect.
- **Spec 004**: token authoring, which deferred wall passability to a later
  spec — this one.
- **Spec 005 and spec 036**: live canvas sync and concurrent sessions, which
  carry moves and their refusals to every client.
- **Spec 028**: the client world cache, whose offline queue replays moves.
- **Spec 030**: interactive elements, which are how doors open and close.
- **Spec 031 (P9)**: the lighting and line-of-sight work this spec writes down.
- **Spec 032**: the pack architecture, through which a game system declares
  its vision rules.

## Decisions (owner, 2026-09-11)

1. **Walls do not stop the Game Master** (Q1: A). A Game Master moves any token
   anywhere; walls and doors rule players' moves only (FR-017).
2. **Game systems decide how their tokens see** (Q2: B). A system declares
   sight in darkness, and the reach of the lights its characters carry, from
   their data. D&D 5e uses it, and Genie declares its own rules (User Story 6,
   FR-060–FR-067).
3. **Players' maps remember what they have explored** (Q3: B). What a player's
   token has seen stays faintly on that player's map, in scenes where the Game
   Master turns exploration on (User Story 7, FR-070–FR-078). It is kept in the
   player's own browser — clearing their storage loses it, and that is theirs
   to lose — and a Game Master can reset it for one player or for everyone.
   Fog a Game Master paints by hand stays out of scope.

Defaults this spec takes, to revisit if play shows otherwise:

4. **A player with no token sees the whole lit board** (FR-035), as today. The
   alternative is that they see nothing until given a token.
5. **Each player sees, and explores, only from their own token.** The
   alternative is shared party sight, where a player sees what any of their
   allies' tokens can see.

Decided since:

6. **A carried light is a light attached to its token** (owner, 2026-09-14).
   It is not part of the token's vision profile. A torch on a character's
   sheet lights the room for everyone at the table, casts shadows like any
   other light, and moves with its token. The alternative — the light as part
   of how its bearer sees — would light the dark for the bearer alone, which is
   not what a torch does. The engine keeps it among the scene's lights, placed
   where its token is; the sheet, not a Game Master, owns it, so nobody drags,
   resizes or deletes it on the board (FR-061, FR-064; tasks.md T065).
