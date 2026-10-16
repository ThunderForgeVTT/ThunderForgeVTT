# Implementation Plan: The World's Front Door

**Branch**: `landing-demo-play` | **Date**: 2026-10-15 | **Spec**: [spec.md](./spec.md)

## Landing (US1, done)

- `apps/landing/src/links.ts`: `DEMO` is the demo world's `/play`. Every
  demo link on the page reads it (`Hero`, `MapLegend`, `Room`,
  `EngineFrame`). A cold load of `/play` seeds the demo, so nothing else
  changes.

## One front door (US2)

- New `apps/web/src/pages/world/overview/WorldOverviewPage.tsx`, routed at
  `/world/:id` inside `WorldSectionShell`. It takes from:
  - `WorldStagingPage`: `SessionNotesPanel`, `SessionSetupInviteLink`, the
    `world-staging` panel slot, `AboutThisTable`.
  - `WorldDashboardPage`: the at-a-glance counts, `WorldPlayersCard`'s
    data, `CampaignSettingsPanel`, the system link.
  - Dropped: "Quick actions", the creator/updater ids, "Enter world".
- `WorldStagingRoutePage` becomes `<Navigate to={`/world/${id}`} replace />`.
  `WorldDashboardPage` and `WorldStagingPage` are deleted once nothing
  imports them. The `world-staging-page` test id moves to the overview's
  `<main>` so the three specs that read it keep reading it.
- Links to `/staging` move to `/world/:id`: `WorldSidebarNav` (Overview),
  `WorldCard`, `WorldsTable`, `WelcomePage`, `CreateWorldPage` (×2),
  `ActorDetailPage` (×2), `GameMastersSideOfTheTable`,
  `WorldSystemSettingsPage`, `useDockSections` (back from play).
- `telemetry/routes.ts` keeps `/world/:id/staging`.
- Play stays where `WorldSectionShell` puts it. The overview keeps the
  hub's actor-claim gate (`useActorClaimGate`), so an unclaimed player
  opening the world lands on actor-select, as they did on the hub.

## The overview's sections (US3)

- `overview/SceneStrip.tsx`: lists scenes with `listScenes`, active first
  (`world.activeSceneId`), hidden ones only for a Game Master. "Play
  here" calls the activation the Scenes section's Launch uses, then
  navigates to `/play`. Card picture `previewUrl ?? backgroundUrl`,
  `loading="lazy"`; credit from `backgroundCredit`.
- `overview/PartyStrip.tsx`: the members and their characters, from the
  players page's loaders, with portraits from the actors' `thumbnailUrl`.
- Layout: from `xl`, scenes span the top row, then party and notes side by
  side; below `xl`, one column: Play, scenes, party, notes, settings.
- Play's label: "Resume in <scene>" when `activeSceneId` is set.
  `WorldSectionShell` takes an optional label so the other hub pages keep
  "Play".
- Empty states: no scenes → "Create a scene"; no players → the invite
  link; no notes → "Write the first recap" (Game Master only).

## Width (US4)

- `apps/web/src/styles/globals.css` (`@theme`): `--breakpoint-3xl: 120rem`,
  `--breakpoint-4xl: 160rem`, and a `--page-max: 150rem` token.
- `Container`, `AppHeader`, the footer, `WorldSectionShell`,
  `AdminSectionShell`, `ActorDetailPage` use `max-w-(--page-max)` and the
  same gutter. `Container narrow` keeps 780px.
- An `AutoGrid` utility class:
  `grid-template-columns: repeat(auto-fill, minmax(min(20rem, 100%), 1fr))`.
  Used by the players cards, the scenes list, compendium lists, and the
  overview's strips.
- Prose wrappers (`SessionNotesPanel` output, descriptions, lore body)
  get `max-w-[75ch]`.

## The character page (US5)

- `ActorDetailPage`'s `actor-view-grid` becomes
  `lg:grid-cols-[minmax(16rem,20rem)_minmax(0,1fr)]`: column A is the
  rail, the sheet and the panels under it are column B. The `2xl` third
  column goes; the sheet's own container decides its columns.
- `ActorImageryPanel` stacks portrait over token in the rail
  (`@container` with `@sm:grid-cols-2` instead of `sm:`).
- `packs/systems/dnd5e/web/src/ActorSheet.tsx`: the grid adds
  `@5xl:grid-cols-4`, and `SPAN_CLASS` gains the 4-column spans. Each
  region's wrapper becomes `@container`, so `AbilitiesSection`,
  `SkillsSection`, `SpellcastingSection`, `PersonaSection`,
  `ClassesSection` and `FeaturesSection` query their region.
- The other system sheets (Roll for Shoes, Genie, and the packs reached
  through `systemActorSheets.ts`) get the same region `@container`
  wrapper; their sections already use container queries or none.

## Demo notice (US6)

- `apps/demo/src/DemoNotice.tsx`: below `sm`, a `<button>` pill with
  `aria-expanded` that toggles the full notice. `useState`, not storage.

## Proof

- Slices: `pnpm e2e:worlds`, `pnpm e2e:scenes`, `pnpm e2e:actors`,
  `pnpm e2e:game-systems`, and the landing desktop project. The demo's
  own e2e (`apps/demo/e2e/gm-pages.spec.ts`) has its route headings
  updated for the overview.
- New specs:
  - `apps/web/e2e/world-front-door.spec.ts` (SC-002 to SC-004).
  - `apps/web/e2e/world-widths.spec.ts` (SC-005, SC-006), its own
    describe per page so each runs alone.
  - `apps/demo/e2e/demo-notice.spec.ts` (SC-007).
- Screenshots at 390, 1440 and 3440 before and after (SC-008).
