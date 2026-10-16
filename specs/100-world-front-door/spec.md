# Feature Specification: The World's Front Door

**Feature Branch**: `landing-demo-play`
**Created**: 2026-10-15
**Status**: Draft
**Input**: The owner, after spec 099 shipped:

> "can we improve our application flow on landing? a world to try can it
> launch straight into …/demo/world/d0000000-0000-4000-0002-000000000001/play ?
> also for the actual app i want to see if i can reduce the flow from
> …/world/d0000000-0000-4000-0002-000000000001 to play maybe knock out
> staging? maybe merge them? what ways can we make it more rich? and how can
> we improve the responsiveness? … the hero designer is like mobile on
> desktop its weird and the rest does a poor job of using available space so
> it needs more responsive design from phone -> desktop -> ultrawide"
>
> "yes please make it clean"

## Overview

A Game Master reaches the table in three pages today, and two of them say
the same thing.

- **The hub** (`/world/:id`, `WorldDashboardPage`) holds an at-a-glance
  strip, recent scenes, the players card, world metadata, a "Quick actions"
  card that only links on, and the campaign settings. Its button is
  "Enter world".
- **Staging** (`/world/:id/staging`, `WorldStagingPage`) holds the sidebar,
  a Play button and Last Session Notes. Its line reads "Confirm the scene and
  roster below", and neither is below.
- **Play** (`/world/:id/play`) is the table.

This spec gives a world one front door. The hub and staging become one page
inside the sidebar shell, Play is its first action, and the page shows what
a Game Master needs before the session: where it starts, who is coming, and
where the story left off.

The four world pages screenshotted at 390, 1440 and 3440px also show that
they were laid out for one width:

- **Ultrawide.** The header and footer stop at 1160px and the content at
  1800px, so a 3440px screen is half empty. Inside that, the grids stop
  growing: the players page keeps two fixed cards, and the hub's middle
  column ends after one card.
- **The character page reads like a phone on a desktop.** The page splits
  in half at `lg`: imagery and identity on the left, the whole system sheet
  squeezed into the right half. The sheet's sections ask the *sheet's*
  container how wide they are, not their own, so the six ability scores lay
  out six across inside a one-third region and their inputs spill over the
  cells.
- **Phone.** The demo notice wraps into a large rounded block over the
  middle of the page and hides the form under it.

The demo's landing links go straight to the table (US1, done).

## User Scenarios & Testing

### US1 — The landing opens the demo's table (P1, done)

A visitor who presses "Try the demo", or any demo link on the landing,
lands on "A World To Try" at `/play`, seated as its Game Master.

**Acceptance**:
1. Every landing link to the demo points at
   `/demo/world/d0000000-0000-4000-0002-000000000001/play`.
2. A cold load of that address, in a browser that has never opened the
   demo, seeds the demo and shows the map with its tokens.

### US2 — One front door for a world (P1)

A member opening a world lands on one page: the world's overview, inside
the sidebar shell, with Play as its first action.

**Acceptance**:
1. `/world/:id` renders the merged overview inside `WorldSectionShell`.
   The sidebar's "Overview" links there.
2. `/world/:id/staging` redirects to `/world/:id`, replacing the history
   entry, so old links and bookmarks keep working.
3. Every in-app link to staging goes to `/world/:id`. The overview offers
   no "Enter world": the worlds list keeps its "Enter world", and it opens
   the overview.
4. From the worlds list, a Game Master reaches the table in two clicks:
   the world, then Play.
5. A player who has not claimed a character still goes through
   `/world/:id/actor-select` (spec 017): the overview keeps the old hub's
   claim gate, so opening the world sends them there. `/play` itself does
   not gate. Staging was how such a player saw the world before choosing,
   so Actor Selection offers "Look around first": the overview opens for
   the rest of that tab, with a reminder that leads back to the picker.
6. The overview keeps everything the two pages held that a member uses:
   last session notes, the invite link, the system's `world-staging`
   panel slot, the play-paused banner, and, for a Game Master, the
   campaign settings and the system link.
7. The "Quick actions" card and the raw creator/updater ids are removed.

### US3 — The overview gets the session ready (P2)

The overview answers the three questions before a session in its first
screen: where do we start, who is coming, what happened last time.

**Acceptance**:
1. **Where we start.** Scene cards with the map's thumbnail, the scene's
   name, and when it last changed. The active scene is first and marked.
   A Game Master can press a card's "Play here", which sets it active and
   opens `/play`. Players see the cards without the action, and never see
   a hidden scene. A map with a credit shows it on the card.
2. **Who is coming.** A party strip: each member with their character's
   portrait, their name and the character's name. A character no one has
   claimed shows as an open seat.
3. **Where we left off.** Last session notes sit beside the party on a
   wide screen and under it on a narrow one, rendered as Markdown with an
   Edit action for a Game Master.
4. **Resume.** When the world has been played, Play reads "Resume" and
   names the scene it resumes in.
5. A world with no scenes, no players or no notes shows each section's
   empty state with the one action that fills it, not an empty card.

### US4 — World pages use the screen they are on (P1)

Every world page fills a phone, a desktop and an ultrawide screen with
content, not margins.

**Acceptance**:
1. The app header, footer and world pages share one width rule: fluid
   with a gutter, capped at 2640px. The header lines up with the content
   beneath it.
2. Card grids (players, scenes, compendium lists, the overview) fill rows
   by available width: about 20rem per card, as many as fit.
3. At 3440×1440 no world page leaves more than 25% of the viewport width
   empty beside its content.
4. At 390px nothing scrolls sideways, and the sidebar is the strip above
   the content it is today.
5. Text measure stays readable: prose blocks (notes, descriptions, lore)
   keep a 75ch cap inside their wider cards.

### US5 — The character page is a sheet, not a column (P1)

On a desktop the character page reads as a character sheet: the picture
and identity in a rail, and the system sheet across the rest of the page.

**Acceptance**:
1. Below `lg`, one column in the order it has today.
2. From `lg`, a narrow rail (portrait, token, name, description, owner
   controls) beside the sheet, which takes the remaining width.
3. The system sheet lays out 1, 2, 3 and 4 columns by its own width.
   At 1440px the 5e sheet shows at least 2 columns; at 3440px at least 4.
4. Each sheet region sizes its insides by the region's own width. The six
   ability scores never overflow their cells at any width from 320px to
   3440px.
5. The play dock's sheet (about 22rem wide) still reads in one column.
6. The rule lives in the host and the pack's region wrapper, so every
   system sheet gains it; no system's sheet code names a viewport width.

### US6 — The demo notice stays out of the way on a phone (P2)

**Acceptance**:
1. Below 640px the notice is a single-line pill ("Demo · nothing leaves
   this browser") that expands on tap to its full text and links, and
   collapses again.
2. Collapsed, it covers no more than 48px of the bottom of the screen.
3. From 640px up it reads as it does today.

## Requirements

- **FR-001**: The merged overview is one component in `apps/web` rendered
  at `/world/:id`. `WorldDashboardPage` and `WorldStagingPage` are folded
  into it, not kept side by side.
- **FR-002**: `/world/:id/staging` stays a route only to redirect (`replace`).
  The telemetry route table keeps the old path so dashboards stay
  comparable.
- **FR-003**: "Play here" uses the scene activation the Scenes section's
  Launch action uses (spec 022), then navigates to `/play`. No new
  mutation.
- **FR-004**: The party strip reads the roster the players page already
  loads. It shows no online indicator: the web app carries no presence
  today, and adding it is its own spec.
- **FR-005**: A scene card's picture is the scene's `previewUrl`, else its
  `backgroundUrl`, lazy-loaded. A scene without either shows its name on
  a plain tile. A scene with a `backgroundCredit` shows the author and
  licence on its card (the example maps are CC BY-SA 4.0).
- **FR-006**: One layout width rule (`--page-max: 2640px` and a gutter)
  replaces the 1160px `Container` cap and the 1800px world/admin shell
  caps. `Container narrow` (780px) is unchanged for forms and prose.
- **FR-007**: Tailwind's theme gains `3xl` (1920px) and `4xl` (2560px)
  breakpoints, and auto-fill grids use `repeat(auto-fill, minmax(20rem,
  1fr))` rather than a column count per breakpoint.
- **FR-008**: Sheet regions are each an `@container`. Section internals
  (abilities, skills, spells, persona, classes, features) query their
  region, never the sheet.
- **FR-009**: The demo notice's collapsed state is per-session UI state,
  not stored.
- **FR-010**: Nothing here changes what a role may do. Every action shown
  or hidden follows the existing GM/player checks.
- **FR-011**: "Look around first" is kept in `sessionStorage` per world,
  so it lasts for the tab and a new tab asks again. Storage that throws
  means the gate asks as before. It grants nothing: the overview shows a
  member only what it already shows them.
- **FR-012**: From `@7xl` the 5e sheet pairs regions of a height in each
  row (CSS `order` and two-row spans), so no row holds a short region
  beside a tall one and no column of a row is left empty.

### Out of scope

- The Select tool's panel opening on a first visit to `/play`. It is a
  standing decision (`SelectionFilterMenu.tsx`): it opens once and
  remembers being collapsed.
- The play screen's own layout.
- A per-player "who is coming tonight" RSVP, and online presence.

## Success Criteria

- **SC-001**: Landing e2e (desktop): every demo link's `href` is the demo
  world's `/play`, and following it shows the board.
- **SC-002**: Web e2e: a Game Master goes worlds list → world → Play and is
  on `/play` after two navigations; `/world/:id/staging` lands on
  `/world/:id`.
- **SC-003**: Web e2e: a player without a character who presses Play lands
  on `/world/:id/actor-select`.
- **SC-004**: Web e2e: "Play here" on a scene card makes it the active
  scene and opens `/play` on it.
- **SC-005**: Web e2e at 390, 1440 and 3440 wide, on the overview, players,
  character and scenes pages: no horizontal scroll; at 3440 the content's
  bounding box is at least 75% of the viewport width.
- **SC-006**: Web e2e: on the 5e character page, every ability score
  input's box lies inside its cell at 320, 390, 1024, 1440, 1920 and 3440
  wide; the sheet has ≥2 columns at 1440 and ≥4 at 3440.
- **SC-007**: Demo e2e at 390: the collapsed notice is ≤48px tall and
  expands and collapses on tap.
- **SC-008**: Screenshots of the four pages at the three widths, before
  and after, are attached to the PR for the owner's review on a phone.
