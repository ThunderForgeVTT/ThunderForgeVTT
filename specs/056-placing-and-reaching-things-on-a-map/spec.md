# Feature Specification: Placing and Reaching the Things on a Map

**Feature Branch**: `056-placing-and-reaching-things-on-a-map`

**Created**: 2026-09-15

**Status**: Draft

**Input**: Owner, 2026-09-15 — "The selection of interactions doesn't really make
sense. If you're selecting a wall — walls don't show IDs on screen, so how is the
GM able to select what wall becomes an interaction point? Really, there should be
a right-click context menu on a wall: left click, select the wall, right click,
see the context menu specifically for it. I like the place light here or place
token, but it'd be nice to have start a wall here if you click anywhere on the
map. The actual interactions need work for interaction points on the map. If I'm
doing interactions for lore, map, et cetera, those should all be different things
we could place. Same with being able to place a token: a token could be an NPC, a
token could also be an object, like a chest. Maybe we want something other than
token — maybe object — because a token is interactable whereas an object is not.
That'd be really nice to have a distinction between the two."

## Overview

Spec 030 built the machinery: an interactive is a row, it carries one effect from
a contributed registry, the server decides who may fire it, and the result
reaches every client. All of that works. What is missing is the half a Game
Master actually touches — **how they put a thing on the map, and how they get
back to it afterwards.**

Today the answer is a rail panel with dropdowns of truncated UUIDs. This spec
replaces the way in with two gestures a Game Master already knows: left-click to
select, right-click for what this thing can do. It adds the things the menu is
missing — an object, a wall, an interaction point — and it says what an object
*is*, which the product has never decided.

This spec does not replace spec 030. It amends it, and every amendment is named
in [Where this amends spec 030](#where-this-amends-spec-030).

## The problem, as the code has it today

### Choosing a wall means choosing a truncated UUID

The interaction panel picks its subject from whatever the canvas has selected:

```
if (selectedWallId) return { kind: "door", ref: selectedWallId };
if (selectedTokenId) return { kind: "prop", ref: selectedTokenId };
return null;
```

(`apps/web/src/components/canvas-tools/InteractionTool/InteractionTool.tsx:151-153`)

and when an effect needs a *target* wall — `door.set_state`, `door.set_lock`,
`door.reveal` all take a `Reference` of kind `wall`
(`crates/thunderforge-canvas-core/src/wall.rs:196-251`) — it offers a list built
like this:

```
label: wall.doorState === "none"
  ? `Wall ${wall.id.slice(0, 8)}`
  : `Door ${wall.id.slice(0, 8)} (${wall.doorState})`
```

(`InteractionTool.tsx:166-176`; lights get `Light ${light.id.slice(0, 8)}` at
`:173-176`.) These are rendered as a `<Select>` per config field whose
`referenceOf` matches (`apps/web/src/components/InteractionAuthor/InteractionAuthor.tsx:253-262`).

Nothing on the board draws that id. The owner's question — how does a Game Master
know which of eleven `Wall 3f2a1b9c` entries is the one they are looking at — has
no answer, because there isn't one.

The panel that *does* the door work says so itself. `DoorControls` — designate,
lock, secret, all three in one place — carries this in its own doc comment
(`apps/web/src/components/InteractionAuthor/DoorControls.tsx:14-18`):

> Meant to be shown from the canvas's secondary interaction — a right-click on a
> door.

It is reachable only from the Interactions rail when a wall happens to be
selected (`InteractionTool.tsx:312, 322-324`). The component was written for the
gesture this spec specifies, and never got it.

### A wall *can* be selected, but only inside the wall tool, and a miss draws a new wall

Wall picking exists and works. `handle_wall_input` grabs an endpoint, else
body-selects within `WALL_SELECT_DISTANCE`, and emits `select_wall` to chrome
(`src/engine/src/systems/wall.rs:320-357`). Two things make it unusable as "left
click, select the wall":

1. It runs only in `AuthoringMode::Walls`
   (`src/engine/src/plugins/wall.rs:84-94`). In any other mode a click on a wall
   selects nothing.
2. In that mode, a click that hits no wall **starts drawing one**
   (`src/engine/src/systems/wall.rs:359-364`). So the mode where a wall is
   reachable is also the mode where a stray click creates geometry.

### The right-click menu knows about tokens and nothing else

The play-field menu landed on 2026-09-15 (`da13758`). The engine reports the
gesture and what its hit test found, and what it can find is tokens:

```
export interface CanvasContextMenuEvent {
  type: "canvas_context_menu";
  worldX: number; worldY: number;
  screenX: number; screenY: number;
  tokenIds: string[];
}
```

(`apps/web/src/engine/bevy/index.ts:588-595`)

and the rule for what that offers is:

```
if (!target) {
  return viewer.isGameMaster
    ? [{ kind: "place-token" }, { kind: "add-light" }]
    : [];
}
```

(`apps/web/src/components/world/CanvasContextMenu/canvasMenuActions.ts:66-70`;
a Game Master on a token gets damage, heal, link/copy, hide name, remove, at
`:72-81`.)

So the gesture is right and the reach is short. A wall cannot be right-clicked at
all — not because the menu refuses, but because the event carries no wall. The
hit test the engine would need already exists and is already used by left-click:
`distance_point_to_segment` against `WALL_SELECT_DISTANCE = 6.0`
(`src/engine/src/systems/wall.rs:34, 147-156`). What is missing is the report
carrying the result (`src/engine/src/plugins/context_menu.rs:184-212`).

One property of that report is worth keeping: it fires on release, and only if
the pointer did not travel, because a right-drag pans the map
(`context_menu.rs:149-168`). And it already filters out tokens the viewer's
board has hidden (`context_menu.rs:186`) — so the menu never mentions a token in
darkness. Extending the report to walls must keep both.

### Two of the five things a Game Master places have no working button

The rail's *Place light* button only flips local React state — its own doc says
it is "ready to be observed by the engine bridge once that lands"
(`apps/web/src/components/canvas-tools/LightingTool/LightingTool.tsx:73-78,
231-239`). The rail's *Draw wall* button is the same
(`apps/web/src/components/canvas-tools/WallTool/WallTool.tsx:36-43, 86-94`); the
working route is arming `AuthoringMode::Walls` from the rail. The only light
placement that works end to end today is the context menu's *Add a light here*
(`CanvasContextMenu.tsx:290-310`).

That is the strongest argument for decision 2 that the code makes on its own:
the menu is already the route that works.

### An interaction point that is not on a token is invisible to the person it is for

Spec 030's `region` subject is already a free-standing interactive: no
`subject_ref`, a `geometry` of a rect or a polygon instead
(`crates/thunderforge-canvas-core/src/interaction.rs:384-398`, XOR enforced at
`:574-584` and by `interactives_subject_shape` in
`src/server/migrations/2026-08-30-100000-0000_create_interactives/up.sql:48-51`).
Two things make it unusable as "a lore marker a player can click":

1. **The server does not send a region to a player at all** —
   `src/server/src/graphql/queries/interactives.rs:270-272` skips every
   `subject_kind == "region"` row for a non-GM, deliberately, so a player never
   learns one exists.
2. **The engine draws a badge only over a token.** `interaction_marker.rs` looks
   its subject up in `TokenEntities` and skips anything it cannot find
   (`src/engine/src/plugins/interaction_marker.rs:179-190`). A region has no
   subject; a *door* has a wall id, which is not in `TokenEntities` either — so a
   designated door carries no badge today.

And `enter` triggers are evaluated in the engine only: `entries_for`'s single
caller is `src/engine/src/plugins/interaction.rs:177-183`. The server has no
movement hook that fires a region.

### `Object` is a kind that already exists and decides nothing

`TokenKind` has carried four variants since spec 029
(`crates/thunderforge-canvas-core/src/token_kind.rs:40-49`), and the `Object`
one is documented as "Scenery a player can interact with: a barrel, a lever, a
corpse" (`:48`). It reaches the wire as `"object"`
(`apps/web/src/types/token.ts:45-52`) and `placeProp` already uses it —
`createToken({ tokenType: "object" })` with no actor
(`apps/web/src/api/interactives.ts:placeProp`).

What the kind does *not* do is change any rule. It picks a fill colour and
nothing else. So the owner's distinction — a token is interactable, an object is
not — is a word the codebase already has and has never spent.

### What an actorless token can and cannot do today, exactly

This matters because decision 3 gives an object hit points, and the shape of that
is not obvious:

- **It blocks nothing, and this needs no work.** Movement is refused by walls
  only — `movement_blocked_by` / `path_blocked_by` over a `WallSet`
  (`src/server/src/movement/mod.rs:75,101`). No token, of any kind, has ever
  blocked a move.
- **It enters no turn order unless somebody puts it there.** Nothing populates
  `world_combatants` automatically; a combatant is created by `addCombatant`
  (`apps/web/src/api/combat.ts`). An object is out of the order for the same
  reason a chair is: nobody added it.
- **It has no hit points, and there is no way to give it any.** ADR-102: a token
  with no actor is a marker — `linked = false`, `system_data = NULL`, no bars.
  `changeHitPoints` on it reaches the copy branch, finds `system_data` null and
  refuses with *"That creature has no hit points recorded"*
  (`src/server/src/combat/hit_points.rs:393-399`). The constraint
  `tokens_linked_has_no_system_data`
  (`src/server/migrations/2026-09-14-210000-0000_token_links/up.sql:60-62`)
  permits `system_data` on an unlinked token, so the column is free; what is
  missing is any path that writes it, and a game system to read it through —
  hit-point fields are resolved from the actor's system, else the world's
  (`src/server/src/combat/hit_points.rs:331-350`).
- **It cannot be hidden from players.** NPC visibility (owner decision,
  2026-09-15) hangs on `world_actors.visible_to_players`
  (`src/server/src/schema.rs:770`;
  `src/server/src/auth/npc_visibility_tests.rs:1-6`). A token with no actor has
  nothing to hang it on. Compare shapes, which carry their own
  `visible_to_players` (`src/server/src/schema.rs:1087`), and lights and wall
  handles, which spec 045 FR-043 keeps off a player's board entirely.

## The gesture this spec builds on, and does not respecify

The play-field right-click menu exists as of `da13758`: the engine reports the
click, `canvasMenuActions` decides what that viewer may do, a Radix menu draws
it, arrow keys walk it, Escape closes it, focus returns to the canvas, and
`ContextMenu` / `Shift+F10` opens it from the keyboard on the selected token —
or, for a Game Master with nothing selected, at the camera's centre
(`apps/web/src/components/world/CanvasContextMenu/useCanvasContextMenu.ts:29-33,
76-118`).

**None of that is respecified here.** Attack, damage, heal, link/copy, hide name
and remove stay exactly as they are. This spec adds what the menu cannot yet
reach: walls, objects, interaction points, and starting a wall.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - A wall becomes a door without anybody typing an id (Priority: P1)

A Game Master looks at a corridor, clicks the wall segment across the doorway,
right-clicks it, and chooses *Make it a door*. It is a door. They right-click it
again and lock it. At no point do they read, choose or recognise an identifier.

**Why this priority**: it is the owner's first complaint and the whole thesis in
one gesture. Everything else in this spec is the same idea applied to another
kind of thing.

**Independent Test**: on a scene with several walls, designate one as a door by
clicking and right-clicking it; confirm it draws as a door, opens to a player's
click, and that the Game Master never saw an id. Delivers the door workflow on
its own.

**Acceptance Scenarios**:

1. **Given** a Game Master in any mode that is not a drawing tool, **When** they
   left-click a wall segment, **Then** it is selected and drawn as selected, and
   nothing is created.
2. **Given** a selected wall, **When** they right-click it, **Then** a menu
   opens naming that wall's own actions, and the actions are the wall's — not
   the board's.
3. **Given** a plain wall, **When** they choose *Make it a door*, **Then** it
   becomes a closed door, draws distinguishably, and a player's click opens it.
4. **Given** a door, **When** they choose *Lock*, *Make it secret* or *Stop
   being a door*, **Then** each takes effect immediately and reaches every
   viewer without a reload.
5. **Given** a wall the Game Master has not selected, **When** they right-click
   it directly, **Then** it is selected and its menu opens — one gesture, not
   two.
6. **Given** a player, **When** they right-click a wall, **Then** they are
   offered only what the server would let them do to it: open or close an
   unlocked door, and nothing at all for a plain wall or a locked one.

---

### User Story 2 - One menu that places everything (Priority: P1)

A Game Master right-clicks bare board and is offered, in one list: a creature, an
object, a light, the start of a wall, and an interaction point. They choose
*Start a wall here* and the next clicks draw one, ending where they say it ends.

**Why this priority**: it is the second half of the same thesis — one gesture to
learn for everything you put down — and it is what makes the rail panel optional
rather than mandatory.

**Independent Test**: right-click empty board and place one of each; confirm each
lands where the click was, and that starting a wall from the menu draws a wall
without the Game Master having first armed the wall tool.

**Acceptance Scenarios**:

1. **Given** a Game Master, **When** they right-click bare board, **Then** the
   menu offers placing a creature, an object, a light, the start of a wall and
   an interaction point.
2. **Given** that menu, **When** they choose *Start a wall here*, **Then** a wall
   begins at that point and is drawn by subsequent clicks, and there is an
   explicit way to finish and to abandon it.
3. **Given** that menu, **When** they choose *Place an object*, **Then** they
   name it and give it artwork, and it appears as an object rather than as a
   creature.
4. **Given** a player, **When** they right-click bare board, **Then** they are
   offered nothing, as today.
5. **Given** any placement made from this menu, **When** it lands, **Then** it is
   at the point that was right-clicked, subject to the scene's snapping rule.

---

### User Story 3 - A chest a player can open (Priority: P1)

A Game Master places an object, names it *Iron-bound chest*, and attaches
*Open a lore page* to it pointing at the entry describing what is inside. A
player clicks the chest and reads it. Nobody rolled initiative for the chest,
nobody can attack it, and it stopped nobody walking past.

**Why this priority**: it is the smallest complete proof that an object is a
distinct thing with its own rules, and it is the success criterion the owner
named.

**Independent Test**: place an object, attach an interaction, click it as a
player, read the page. Then confirm it is absent from the turn order, offered no
attack, and blocks no movement.

**Acceptance Scenarios**:

1. **Given** a placed object with no interaction, **When** a player clicks it,
   **Then** nothing happens and no error is shown.
2. **Given** a Game Master, **When** they right-click an object, **Then** the
   menu offers giving it an interaction, giving it hit points, renaming it,
   hiding it and removing it — and does **not** offer damage, heal or
   link-to-actor unless it has been given hit points.
3. **Given** an object with an interaction, **When** a player clicks it,
   **Then** the effect runs or is refused exactly as spec 030 decides, and a
   badge showed them it was there to click.
4. **Given** an object, **When** a creature moves through its square, **Then**
   the move is allowed.
5. **Given** a running combat, **When** the turn order is read, **Then** no
   object appears in it unless a Game Master added it by hand.
6. **Given** a player, **When** they right-click an object with no hit points,
   **Then** no attack is offered, and an attack sent anyway is refused by the
   server.

---

### User Story 4 - A lore marker on bare board (Priority: P2)

A Game Master right-clicks the mouth of a cave, chooses *Place an interaction
point*, and picks *Open a lore page* and the entry. A marker appears. A player
clicks it and reads the entry. The marker is not a creature, is not an object,
and has no artwork to choose.

**Why this priority**: the owner asked for lore, map and the rest to be
"different things we could place". This is the thing that is none of the others —
a point on the board whose whole purpose is to open something. It is P2 because
it needs the place-here menu of Story 2 to exist.

**Independent Test**: place an interaction point on bare board, click it as a
player, read what it opens. Confirm the Game Master sees it as theirs and a
player sees it as something to click.

**Acceptance Scenarios**:

1. **Given** a Game Master, **When** they place an interaction point and choose
   what it opens, **Then** it is drawn on the board for them, distinguishably
   from a token.
2. **Given** an interaction point a player may activate, **When** that player
   views the scene, **Then** they are shown it is there, and clicking it runs
   the effect.
3. **Given** an interaction point restricted to the Game Master, **When** a
   player views the scene, **Then** it is not shown to them and not offered.
4. **Given** an interaction point, **When** the Game Master right-clicks it,
   **Then** they may change what it opens, restrict it, require their approval,
   move it and remove it.
5. **Given** an interaction point whose target has been deleted, **When** it is
   activated, **Then** the Game Master is told which target is missing and the
   scene stays usable.

---

### User Story 5 - An object the Game Master decides can be broken (Priority: P2)

A barred door is an object in the middle of a room. The Game Master gives it 20
hit points. Now it can be attacked and damaged, and a bar appears on it. It still
takes no turn, and when it reaches zero the Game Master decides what that means —
the product marks it out of the fight and does nothing else to it.

**Why this priority**: it is the boundary decision 3 draws, and it is the one
place where an object stops being inert. It is P2 because a table gets value from
the chest before it gets value from the breakable door.

**Independent Test**: give an object hit points, attack it, damage it, watch the
bar move on every client, and confirm it never took a turn.

**Acceptance Scenarios**:

1. **Given** an object with no hit points, **When** the Game Master gives it hit
   points, **Then** it gains a current and a maximum, and a bar the table can
   see.
2. **Given** an object with hit points, **When** a player attacks it, **Then**
   the attack resolves the way spec 046 resolves any attack.
3. **Given** an object with hit points, **When** it reaches zero, **Then** it is
   marked out, it is not removed from the board, and nothing else about the
   scene changes on its own.
4. **Given** an object with hit points, **When** a combat is running, **Then**
   it is still absent from the turn order unless the Game Master adds it.
5. **Given** an object with hit points that the Game Master takes away again,
   **When** they do, **Then** it becomes inert scenery again and no attack is
   offered on it.

---

### User Story 6 - Everything by keyboard (Priority: P2)

A Game Master who does not use a pointer selects a wall with the keyboard, opens
its menu with the ContextMenu key, and makes it a door. They place an object at
the camera's centre the same way.

**Why this priority**: a right-click menu is unreachable otherwise, and this spec
moves the primary route to work *into* one. Adding a gesture that a keyboard
cannot reach would be a regression against what the rail panel offers now.

**Independent Test**: complete Stories 1, 2, 3 and 4 with the pointer
unavailable, and confirm each produces the same result.

**Acceptance Scenarios**:

1. **Given** the keyboard alone, **When** the Game Master cycles selection,
   **Then** walls and interaction points are reachable alongside tokens, and the
   selected one is drawn as selected.
2. **Given** a wall selected by keyboard, **When** ContextMenu or Shift+F10 is
   pressed, **Then** that wall's menu opens, with focus on its first item.
3. **Given** nothing selected, **When** a Game Master presses ContextMenu,
   **Then** the place-here menu opens at the camera's centre, as it does today.
4. **Given** a menu open, **When** the person presses Escape, **Then** it closes
   and focus returns to where it came from.
5. **Given** any menu item that needs more than a click, **When** it is chosen,
   **Then** the dialog it opens takes focus and hands it back on close.

---

### User Story 7 - A player is never offered what the server would refuse (Priority: P1)

A player right-clicks a locked door, a secret door, a hidden object and a
GM-only interaction point. They are offered nothing they cannot do, and told
nothing they should not know.

**Why this priority**: the menu is a new surface that states permission, and a
menu that guesses wrong either lies to a player or leaks the Game Master's
preparation. It is P1 because it has to be true from the first item added, not
audited afterwards.

**Independent Test**: as a player, open a menu on each of those and assert the
item list; then send each refused action to the server directly and assert it is
refused there too.

**Acceptance Scenarios**:

1. **Given** a locked door, **When** a player right-clicks it, **Then** no
   open/close item is offered, and one sent anyway is refused.
2. **Given** a secret door not yet revealed, **When** a player right-clicks
   where it is, **Then** they get the bare-board menu, and nothing tells them a
   door is there.
3. **Given** a GM-only interaction point, **When** a player right-clicks where
   it is, **Then** they get the bare-board menu.
4. **Given** an interaction point requiring approval, **When** a player
   activates it, **Then** they are told a request was raised, not that it
   worked.
5. **Given** any item offered to a player, **When** they choose it, **Then** the
   server performs it — no menu item exists that the server refuses.

---

### Edge Cases

- **A right-click lands on a wall and a token at once.** The topmost drawable
  thing wins, and the rule is stated rather than being draw order by accident;
  the menu says what it is about in its own title.
- **A right-click lands on two walls crossing.** One wins deterministically, and
  there is a way to reach the other — otherwise a wall under a junction is
  unreachable forever.
- **A wall is deleted while its menu is open.** The menu closes rather than
  acting on a wall that is gone.
- **An interaction is attached to a wall, and the wall is deleted.**
  `drop_for_subject` exists for exactly this and has no callers
  (`src/server/src/interaction.rs:276-284`); deleting a wall
  (`src/server/src/graphql/mutations_walls.rs:238-239`) or a token
  (`src/server/src/graphql/mutations_tokens.rs:265-266`) leaves an orphan row
  pointing at a vanished id, invisible to the database because `subject_ref` is
  deliberately not a foreign key
  (`src/server/migrations/2026-08-30-100000-0000_create_interactives/up.sql:13-18`).
  Making a wall reachable makes this reachable too.
- **An interaction point is placed on top of an object.** Both are clickable;
  which one a click means must be decided, not discovered.
- **A wall is made a door twice.** Designation already auto-creates a toggle
  interactive when none exists
  (`src/server/src/graphql/mutations_interactives_support.rs:488-517`); a second
  designation must not create a second one.
- **An object is given hit points in a world whose game system declares none.**
  `combat_for_system` has no `hit_points` and the change is refused
  (`src/server/src/combat/hit_points.rs:352-355`). The menu must not offer it.
- **An object with hit points is copied with the scene.** `scene_copy` does not
  copy interactives; it counts them and notes they were left behind
  (`src/server/src/collections/scene_copy.rs:307-317`). A copied object arrives
  inert.
- **A player's own token is standing on an interaction point.** The point stays
  reachable; a token must not make it unclickable.

## Requirements *(mandatory)*

### Functional Requirements

#### What may be placed

- **FR-001**: A Game Master MUST be able to place, from one menu opened on bare
  board, each of: a **creature**, an **object**, a **light**, the **start of a
  wall**, and an **interaction point**.
- **FR-002**: Each entry MUST say what it places in a Game Master's words, and
  the five MUST be distinguishable without documentation.
- **FR-003**: Everything placed MUST land at the point the menu was opened on,
  subject to the scene's snapping rule.
- **FR-004**: *Start a wall here* MUST begin a wall at that point without the
  Game Master having first armed a drawing tool, and MUST offer an explicit way
  to finish the wall and to abandon it.
- **FR-005**: The placing menu MUST offer a player nothing, as it does today
  (`canvasMenuActions.ts:66-70`).
- **FR-006**: The rail panel MUST keep working. This spec adds a way in; it does
  not remove the one that exists.

#### Reaching a wall

- **FR-010**: A Game Master MUST be able to select a wall by left-clicking it,
  in the ordinary selecting mode — not only while a drawing tool is armed.
- **FR-011**: A left-click that selects a wall MUST NOT create anything. The
  present behaviour, where a miss in `AuthoringMode::Walls` starts a new wall
  (`src/engine/src/systems/wall.rs:359-364`), MUST remain confined to the
  drawing tool.
- **FR-012**: A right-click on a wall MUST open a menu for **that wall**, and
  MUST select it if it was not already selected.
- **FR-013**: The engine's context-menu report MUST carry what its hit test
  found under the pointer including walls and interaction points, not tokens
  alone (`apps/web/src/engine/bevy/index.ts:588-595`).
- **FR-014**: A wall's menu MUST offer a Game Master, at minimum: make it a door
  or stop being one; lock and unlock; make it secret and reveal it; make it an
  interaction point; delete it.
- **FR-015**: A wall's menu MUST offer a player only what the server would allow
  them: opening or closing an unlocked door, and nothing for a plain wall, a
  locked door or an unrevealed secret door.
- **FR-016**: Where the same click could mean two things — a wall under a token,
  two walls crossing — the product MUST resolve it by a stated rule, and MUST
  provide a way to reach the thing that lost.
- **FR-017**: The id-picker lists MUST become a fallback rather than the way in.
  Where a reference is still chosen from a list, each entry MUST be named by
  something a Game Master can recognise on the board, never by a truncated
  identifier (`InteractionTool.tsx:168-176`).

#### Reaching a token, an object and an interaction point

- **FR-020**: A right-click on an **object** MUST offer a Game Master: attach or
  change an interaction; give or remove hit points; rename; hide from players;
  remove from the board.
- **FR-021**: An object's menu MUST NOT offer damage, heal or link-to-actor
  while the object has no hit points — those are refused by the server today
  (`src/server/src/combat/hit_points.rs:393-399`) and a menu that offers them
  would be offering a refusal.
- **FR-022**: A right-click on an **interaction point** MUST offer a Game
  Master: change what it opens; restrict it to themselves; require their
  approval; move it; remove it.
- **FR-023**: The creature menu MUST keep exactly what it has today — attack,
  damage, heal, link or copy, hide name, remove. This spec does not change it.
- **FR-024**: Every menu MUST name what it is about, so a Game Master with two
  things near one another knows which one they opened.

#### An interaction point

- **FR-030**: An **interaction point** MUST be a placeable thing in its own
  right: a position on the scene carrying one effect, with no artwork to choose,
  no sheet, no turn and no hit points.
- **FR-031**: An interaction point MUST be able to open a lore entry, ask to
  travel to a scene, or carry any other effect the registry contributes for it —
  the vocabulary MUST remain the contributed union, never a list written here
  (spec 030 FR-021, FR-039).
- **FR-032**: A player who may activate an interaction point MUST be shown that
  it is there. This MUST NOT reuse the present region behaviour, which withholds
  a region from a player entirely
  (`src/server/src/graphql/queries/interactives.rs:270-272`).
- **FR-033**: An interaction point restricted to the Game Master, or one a
  player may not activate, MUST NOT be drawn for that player and MUST NOT be
  offered to them.
- **FR-034**: An interaction point MUST be drawn distinguishably from a token
  and from an object, for the Game Master and for a player who may see it.
- **FR-035**: An interaction point MUST be movable after placement without being
  deleted and re-made.
- **FR-036**: Whether the thing behind an interaction point is a `region` with
  point-like geometry, or a fourth subject kind, is an implementation choice
  this spec does not make — but the player-facing behaviour of FR-032 to FR-034
  MUST hold whichever is chosen.
- **FR-037**: A designated door MUST carry a visible sign that it responds, the
  way a prop does. Today it carries none: badges are drawn only over subjects
  found in `TokenEntities`
  (`src/engine/src/plugins/interaction_marker.rs:179-190`), and a wall id is not
  one.

#### What an object is

- **FR-040**: An **object** MUST be scenery by default: drawn on the board,
  named, blocking nothing, taking no turn, taking no damage, and offering
  nothing to click.
- **FR-041**: An object MUST use the existing `Object` token kind
  (`crates/thunderforge-canvas-core/src/token_kind.rs:48-49`,
  `apps/web/src/types/token.ts:45`). No new entity is introduced.
- **FR-042**: An object MUST NOT block movement. This is already true of every
  token — only walls refuse a move
  (`src/server/src/movement/mod.rs:75,101`) — and this spec keeps it true.
- **FR-043**: An object MUST NOT enter the turn order on its own. It enters only
  when a Game Master adds it deliberately, the same way a lair does.
- **FR-044**: An object MUST NOT be attackable and MUST NOT accept damage or
  healing while it has no hit points. The server MUST refuse, and it does today.
- **FR-045**: A Game Master MUST be able to give an object hit points, and to
  take them away again.
- **FR-046**: An object with hit points MUST behave as spec 046 says a creature
  behaves *for attacks and damage only*: it can be attacked, damage and healing
  apply, the change reaches every client, and zero marks it out. It MUST NOT
  gain a turn, a turn budget, a sheet or an entry in the roster.
- **FR-047**: An object at zero hit points MUST NOT be removed from the board by
  the product, and MUST NOT change the scene on its own. What a broken thing
  means is the Game Master's.
- **FR-048**: Giving an object hit points MUST NOT be offered where the world's
  game system declares none — the change is already refused there
  (`src/server/src/combat/hit_points.rs:352-355`).
- **FR-049**: An object with an interaction attached MUST remain an object. An
  interaction does not make it a creature, and does not put it in the order.
- **FR-050**: A Game Master MUST be able to hide an object from players. There is
  no field for this today: visibility hangs on
  `world_actors.visible_to_players` (`src/server/src/schema.rs:770`) and an
  object has no actor.

#### Who sees what, and who is offered what

- **FR-060**: No menu item MUST be offered to a viewer that the server would
  refuse them. The rule deciding the items MUST be readable and testable as a
  rule, the way `canvasMenuActions` is today.
- **FR-061**: A menu MUST NOT disclose the existence of anything the viewer is
  not shown — an unrevealed secret door, a GM-only interaction point, a hidden
  object. For those, the viewer gets the menu they would have got for bare
  board. The token path already works this way: the engine drops hidden tokens
  from its hit test before reporting
  (`src/engine/src/plugins/context_menu.rs:186`), and a secret door is already
  not drawn for a non-Game-Master (`src/engine/src/systems/wall.rs:782-792`).
  Walls and interaction points MUST follow the same rule.
- **FR-062**: A player's menu on a creature MUST continue to respect NPC
  visibility as it landed on 2026-09-15 — a hidden NPC reaches no player by any
  path (`src/server/src/auth/npc_visibility_tests.rs:1-6`).
- **FR-063**: Whatever is offered MUST be decided against what the server
  allows, not against a second copy of the permission rules kept in the client.

#### Keyboard

- **FR-070**: Every gesture this spec adds MUST have a keyboard equivalent that
  reaches the same menu and the same result.
- **FR-071**: Keyboard selection MUST reach walls and interaction points, not
  tokens alone. Today `selectedTokenId` is the only thing the keyboard path
  consults (`useCanvasContextMenu.ts:96-101`).
- **FR-072**: ContextMenu and Shift+F10 MUST open the menu for whatever is
  selected, and the place-here menu at the camera's centre when nothing is —
  which is today's behaviour and MUST NOT regress
  (`useCanvasContextMenu.ts:29-33, 103-118`).
- **FR-073**: A menu MUST open with focus on its first item, move by arrow keys,
  choose by Enter, close by Escape, and return focus to where it was opened
  from — as the menu already does.
- **FR-074**: *Start a wall here* MUST be completable and abandonable from the
  keyboard, since it is the one placement that continues after the menu closes.
- **FR-075**: The keyboard equivalents MUST be discoverable from inside the
  product. There is nowhere to list them today — the app has no shortcut
  registry and no help overlay, so the engine's existing wall keys (`V`, `B`,
  `O`, Delete — `src/engine/src/systems/wall.rs:501-512`) and the menu's own
  Shift+F10 are undiscoverable. This requirement is met by naming the keys where
  the action is offered, or by whatever the product adopts as its shortcut
  surface; it is not met by this document.
- **FR-076**: A new overlay MUST stay compatible with the canvas key routing —
  arrow keys must not walk a token while a menu is open, as
  `isInOverlay` already ensures (`apps/web/src/engine/canvasKeyboard.ts:69-76`).

#### Where this amends spec 030

- **FR-080**: Spec 030 FR-006 said a Game Master sees, *while editing*, which
  objects are interactive and what each targets. This spec amends it: that view
  MUST also be reachable by right-clicking the thing itself, and MUST NOT be the
  only route.
- **FR-081**: Spec 030 FR-007 said a Game Master designates a wall segment as a
  door. This spec amends it: designation MUST be reachable from the wall's own
  menu, without the wall being identified by an id.
- **FR-082**: Spec 030 FR-023 said a Game Master has "a distinct secondary
  interaction on an interactive". This spec amends it: that secondary
  interaction is the play-field right-click menu, it extends to walls and to
  free-standing interaction points, and it is the primary route to authoring
  rather than an override.
- **FR-083**: Spec 030 FR-029 to FR-032 define a region as an area that fires on
  entry. This spec does **not** change them. A point-like interaction point is a
  *click* subject; region entry stays as specified, and stays unimplemented on
  the server (`entries_for` has one caller, in the engine:
  `src/engine/src/plugins/interaction.rs:177-183`).
- **FR-084**: Spec 030's assumption that "props reuse the existing object token
  kind" is amended into a rule: an object is the `Object` kind, an object is not
  a creature, and the difference is enforced rather than cosmetic (FR-040 to
  FR-049).
- **FR-085**: Spec 030's decision that secrets are protected by the table, not
  by the wire, is unchanged and governs FR-061. A menu withholds an item; it is
  not a security boundary, and the spec does not claim it is one.

### Key Entities

- **Object**: A placed thing that is not a creature. Has a position, artwork, a
  name, and by default no hit points, no turn, no interaction and no ability to
  block. A Game Master may give it an interaction, hit points, or both. Uses the
  `Object` token kind.
- **Interaction point**: A position on a scene carrying one effect and nothing
  else — no artwork, no sheet, no hit points. Drawn for whoever may use it.
- **Wall menu**: The set of actions a wall offers the viewer who opened it —
  door designation, lock, secrecy, interaction, deletion for a Game Master;
  opening and closing an unlocked door for a player.
- **Place-here menu**: The set of things a Game Master may put at a point on the
  board: creature, object, light, the start of a wall, interaction point.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A Game Master makes an existing wall into a door **without seeing,
  choosing or typing any identifier**, in under fifteen seconds, in a scene
  containing at least ten walls.
- **SC-002**: A Game Master places an object, attaches an interaction that opens
  a lore entry, and a player opens it, in under two minutes and without
  documentation.
- **SC-003**: A Game Master places an interaction point on bare board pointing at
  a lore entry, and a player reads that entry by clicking the marker — with no
  token, no object and no wall involved.
- **SC-004**: Every one of SC-001 to SC-003 is completable with the keyboard
  alone, producing the same result.
- **SC-005**: For every menu item offered to a player, sending that action to the
  server succeeds; for every action the server refuses a player, no menu item
  offers it. Verified by enumerating both directions over a scene containing a
  locked door, a secret door, a hidden object and a GM-only interaction point.
- **SC-006**: A player opening a menu anywhere in such a scene learns nothing
  about a secret door, a hidden object or a GM-only interaction point that they
  could not already see.
- **SC-007**: An object placed and left alone appears in no turn order, offers no
  attack, accepts no damage, and stops no movement — asserted, not assumed.
- **SC-008**: An object given hit points can be attacked and damaged, its bar
  moves on every client within one second, and it is still absent from the turn
  order.
- **SC-009**: A Game Master can reach every action in this spec from the board
  without opening the rail, and the rail still reaches them too.
- **SC-010**: A designated door is visibly a door that responds, to a Game Master
  and to a player who may open it, without either hovering or clicking to find
  out.
- **SC-011**: A wall clicked in the ordinary selecting mode is selected and
  nothing is created, in 100% of attempts.

## Assumptions

- **The play-field right-click menu is the gesture.** It landed on 2026-09-15
  (`da13758`) with its keyboard path and its focus handling. This spec extends
  its reach and its rule; it does not redesign it.
- **Walls stay walls.** A door is still `walls.door_state` plus `locked` plus
  `secret` (`src/server/src/schema.rs:896-913`) — there is no `is_door` column
  and this spec does not ask for one.
- **An object is the `Object` token kind**, per decision 3, which keeps one
  placement, artwork, movement and sync pipeline. A parallel entity would
  duplicate all four.
- **Hit points on an object go where a copy's hit points already go** — the
  token's own `system_data`, permitted by
  `tokens_linked_has_no_system_data` and written by the copy branch of
  `changeHitPoints`. That is the seam; what seeds it is the new part.
- **An object's hit-point fields come from the world's game system**, since it
  has no actor to take a system from
  (`src/server/src/combat/hit_points.rs:331-350`).
- **Nothing here adjudicates.** Whether a chest is locked, whether a character
  notices the marker, whether the barred door gives way — all of that is the
  Game Master's, per spec 030 FR-034.
- **The effect vocabulary stays contributed.** This spec names lore and scenes as
  examples because they are contributed today
  (`src/server/src/interaction.rs:45-65`); it does not fix the list.

## Out of Scope

- **Region entry on the server.** Spec 030 FR-030's "fires when a token crosses
  into the region" is still engine-only. Making it server-adjudicated is real
  work and belongs with spec 045's movement adjudication, not here.
- **Cleaning up orphaned interactives.** `drop_for_subject` having no callers is
  a defect this spec surfaces and does not fix; it deserves its own change.
- **Copying interactives with a scene.** `scene_copy` leaving them behind is
  known and noted (`scene_copy.rs:307-317`).
- **Inventory and loot.** What is inside the chest, and moving it to a character,
  is `item.pickup` and separate work.
- **Cover, and objects that block.** An object that blocks line of sight or
  movement would be geometry, not a token, and is a different feature.
- **Destruction.** What a broken object does to the scene — rubble, a hole, an
  opened path — is the Game Master's to narrate. FR-047 is deliberate.
- **New effects.** Nothing here contributes one.
- **Redesigning the rail.** It stays; whether it eventually goes is a later
  question.

## Dependencies

- **The play-field right-click menu** (`da13758`): the gesture, the permission
  rule in `canvasMenuActions.ts`, and the keyboard path in
  `useCanvasContextMenu.ts`.
- **Spec 030** — interactives, the effect registry, activation, permission,
  approval, doors as a contributing subsystem. This spec amends it per FR-080 to
  FR-085.
- **Spec 045** — door state, locked, secret, and what each means for movement and
  vision; and FR-043, which keeps light markers and wall handles off a player's
  board.
- **Spec 046 / ADR-102** — a token is linked or a copy; hit points live on the
  actor or on the token's own `system_data`; attacks and damage resolve on the
  server; a lair shows that a combatant need not be a creature on the board.
- **Spec 029** — `TokenKind` and its tested palette; adding no kind here means
  adding no colour.
- **NPC visibility** (owner decision, 2026-09-15) — a hidden NPC reaches no
  player by any query path, which FR-062 relies on and FR-050 extends to
  something with no actor.
- **Lore entries and scenes** exist, and are what an interaction point opens.

## Decisions (owner, 2026-09-15)

### 1. A wall is reached by selecting it and right-clicking it

Left-click selects the wall; right-click opens that wall's own menu — make it a
door, lock, secret, make it an interaction point, delete. The id-picker panel
becomes a fallback, not the way in.

*Why it settles the complaint*: a wall has no name and no id on the board, so any
route that asks a Game Master to identify one in a list is asking a question the
board does not answer. Pointing at it is the answer.

### 2. One "place here" menu

Right-clicking empty board offers everything placeable: a token (a creature), an
object, a light, the start of a wall, and an interaction point that opens lore or
a scene. One gesture to learn.

*Why it settles the complaint*: the menu already offers two of the five
(`canvasMenuActions.ts:66-70`), and one of those two — *Add a light here* — is
the **only** light placement in the product that is wired to anything, the rail's
button being local state and nothing else (`LightingTool.tsx:73-78`). The rail's
*Draw wall* is the same (`WallTool.tsx:36-43`). The other placements live in
separate rail tools with separate arming rules, which is why switching tools used
to leave stray marks on the map. One menu, one gesture, one place to look — and
it is already the route that works.

### 3. An object is scenery unless someone makes it interactive

An object is drawn, blocks nothing by default, is not in the turn order, takes no
damage and cannot be attacked — until a Game Master attaches an interaction
(open, search, a lore entry) or gives it hit points. It uses the `Object` kind
that already exists rather than a new entity.

*Why it settles the complaint*: the owner's distinction — a token is
interactable, an object is not — is already spelled in the code and has never
meant anything. This gives it the rules it should have had, without a second
placement pipeline.

**A consequence worth naming**: an object *with* hit points is a creature for
attacks and damage, and is not a creature for anything else. It does not enter
the turn order, get a turn budget, appear in the roster or acquire a sheet. That
is the owner's answer read strictly, and it is the answer this spec implements —
a Game Master who wants the barred door to act in initiative adds it by hand, the
way they would a lair.

## Questions for the owner

Two, both with a recommendation.

### Q1. Does a player see an interaction point they may not use?

FR-033 says no: a GM-only marker is not drawn for a player. The alternative is
that a player sees a marker and is told "only the GM can do this" when they click
it — which is what `refusalNotice` already says for `gmOnly`
(`apps/web/src/api/interactives.ts:refusalNotice`).

**Recommendation: not drawn.** A marker a player can see but not use is a
question mark on the board that a Game Master has to answer out loud, every time,
for the rest of the session. It also leaks preparation in the one place the
product can cheaply not leak it: a thing that is not drawn is not a filter over
scene data, it is a marker the client never spawns.

### Q2. When a click could mean a wall or a token, which wins?

An interaction point placed on a doorway sits on top of a door; a token standing
in that doorway sits on top of both.

**Recommendation: topmost drawable thing wins, with tokens above interaction
points above walls** — and a modifier (or a second right-click in the same spot)
walks down the stack. Draw order is what a person's eye already used to decide
what they were pointing at, so matching it is the only rule that needs no
explanation; the walk-down exists because a wall under a token would otherwise be
permanently unreachable, which is the bug this spec was written to fix.
