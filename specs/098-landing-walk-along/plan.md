# Implementation Plan: The Hero Walks the Page With You

**Branch**: `landing-tavern-hero` | **Date**: 2026-10-15 | **Spec**: [spec.md](./spec.md)

## Summary

The page table (`apps/landing/src/engine/PageTable.tsx`) today:
- chases a point picked by `along(path, fraction(scroll))`;
- moves straight toward it.

This plan replaces that with a **route** (`src/engine/route.ts`):
- an ordered list of stations, each with a page box and a hold band;
- legs between them, planned by `plan_path` (097) when the engine runs;
- a scroll → `s` mapping that keeps the hero near the viewport centre and
  holds at stations.

The hero eases `s`, not position, so it can only ever be on the route.

The surroundings come from small modules:
- **goblins** (`src/engine/goblins.ts`): place three tokens below the
  tavern and send `defeated` from `s`.
- **map selection** (`src/sections/mapPick.ts`): an event from
  `MapLegend`.
- **dungeon adapter** (`src/room/pageRoom.ts`): `sceneCommands` and
  `outcomeCommands` re-projected onto the page grid.
- **campfire** (`src/sections/Campfire.tsx`): the same shape as
  `HeroTavern`.

PageTable.tsx (~731 lines) only gains the wiring. Everything new lives in
these modules, to stay well under the 1000-line rule.

## Technical Context

- **Page world**: `toWorld` in `matScene.ts`, with the page cell from
  `HeroTavern`'s grid snap.
- **Tavern walls**: taken from `public/maps/tavern-sample.json` (dd2vtt
  `line_of_sight` and `portals`) and scaled to the page cell. The south door
  is sent open.
- **Route**: stations are recomputed on `measure()` (resize, fonts,
  selection). Legs are re-planned only when a station moves.
- **Scroll → s**: for each station `k`, `anchor_k` is the scroll at which
  its box centre is at the viewport centre. `s(scroll)` is piecewise
  linear between `(anchor_k + hold_k, s_k)` and
  `(anchor_{k+1}, s_{k+1})`, and flat over the hold. Monotone, so scrolling
  back up reverses it exactly.
- **Dungeon on the page**:
  - The room stays a `Room` component for its UI (buttons, words, goals),
    but on wide its frame does not take the table.
  - Its `Send` is wrapped by the adapter and routed to the page table.
  - "Play" sets the route's dungeon station to the room's start square and
    turns on room mode (viewer = walker, room lights).
  - "Not played": the room's station is its entrance above the map, and
    the next leg plans round the dungeon's walls and its map rectangle.
    The rectangle is added as blocked cells, so the hero walks round the
    map rather than through it.
- **Dusk**: `journey.ts` `duskAt` and `nightAt` are reworked to page
  distances, with a slew cap.

## Phases

1. The route and `s` easing, with the tavern's walls and door
   (US1, US3, FR-002–FR-005).
2. Goblins (US2).
3. Map selection station (US4).
4. Dungeon on the page grid, played and not played (US5).
5. Campfire (US6), when the owner's map arrives.
6. Dusk at size (US7).

Each phase gets its own e2e proof, and is committed locally as it lands.
Nothing is pushed before the owner's greenlight.
