# Implementation Plan: Smooth World Navigation

**Branch**: `landing-demo-play` | **Date**: 2026-10-16 | **Spec**: [spec.md](./spec.md)

Follows spec 100 on the same branch; it ships with it, after the owner's
phone check.

## Summary

Spec 100's `WorldChromeProvider` already keeps two values per world (resume
scene, interface pack) above the routes, stale-while-revalidate. This plan
widens it to the four reads that still restart on every world page: the
world record, the member list, the claim gate and the play state. Pages
render from what is known and refresh in the background; the full-screen
loader stays only for a world the tab has never loaded. The store empties
when the signed-in user changes. No new dependency.

## Technical Context

**Language/Version**: TypeScript 5, React 19 (apps/web, apps/demo)
**Primary Dependencies**: React context only; react-router 7 (existing)
**Storage**: in-memory, per tab; nothing persisted
**Testing**: vitest (store unit tests), Playwright e2e (`worlds` slice),
demo e2e
**Target Platform**: browsers; web app and the in-browser demo
**Project Type**: web app (pnpm workspace)
**Performance Goals**: no full-screen loader on a world page after the
world's first page in a tab; one request per key per world in flight
**Constraints**: no query library (FR-010); a player never sees GM controls
(FR-006); no state carries across users (FR-007)
**Scale/Scope**: 6 route pages, 4 hooks, 1 provider; ~10 files

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

- **I. ECS owns simulation, React owns chrome** — pass. This is chrome
  only; `/play` and the engine are out of scope.
- **III. Authorization at the data boundary** — pass. Nothing is granted
  client-side: the remembered role and claim only decide what renders
  first, the server still checks every mutation, and the background read
  corrects the view (FR-005, FR-006).
- **IV. ADRs before divergent implementation** — an ADR records that
  shared client world state is a stale-while-revalidate context and not a
  query library, and the threshold for revisiting it; it lands with the
  code.
- **V. Verify before done** — `tsc -b`, eslint, vitest, the slice, and the
  demo build served to the owner's phone.
- **VI. Every feature is proven by its own slice** — slice `worlds`
  (`pnpm e2e:worlds`). Own spec: new `world-navigation.spec.ts`, added to
  the slice's `own`. Neighbours for the surfaces touched: `actor-claim.spec.ts`
  (claim gate), `players-section.spec.ts` (members and roles),
  `play-pause.spec.ts` (paused banner), `gm-staging-page.spec.ts`
  (overview), added to `neighbours`. No standalone half: the store's logic
  is covered by vitest, and the demo e2e (`demo-pages`, `demo-board`) is
  the no-server run.

Post-design re-check: pass; no violations, Complexity Tracking empty.

## Project Structure

### Documentation (this feature)

```text
specs/101-smooth-world-navigation/
├── spec.md
├── plan.md            # this file
├── research.md        # Phase 0
├── data-model.md      # Phase 1
├── quickstart.md      # Phase 1
├── contracts/
│   └── world-chrome.md  # the hooks pages call
└── checklists/requirements.md
```

### Source Code

```text
apps/web/src/hooks/
├── world-chrome-context.ts   # widen WorldChrome; in-flight dedupe; useWorldChromeRefresh
├── WorldChromeProvider.tsx   # reset on user change; dedupe map
├── useWorldMembers.ts        # seed from + write to the store
├── useWorldRole.ts           # unchanged API; loading=false when known
├── useActorClaimGate.ts      # remembered clearance + background re-check
├── useWorldPlayState.ts      # store-backed; focus refresh writes the store
└── useKnownWorld.ts          # new: world record from the store (+ forget on refusal)
apps/web/src/pages/world/
├── overview/WorldOverviewPage.tsx
├── players/PlayersRoutePage.tsx
├── scenes/ScenesRoutePage.tsx
├── scenes/SceneDetailRoutePage.tsx
├── WorldCompendiumRoutePage.tsx
└── settings/WorldSystemSettingsPage.tsx   # all: loader only when world unknown
apps/web/e2e/world-navigation.spec.ts       # new
scripts/e2e/slices.json                     # worlds: own + neighbours
docs/adrs/<date>-116-shared_world_state_is_a_context.md
```

**Structure Decision**: everything stays in `apps/web/src` (the demo
imports it), matching spec 100's provider. No new package: the store is
React glue with no logic worth a port.

## Phases

1. **Store** — widen `WorldChrome` (data-model.md), dedupe in-flight reads
   per `worldId:key`, reset on user change, add `useKnownWorld` and the
   refresh helper. Vitest for: dedupe, reset on user change, refusal
   forgets the world, unchanged answers do not re-render.
2. **World record (US1)** — the six route pages read `useKnownWorld`; the
   loader condition becomes "world unknown". Edits (session notes, system
   settings, appearance) call `remember` with the saved record (FR-004).
3. **Members and role (US2)** — `useWorldMembers` seeds from the store and
   writes every fetch and `refetch` to it; `loading` is false when known.
4. **Claim gate (US3)** — cleared state remembered per world; re-check
   always runs; a missing claim forgets it and redirects.
5. **Play state (US4)** — `useWorldPlayState` reads and writes the store;
   the focus refresh stays.
6. **Proof** — `world-navigation.spec.ts`, slice `worlds`, demo e2e, demo
   rebuild for the phone, owner check.

## Complexity Tracking

None.
