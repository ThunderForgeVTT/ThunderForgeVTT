# Feature Specification: Smooth World Navigation

**Feature Branch**: `landing-demo-play`
**Created**: 2026-10-16
**Status**: Draft
**Input**: The owner, after spec 100's shared Play label landed:

> "the button flickers a lot on transitions could we add some shared app
> state via react to make things more smooth?"
>
> "this is looking really good but before we ship can you survey anywhere
> else some shared state may be beneficial to make the app more smooth?"
>
> "use speckit to plan these upgrades"

## Overview

Spec 100 put a world's pages in one sidebar shell and added a
`WorldChromeProvider` above the routes, so Play's "Resume in …" and the
world's look carry from page to page. Everything else a world page needs is
still read again from nothing on each page change, because every page
mounts its own copy of the shell and its own hooks:

- **The world itself.** Overview, Players, Scenes, Compendium, System
  Settings and the scene detail page each call `getWorld` and return a
  full-screen `Loader` until it answers. Every sidebar click blanks the
  whole page, the sidebar included, for a world that was on screen a
  moment ago.
- **The member's role.** `useWorldRole` reads `useWorldMembers`, which
  starts empty on every page. Until it answers a Game Master looks like a
  player, so GM-only controls appear late; the overview and settings wait
  on it behind their loader.
- **The claim gate.** The overview's `useActorClaimGate` runs
  `getMyWorldMemberRole` and then `getMyActorClaim` on every visit, and the
  overview stays behind its loader until both answer.
- **The pause.** `useWorldPlayState` starts empty, so a paused world's
  banner appears after the page does.

This spec makes the app remember what it already knows about a world for
the session, show it at once, and confirm it in the background.

## User Scenarios & Testing

### US1 — Changing pages keeps the page on screen (P1)

A member moving between a world's pages sees the new page straight away,
with the sidebar and header in place, instead of a full-screen spinner.

**Why this priority**: it is the jolt the owner sees on every click.

**Independent test**: open a world's overview, wait for it, then click each
sidebar section in turn; no full-screen loader appears after the first.

**Acceptance**:
1. **Given** a world's overview has loaded, **when** the member opens
   Players, Scenes, Compendium or System Settings from the sidebar,
   **then** the page renders inside the shell without a full-screen
   loader.
2. **Given** a page opened from a known world, **when** the background read
   returns a changed world (for example a new name), **then** the page
   shows the new value without a reload or a loader.
3. **Given** the first page of a world in a tab, **when** it opens, **then**
   it shows the loader it shows today, once.
4. **Given** a member edits the world on a page (session notes, system
   settings, look), **when** they move to another page, **then** that
   page shows the edited value, not the one read before the edit.

### US2 — A Game Master's controls are there from the start (P1)

A Game Master moving between pages sees GM-only navigation and controls
on the first frame of each page, not a moment later.

**Independent test**: as a GM, move overview → players → scenes, recording
the GM-only sidebar group's presence on each page's first render.

**Acceptance**:
1. **Given** a GM has loaded any page of the world, **when** they open
   another, **then** GM-only sidebar items and controls are present when
   the page first appears.
2. **Given** a player, **then** no GM-only control appears at any point,
   including while a read is in flight.
3. **Given** the GM changes a member's role on the Players page, **when**
   they open another page, **then** that page reflects the new role.

### US3 — Returning to the overview does not re-check the character (P2)

A player who has a character, or chose "Look around first", returns to
the overview without waiting on the claim check.

**Acceptance**:
1. **Given** the gate cleared once for this world in this tab, **when**
   the player returns to the overview, **then** it renders at once.
2. **Given** the player's claim was released meanwhile, **when** the
   background re-check finds no claim, **then** they are sent to choose a
   character, as today.
3. **Given** the re-check fails (network), **then** the cleared state
   stands; the gate already fails open today.

### US4 — A paused world says so on the first frame (P3)

**Acceptance**:
1. **Given** a paused world whose pause was seen on one page, **when** the
   member opens another, **then** the paused banner is there when the
   page appears.
2. **Given** the pause ends, **when** the next read or the focus refresh
   answers, **then** the banner goes on every page.

### Edge cases

- **Signing out, or another account in the same tab.** Nothing one account
  learned may show for another: the remembered state empties when the
  signed-in user changes.
- **A world that was deleted, or access removed.** The background read
  fails or returns nothing: the page shows what it shows today for a
  missing world, not the remembered one.
- **Two worlds in one tab.** State is per world; opening world B never
  shows A's name, role, look or pause.
- **A refresh that fails** keeps what was known (spec 100's rule for the
  Play label), except for the missing-world case above.
- **The demo** has no server; the same state works over its in-browser
  data.

## Requirements

### Functional Requirements

- **FR-001**: The session remembers, per world: the world record, the
  member list, whether the claim gate cleared, and the play state, beside
  the resume scene and interface pack spec 100 already keeps.
- **FR-002**: A page with a remembered value renders with it and refreshes
  it in the background. A page with none behaves as today.
- **FR-003**: The world pages listed in the overview show a full-screen
  loader only when the world is not yet known in this tab.
- **FR-004**: An edit a page makes to the world or its members updates the
  remembered value as it saves, so other pages show the edit.
- **FR-005**: The remembered claim gate is a hint, never a grant: the
  re-check still runs on every overview visit and a missing claim still
  redirects. Nothing here changes what a role may do (spec 100 FR-010).
- **FR-006**: The remembered role must not show a player any GM-only
  control. A page may show GM controls from a remembered GM role; the
  background read removes them if the role changed.
- **FR-007**: All remembered state is cleared when the signed-in user
  changes or signs out, and none of it is persisted beyond the tab's
  memory.
- **FR-008**: A background read that reports the world missing or
  forbidden replaces the remembered world with that answer; any other
  failure keeps the remembered value.
- **FR-009**: Works the same in the web app and the demo, each mounting the
  provider once at its root.
- **FR-010**: No new runtime dependency. If the remembered kinds grow past
  about five more, the owner decides on a query library first.

### Key entities

- **World chrome (per world id)**: resume scene, interface pack (spec
  100), world record, members, claim gate cleared, play state. Held in
  memory for the tab and the signed-in user.

### Out of scope

- Game system manifests, actor, item, ability and lore lists, instance and
  settings data: read on one page or inside content with its own loading.
- The play page (`/play`) and the engine's own state.
- Persisting anything across tabs or reloads.
- A query library (TanStack Query or similar).

## Success Criteria

- **SC-001**: Web e2e: after the overview loads, moving to Players, Scenes,
  Compendium and System Settings and back shows no full-screen loader on
  any of them.
- **SC-002**: Web e2e: as a GM, the GM-only sidebar group is present on
  each page's first paint after the first page; as a player, it is never
  present.
- **SC-003**: Web e2e: a player whose claim is released while on another
  page is sent to choose a character on returning to the overview.
- **SC-004**: Web e2e: signing out and in as another user in the same tab
  shows none of the first user's world, role or pause.
- **SC-005**: Demo e2e: the same page-to-page walk shows no full-screen
  loader.
- **SC-006**: Existing slices (worlds, scenes, actors, game-systems) and
  the demo suite stay green.
- **SC-007**: The owner, on a phone, reports page changes in a world as
  smooth (the greenlight before ship).

## Assumptions

- Data on screen may be up to one background read old; the owner accepted
  this trade-off when asking for shared state.
- "Session" means the browser tab's memory; a reload starts clean.
- Live events already reaching a page (appearance, pause) keep writing to
  the same remembered state, as spec 100's appearance sync does.
