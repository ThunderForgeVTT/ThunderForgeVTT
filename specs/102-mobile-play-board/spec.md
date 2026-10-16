# Feature Specification: A Board That Fits a Phone

**Feature Branch**: `102-mobile-play-board`

**Created**: 2026-10-16

**Status**: Draft

**Input**: The owner, 2026-10-16, after opening the demo board on a phone:
"on mobile that ui feels cluttered … maybe we can find another simpler ui for
mobile something cleaner for admin vs user view". Answers the same day: a Game
Master keeps every tool on a phone, reorganised rather than removed; a player's
phone is for moving their token, rolling dice, their character sheet, and chat
and initiative.

## Why

Measured on the demo board at 412 × 915 (a Galaxy S23 Ultra in portrait),
2026-10-16:

- Two icon rails stand on the left and right for the whole session, open or
  not. The map gets about 310 of the 412 pixels across.
- The Select filter opens when the board loads. It covers the top of the map
  and hides the level tabs under it.
- The dice tray sits on the Tokens toggle and on the demo notice. The notice
  cannot be tapped at all, so on a phone the demo cannot switch to the
  player's view.
- An open panel (Actors, Chat …) covers the whole map.
- Every control is 36 pixels or smaller; the Select panel's close button is
  18.
- A player gets 7 of the Game Master's 12 controls, and their Select filter
  offers Walls and Lights, which a player cannot edit.

Spec 069 made the board itself answer a finger. This spec is about everything
around it: on a phone the map is the screen, and the rest comes when asked
for.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - A player plays from their phone (Priority: P1)

A player joins a session from their phone. The map fills the screen. What
they came to do is each one tap away: find and move their character, roll,
open their sheet, read chat and see whose turn it is. Nothing on screen is a
tool they are not allowed to use.

**Why this priority**: Players are most of the people at a table, and a phone
is what a player in the room, or on a couch, has in hand. Today they get the
Game Master's clutter without the Game Master's tools.

**Independent Test**: Open a world's board as a player at a phone viewport.
Measure the share of the screen the map gets with nothing open, then reach
each of the four jobs from the board and come back.

**Acceptance Scenarios**:

1. **Given** a player opens the board on a phone, **When** it finishes
   loading, **Then** the map takes at least 85% of the screen and no panel is
   open.
2. **Given** the player has a character on this scene, **When** they tap
   their character control, **Then** the view moves to their token and
   selects it, ready to move.
3. **Given** the board is showing, **When** the player opens the dice,
   **Then** they can roll a formula and see the result without leaving the
   board, and closing the dice returns the full map.
4. **Given** the board is showing, **When** the player opens their sheet,
   **Then** it opens over the board, and closing it returns them to the same
   view of the map.
5. **Given** combat is running, **When** the player looks at the board with
   nothing open, **Then** they can see whose turn it is, and the chat is one
   tap away.
6. **Given** a player's controls, **When** they are listed, **Then** none of
   them is a tool the player cannot use (walls, lights, levels, player
   requests, world settings).

---

### User Story 2 - A Game Master runs the table from a phone (Priority: P1)

A Game Master opens their world on a phone. They see the same uncluttered map
a player does, plus one way into all their tools. Every tool the desktop
board offers is reachable, grouped so they can find it, and the tool in hand
shows itself without covering the map.

**Why this priority**: The owner's answer: a Game Master loses nothing on a
phone. Without it the phone layout would be a player-only view.

**Independent Test**: Open a world's board as its Game Master at a phone
viewport. Reach every tool and panel the desktop board has, from one entry
point, and confirm the map share with nothing open.

**Acceptance Scenarios**:

1. **Given** a Game Master opens the board on a phone, **When** it finishes
   loading, **Then** the map takes at least 80% of the screen and no panel is
   open.
2. **Given** the board is showing, **When** the Game Master opens their
   tools, **Then** every board tool (Select, Walls, Lights, Shapes, Tokens,
   Interactions, Levels) and every table panel (Chat, Actors, Combat, Player
   requests, Settings, About & feedback) is there, grouped by what it acts on.
3. **Given** the Game Master picks a tool, **When** the tool list closes,
   **Then** the chosen tool stays named on screen in a compact form, its
   options are one tap away, and leaving it is one tap.
4. **Given** a player request arrives, **When** the Game Master is on the
   board with nothing open, **Then** they can see a request is waiting without
   opening the tools.

---

### User Story 3 - Nothing on the phone covers anything else (Priority: P2)

Whatever is open, every control on the phone can be seen and tapped: the
dice, the token tray, the level tabs, banners, and the demo's own notice.

**Why this priority**: These are today's defects. A control that cannot be
tapped is not there at all for a phone user, and one of them blocks the demo's
player view outright.

**Independent Test**: At a phone viewport, open the board as a Game Master and
as a player, in the app and in the demo, and check that no visible control
lies under another one, in each state the board can be in (nothing open, dice
open, a panel open, a banner showing).

**Acceptance Scenarios**:

1. **Given** the board on a phone, **When** it loads, **Then** the Select
   filter is closed and the level tabs, if the scene has levels, are visible.
2. **Given** the demo on a phone, **When** the board shows, **Then** the demo
   notice can be tapped, and the switch between the Game Master's and the
   player's view works from it.
3. **Given** any of the board's states, **When** controls are measured,
   **Then** no tappable control is covered by another, and each is at least
   44 × 44 pixels to the touch.
4. **Given** a banner shows (paused play, a lost connection, a reconnect),
   **When** it appears, **Then** it pushes or sits beside the controls rather
   than over them.

---

### User Story 4 - Panels leave the table in view (Priority: P2)

On a phone a panel opens as a sheet over the lower part of the screen, so the
map stays in sight above it. A person can pull a sheet up to read more, push it
down, or close it, and the board under it keeps playing.

**Why this priority**: Today a panel replaces the map. A player reading chat or
a Game Master checking combat loses sight of the table they are running.

**Independent Test**: At a phone viewport, open each panel and confirm part of
the map stays visible and live; expand and close the sheet.

**Acceptance Scenarios**:

1. **Given** the board on a phone, **When** a panel opens, **Then** it covers
   no more than about the lower 60% of the screen, and the map above it keeps
   moving with the session.
2. **Given** an open sheet, **When** the person expands it, **Then** it can
   fill the screen for reading, and returns to its smaller size or closes in
   one gesture or tap.
3. **Given** a sheet with a text field (chat), **When** the on-screen keyboard
   opens, **Then** the field and what is typed stay visible above the
   keyboard.
4. **Given** a sheet is open, **When** the person uses the system back gesture
   or button, **Then** the sheet closes before the browser leaves the board.

---

### User Story 5 - Desktop and tablet stay as they are (Priority: P3)

People on a laptop, a desktop or a tablet see the board exactly as today. A
phone turned sideways, or a window narrowed past the threshold, changes layout
without losing what was open.

**Why this priority**: The phone layout must not cost the layouts that work.

**Independent Test**: Run the existing board tests at desktop size unchanged;
resize a board across the threshold and back with a panel and a tool open.

**Acceptance Scenarios**:

1. **Given** a desktop or tablet viewport, **When** the board loads, **Then**
   it looks and works as before this spec.
2. **Given** a board with a panel and a tool open, **When** the viewport
   crosses into or out of the phone layout, **Then** the same panel and tool
   are still open in the new layout.

---

### Edge Cases

- A player with no token on this scene: the character control says so and
  offers what they can do (open the sheet), instead of moving the view
  nowhere.
- A player with more than one character: the character control lets them
  choose which one.
- A person's role changes mid-session (a Game Master makes a player Trusted,
  or the reverse): the controls change to match without reloading.
- A Trusted Player gets the player's phone view on the board; their extra
  authority is on the world's pages, not the board.
- Play is paused by an operator: the pause notice shows as it does today, told
  calmly, and is not covered.
- The engine is still loading or has stopped: the loading and stopped states
  stay readable and are not covered by phone controls.
- A phone in landscape: short and wide, it gets the phone layout, arranged for
  the short side.
- Notches, rounded corners and the gesture bar: no control sits under them.
- Text enlarged to 200% by the browser or the system: controls and sheets still
  fit and nothing is cut off.
- Several sheets asked for at once (a request arrives while chat is open):
  one sheet shows at a time, and the waiting item is signalled, not stacked.

## Requirements *(mandatory)*

### Functional Requirements

**The phone layout**

- **FR-001**: The board MUST use a phone layout when the viewport's shorter
  side is below the phone threshold (see Assumptions), and today's layout
  otherwise.
- **FR-002**: In the phone layout, with nothing open, the board MUST show the
  map with only the controls this spec names; no rail stands along the full
  height of the screen.
- **FR-003**: Crossing the threshold in either direction MUST keep the open
  panel, the chosen tool and the view of the map.

**What a player sees**

- **FR-004**: A player's phone board MUST offer, each one tap from the map:
  their character (find and select their token), the dice, their character
  sheet, and chat.
- **FR-005**: While combat runs, a player's phone board MUST show whose turn it
  is without opening anything.
- **FR-006**: A player's phone board MUST NOT show any control for a tool or
  panel the player cannot use; its Select options MUST be only the kinds of
  thing the player can act on.
- **FR-007**: A player MUST still reach the remaining panels a player has
  today (Actors, Combat, Settings that apply to them, About & feedback) from
  a single "more" entry.

**What a Game Master sees**

- **FR-008**: A Game Master's phone board MUST reach every board tool and every
  panel that their desktop board offers, from one entry point on the map.
- **FR-009**: The Game Master's tools MUST be grouped by what they act on: the
  board (Select, Walls, Lights, Shapes, Tokens, Interactions, Levels) and the
  table (Chat, Actors, Combat, Player requests, Settings, About & feedback).
- **FR-010**: The tool in use MUST stay named on screen in a compact form with
  its options one tap away, and MUST NOT cover the map's centre.
- **FR-011**: Waiting player requests MUST be signalled on the Game Master's
  board without opening the tools.

**Panels as sheets**

- **FR-012**: On a phone, a panel MUST open as a sheet over the lower part of
  the screen, leaving the map visible and live above it; a sheet MUST be
  expandable to the full screen and closable in one tap or gesture.
- **FR-013**: Only one sheet MUST show at a time; anything that would open a
  second is signalled instead.
- **FR-014**: The system back gesture or button MUST close an open sheet before
  it leaves the board.
- **FR-015**: A sheet with a text field MUST keep the field visible above the
  on-screen keyboard.

**Nothing covered**

- **FR-016**: On a phone, the Select filter MUST start closed.
- **FR-017**: No tappable control on the phone board MUST lie under another,
  in any state of the board; the demo's notice and its view switch included.
- **FR-018**: Every tappable control in the phone layout MUST be at least
  44 × 44 CSS pixels to the touch.
- **FR-019**: Controls MUST stay clear of the screen's unsafe areas (notches,
  rounded corners, the gesture bar).
- **FR-020**: Banners (pause, connection) MUST NOT cover controls, and controls
  MUST NOT cover them.

**Unchanged**

- **FR-021**: At tablet and desktop sizes the board MUST look and behave as
  before this spec.
- **FR-022**: What a person may see and do MUST still be decided by the server;
  the phone layout only chooses which permitted controls to show.
- **FR-023**: Drawing on, dragging across and gestures on the map stay the
  engine's; this spec changes the controls around the map only.
- **FR-024**: The phone layout MUST work in the app and in the demo.

**Accessibility**

- **FR-025**: The phone layout MUST meet WCAG 2.2 AA, including reflow at 200%
  text, visible focus, names for every control, and sheets that hold and
  return focus.

### Key Entities

- **Phone layout**: the board's arrangement when the viewport's shorter side
  is below the threshold. It holds no data of its own.
- **Viewer role on the board**: Game Master (owner included) or player
  (Trusted Player included), as the server already reports it. It picks which
  controls the phone layout offers.
- **Sheet**: a panel shown over the lower part of a phone screen, with a
  smaller and a full size.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: At 412 × 915 with nothing open, the map takes at least 85% of the
  screen for a player and at least 80% for a Game Master (today about 75% for
  both, before the open Select filter and the bottom tray are counted).
- **SC-002**: Each of a player's four jobs (find their character, roll, open
  their sheet, read chat) is reached in one tap from the board, and the
  current turn is visible in combat with zero taps.
- **SC-003**: Every board tool and panel a Game Master has on desktop is
  reached on a phone in at most two taps from the board.
- **SC-004**: In every board state tested, no tappable control is covered by
  another, and none is smaller than 44 × 44 pixels to the touch.
- **SC-005**: A player's phone board shows no control the player cannot use.
- **SC-006**: The existing board tests at desktop size pass unchanged.
- **SC-007**: The owner plays a session's worth of the demo on their phone, as
  a Game Master and as a player, and greenlights it before anything ships.

## Assumptions

- **The phone threshold** is a shorter side below 600 CSS pixels: every
  current phone in either orientation is under it, and the smallest common
  tablet in portrait (about 744) is over it. The plan may move it with
  evidence.
- **44 × 44 pixels** is the touch size, above the WCAG 2.2 AA minimum of 24,
  because the people using it are holding the phone, not pointing a mouse.
- **A Trusted Player** gets the player's board on a phone; what sets them apart
  is on the world's pages.
- **The 85% and 80% shares** count the compact controls that stay on the map;
  an open sheet or the expanded dice do not count against them.
- **Spec 069's touch input** is how a player moves their token once it is
  selected; this spec does not change gestures on the map.
- **Today's limits apply**: Chromium is the supported browser. The owner's
  phone test uses Firefox as well, and it is worth what it finds, but it is
  not a gate.

## Out of Scope

- The world's other pages (Overview, Players, Scenes, Compendium, Settings).
- Gestures on the map, and anything else the engine draws.
- A tablet layout of its own.
- **Recorded, not fixed here**: `crypto.randomUUID` exists only in a secure
  context. A phone opening a dev or self-hosted server by plain `http://` LAN
  address gets a board that throws on load ("Something went wrong on this
  page"). Found 2026-10-16 on the owner's phone; `http://localhost` through
  `adb reverse` works around it for testing. A self-hosted table on a LAN
  without TLS would hit it too, so it needs its own fix.
