# Implementation Plan: A Board That Fits a Phone

**Branch**: `102-mobile-play-board` (work lands on `landing-demo-play` until
the owner names a branch) | **Date**: 2026-10-16 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/102-mobile-play-board/spec.md`

## Summary

On a phone the play board stops being a desktop board squeezed small. The map
goes to every edge; a row of five floating buttons sits at the bottom within
thumb reach (a player's: Me, Dice, Sheet, Chat, More; a Game Master's: Tools,
Dice, Actors, Chat, Combat); panels open as half-height sheets over a live
map; the tool in hand shows as a small chip. Everything is a second
*arrangement* of the chrome the page already builds: the page already owns
the tool list, the dock sections and which of each is open, so the phone
components take the same props and crossing the threshold loses nothing. The
web app also becomes installable (a manifest and icons, no offline cache) and
the board gains a full-screen control. Desktop and tablet keep today's rails.

The design direction is impeccable's `shape` brief in
[research.md § Design brief](research.md#design-brief).

## Technical Context

**Language/Version**: TypeScript 5 (strict), React 19

**Primary Dependencies**: Vite, Tailwind, `radix-ui` (already a dependency;
its Dialog backs the sheet), the existing `FantasyIcon` set. No new
dependency.

**Storage**: None. The armed tool and open panel already live in page state;
the Select filter keeps its existing `localStorage` key. Install adds no
cache (FR-027).

**Testing**: Vitest for the pure pieces (`phoneRows`, the threshold);
Playwright at a 412 × 915 touch viewport, Chromium, in both the demo
(standalone, no server) and the app (integration, real roles and grants).

**Target Platform**: Chromium on Android first (supported browser); Firefox
on Android checked by the owner, not a gate; iPhone Safari for install only.

**Project Type**: Web app chrome (`apps/web`), inherited by `apps/demo`.

**Performance Goals**: No extra work at desktop size: the phone components
are a lazy chunk loaded only when the phone layout is chosen. A sheet opens
in under 200 ms and never stalls the board's frame loop (React chrome only).

**Constraints**: React owns the chrome, the engine owns the canvas
(Principle I): no engine change. Server stays the authority on what a person
may do (Principle III). WCAG 2.2 AA. 44 × 44 targets. Safe areas.

**Scale/Scope**: One page (`/world/:id/play`), two roles, five stories,
about ten new small components, no backend change.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-checked after Phase 1 design:
still passes.*

- **I. ECS owns simulation, React owns chrome** — PASS. Every change is DOM
  chrome around the canvas. "Me" uses the existing `lookAtToken` command and
  the existing selection facet; gestures on the map stay spec 069's.
- **II. Plugin-modular engine** — PASS, not touched.
- **III. Authorization at the data boundary** — PASS. The phone rows are
  built from the same `railTools(...)` / `useDockSections(...)` output the
  rails use, so a tool or panel the server withholds is absent in both. The
  Select filter offering only permitted kinds (FR-006) is presentation; the
  engine and server still refuse the rest.
- **IV. ADRs and specs before divergent implementation** — PASS without an
  ADR: no new subsystem, dependency or ownership boundary. The decisions are
  recorded in research.md and land with the feature. (If the sheet grows into
  a shared primitive used outside the board, that is the point to write one.)
- **V. Verify before claiming done** — PASS by plan: `pnpm -F
  @thunderforge/web check` (tsc + lint), both builds, the slice, a running
  dev instance, and the owner's phone (SC-009).
- **VI. Every feature is proven by its own slice** — new slice **`phone`**,
  run as `pnpm e2e:phone`.
  - Own specs: `apps/web/e2e/phone-board.spec.ts` (GM and player, real
    roles, a granted tool, combat turn, requests badge) and
    `apps/web/e2e/phone-app.spec.ts` (manifest, icons, full screen).
  - Standalone half: `e2e:phone:standalone` =
    `pnpm -F @thunderforge/demo e2e phone-board`
    (`apps/demo/e2e/phone-board.spec.ts`: both views of the demo, the demo
    notice and its view switch, map share, overlap scan).
  - Neighbours crossing the seam (desktop must be unchanged, SC-006):
    `play-panels-by-role.spec.ts` (dock sections by role), `dice-roll.spec.ts`
    (the dice roller moves into a sheet), `board-loading.spec.ts` (overlays
    the phone layout must not cover), `play-pause` specs (banners),
    `world-navigation.spec.ts` (the page shell), and the demo's own
    `board-panels.spec.ts` and `demo-notice.spec.ts`.
  - Not cross-cutting: no schema, auth, harness or shared primitive change.
    `pnpm e2e:which --diff` confirms the neighbour list before landing.
- **VII. Telemetry is on** — PASS. One bounded label each on the existing
  board-load timing: `layout` = `phone` | `wide` and `display` = `browser` |
  `standalone` | `fullscreen`. No ids, nothing typed. Added to the code
  allow-list in `packages/telemetry`.

## Project Structure

### Documentation (this feature)

```text
specs/102-mobile-play-board/
├── plan.md              # This file
├── research.md          # Decisions + impeccable design brief
├── data-model.md        # UI state: arrangement, rows, sheet, armed tool
├── quickstart.md        # How to prove it, desk and phone
├── contracts/
│   └── phone-board-ui.md  # Test ids, roles, row contents, manifest fields
└── tasks.md             # /speckit-tasks
```

### Source Code (repository root)

```text
apps/web/
├── index.html                          # viewport-fit=cover, interactive-widget, manifest link
├── public/
│   ├── manifest.webmanifest            # new (FR-025, FR-026)
│   └── icons/{icon-192,icon-512,maskable-512}.png   # new, generated
└── src/
    ├── hooks/
    │   ├── usePhoneLayout.ts           # new: shorter side < 600, live
    │   └── useActiveCombat.ts          # new: lifted out of CombatPanel
    ├── layouts/world-layout/WorldLayout.tsx   # h-screen → h-dvh; arrangement slot
    ├── pages/world/WorldPage.tsx       # picks rails+dock or PhoneBoard
    └── components/world/
        ├── PhoneBoard/                 # new, lazy chunk
        │   ├── PhoneBoard.tsx          # composes the pieces below
        │   ├── BottomRow.tsx
        │   ├── BoardSheet.tsx          # half / full, back gesture, focus
        │   ├── ToolsSheet.tsx          # GM: Board + Table groups
        │   ├── MoreSheet.tsx           # player: the rest + granted tools
        │   ├── ToolChip.tsx
        │   ├── TurnLine.tsx
        │   ├── FullScreenButton.tsx
        │   ├── phoneRows.ts            # pure: role + grants → row + groups
        │   └── __tests__/phoneRows.test.ts
        ├── GmToolRail/SelectionFilterMenu.tsx  # kinds limited to permitted
        ├── PlayDock/CombatPanel.tsx    # reads useActiveCombat
        └── DiceRollerPanel/DiceRollerPanel.tsx  # renders inside a sheet on phone
apps/demo/
├── index.html                          # same viewport meta (no manifest)
├── src/DemoNotice.tsx                  # top chip on phone, clear of the row
└── e2e/phone-board.spec.ts             # new (standalone half)
apps/web/e2e/{phone-board,phone-app}.spec.ts    # new (integration half)
packages/telemetry/src/…                # allow-list: layout, display labels
scripts/brand-icons.mjs                 # new: renders brand-mark.svg to PNGs
scripts/e2e/slices.json                 # new `phone` slice
package.json                            # e2e:phone, e2e:phone:standalone, e2e:phone:integration
```

**Structure Decision**: Phone chrome lives beside the desktop chrome under
`apps/web/src/components/world/PhoneBoard/`, following the repo's existing
placement of board chrome in `apps/web` (not a new package: it is page
chrome with no second consumer besides the demo, which already consumes
`apps/web`). The pure decision of "which buttons for whom" is isolated in
`phoneRows.ts` so it is unit-tested without a browser.

**Slice**: `scripts/e2e/slices.json` gains `phone` (own prefix `phone-`,
the neighbours above with their seams, `paths` = the files above), and the
root `package.json` gains `e2e:phone`, `e2e:phone:standalone` and
`e2e:phone:integration`.

## Delivery order

1. **Try-out first** (owner request): US1's player row, the edge-to-edge map
   and one sheet, in the demo, served through the https tunnel for the owner
   to hold before the rest is built. Their reaction may move the design
   brief; nothing after this step starts until they have seen it.
2. US1 → US2 → US3 → US4 → US6 → US5 regression proof.
3. Owner's phone session as GM and player, installed app included (SC-009).
   No push, no ship until they greenlight.

## Complexity Tracking

No constitution violations to justify.
