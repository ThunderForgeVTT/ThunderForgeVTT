# Feature Specification: Playability 001 — From Demonstrable to Playable

**Feature Branch**: `031-playability`

**Created**: 2026-09-01

**Status**: Draft

**Input**: User description: playtest of 2026-09-01 against a live dev stack, 26 recorded findings, covering play-screen authoring ergonomics, View/Place interaction, the first interaction primitives, scene lifecycle, combat rosters, content-management surfaces, and four defects.

## Context

Every feature this spec touches already exists in some form. The engine draws,
tokens drag, scenes launch, combat runs, lore and items and actors are all
authored and permissioned. What the playtest found is that the seams between
them are not yet walkable: a Game Master can *demonstrate* the app but cannot
comfortably *run a session* in it.

The unifying complaint is that the play screen asks the user to leave it.
Viewing a character navigates away from the table. Placing a token means
authoring one. Changing a scene loses the party. This specification is about
closing those seams, not about adding subsystems.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - A Game Master runs a scene without leaving the table (Priority: P1)

A GM has a scene open and players connected. They need to look at a character,
put a creature on the map, and move things around — without the table
disappearing from their screen and without disturbing what players see.

**Why this priority**: This is the session itself. Every other story here is
preparation for or decoration on this one. It is also the story where the
current app most visibly fails: the actors pane links *away* from play.

**Independent Test**: Open a scene with a connected player, view a character,
place a token, and move it — confirming the play view is never navigated away
from and the player's view is unaffected except by the placement.

**Acceptance Scenarios**:

1. **Given** a GM on the play screen, **When** they choose View on an actor in
   the actors pane, **Then** the character opens in a separate browser tab and
   the play screen remains open and connected.
2. **Given** a GM on the play screen, **When** they choose Place on an actor,
   **Then** that actor's token follows the cursor and a left click on the map
   places it at the clicked position.
3. **Given** a GM carrying a token, **When** they cancel, **Then** no token is
   created and the map is unchanged.
4. **Given** a GM with the Select tool active, **When** they restrict selection
   to tokens, **Then** clicking a wall or light selects nothing and dragging
   moves only tokens.
5. **Given** a GM who collapsed the Select tool's filter menu, **When** they
   return to the world later, **Then** the menu is still collapsed and their
   filter choices are unchanged.

---

### User Story 2 - A player uses their character during play (Priority: P1)

A player at the table wants their character in front of them — stats,
abilities, and the ability to roll — without leaving the map they are playing
on.

**Why this priority**: Equal-highest because a virtual tabletop that makes
players leave the table to act is not yet a tabletop. It is also the half of
View that differs from the GM's, and the difference is deliberate.

**Independent Test**: As a connected non-GM, open your own character from the
actors pane and trigger a roll, confirming the play view stays mounted
throughout and the roll is visible to the table.

**Acceptance Scenarios**:

1. **Given** a player on the play screen, **When** they choose View on their
   own character, **Then** a compact character view opens *within* the play
   screen's pane rather than replacing it.
2. **Given** that in-pane character view, **When** the player triggers a stat
   or ability roll from it, **Then** the roll resolves and is reported to the
   table as any other roll is.
3. **Given** a player viewing their character in the pane, **When** they
   dismiss it, **Then** they are returned to the pane's previous content with
   the map still live.

---

### User Story 3 - Things on the map can be interacted with (Priority: P2)

A GM places a piece of lore and an item on the map. A player clicks the lore
and reads it; clicks the item and either inspects it or picks it up, at which
point it leaves the map and enters their inventory.

**Why this priority**: The first two interaction primitives turn a map from a
picture into a place. Placed *after* the play-screen stories because those are
what make placing anything comfortable in the first place.

**Independent Test**: Place one lore marker and one item, then as a player open
the lore and pick up the item, confirming the item leaves the map for exactly
one inventory.

**Acceptance Scenarios**:

1. **Given** a lore marker on the map, **When** a player clicks it, **Then**
   the lore entry opens in a separate browser tab and play is uninterrupted.
2. **Given** an item on the map, **When** a player clicks it, **Then** they are
   offered Pickup and View.
3. **Given** a player choosing Pickup, **When** the server accepts it, **Then**
   the item is removed from the map for every connected client and appears in
   that player's inventory.
4. **Given** two players choosing Pickup on the same item at the same moment,
   **When** both requests are processed, **Then** exactly one player receives
   the item and the other is told it is gone.
5. **Given** a player choosing Pickup that the server refuses, **When** the
   refusal arrives, **Then** the item remains on the map and no inventory
   change persists.

---

### User Story 4 - Moving the party to a new scene (Priority: P2)

A GM changes the active scene. The new scene's content loads and the old
scene's content clears — but the party's characters can come along rather than
being placed again by hand.

**Why this priority**: The moment a session moves rooms. Without it, every
scene change costs the GM a manual re-placement of every player token while the
table waits.

**Independent Test**: With player tokens on scene A, change to scene B with the
"bring the party" option and confirm scene A's walls, lights and non-party
tokens are gone, scene B's are present, and the party's tokens are on scene B.

**Acceptance Scenarios**:

1. **Given** a scene change, **When** the new scene loads, **Then** the
   previous scene's tokens, walls and lights are no longer displayed and the
   new scene's are.
2. **Given** a GM changing scenes with player characters on the map, **When**
   they choose to bring the party, **Then** those player character tokens are
   present on the new scene.
3. **Given** a GM changing scenes without bringing the party, **When** the new
   scene loads, **Then** no tokens from the previous scene are present.

---

### User Story 5 - Preparing a scene without revealing it (Priority: P2)

A GM wants the next scene ready before the party arrives — without players
seeing the change, and without the GM being dropped into the play view.

**Why this priority**: Preparation is most of a GM's work, and today the single
Launch action conflates "get ready" with "go now".

**Independent Test**: From the scene list, Preload a scene and confirm no
connected player's view changes and the GM stays on the scene list; then Launch
it and confirm both change.

**Acceptance Scenarios**:

1. **Given** a GM on the scene list, **When** they choose Launch on a scene,
   **Then** the scene becomes the table's scene and the GM is taken into the
   play view.
2. **Given** a GM on the scene list, **When** they choose Preload on a scene,
   **Then** the GM remains on the scene list.
3. **Given** a connected player, **When** the GM Preloads a scene, **Then**
   nothing the player can see changes.
4. **Given** the scene list, **When** a GM reads it, **Then** each scene shows
   its description and a visual render of itself, and the difference between
   Launch and Preload is stated in the interface.

---

### User Story 6 - Authoring a map quickly (Priority: P3)

A GM builds a room: walls that follow the grid, a door, a light — without
drawing every segment by hand or fighting the alignment.

**Why this priority**: Speed of authoring decides whether a GM prepares in the
app or elsewhere. Lower than the session stories because a slow authoring
session is survivable; a broken table is not.

**Independent Test**: Draw a four-walled room with a door using the wall tool's
helpers and confirm the walls align to the grid and the door is functional.

**Acceptance Scenarios**:

1. **Given** grid snapping enabled, **When** a GM draws a wall, **Then** it
   follows the scene's grid lines, whether the grid is square or hex.
2. **Given** a GM who disables grid snapping, **When** they draw a wall,
   **Then** it follows the cursor freely.
3. **Given** the wall tool, **When** a GM selects the room helper, **Then**
   they can create a closed room in a single gesture rather than four.
4. **Given** the wall tool, **When** a GM places a door, **Then** it is a real
   door — openable, closable and lockable — not decoration.
5. **Given** a new world, **When** a GM first authors, **Then** grid snapping
   is already on.

---

### User Story 7 - Running combat the way this ruleset runs it (Priority: P3)

A GM selects several tokens and starts combat. The roster reflects what they
selected, and the turn structure matches the game system in play rather than
assuming everyone takes turns in rounds.

**Why this priority**: Combat already works; this makes it honest across
systems. Lower priority because the current model is usable for the systems
that do have rounds.

**Independent Test**: Select three tokens, start combat, and confirm the roster
contains exactly those three and that the turn presentation is the one the
active game system defines.

**Acceptance Scenarios**:

1. **Given** tokens selected on the map, **When** the GM opens the combat
   panel, **Then** the selected tokens are offered as the combat roster.
2. **Given** a game system that uses rounds and turns, **When** combat starts,
   **Then** the round and the current participant are shown and advance.
3. **Given** a game system that does not use rounds, **When** combat starts,
   **Then** no round counter or turn order is imposed on it.

---

### User Story 8 - Managing world content comfortably (Priority: P3)

A GM administers their world: finds a player among many, sees who plays which
character, creates an NPC or item on a page with room to work, gives an actor a
portrait and a token image, and organises lore into a structure.

**Why this priority**: Preparation and administration. Valuable, frequently
touched, but never blocks a session in progress.

**Independent Test**: With twenty players in a world, find one by search, see
their bound character, and change it; separately, create an NPC through the
full editor and give an actor both images.

**Acceptance Scenarios**:

1. **Given** a world with many players, **When** a GM opens the players
   section, **Then** players are presented as searchable cards showing which
   character each is bound to.
2. **Given** a player card, **When** the GM changes that player's character,
   **Then** the binding changes and is reflected wherever the character is
   shown.
3. **Given** the NPC or item compendium, **When** a GM chooses to create one,
   **Then** they are taken to a full editing page with an explicit save, and
   the list itself carries no inline creation form.
4. **Given** an actor, **When** a GM uploads a portrait and a token image,
   **Then** both are stored and shown in their respective places.
5. **Given** an item, **When** a GM sets a price or suggested price, **Then**
   it is visible where the item is presented for trade.
6. **Given** many lore entries, **When** a GM organises them, **Then** they can
   arrange them in a tree and tag them, and find entries by either.
7. **Given** an actor's screen, **When** a GM adds an item or lore entry,
   **Then** it can be created or attached without leaving the actor.

---

### User Story 9 - The interface does not lie or misfire (Priority: P1)

The four defects found by the playtest.

**Why this priority**: P1 despite being small. One of them silently disables
content caching for an entire browser, and another creates map content the user
did not ask for. Both erode trust in everything else in this list.

**Independent Test**: Each defect has a direct reproduction; each is fixed when
that reproduction no longer occurs.

**Acceptance Scenarios**:

1. **Given** any authoring tool is active, **When** the GM switches to another
   tool, **Then** nothing is placed on the map.
2. **Given** the play view is loading, **When** the user watches it, **Then**
   exactly one loading indicator is visible at any moment.
3. **Given** any supported browser, **When** a user opens a world with cached
   content, **Then** either the content is served from the device and reported,
   or the user is told plainly that this browser cannot keep content on device.
4. **Given** normal use of the app, **When** the browser console is inspected,
   **Then** no repeated failed request appears for an absent identifier.

### Edge Cases

- A player carrying a token when their connection drops: the placement must
  either complete against the server or not happen at all.
- A scene change while a player holds an in-pane character view open.
- Picking up an item that another player picked up moments earlier.
- Bringing the party to a scene where one of those characters already has a
  token.
- Grid snapping on a scene whose grid type changes after content was authored.
- A game system that defines no character sheet, when a player chooses View.
- Selection filters that exclude everything, leaving the Select tool inert —
  the interface must make that state obvious rather than appearing broken.
- A mode change that arrives while a gesture is already in flight: a drag
  begun under one tool, or a placement being carried when the user switches
  away. Neither may complete under the new mode's rules.
- A player's tool permission being revoked while they are mid-gesture with that
  tool: the gesture must not complete, and the loss must be legible rather than
  the tool silently ceasing to respond.

## Requirements *(mandatory)*

### Functional Requirements

**Play-screen interaction**

- **FR-001**: Users MUST be able to view an actor from the play screen without
  the play screen being navigated away from.
- **FR-002**: The system MUST present a player's own character within the play
  screen, and MUST open a separate browser tab for a Game Master.
- **FR-003**: A player MUST be able to trigger stat and ability rolls from the
  in-play character view, and those rolls MUST be reported to the table
  identically to rolls made elsewhere.
- **FR-004**: Users MUST be able to place an actor's token by attaching it to
  the cursor and confirming with a left click on the map.
- **FR-005**: A placement in progress MUST be cancellable, leaving no content
  created.
- **FR-006**: A placed token MUST obey the same grid rules as a dragged one.
- **FR-007**: Token placement MUST be subject to the same ownership and
  permission rules as token creation.

**Who may use a tool**

- **FR-044**: Which authoring tools a person may use MUST be a declared
  permission, resolved from the single permission declaration the world already
  uses — not a role check written separately per tool.
- **FR-045**: The default MUST be that only a Game Master may author, so
  existing worlds behave exactly as they do today until a Game Master decides
  otherwise.
- **FR-046**: A Game Master MUST be able to grant a specific player the use of
  specific tools for their world.
- **FR-047**: A tool a person may not use MUST be neither offered nor usable —
  it does not appear in their rail, and the engine refuses its input even if
  the request is made directly. Hiding alone is not sufficient.

**Selection**

- **FR-008**: The Select tool MUST let a user choose which kinds of content it
  selects, with every kind enabled by default.
- **FR-009**: Selection filter choices MUST persist for that user across
  sessions.
- **FR-010**: The selection filter interface MUST be collapsible, MUST remember
  whether it was collapsed, and MUST NOT occupy the map when collapsed.

**Interaction primitives**

- **FR-011**: A Game Master MUST be able to place a lore marker on a map.
- **FR-012**: Activating a lore marker MUST open that lore entry in a separate
  browser tab.
- **FR-013**: A Game Master MUST be able to place an item on a map.
- **FR-014**: Activating a placed item MUST offer inspection and pickup.
- **FR-015**: A successful pickup MUST remove the item from the map for all
  connected clients and add it to the acting player's inventory.
- **FR-016**: Concurrent pickups of the same item MUST result in exactly one
  player receiving it.
- **FR-017**: A refused pickup MUST leave the map and all inventories unchanged.

**Scene lifecycle**

- **FR-018**: Changing scenes MUST clear the previous scene's tokens, walls and
  lights from display and load the new scene's.
- **FR-019**: A Game Master MUST be able to bring player character tokens with
  them across a scene change.
- **FR-020**: Preload MUST prepare a scene without changing what connected
  players see and without navigating the Game Master into play.
- **FR-021**: Launch MUST change the table's scene and take the Game Master
  into play.
- **FR-022**: The interface MUST explain the difference between Launch and
  Preload.
- **FR-023**: The scene list MUST show each scene's description and a visual
  render of it.

**Authoring**

- **FR-024**: Grid snapping MUST be a Game-Master-controlled setting, enabled by
  default, applying to walls, lights and other placed content.
- **FR-025**: Snapping MUST honour the scene's grid type, including hex.
- **FR-026**: The wall tool MUST offer room and door primitives selectable while
  drawing.
- **FR-027**: Doors created this way MUST be functional doors, supporting the
  states doors already support.
- **FR-028**: The interactions authoring surface MUST offer helper controls for
  placing the interaction kinds the system supports.
- **FR-029**: The play canvas MUST support right-click interaction without the
  browser's own context menu appearing over the map.

**Combat**

- **FR-030**: Tokens selected on the map MUST be offerable as the combat roster.
- **FR-031**: Turn and round structure MUST be determined by the active game
  system, and MUST NOT be imposed on systems that do not use it.

**Content management**

- **FR-032**: The administration area MUST provide persistent navigation between
  its sections.
- **FR-033**: Players MUST be presented as searchable cards showing each
  player's bound character.
- **FR-034**: A Game Master MUST be able to set a player's character binding
  from the players section, and that MUST agree with every other surface that
  can change the same binding.
- **FR-035**: NPC and item creation MUST occur on a dedicated editing page with
  an explicit save, and the corresponding lists MUST NOT carry inline creation
  forms.
- **FR-036**: Actors MUST support a portrait image and a token image as distinct
  images, uploaded through the existing image-conversion and storage path.
- **FR-037**: A Game Master MUST be able to record a price or suggested price
  for an item, presented where items are offered.
- **FR-038**: Lore entries MUST be organisable in a tree and taggable, and
  findable by both.
- **FR-039**: A Game Master MUST be able to create or attach an item or lore
  entry from an actor's screen without leaving it.

**Defects**

- **FR-040**: Switching between authoring tools MUST NOT place content.
- **FR-040a**: A change of authoring mode MUST be atomic from the user's point
  of view: no input may be attributed to a tool the user has just left, nor to
  one they have not yet entered. Exactly one authority decides which mode is
  active at any instant.
- **FR-041**: At most one loading indicator MUST be visible at any moment.
- **FR-042**: On a browser that cannot keep content on device, the system MUST
  tell the user so rather than reporting an empty cache.
- **FR-043**: The system MUST NOT issue requests for records it has no
  identifier for.

### Key Entities

- **Selection Filter Preference**: which content kinds the Select tool acts on
  for a given user, and whether its menu is collapsed. Persists per user.
- **Placement In Progress**: a token attached to the cursor, awaiting
  confirmation or cancellation. Transient; never persisted.
- **Placed Interaction**: a lore marker or item positioned on a scene, carrying
  what it refers to and what activating it offers.
- **Actor Imagery**: the images belonging to an actor, distinguished by the role
  each plays (portrait, token). Structured to admit further roles later without
  redefinition.
- **Item Price**: a value a Game Master records against an item for
  presentation, distinct from any system-specific economy.
- **Lore Organisation**: the tree position and tags of a lore entry.
- **Combat Roster**: the participants in an active combat, with turn structure
  supplied by the game system rather than assumed.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A Game Master can place ten creatures on a map in under one
  minute, without leaving the play screen.
- **SC-002**: A player can open their character and make a roll in under five
  seconds from the play screen, without the map unloading.
- **SC-003**: Moving the party to a new scene requires no manual re-placement of
  player character tokens.
- **SC-004**: A Game Master can prepare a scene without any connected player
  observing a change.
- **SC-005**: A four-walled room with a door can be authored in under thirty
  seconds.
- **SC-006**: Concurrent pickup of one item by two players results in exactly
  one inventory containing it, in 100% of attempts.
- **SC-007**: Exactly one loading indicator is visible at any moment during
  world load, in 100% of loads.
- **SC-008**: Switching authoring tools places content in 0% of switches, for
  every ordered pair of tools — not only the pair in which it was noticed.
- **SC-009**: In every browser the project supports, a user either sees content
  served from their device or is told their browser cannot do so — never a
  silent zero.
- **SC-010**: A Game Master can find a specific player among fifty and change
  their character binding in under fifteen seconds.
- **SC-011**: A game system that does not use rounds presents no round counter.
- **SC-012**: A person without permission for a tool cannot author with it by
  any route, including a directly issued request, in 100% of attempts.

## Assumptions

- **Existing subsystems are reused, not rebuilt.** Image upload uses the
  existing conversion and object-storage path; rolls use the existing dice
  path; interactions extend the existing effect vocabulary; invites,
  permissions and claims are unchanged.
- **"Bring the party" means player-character tokens**, not every token on the
  map. Non-party tokens are scene-bound.
- **Preload does not change what players see.** Where this conflicts with the
  table's scene being server-authoritative and broadcast, preparation wins and
  players are unaffected until Launch.
- **The in-pane character view is a presentation of the character sheet the
  active game system already supplies**, not a second, parallel sheet.
- **Right-click on the canvas is reserved by the application** on the play
  surface; the browser's context menu is suppressed there and nowhere else.
- **Supported browsers are those the project intends to support at release**;
  determining that set is a prerequisite of FR-042 but not part of this
  feature's delivery.
- **Item price here is presentational**, for a Game Master to role-play from. A
  game system with its own economy continues to own it.
- **Existing e2e coverage will need updating** where creation surfaces move;
  tests that create content incidentally should do so through shared fixtures
  rather than through whichever screen currently offers a form.

## Out of Scope

Recorded during the same playtest, deliberately excluded here, each warranting
its own specification:

- Interface packs as themes, and the naming of the base interface.
- Game systems driving interface *function* (sheets, item presentation, combat
  structure) through contributed hooks rather than a built-in registry.
- An open, system-supplied vocabulary for ability types, replacing the fixed
  set, and the guarded world-system switch that warns about existing content.
- Lore synchronisation to an external Git host.
- Application-wide signup control, invitations, and a request-access webhook.
- Additional actor imagery roles for animated or state-driven presentation.
