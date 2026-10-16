# Tasks: Smooth World Navigation

**Input**: [spec.md](./spec.md), [plan.md](./plan.md),
[research.md](./research.md), [data-model.md](./data-model.md),
[contracts/world-chrome.md](./contracts/world-chrome.md),
[quickstart.md](./quickstart.md)

**Tests**: requested by the spec (SC-001 to SC-006): vitest for the store,
one e2e spec for the walk, demo e2e. Write each test before the code it
proves and see it fail first.

Paths are relative to the repo root; `web/` means `apps/web/src/`.

## Phase 1: Setup

- [x] T001 Record the decision as ADR 116 in
  `docs/adrs/20261016-116-shared_world_state_is_a_context.md`: per-world
  stale-while-revalidate React context, no query library, revisit when
  the remembered kinds grow past about five more (FR-010, research R1).
- [x] T002 [P] Add the test hooks the e2e needs: a `data-testid="page-loader"`
  on `Loader` when `fullScreen` in `web/components/ui/loader/Loader.tsx`,
  and `data-testid="world-nav-admin"` on the GM group in
  `web/layouts/world-layout/WorldSidebarNav.tsx`.
- [x] T003 [P] Add `world-navigation.spec.ts` to the `worlds` slice's `own`,
  and `actor-claim.spec.ts`, `players-section.spec.ts`,
  `play-pause.spec.ts`, `gm-staging-page.spec.ts` to its `neighbours`, in
  `scripts/e2e/slices.json`; confirm with `pnpm e2e:which`.

## Phase 2: Foundational (the store)

**Blocks every story.**

- [x] T004 Write `web/hooks/world-chrome-store.test.ts` (vitest, node
  environment, plain functions as `useUnsavedChanges.test.ts` does) for
  `world-chrome-store.ts`: `patchChrome` returns the same object for an
  equal patch and replaces `members` whole; `joinRead` gives a second
  caller of one `worldId:key` the first caller's promise and forgets it
  once settled; `isRefusal` is true for a `GraphQLRequestError` with codes
  and false for one without and for a `TypeError`. See it fail.
- [x] T005 Create `web/hooks/world-chrome-store.ts` with `patchChrome`,
  `joinRead` and `isRefusal` (no React). Widen `WorldChrome` in
  `web/hooks/world-chrome-context.ts` with `world`, `members`,
  `claimCleared`, `playState` (data-model.md), and make `useWorldChrome`
  read through `joinRead`. T004 passes.
- [x] T006 In `web/hooks/WorldChromeProvider.tsx`: hold the in-flight map
  in a ref; apply patches with `patchChrome`; read `useAuth().user?.id` and
  start from an empty store (and an empty in-flight map) when it changes,
  so an answer started for the previous user is dropped. Proven in e2e by
  T025 (no DOM test environment in apps/web).
- [x] T007 Add `useKnownWorld(worldId)` to `web/hooks/world-chrome-context.ts`
  per contracts/world-chrome.md: `{ world, known }`; a `null` answer or a
  `GraphQLRequestError` with a non-empty `codes` writes `world: null`, any
  other rejection keeps the known world (research R3), decided by
  `isRefusal`.

**Checkpoint**: `vitest run src/hooks` green; nothing visible changed yet.

## Phase 3: US1 — Changing pages keeps the page on screen (P1) 🎯 MVP

**Goal**: no full-screen loader after a world's first page in a tab.
**Independent test**: SC-001 in `world-navigation.spec.ts`.

- [X] T008 [US1] Write `apps/web/e2e/world-navigation.spec.ts` "a Game
  Master moves between a world's pages without a loader": register and
  create a world (`registerAndCreateWorld`), open the overview, then click
  Players, Scenes, Compendium, System Settings, Overview in the sidebar;
  after each click assert `page-loader` never becomes visible (watch with
  a `MutationObserver` set up before the click, not a single poll) and the
  page's heading shows. See it fail.
- [x] T009 [P] [US1] `web/pages/world/players/PlayersRoutePage.tsx`: read
  `useKnownWorld(id)`; render the loader only when `!known`; drop its own
  `getWorld` effect and `useResetOnChange`.
- [x] T010 [P] [US1] Same change in `web/pages/world/scenes/ScenesRoutePage.tsx`.
- [x] T011 [P] [US1] Same change in `web/pages/world/WorldCompendiumRoutePage.tsx`.
- [x] T012 [P] [US1] Same change in `web/pages/world/scenes/SceneDetailRoutePage.tsx`
  (the scene's own read and its loader stay; only the world's moves).
- [x] T013 [US1] `web/pages/world/settings/WorldSystemSettingsPage.tsx`:
  world from `useKnownWorld`; every `setWorld(updated)` after a save
  (`updateWorldGameSystem`, `updateWorldDefaultSceneGridType`, the
  appearance card's callback) becomes `remember(worldId, { world: updated })`
  (FR-004).
- [x] T014 [US1] `web/pages/world/overview/WorldOverviewPage.tsx`: world
  from `useKnownWorld`; keep `status` for its own errors; `setNotes`
  remembers the edited world; the loader condition uses `!known` (role and
  claim parts follow in US2 and US3).
- [x] T015 [US1] `web/appearance/WorldAppearance.tsx`: read
  `interfacePackId` from the known world when present, so the shell's
  appearance and the page share one `getWorld` (keep the appearance event
  write).
- [x] T016 [US1] Add to `world-navigation.spec.ts`: the GM renames the
  world on one page and the next page shows the new name; a world deleted
  in another context shows the not-found state on the next page, not the
  remembered one. T008 and these pass.
  *Done as:* there is no rename in the app or the API, so the "changed
  elsewhere" case changes the session notes through GraphQL and the
  overview shows them; a grid-type save on settings survives a round trip
  through Scenes (FR-004).

**Checkpoint**: US1 stands alone; ship-able with the spinner gone.

## Phase 4: US2 — A Game Master's controls are there from the start (P1)

**Goal**: GM-only nav and controls on each page's first paint.
**Independent test**: SC-002.

- [X] T017 [US2] Add to `world-navigation.spec.ts`: on the GM's walk,
  `world-nav-admin` is present at the first paint of each page after the
  first (assert inside the same `MutationObserver` window as T008); a
  player (`inviteAndJoinAsPlayer`, with a claimed character) walks the
  same pages and `world-nav-admin` is never attached. See the GM half fail.
- [X] T018 [US2] `web/hooks/useWorldMembers.ts`: seed `members` from
  `useWorldChrome(worldId, "members", …)`; `loading` is true only when
  none are known; every fetch and `refetch` writes the store; an error
  keeps known members instead of clearing them.
- [X] T019 [US2] `web/hooks/useWorldRole.ts`: no API change; confirm
  `loading` follows T018 and a player's role never reads as GM while a
  read is in flight (FR-006): extract the role decision `useWorldRole`
  already makes (members, user, world → role) as a pure function if it is
  not one, and cover "no members known, not the creator → not GM" in
  `web/hooks/useWorldRole.test.ts`.
- [X] T020 [US2] Role changes on the Players page
  (`web/pages/world/players/PlayersPage.tsx`) call the members `refetch`
  or remember the new list, so the next page shows the new role
  (US2.3). Add the case to `world-navigation.spec.ts`. T017 passes.

## Phase 5: US3 — The overview does not re-check the character (P2)

**Goal**: a cleared gate is remembered; a lost claim still redirects.
**Independent test**: SC-003.

- [X] T021 [US3] Add to `world-navigation.spec.ts`: a player with a
  character returns to the overview with no `page-loader`; then the GM
  releases the claim (`unclaimActor` via `graphql`), the player opens
  Players and then Overview and lands on `/world/:id/actor-select`. See
  the first half fail.
- [X] T022 [US3] `web/hooks/useActorClaimGate.ts`: initial `cleared` from
  `claimCleared`; write `true` when the check clears, `false` before
  redirecting; the re-check runs on every mount; errors keep the existing
  fail-open (research R5). The overview's loader stops waiting on a
  remembered clearance. T021 and `actor-claim.spec.ts` pass.

## Phase 6: US4 — A paused world says so on the first frame (P3)

**Goal**: the paused banner is there on each page's first paint.
**Independent test**: US4 acceptance.

- [X] T023 [US4] Add to `world-navigation.spec.ts`: the GM pauses play
  (as `play-pause.spec.ts` does); the player moves between two pages and
  the paused banner is present at first paint of the second; the pause is
  lifted and the banner goes after the focus refresh.
- [X] T024 [US4] `web/hooks/useWorldPlayState.ts`: read and write
  `playState` through the store; the focus refresh writes it too. T023
  and `play-pause.spec.ts` pass.

## Phase 7: Polish and proof

- [X] T025 Add to `world-navigation.spec.ts` the account switch (SC-004):
  user A loads a paused world's pages, signs out, user B signs in in the
  same tab and opens their own world; none of A's world name, admin nav or
  banner appears at any point.
- [X] T026 [P] Update the doc comments that describe the old behaviour:
  `web/layouts/world-layout/WorldSectionShell.tsx`,
  `web/hooks/useWorldMembers.ts`, `web/hooks/useActorClaimGate.ts`.
- [X] T027 Checks: `tsc -b --noEmit`, eslint, prettier on touched
  `apps/web` and `apps/demo` files, `vitest run src/hooks src/pages/world`,
  and `tsc -b --noEmit` in `apps/demo`.
- [X] T028 Proof: `THUNDERFORGE_DISABLE_AUTH_RATE_LIMIT=1 pnpm e2e:worlds`
  green; record the counts here. Slices `actors` and `scenes` green.
  2026-10-16: worlds 61 passed, actors 33, scenes 21; 0 failed, 0 flaky.
- [X] T029 Demo: `pnpm run e2e e2e/demo-pages.spec.ts e2e/demo-board.spec.ts`
  in `apps/demo` (SC-005), then a plain `pnpm run build` for the phone
  preview. 2026-10-16: 20 passed.
- [ ] T030 `make ci` passes. Owner's phone check (SC-007); no push or ship
  before the greenlight. 2026-10-16: `make ci` passed; phone check open.

## Dependencies

- Phase 1 → Phase 2 → stories. T002 and T003 run in parallel.
- US1 (Phase 3) first: US2–US4 add to its e2e file and its overview edit.
- US2, US3, US4 touch different hooks and may proceed in any order after
  US1; their e2e additions go into the one spec file, so write them one at
  a time.
- Phase 7 after the stories it covers; T025 needs US2 and US4.

## Parallel examples

- Phase 1: T002 and T003.
- US1: T009, T010, T011, T012 (one route page each) after T007.
- After US1: T018 (members), T022 (claim gate), T024 (play state) are in
  separate files, so the code can proceed in parallel; their tests are
  added in sequence.

## Implementation strategy

MVP is US1: the full-screen loader goes, which is the jolt the owner sees.
Commit after each phase (signed). Prove each story with
`--only=world-navigation.spec.ts` plus its neighbour while working, and
the whole `worlds` slice at T028. Ship only after the owner's greenlight.
