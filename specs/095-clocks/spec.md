# Feature Specification: Clocks

**Feature Branch**: `095-clocks`
**Created**: 2026-10-13
**Status**: Draft
**Input**: Spec 029's follow-up F001: "Blades' progress clocks and Genie's
puzzle/doom clocks fill toward an event rather than depleting toward zero,
belong to a situation rather than to a token, and are counted in discrete
named segments rather than as a proportion. Declaring one as a token resource
to make it fit would be the fixture-shaped mistake this feature twice
avoided."

## Overview

A clock is a circle cut into a small number of segments that the Game Master
fills in as a situation develops: the alarm in the manor rising, the crew's
long-term project nearing done, the villain's plan about to land. When the
last segment fills, the thing the clock was named for happens. Blades in the
Dark built its whole pace around them; Genie's Doom Clock and Puzzle Clocks
are the same device.

Spec 029 gave tokens their resources and twice refused to make a clock one
of them, and it was right to. A resource and a clock differ in every way
that matters to the model:

| | Resource (029) | Clock (this spec) |
| --- | --- | --- |
| Belongs to | a token, through its actor | a situation: this scene, or the whole world |
| Direction | depletes toward zero | fills toward an event |
| Counted as | a proportion of a maximum | so many of a few named segments |
| Drawn | on the board, under the token | in the dock, beside the table's other state |
| Who sees it | whoever may see the token | whoever the Game Master has shown it to |

So a clock gets its own home, its own direction, and segment rendering
rather than a fraction. Four of six is four of six, not 67%.

This spec follows 029's one rule above all others: **the engine and the
shared web code know no game system.** A system declares the kinds of clock
it uses in its manifest; the host gives a Game Master the means to create,
tick, show and clear them; every client at the table sees the result through
the same event flow everything else uses. Nothing in `apps/web/src` or
`crates/` names Blades, Genie or any other system, and a system whose
manifest declares no clocks gets no clocks tab at all.

### Where a clock lives

A clock belongs to a **world** and, when the Game Master says so, to one of
its **scenes**.

The two homes are both real. A Blades faction clock ("The Lampblacks move
against us", 8 segments) runs across a whole season and would be absurd
pinned to one map; it is the world's. A heist's alarm clock ("Bluecoats
arrive", 6 segments) belongs to the manor the crew is standing in and should
not follow them to the next job; it is the scene's. Genie's Doom Clock is a
session-long thing; a Puzzle Clock is this room's. Forcing either onto the
other home would make the Game Master clean up after the model.

A world-level clock has no scene; a scene-level clock names its scene. The
dock shows the world's clocks and the current scene's clocks together, the
world's first, each group labelled. A clock never belongs to a token, an
actor, or a member. (The table is not a third home: "the table" and "the
world" are the same thing from the clock's point of view.)

### Dock, not board

A clock is drawn by React in the play dock. It is not drawn on the board.

The argument for the board was considered and rejected. A clock has no
position: it is not at a place on the map, and it is not attached to a
thing that is. Everything the board draws is spatial or belongs to something
spatial (tokens, their bars, their condition badges, walls, lights, shapes).
A clock drawn on the board would have to be pinned to a corner of the
viewport, which is chrome, and Constitution principle I names panels and
chrome as React's. Drawing it in the engine would also mean a new render
plugin, a new sync message, a new engine type and a wasm rebuild to deliver
what a React component delivers in one file. The smallest valid solution is
the dock. If a later spec finds a clock that genuinely lives on the map (a
ritual circle that fills as tokens step on it, say), that spec can argue for
the board with a reason this one does not have.

### Relationship to Genie's session clocks

Genie already carries a Doom Clock and Puzzle Clocks (spec 018 US7, spec 020
rewards) in its own tables (`world_genie_sessions`,
`world_genie_puzzle_clocks`, `world_genie_puzzle_clock_rewards`), its own
mutations (`advanceDoomClock`, `createPuzzleClock`, `advancePuzzleClock`) and
its own dock component (`SessionClocks.tsx`). Those clocks carry **rules**:
a full Doom Clock ends the session as a loss (018 FR-016), a Puzzle Clock pays
rewards at trigger segments (020), and wishes spend against them. Rules are
the pack's business and stay in the pack.

The decision: this spec does not move or replace Genie's session clocks. It
gives Genie, like every other system, the plain situation clock a Game Master
reaches for ad hoc (a clock with a label, some segments, and no rules), and
Genie declares the kinds it uses (`doom`, `puzzle`) in its manifest so its
ad hoc clocks look like its rulebook's. Re-basing 018's session clocks onto
this spec's generic clock, so that one component draws both, is a follow-up
recorded in this spec's tasks and is not blocked on: it would be a change to
a full-time system's rules path, and that deserves its own proof.

## Clarifications

### Session 2026-10-13

- Q: Does a clock belong to a scene or to the world? → A: Either. A clock
  belongs to a world and optionally to one of its scenes. Argued above.
- Q: Does the engine draw clocks? → A: No. Dock only. Argued above.
- Q: What is a hidden clock? → A: One the Game Master has not shown. A
  hidden clock does not reach a player's client at all: not greyed, not
  "a clock exists", not present in the subscription payload. Its existence
  is information, exactly as 029 treated a token's hidden resource and
  spec 051 a GM-only roll. Proven on the wire, never on the screen.
- Q: What are the allowed segment counts? → A: The kind declares them.
  A kind's `segments` is a non-empty list of allowed sizes, each between 2
  and 12; the first is the default when the Game Master does not choose.
  Blades declares `[4, 6, 8]`; Genie's `doom` declares `[6, 8, 10, 12]` and
  `puzzle` `[4, 6, 8]`. The engine has no opinion.
- Q: What happens when a clock fills? → A: It stays full. The event has
  happened; the Game Master narrates it and clears the clock (deletes it)
  or leaves it full as a record. There is no automatic consequence; a clock
  with consequences is a rule, and rules are a pack's (see Genie above).
- Q: Can a clock be ticked past full or below empty? → A: No. A tick is
  clamped to `[0, segments]`; a tick that would leave the count unchanged
  is refused as a no-op rather than recorded as a change.
- Q: Who may tick? → A: Only someone who runs the world
  (`runs_the_world()` in `thunderforge-authz`: Game Master or admin). Players
  read. This spec does not give players their own clocks; spec 093's roles
  work may open that later.
- Q: Is a clock a proportion anywhere? → A: Never on the wire and never in
  storage: `filled` and `segments` are two integers. A component may compute
  an angle to draw an arc; that is derived data and stays in the component.

## User Scenarios & Testing

### User Story 1 — A system declares its clock kinds (Priority: P1)

A system author adds a `clocks` block to the manifest naming the kinds of
clock that system uses, each with an id, a label, the segment counts it
allows, and a colour from the host's palette. A system that uses no clocks
writes nothing and gets no clocks tab.

**Why this priority**: Nothing else in this spec exists without the
declaration, and the declaration is what keeps the engine ignorant of the
systems. It is the same shape as 029's `resources` and 067's `conditions`.

**Independent Test**: `cargo test -p thunderforge-pack-system-spec` accepts
the Blades and Genie manifests, refuses a manifest with two kinds sharing an
id, a blank label, an empty `segments` list, a segment count outside 2–12, a
colour not on the palette, or an unknown field; `make check-packs` passes.

**Acceptance Scenarios**:

1. **Given** the Blades manifest declares a `progress` kind with
   `segments: [4, 6, 8]`, **When** the server loads its registry, **Then**
   `worldSystemClockKinds(worldId)` for a Blades world returns that kind
   with those sizes and nothing else.
2. **Given** a manifest declaring a kind with `segments: [13]`, **When**
   it is validated, **Then** it is refused with a message naming the kind
   and the bound.
3. **Given** a system whose manifest has no `clocks` block (D&D 5e), **When**
   a Game Master opens a world of that system, **Then** the dock has no
   clocks tab and `worldSystemClockKinds` returns an empty list.

---

### User Story 2 — A Game Master creates and ticks a clock (Priority: P1)

The Game Master opens the dock's Clocks tab, picks a kind, gives the clock a
label, chooses how many segments, and decides whether it lives in this scene
or the world. The clock appears. Later they tick it up (or back, when the
fiction undoes progress). When the last segment fills the clock reads full.

**Why this priority**: This is the feature. Without it the declaration is
inert.

**Independent Test**: Playwright, one GM session: create a 6-segment scene
clock, tick it three times, see "3 of 6" and three filled segments; tick it
to full; see it full and the tick-up control unavailable; delete it.

**Acceptance Scenarios**:

1. **Given** a Blades world and its scene, **When** the Game Master creates
   "Bluecoats arrive", 6 segments, in this scene, **Then** the dock lists it
   under the scene's heading at 0 of 6 and a `WorldEvent` records the
   creation.
2. **Given** that clock, **When** the Game Master ticks it twice, **Then**
   it reads 2 of 6, two segments are drawn filled, and each tick is one
   `WorldEvent`.
3. **Given** a clock at 6 of 6, **When** the Game Master ticks it up,
   **Then** the server refuses and the clock is unchanged.
4. **Given** a clock at 0 of 6, **When** the Game Master ticks it down,
   **Then** the server refuses and the clock is unchanged.
5. **Given** a world-level clock, **When** the Game Master changes to another
   scene, **Then** the clock is still listed under the world's heading.
6. **Given** a scene-level clock in scene A, **When** the Game Master changes
   to scene B, **Then** it is not listed; returning to A lists it again.

---

### User Story 3 — Everyone at the table sees the same clock (Priority: P1)

A player at the table sees the clocks the Game Master has shown. When the
Game Master ticks one, the player's dock updates without a reload, because
the change went mutation → server → `WorldEvent` → NOTIFY → every client's
store, like every other change at the table.

**Why this priority**: A clock no one else can see is a note on the Game
Master's pad. The point is the shared dread as the segments fill.

**Independent Test**: Playwright, two sessions (GM and player) on one world:
GM creates and shows a clock; the player's dock shows it; GM ticks; the
player's dock shows the new count within the subscription's usual latency.

**Acceptance Scenarios**:

1. **Given** a shown clock, **When** the Game Master ticks it, **Then** the
   player's dock reads the new count without a reload.
2. **Given** a shown clock, **When** the Game Master deletes it, **Then** it
   leaves the player's dock.
3. **Given** a player who joins after a clock was created and ticked,
   **When** their dock mounts, **Then** it reads the current count (state is
   fetched, not replayed).
4. **Given** a player, **When** they look at a clock, **Then** there is no
   control to tick, show, or delete it.

---

### User Story 4 — The Game Master keeps a clock to themselves (Priority: P2)

Some clocks are for the Game Master alone: the villain's plan, the betrayal
brewing. The Game Master creates a clock hidden, ticks it in private, and
shows it when the fiction calls for the reveal. A hidden clock is not on a
player's screen and not in a player's data.

**Why this priority**: Not needed to use clocks at all, but needed to use
them the way Blades and Genie are played, and the disclosure rule is the
kind of thing that is cheap to get right now and expensive to retrofit.

**Independent Test**: Playwright, GM and player sessions, asserting on the
player's GraphQL responses and subscription payloads, not the screen: a
hidden clock is absent from the player's `worldClocks` result and no
`WorldEvent` about it reaches the player's subscription; after the GM shows
it, the player receives it and the dock lists it.

**Acceptance Scenarios**:

1. **Given** a hidden clock, **When** the player's client queries
   `worldClocks`, **Then** the response does not contain it.
2. **Given** a hidden clock, **When** the Game Master ticks it, **Then** no
   event for it reaches the player's subscription.
3. **Given** a hidden clock, **When** the Game Master shows it, **Then** the
   player receives one event, fetches it, and the dock lists it at its
   current count (the ticks made while hidden are not replayed).
4. **Given** a shown clock, **When** the Game Master hides it, **Then** the
   player receives a removal and it leaves the dock.
5. **Given** any clock, **When** the Game Master looks, **Then** it is listed
   with its visibility marked.

---

### User Story 5 — Two systems, one engine (Priority: P1)

The same host code gives a Blades table progress clocks and a Genie table
doom and puzzle clocks, each with its own kinds, labels, sizes and colours,
without the host naming either.

**Why this priority**: This is the proof that the design is right, in the
way 029's four-system e2e was. It is a P1 because its absence would let a
Blades-shaped implementation land.

**Independent Test**: one Playwright spec looping over Blades and Genie,
creating a world of each, reading `worldSystemClockKinds`, creating a clock
of the system's first kind at its default size, ticking it, and checking the
dock reads the system's own label. Plus a static check: no file under
`apps/web/src/` or `crates/` added or changed by this feature contains a
system id.

**Acceptance Scenarios**:

1. **Given** a Blades world, **Then** the Clocks tab offers "Progress clock"
   at 4, 6 or 8 segments.
2. **Given** a Genie world, **Then** the Clocks tab offers "Doom clock" and
   "Puzzle clock" with their own sizes.
3. **Given** a D&D 5e world, **Then** there is no Clocks tab.
4. **Given** the host's web and server source, **Then** `grep` for
   `blades_in_the_dark` and `genie` in the files this feature touches finds
   nothing outside `packs/` and `e2e/`.

---

### Edge Cases

- **The manifest stops declaring a kind that stored clocks use.** The clock
  is listed with its stored label, its count, and a neutral colour, and may
  still be ticked and deleted. Declare the kind again and it is as it was.
  Same stance as 067's undeclared condition: the data is not deleted.
- **The scene a clock belongs to is deleted.** The clock goes with it
  (`ON DELETE CASCADE`); a world-level clock is unaffected.
- **A Game Master changes a clock's label.** Allowed; it is a
  `WorldEvent` like a tick.
- **Two Game Masters tick at once.** Each tick is `filled = filled + n`
  applied in the database and clamped there, so two +1 ticks make +2; the
  last write does not win over the first.
- **Play is paused.** Ticking a clock is a change to the table's state;
  it is refused while paused, like a condition change (067). Showing,
  hiding and creating are the Game Master's bookkeeping and are allowed.
- **A kind declares a single size.** The size picker is not shown; the
  clock is created at that size.
- **The world's system has no clocks but old data has.** Impossible by
  construction: a world does not change system. Noted so no one adds a
  migration path for it.
- **A clock event arrives for a scene the client is not in.** The store
  keeps every clock the client may see for the whole world; the dock
  filters by current scene. Changing scene is then a filter change, not a
  fetch.

## Requirements

### Manifest

- **FR-001**: `SystemManifest` gains an optional `clocks` block: a list of
  `SystemClockKind { id, label, description?, segments, color }`. Absent or
  empty means the system uses no clocks. The block is validated with the
  manifest: unique ids, non-blank labels, `segments` a non-empty list of
  distinct integers in `[2, 12]`, `color` one of the board's colour tokens
  (`danger`, `warning`, `positive`, `info`, `arcane`, `neutral`), and no
  unknown fields. Refusals name the kind.
- **FR-002**: The first entry of `segments` is the kind's default size.
- **FR-003**: `worldSystemClockKinds(worldId)` returns the world's system's
  declared kinds in manifest order to any member of the world.
- **FR-004**: Blades declares `progress` (`[4, 6, 8]`); Genie declares
  `doom` (`[6, 8, 10, 12]`) and `puzzle` (`[4, 6, 8]`). No other shipped
  system is changed. Each declaration is reviewed against the system's own
  rulebook and is the pack author's, not the host's.
- **FR-005**: Nothing in `crates/thunderforge-server`,
  `crates/thunderforge-pack-system-spec`, `crates/thunderforge-canvas-core`,
  or `apps/web/src` names a game system or a clock kind. Enforced by the
  existing `scripts/check-system-registry.mjs` stance and a grep in the
  feature's proof.

### Storage and events

- **FR-006**: A `world_clocks` table: `id`, `world_id` (not null, cascade),
  `scene_id` (nullable, cascade), `kind_id` (text, the manifest kind's id),
  `label`, `segments` (2–12, checked), `filled` (0–segments, checked),
  `shown` (bool, default false), `created_by`, `updated_by`, `created_at`,
  `updated_at`. Indexed on `(world_id)` and `(scene_id)`.
- **FR-007**: One new event code, `EVENT_CODE_CLOCK_CHANGED` (43), with
  payload `{"clockId", "action": "created" | "updated" | "deleted",
  "shown": bool}`. The payload carries no count; the client fetches the
  clock on `created` and `updated`, and removes it on `deleted`. (Carrying
  the count would mean a player who later gains sight of the clock could
  have read its history from the stream.)
- **FR-008**: Per-viewer reach, as spec 051's rolls do: an event for a clock
  whose `shown` is false reaches only subscribers who run the world. A
  show (`shown` false → true) is delivered to players as `updated`; a hide
  (true → false) is delivered to players as `deleted`, carrying `shown:
  false` so a client that somehow holds the clock drops it. The Game Master
  always receives the true action.
- **FR-009**: Every write records exactly one `WorldEvent` after it commits,
  best-effort like every other announcement.

### Mutations and queries

- **FR-010**: `createClock(worldId, sceneId?, kindId, label, segments?,
  shown?)` creates a clock at `filled = 0`. `segments` defaults to the
  kind's default and must be one of the kind's allowed sizes. `kindId`
  must be declared by the world's system. Requires `runs_the_world()`.
- **FR-011**: `tickClock(clockId, by: Int)` adds `by` (which may be
  negative) to `filled`, applied and clamped in one SQL statement. A tick
  that would not change `filled` is refused with a message saying the clock
  is full or empty. Requires `runs_the_world()`; refused while play is
  paused.
- **FR-012**: `setClockShown(clockId, shown)` and `updateClockLabel(clockId,
  label)` and `deleteClock(clockId)`. Requires `runs_the_world()`.
- **FR-013**: `worldClocks(worldId)` returns every clock of the world the
  caller may see: all of them for someone who runs the world; only `shown`
  ones for anyone else. The caller must be a member. There is no
  `sceneClocks` query; the client filters.
- **FR-014**: Every refusal is a GraphQL error with a message a Game Master
  can act on ("clock is full", "kind `x` is not declared by this system",
  "7 is not an allowed size for `progress`").

### Store and sync

- **FR-015**: `WorldState` gains `clocks: Record<string, WorldClock>` and
  the commands `upsert_clock` and `remove_clock` (from sync) and the intents
  `create_clock`, `tick_clock`, `set_clock_shown`, `update_clock_label`,
  `delete_clock` (from the UI). `WorldClock` is
  `{ id, worldId, sceneId: string | null, kindId, label, segments, filled,
  shown }`.
- **FR-016**: `apps/web/src/engine/world/sync/clocks.ts` provides
  `startClockMutationBridge` (intent → GraphQL mutation → confirmed
  upsert, with the optimistic tick rolled back on refusal) and
  `applyClockWorldEvent` (event 43 → fetch via `@/api/clocks` → upsert, or
  remove on `deleted`), registered beside the other kinds in `sync/index.ts`.
- **FR-017**: The optimistic tick: on `tick_clock` the store applies the
  clamped count at once; a refusal restores the cached value and surfaces
  the server's message. Create, show, hide and delete wait for the server.
- **FR-018**: Components dispatch into the store and read from it; no
  component imports the GraphQL client.
- **FR-019**: `prepareDerivedData` is not needed: a clock has no derived
  fields on the wire. The arc a component draws is computed in the
  component.

### Dock

- **FR-020**: A `clocks` dock section (`DockSectionId "clocks"`, label
  "Clocks", `PlayDock/ClocksPanel.tsx`) is mounted by `useDockSections` only
  when `worldSystemClockKinds` is non-empty. A system with no kinds has no
  tab, not an empty tab.
- **FR-021**: The panel lists world-level clocks under "This world" and the
  current scene's under the scene's name, in creation order. Each clock
  shows its label, its kind's label, a ring of `segments` wedges with
  `filled` of them in the kind's colour, and the text "`filled` of
  `segments`". A full clock is marked full.
- **FR-022**: For someone who runs the world, each clock has tick-up,
  tick-down, show/hide, rename and delete controls, and the panel has a
  create form (kind, label, size from the kind's list, this scene / the
  world, shown). Tick-up is unavailable at full and tick-down at empty.
  For anyone else there are no controls.
- **FR-023**: The ring is accessible: the list item carries
  `aria-label="<label>, <filled> of <segments>"`, and the wedges are
  `aria-hidden`. Spec 092's accessibility pass will audit it; this spec
  does not wait for it.
- **FR-024**: A clock whose kind is no longer declared is drawn with its
  stored label, the kind id as its kind label, and the `neutral` colour.

### Out of scope

- Players' own clocks, or any clock a player may tick. (093.)
- Moving Genie's session clocks (018/020) onto this model. (Follow-up F001
  in tasks.)
- Clocks on the board. (Argued above.)
- Linked clocks, racing clocks, tug-of-war clocks (Blades' variants). The
  model does not prevent them; nothing here needs them.
- A clock in the demo app.

## Key Entities

- **SystemClockKind** (manifest): what a system calls a clock, the sizes it
  allows, and its colour. Lives in
  `crates/thunderforge-pack-system-spec/src/clocks.rs`; data, never code.
- **WorldClock** (server row, GraphQL type, store record): one clock at one
  table. Belongs to a world, optionally to a scene; names a kind by id.
  `filled` of `segments`; `shown` or not.
- **WorldEvent 43** (`EVENT_CODE_CLOCK_CHANGED`): the announcement that a
  clock was created, updated or deleted, reach-filtered by `shown`.

## Success Criteria

- **SC-001**: On a Blades world and a Genie world, a Game Master creates a
  clock of each system's first kind, ticks it, and the dock reads the
  system's own kind label and the new count. One Playwright spec, one loop,
  two systems, no system named in host code. (`clocks-systems.spec.ts`.)
- **SC-002**: A tick made by the Game Master is on the player's dock within
  2 s on a local stack, without a reload. (`clocks-tick.spec.ts`.)
- **SC-003**: A hidden clock is absent from the player's `worldClocks`
  response and from the player's subscription stream across a create and
  two ticks; showing it delivers it at its current count. Asserted on the
  wire. (`clocks-hidden.spec.ts`.)
- **SC-004**: A tick past full or below empty is refused by the server and
  the Game Master's dock shows the clock unchanged with the refusal's
  message. (`clocks-tick.spec.ts`.)
- **SC-005**: A D&D 5e world has no Clocks tab; `worldSystemClockKinds`
  returns `[]`. (`clocks-systems.spec.ts`.)
- **SC-006**: A manifest with a kind outside the rules is refused by
  `validate_system_manifest` with a message naming the kind; the shipped
  Blades and Genie manifests pass. (`cargo test -p
  thunderforge-pack-system-spec`, `make check-packs`.)
- **SC-007**: The `clocks` slice (`pnpm e2e:clocks`) runs alone and green
  in under 10 minutes, and `pnpm verify` passes with the slice declared.

## Assumptions

- The world-event subscription and the `reaches` filter exist as spec 051
  left them; this spec adds one more code to the filter, it does not change
  the transport.
- The dock's section list is the right place for a tab; `useDockSections`
  already decides per world what to mount.
- `runs_the_world()` is the correct gate today; 093 may replace it with a
  capability, in which case the clock mutations take the new gate like
  everything else.
- Genie's session clocks keep working untouched; the Genie e2e
  `genie-puzzle-clock-rewards.spec.ts` is a neighbour of this slice to
  prove it.
