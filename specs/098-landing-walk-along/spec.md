# Feature Specification: The Hero Walks the Page With You

**Feature Branch**: `098-landing-walk-along`
**Created**: 2026-10-15
**Status**: Draft
**Input**: The owner, on the wide landing:

> "if they don't move the hero i want the hero to naturally path through
> the southern exit of the tavern, and we should add those monsters back in
> just below the tavern 3 goblins, so when you scroll the monsters get
> little skull and bones over them when you scroll past simulating combat
> ... as you scroll it traces the content keeping the hero in the viewport
> near the center of it ... the darkness transition is still a bit jarring
> on larger resolutions ... the token should stand under the selected map,
> if the user selects another map the token follows them ... when we hit
> play one room the token enters the map from the top seamlessly as they
> did for the tavern, if the user doesn't play the map when the token's
> about to get out of the viewport the token should walk out of the map and
> around it ... if they scroll back up we walk back to take our position on
> the map ... then if they just keep scrolling we go down ... kinda like a
> sing-along with the token. when we hit pull up a chair the token enters
> again an area with all the same heroes as above around a campfire map."
>
> "and if the user scrolls up we just reverse the pathing"

The dungeon is drawn on the page grid like the tavern (owner's choice,
2026-10-15). There is one table and one hero.

## Overview

On a wide screen (≥ 1200 px, `useWide`), the page's own table carries one
hero from the tavern at the top to a campfire at the bottom. Scroll is the
clock:

- Scrolling down walks the hero forward along the route.
- Scrolling up walks it back along the same route.
- Stopping lets it finish the step it was on and stand.

Every leg of the route is planned by the engine (097) round the walls the
page has put on the table, and the goblins fall by the engine's own
down-state (096).

The route follows the content:

1. Out of the tavern by its south door.
2. Through three goblins standing just below it, each falling as the hero
   passes.
3. Into the map section, to stand under whichever atlas tile is chosen.
4. Into the dungeon from the top when it is played, or round it when it is
   not.
5. Down past the systems and numbers.
6. Into a campfire beside "Pull up a chair", where the party sits.

Phones and narrow screens keep the page they have. Nothing in this spec
changes below 1200 px.

## User Stories

### US1: Out of the tavern by the door (P1)

A visitor who does not touch the hero scrolls down. The hero walks from its
seat to the tavern's south door, through it and out. It never crosses a
tavern wall. Scrolling back up walks it back in, to the same seat.

**Acceptance**:
1. At the top of the page the hero stands on its seat.
2. While the hero is inside the tavern, every position it takes is in a
   cell inside the tavern's walls, or in the doorway.
3. Scrolling back to the top returns it to the seat.

### US2: Three goblins fall (P1)

Three goblins stand on the path just below the tavern. Each falls (096)
as the hero passes it, and gets up again when the visitor scrolls back
above it.

### US3: The hero stays near the middle (P1)

While the route is walked, the hero stays within the middle 40% of the
viewport's height. Where the content is wider than one column, it holds
at stations (the tavern seat, the chosen tile, the dungeon entrance, the
campfire) for some scroll before moving on, so the walk reads as
following the page rather than racing it.

### US4: Under the chosen map (P1)

In the map section, the hero stands under the selected atlas tile. When the
visitor selects another tile, the hero walks to it by an engine path.

### US5: Into the room, or round it (P1)

At "Then play one room":
- **Played**: the hero enters the dungeon from its top edge and is the
  room's hero. The referee (`@thunderforge/encounter`) drives it exactly
  as the framed room does today, and sight closes to the hero's while
  play lasts.
- **Not played**: as the hero nears the top of the viewport, it leaves
  its entrance spot and walks round the outside of the dungeon map by an
  engine path, then on down the page. Scrolling back up retraces the
  path and returns it to its entrance spot.

### US6: The campfire (P2)

Beside "Pull up a chair" there is the owner's camp map
(`camp-sample.dd2vtt`, 8x8 squares). Every hero the visitor did not pick
sits on the logs round the fire. The walker comes in from the top and
parks on the log in the empty seat. The owner, 2026-10-15: "position all
unused hero tokens on the logs ... and the hero that's moving with the
scroll bar parks themselves on the log on an empty space".

### US7: Dusk at any size (P2)

Evening falls gently at 1920 and 2560 px tall-and-wide screens, as it does
at 1440. No single wheel notch darkens the page by more than a set step.

## Requirements

- **FR-001**: Wide only. Narrow and phone render paths are not changed;
  the e2e `phone` and `tablet` projects pass unchanged.
- **FR-002**: The route is a list of legs between stations. Each leg is a
  waypoint list from the engine's `plan_path` (097) over the walls on the
  page table. When the engine is not running, a leg is the straight gutter
  path used today.
- **FR-003**: Scroll maps to a route position `s` (length along the route)
  by a monotone function that keeps the hero near the viewport centre
  (US3). It holds `s` constant over a station's hold band.
- **FR-004**: The hero eases toward `s` along the route, never straight to
  a point off it. This replaces the current "move straight toward the
  target", which cuts through walls.
- **FR-005**: The tavern sends its real walls and its south door, open.
- **FR-006**: Three goblin tokens stand below the tavern. A goblin is down
  (`defeated: true`) while the hero's `s` is past the goblin's
  `s`, and up otherwise.
- **FR-007**: `MapLegend` publishes the selected tile. The route's map
  station is the cell under that tile's box, recomputed on selection and
  resize.
- **FR-008**: The dungeon is drawn on the page grid as a canvas image, with
  its walls, doors and chest sent in page coordinates. An adapter
  translates the room's scene and outcome commands:
  - map coordinates → page world;
  - `gridSize` → the page cell;
  - background → canvas image;
  - camera and grid commands are dropped;
  - the room's `HERO_ID` → the page walker.
- **FR-009**: While the room is played, the page sends the room's lights
  and ambient darkness and views as the hero. When play ends or the
  visitor scrolls past, the room's scene comes down and the page's light
  returns.
- **FR-010**: The camp uses `camp-sample` (the owner's map, CC BY-SA
  4.0, credited like the tavern). It is drawn on the page grid to the right
  of the "Pull up a chair" heading.
  - The log seats (top-left cell, map cells) are the top log (1,1),
    (3,1) and (5,1), and the right log (6,3) and (6,5). Seats are two
    squares apart so the nameplates never overlap.
  - The unused heroes take (1,1), (6,3), (6,5), then (5,1).
  - The walker parks on (3,1), the empty seat above the fire.
  - The fire, centred on (4,4), is a light: by night it is the camp's only
    light.
  - The camp has no walls, so its leg is planned only round the page's
    other walls.
- **FR-011**: Under reduced motion, the hero stands at each station
  without walking between them, and goblins are drawn down or up without
  delay.
- **FR-012**: The darkness ramp is computed from page distance, not
  viewport height, with a cap on change per scroll pixel (US7).

## Success Criteria

- **SC-001**: e2e (desktop): with no input, scrolling from the top to the
  goblins shows the hero leaving by the door. Every sampled position lies
  in the tavern's floor cells, the doorway, or outside the tavern.
- **SC-002**: e2e: past the goblins `token_defeated()` lists all three;
  back at the top it lists none.
- **SC-003**: e2e: at sampled scrolls, the hero's screen y is within
  30–70% of the viewport height.
- **SC-004**: e2e: selecting another atlas tile moves the hero under it.
- **SC-005**: e2e: playing the room puts the hero inside the dungeon by
  its top edge, and the existing room specs pass against the page-grid
  room.
- **SC-006**: e2e: at 1920×1080 and 2560×1440, no 100 px scroll step
  changes the darkness by more than 0.08.
- **SC-007**: `phone` and `tablet` e2e projects pass unchanged.
- **SC-008**: The owner tests it on a phone and a desktop and says ship
  before anything is pushed.
