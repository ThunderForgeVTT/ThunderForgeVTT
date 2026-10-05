# Feature Specification: Doors From a Right-Click

**Feature Branch**: `071-doors-from-a-right-click`
**Created**: 2026-10-04
**Status**: Draft
**Input**: The owner, 2026-10-04: "Do we have adequate wall, door, secret door, drawing tools, door locked, door unlocked, door GM locked making it appear like a wall, options via right-click context menus? Right click to locked, right click to unlock, double right click to admin lock."

## Why

Everything a door can be was already built (spec 030): open or shut, locked
or not, hidden from the table or shown. A locked door refuses a player and
accepts the Game Master, on the server. A hidden door is not drawn for a
player and still blocks their sight and their way, so to them it is wall.

None of it could be reached from the door. Locking, unlocking and hiding were
three buttons in the Interaction tool's panel, after selecting the wall. The
right-click menu knew about tokens and bare board and nothing else, because
the engine's right-click reported only tokens. `DoorControls` says in its own
comment that it was meant to open from a right-click on a door.

## Decisions already made

- **Nothing new is stored.** The owner's three states are what spec 030
  keeps: unlocked is `locked = false`; locked is `locked = true`; "GM locked,
  appearing as a wall" is shut, `locked = true` and `secret = true` together.
  Spec 030's reason for keeping lock and state apart stands, and a fourth
  state would undo it.
- **A single right-click opens a menu; it does not toggle the lock.** The
  owner asked for both a context menu and "right click to lock, right click
  to unlock". Lock or Unlock is in the menu a right-click opens, one press
  away. A bare right-click that toggled would leave a door with no menu, and
  would fire once before every double right-click.
- **A double right-click locks as a wall**, with no menu. It is also an item
  in the menu, so it does not depend on a gesture a keyboard or a finger does
  not have.
- **A player is offered only what the server would let them do.** A locked
  door says it is locked. A hidden door and a plain wall are not reported to
  a player's right-click at all.

## User Scenarios & Testing

### User Story 1 — The Game Master works a door where it stands (Priority: P1)

Mid-scene, the party reaches a door. The Game Master right-clicks it and
locks it; later unlocks it; later still double right-clicks a door the party
must not find, and it is wall to everyone but them.

**Independent test**: two browsers at one table; every step a right-click on
the board, asserted on the door the server holds.

**Acceptance scenarios**:

1. **Given** a plain wall, **when** the Game Master right-clicks it, **then**
   the menu offers to make it a door, and nothing else.
2. **Given** a door, **when** the Game Master right-clicks it, **then** the
   menu offers Open or Close, Lock or Unlock, Lock as a wall, and making it an
   ordinary wall — each named for what it would do now.
3. **Given** a door, **when** the Game Master right-clicks it twice in half a
   second, **then** it is shut, locked and hidden, and no menu is left open.
4. **Given** a hidden door, **when** the Game Master right-clicks it, **then**
   the menu says it is hidden and offers to show it to the table.

### User Story 2 — A player uses a door from the same gesture (Priority: P2)

A player right-clicks a door and opens it. If it is locked they are told so.
If it is hidden, their right-click is a right-click on a wall.

**Acceptance scenarios**:

1. **Given** an unlocked door a player may use, **when** they right-click it,
   **then** the menu offers Open or Close and the door changes for the table.
2. **Given** a locked door, **when** a player right-clicks it, **then** the
   menu says Locked and offers nothing to press.
3. **Given** a hidden door or a plain wall, **when** a player right-clicks
   it, **then** nothing about a door is shown.

### Edge cases

- A token standing in a doorway: the right-click is about the token.
- A door set in a wall, clicked where they meet: it is about the door.
- A double right-click whose second click lands on the menu the first one
  opened still locks the door as a wall.
- Locking as a wall an open door shuts it first: a hidden door standing open
  is a hole the table can see through and cannot see.
- A step of locking as a wall that fails leaves a locked door in view, never
  a hidden one that opens.

## Requirements

- **FR-001** The engine's right-click event names the wall or door within ten
  screen pixels of the pointer: any wall for a Game Master; for anybody else,
  only a door that is not hidden.
- **FR-002** What the menu offers on a wall or door is one rule, apart from
  the menu, decided from who is asking and the door's state.
- **FR-003** The Game Master's menu on a door: Open/Close, Lock/Unlock, Lock
  as a wall (absent when it already is one), Show it to the table (when
  hidden), Make it an ordinary wall. On a plain wall: Make this a door.
- **FR-004** A second right-click on the same door within 500 ms and 12
  pixels, by a Game Master, locks it as a wall and opens no menu.
- **FR-005** A player's menu on a door: Open/Close when the door's
  interactive lets them; a disabled Locked when it is locked; otherwise none.
- **FR-006** Every change goes through the mutations spec 030 already has.
  No new mutation, column or event.

## Success Criteria

- **SC-001** `interactive-door-menu.spec.ts` passes from the main checkout:
  designate, lock, refused player, unlock, player opens, double right-click,
  player sees wall, reveal.
- **SC-002** The interactive and canvas slices still pass.

## What this spec does not do

- It does not add a right-click menu for shapes or lights.
- It does not open the door menu from the keyboard. The ContextMenu key
  opens the selected token's menu; a wall is only selectable in the Wall
  tool. The Interaction tool's panel remains the keyboard path.
- It does not add keys, lock-picking or a check to open a locked door. A
  locked door is the Game Master's to open.
- It does not change how a secret is kept: the geometry still reaches every
  client (spec 030 US4).
