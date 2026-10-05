# Feature Specification: Pay for What You Open

**Feature Branch**: `068-pay-for-what-you-open`
**Created**: 2026-10-04
**Status**: Draft
**Input**: The owner's direction on 2026-10-04, after the repository's layout was settled (spec 065): split the frontend's code properly, and have a feature-flag practice for everything built from here on.

## Why

The web app already splits by route — `AppRoutes.tsx` loads nearly every page
with `React.lazy`. What it does not do is split *inside* a route, or keep the
entry honest, and a production build of `0314d369` shows where that costs:

| What somebody opens | What they download beyond the entry (raw / brotli) | The part they did not ask for |
| --- | --- | --- |
| Any page at all — the entry and what it imports statically | 504 kB / 139 kB | The whole feedback feature: the form, its review step, the screenshot and redaction code. A click needs it; a page load does not. |
| A scene's detail page | 819 kB / 196 kB | 767 kB / 178 kB of it is CodeMirror, imported statically for the summary editor. A lore entry and the compendium overview pay the same. |
| The board | 487 kB / 124 kB | Every bundled system's panels (98 kB) and sheets (61 kB), whichever one the table plays; and the Game Master's authoring tools, for a player who can never open them. |

One place already does it right: the session notes panel loads its editor
with `lazy`. The other three editors are the same library imported the other
way.

The second half is a practice the project does not have. Nothing in the web
app or the server is a feature flag. A feature that is not ready is either
not merged or on for everyone, and the first field test is when that stops
being acceptable.

## Decisions already made

- **Splitting is by what somebody opens, not by library.** A chunk exists
  because there is a moment a person does not need it. `manualChunks` rules
  that name libraries are for caching, and the one naming tldraw, RxDB and
  RxJS names nothing this app imports any more.
- **Flags are runtime-driven.** A flag changes without a rebuild and without a
  restart. Compile-time switches are for local development only.
- **Flags are instance settings.** Spec 040 already has one rule for
  configuration — the environment beats the instance's store beats the
  declared default, resolved per request, every change recorded. A flag is a
  declared boolean under that rule, not a second mechanism beside it.
- **RxDB stays out**, and nothing here adds a client-side store.

## User Scenarios & Testing

### User Story 1 — A page costs what is on it (Priority: P1)

Somebody on a phone at a table opens the scene list, then a scene. They get
the page; the editor arrives when the editor is on screen. A player joins the
board and downloads their own system's sheet and panels, and none of the
tools only a Game Master can open.

**Why this priority**: it is the owner's stated standard, it is measurable,
and the field test is the first time the app meets other people's networks.

**Independent test**: build, and read the output. The entry's static closure
and each route's are under a recorded budget, and no route reaches CodeMirror
statically.

**Acceptance scenarios**:

1. **Given** a production build, **when** the entry's static imports are
   followed, **then** the feedback form, its review step and the screenshot
   code are not among them, and log capture still starts before anything else
   can throw.
2. **Given** a signed-in person, **when** they press the feedback control,
   **then** the form opens as it does today, with whatever was captured before
   they pressed.
3. **Given** a scene's detail page, a lore entry or the compendium overview,
   **when** it is opened, **then** the page renders before its editor's code
   has arrived, shows the text in the meantime, and the editor takes over in
   place.
4. **Given** a table playing one system, **when** a member opens the board,
   **then** no other system's panels or sheets are downloaded.
5. **Given** a player, **when** they open the board, **then** the wall,
   lighting, shape, token and interaction tools are not downloaded; **given**
   a Game Master, **when** they open a tool, **then** it opens.
6. **Given** a build that breaks any of the above, **when** the budget check
   runs, **then** it fails and names the chunk and the import that put it
   there.

### User Story 2 — A feature can be merged before it is switched on (Priority: P2)

An administrator opens the instance's settings and finds a Features group. A
feature that is declared there is off, or on, for the whole instance; they
change it, and the next request plays by it. A developer adding a feature
declares it, asks the server whether it is on where the rule is enforced, and
asks a hook in the web app whether to show it.

**Why this priority**: the practice has to exist before the features that
need it do, and it is small because the mechanism it needs already exists.

**Independent test**: declare one flag; with it off, the server refuses the
action and the web app shows no control; an administrator turns it on, and
both change without a restart or a reload of the server.

**Acceptance scenarios**:

1. **Given** a declared flag nobody has set, **when** anyone asks, **then** it
   reads as its declared default.
2. **Given** an administrator, **when** they change a flag, **then** the
   change is recorded with who and when, and the next request sees it.
3. **Given** a flag set in the environment, **when** an administrator opens
   it, **then** it reads as set by the environment and cannot be changed
   there — the same as every other setting.
4. **Given** a flag that is off, **when** a client calls what it guards,
   **then** the server refuses; hiding the control is not the rule.
5. **Given** a signed-out visitor, **when** they ask for flags, **then** they
   are told only the flags declared as public.

### Edge cases

- A chunk that fails to load — a deploy replaced it while a tab was open —
  says so and offers a reload, rather than leaving a blank where the editor
  or the tool should be.
- An editor's code arrives after somebody has started typing in the fallback:
  the fallback is read-only, so nothing typed is lost.
- A system with no web package contributes no chunk and the board does not
  wait on one.
- A flag whose declaration was removed leaves an inert row, as any undeclared
  setting does.

## Requirements

### Story 1 — splitting

- **FR-001** The feedback form, review and screenshot code load when the
  feedback control is pressed or the help panel's report button is. Log
  capture (`feedbackLogBuffer`) stays in the entry.
- **FR-002** No module imports `@uiw/react-codemirror` or `@codemirror/*`
  statically except the editor components themselves, and every editor
  component is reached through `React.lazy`.
- **FR-003** While an editor loads, the text it will edit is shown read-only
  in its place, at the editor's size.
- **FR-004** A system's panels and sheets are one chunk per system, loaded for
  the system of the world being opened. What a pack's author writes does not
  change.
- **FR-005** What is known about a system before its chunk arrives — whether
  it fills a slot, and what it calls the panel there — is known without
  loading it.
- **FR-006** The Game Master's authoring tools on the board load when one is
  opened.
- **FR-007** A dynamic import that fails is caught at the boundary that asked
  for it, which says what did not load and offers a reload.
- **FR-008** `manualChunks` loses the rule naming libraries the app no longer
  imports.
- **FR-009** A script reads a production build and fails when the entry's
  static closure, or a named route's, exceeds its recorded budget, or when a
  module on a forbidden list is statically reachable from a route. Budgets are
  brotli bytes and live beside the script.

### Story 2 — flags

- **FR-010** A flag is a `Kind::Bool` declaration in the settings registry, in
  a `Features` group, with an environment variable, a default, and whether it
  is public.
- **FR-011** Server code asks one function whether a flag is on; it resolves
  per request.
- **FR-012** One GraphQL read returns the flags a caller may know: the public
  ones to anyone, all of them to a signed-in member.
- **FR-013** The web app reads flags once per session start and on the
  administrator's change, and exposes them through one hook. No component
  reads the environment or a build-time constant to decide what to show.
- **FR-014** `CONTRIBUTING.md` says when a feature takes a flag, how to
  declare one, and when to remove it.
- **FR-015** The first flag guards a real feature, chosen with the owner.

## Success Criteria

- **SC-001** The entry's static closure is smaller than 139 kB brotli by at
  least the feedback feature's share.
- **SC-002** A scene's detail page's static closure beyond the entry is under
  25 kB brotli (196 kB today).
- **SC-003** A player's board downloads no authoring tool and one system's
  panels and sheets.
- **SC-004** The budget check fails on a build with any of the three static
  CodeMirror imports restored.
- **SC-005** Every e2e slice that touches a changed surface passes from the
  main checkout.
- **SC-006** A flag changes what the server allows and what the web app shows
  with no rebuild and no restart.

## Assumptions

- The Bevy engine's wasm is already loaded on demand and is out of scope; its
  budget is its own (100 MB compressed is the concern threshold).
- Brotli is what a deployment serves. Raw bytes are recorded but not budgeted.
- Budgets are set from what the build measures after each split, with a small
  allowance, not from a target picked in advance.

## What this spec does not do

- It does not split `WorldPage.tsx` the file. It is 3,361 lines and that is
  its own problem; this spec changes what it imports, not how it is arranged.
- It does not add per-user or per-world flags, percentages or experiments. A
  flag is on or off for an instance.
- It does not add telemetry for chunk load times.
- It does not prefetch. `schedulePagePrefetch` stays as it is.
