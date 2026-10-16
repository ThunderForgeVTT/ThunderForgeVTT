# Tasks: A Board That Fits a Phone

**Input**: [spec.md](./spec.md), [plan.md](./plan.md),
[research.md](./research.md), [data-model.md](./data-model.md),
[contracts/phone-board-ui.md](./contracts/phone-board-ui.md),
[quickstart.md](./quickstart.md)

**Tests**: requested by the spec (SC-001 to SC-008 are measured at a phone
viewport by Playwright; SC-009 is the owner's phone). Vitest for the pure
row builder and the layout hook; one demo e2e spec (standalone half) and two
app e2e specs (integration half). Write each test before the code it proves
and see it fail first.

**Design**: every UI task follows impeccable (Operate mode). Read
`~/.claude/skills/impeccable/reference/craft-floor.md` before the first UI
edit of a session, and run
`~/.claude/skills/impeccable/scripts/impeccable detect --json <files>` on the
changed components after each phase. The direction is research.md's design
brief; do not invent buttons beyond it.

Paths are relative to the repo root; `web/` means `apps/web/src/`, `pb/`
means `apps/web/src/components/world/PhoneBoard/`.

## Phase 1: Setup

- [x] T001 Add the `phone` slice to `scripts/e2e/slices.json`: `own`
  `phone-board.spec.ts`, `phone-app.spec.ts`; `standalone` running the demo
  `phone-board` spec; `neighbours` with seams: `play-panels-by-role.spec.ts`
  (dock sections by role), `dice-roll.spec.ts` (dice roller moves into a
  sheet), `board-loading.spec.ts` (overlays), the `play-pause` specs
  (banners), `world-navigation.spec.ts` (page shell); `paths` as plan.md's
  source tree. Add `e2e:phone`, `e2e:phone:standalone`
  (`pnpm -F @thunderforge/demo e2e phone-board`) and `e2e:phone:integration`
  (`node ./scripts/e2e-parallel.mjs --shards=1 --slice=phone`) to the root
  `package.json`, matching the `e2e:worlds` pattern. Confirm with
  `pnpm e2e:which`. The pre-commit hook refuses entries that match no
  tracked file, so the slice lands with the first spec file and grows its
  `own` and `paths` as each file is created.
  *Done note*: `own` names apps/web specs only, so it stays empty until T020's web spec exists and `e2e:phone` runs the demo standalone half (as `landing` does); `e2e:phone:integration` joins with T020. `paths` list only files that exist (the pre-commit hook requires it); `phone-app.spec.ts`, the manifest, icons, `brand-icons.mjs` and `useActiveCombat.ts` join with T018 and US5.
- [x] T002 [P] Viewport meta in `apps/web/index.html` and
  `apps/demo/index.html`: `width=device-width, initial-scale=1,
  viewport-fit=cover, interactive-widget=resizes-content` (R5).
- [x] T003 [P] In `web/layouts/world-layout/WorldLayout.tsx`, `h-screen` →
  `h-dvh` (R5). Desktop renders the same.

## Phase 2: Foundational

**Blocks every story.**

- [x] T004 Write `web/hooks/usePhoneLayout.test.ts` (vitest, jsdom, a
  stubbed `matchMedia` whose `change` listeners the test fires): false at
  1280×800 and 820×1180, true at 412×915 and 915×412, flips when the query
  changes. See it fail.
- [x] T005 Create `web/hooks/usePhoneLayout.ts`: `useSyncExternalStore` over
  `matchMedia("(max-width: 599px), (max-height: 599px)")`, server snapshot
  `false` (R1). T004 passes.
- [x] T006 Write `pb/__tests__/phoneRows.test.ts` for
  `phoneRows(isGm, permittedTools, sections)` per data-model.md: player row
  ids `me, dice, sheet, chat, more`; GM row `tools, dice, actors, chat,
  combat`; GM groups `board` (permitted tools in `GM_TOOL_IDS` order, then
  `levels`, `token-tray`) and `table` (requests, settings, the system's own
  section, help, `fullscreen`); player `more` holds the remaining permitted
  sections, granted tools and `fullscreen`; a section or tool absent from
  the input never appears (FR-006); Trusted Player input gives the player
  rows. See it fail.
- [x] T007 Create `pb/phoneRows.ts` with the `RowItem`/group types from
  data-model.md, labels and `FantasyIcon` names reused from `GmToolRail.tsx`
  and `WorldDock.tsx` (export their maps if they are module-private). T006
  passes.
- [x] T008 Create `pb/BoardSheet.tsx` (R4): radix-ui `Dialog` with
  `modal={false}`, bottom-anchored, `data-testid="board-sheet"`,
  `data-size` `half` (max-height 60dvh) | `full`, `data-content`; a grab
  handle (pointer drag up past 48px → full, down → half/close) and
  `board-sheet-expand` / `board-sheet-close` buttons, each ≥ 44×44;
  `history.pushState` on open and close on `popstate` (popping its own entry
  when closed by a button); focus in on open, back to the opening tile on
  close; bottom padding `env(safe-area-inset-bottom)`; 180ms slide,
  none under `prefers-reduced-motion`. Background `bg-background/95` and
  `border-border` as the dock's panel.
  *Done note*: the sheet floats above the row (row stays visible and tappable) rather than sitting flush at the bottom edge.
- [x] T009 Create `pb/PhoneBoard.tsx` (default export, for `React.lazy`)
  taking the page's `tools`, `openToolId`, `onOpenToolChange`, `sections`,
  `openSectionId`, `onOpenSectionIdChange`, `isGm`, `worldId`, and render
  slots for the dice panel and top notices; holds `toolOptionsOpen` and the
  one open sheet (`content`, `size`) per data-model.md; renders
  `phone-board` (`data-role`), `phone-top-stack`, the row and the sheet.
- [x] T010 In `web/pages/world/WorldPage.tsx`, read `usePhoneLayout()`; when
  true render `<Suspense><PhoneBoard …/></Suspense>` (lazy import) in place
  of the `toolRail` and `dock` slots, passing the same state (R2). Move the
  companion notice, revoked-tool notice, `PlayPausedBanner`,
  `ConnectionStatus` and the selection bar into `phone-top-stack` when
  phone; desktop render paths unchanged.

**Checkpoint**: at 412×915 the rails and dock are gone and an (empty)
`phone-board` renders; at 1280×800 nothing changed.

## Phase 3: User Story 1 — A player plays from their phone (P1) 🎯 MVP

**Goal**: player sees the map edge to edge with Me, Dice, Sheet, Chat, More.
**Independent test**: demo, player view, 412×915: five tiles, map share
≥ 85%, each of the four jobs one tap away.

  *Done note*: the companion and revoked notices and the selection bar move into the top stack; `PlayPausedBanner` and `ConnectionStatus` still render from `WorldBoardOverlays` (revisit after T017).
- [x] T011 [P] [US1] Write `apps/demo/e2e/phone-board.spec.ts` (player
  part): `test.use({ viewport: { width: 412, height: 915 }, hasTouch: true,
  isMobile: true })`; switch the demo to the player view; assert
  `phone-row` buttons are exactly Me, Dice, Sheet, Chat, More; no
  `gm-tool-*` or `world-dock-tab-*` visible; map share ≥ 0.85 per the
  contract formula; Me centres and selects the player's token
  (selection count 1); Dice, Sheet and Chat each open `board-sheet` with
  their content. See it fail.
  *Done note*: `openDemo` makes its own 1440×900 context, so the spec calls `page.setViewportSize` instead of `test.use`; Me is asserted as "no note, no chooser" (the demo player's token is on the scene). Also asserts ≥44×44 and hit-testing for every visible button, and Back closing the sheet.
- [x] T012 [P] [US1] Create `pb/BottomRow.tsx`: `nav` "Board",
  `data-testid="phone-row"`, five tiles 56×52 (icon over 11px label),
  rounded-2xl, each `bg-background/85 backdrop-blur border border-border
  shadow`, gap 8px, centred, `bottom: max(12px,
  env(safe-area-inset-bottom))`; open tile `aria-expanded` and the primary
  colour; badge slot. Landscape (shorter side is height): a right-edge
  column, same tiles.
- [x] T013 [US1] Me (R7) in `pb/PhoneBoard.tsx`: claimed actor from
  `useScenePlay` → its token on the scene (`useSceneTokens`, by `actorId`)
  → `selection.selectOne` + `lookAtToken`; several → a chooser sheet; none
  on this scene → open Sheet with "Your character isn't on this scene".
- [x] T014 [US1] Sheet: render `InPaneCharacterSheet` for the claimed actor
  inside `BoardSheet`; no claimed actor → the Actors section's claim prompt.
  *Done note*: inside the phone sheet `InPaneCharacterSheet` gets no `onDismiss`, so its 26px "‹ Back" is not drawn (the sheet's own close and Back do that job). No claimed actor shows a note pointing to More → Actors.
- [x] T015 [US1] Dice: render the existing `DiceRollerPanel` content inside
  `BoardSheet` when phone (`web/components/world/DiceRollerPanel/`), not the
  floating bottom-left tray; the Tokens toggle moves into More/Tools (R8).
  *Done note*: the GM's token tray is in the Tools sheet's board group; the player has no token tray item (open question for T017).
- [x] T016 [US1] Create `pb/MoreSheet.tsx`: the player's `more` group from
  `phoneRows` as `phone-item-<id>` buttons (≥ 44 tall rows with icon and
  label), each opening its section or arming its tool.
- [ ] T017 [US1] Try-out: build the demo with `ENGINE_PROFILE=release`,
  restart the preview, send the tunnel URL to the owner's phone; capture
  `adb exec-out screencap` screenshots. **Stop here until the owner has
  reacted**; fold changes into research.md's brief before Phase 4.
- [ ] T018 [P] [US1] Create `web/hooks/useActiveCombat.ts` lifted from
  `PlayDock/CombatPanel.tsx` (`getActiveCombat` + `subscribeToWorldEvents`)
  and make `CombatPanel` use it; `play-panels-by-role.spec.ts` still passes.
- [ ] T019 [US1] Create `pb/TurnLine.tsx`: `turn-line` `status` "Round N ·
  <name>'s turn" above the row while combat runs (FR-005); tap opens
  Combat. Extend T011 with a combat turn where the demo can start one.
- [ ] T020 [US1] Write the player half of `apps/web/e2e/phone-board.spec.ts`
  (real player account, claimed actor, combat started by the GM fixture):
  rows, Me, turn line, no editing controls without grants (FR-006).

**Checkpoint**: T011 and T020 pass; owner has seen the try-out.

## Phase 4: User Story 2 — A Game Master runs the table from a phone (P1)

**Goal**: same row with Tools; every tool reachable in two groups.
**Independent test**: GM at 412×915: every `GM_TOOL_IDS` tool and every
dock section reachable within two taps; map share ≥ 80%.

- [ ] T021 [P] [US2] Extend `apps/demo/e2e/phone-board.spec.ts` (GM part):
  row is Tools, Dice, Actors, Chat, Combat; Tools sheet has
  `phone-group-board` and `phone-group-table` holding every tool and
  section the desktop rail and dock show; arming Walls closes to a
  `tool-chip` (`data-tool=walls`); `tool-chip-leave` returns to Select;
  map share ≥ 0.80. See it fail.
- [ ] T022 [P] [US2] Create `pb/ToolsSheet.tsx`: two `group`s with headings
  "Board" and "Table", items from `phoneRows`; picking a tool calls
  `onOpenToolChange(id)` and sets `toolOptionsOpen` true (R3); picking a
  section opens it.
- [ ] T023 [P] [US2] Create `pb/ToolChip.tsx`: top-left chip "<Tool>" with
  options (`tool-chip`) and ✕ (`tool-chip-leave`), each ≥ 44×44, in
  `phone-top-stack`; options render the tool's existing flyout content
  (the `GmToolRail` flyout body, factored out if inline) in `BoardSheet`.
- [ ] T024 [US2] Requests badge (FR-011): count from the requests section's
  existing source onto the Tools tile (`phone-row-tools-badge`); arriving
  requests never open a sheet (FR-013).
- [ ] T025 [US2] Levels: `level-chip` in `phone-top-stack` when the scene has
  > 1 level, opening `LevelTabs` content in a sheet; level management under
  Tools → Board.
- [ ] T026 [US2] Write the GM half of `apps/web/e2e/phone-board.spec.ts`:
  every tool and section reachable, a granted tool appears for a player
  only after the grant, a request badges Tools.

**Checkpoint**: T021 and T026 pass.

## Phase 5: User Story 3 — Nothing covers anything else (P2)

**Goal**: no overlaps, 44px targets, Select starts closed, demo notice usable.
**Independent test**: overlap scan + size check pass for both roles.

- [ ] T027 [P] [US3] Add to both phone specs a shared helper
  `apps/web/e2e/helpers/phoneChecks.ts` (imported by the demo spec as the
  demo already imports web helpers, or duplicated if it does not):
  every visible button in `phone-board`, `phone-top-stack` and the demo
  notice is ≥ 44×44 and `elementFromPoint` at its centre is itself or
  inside it; run with nothing open, a sheet half, a tool armed, combat on,
  paused. See it fail.
- [ ] T028 [US3] FR-016: on phone, Select is armed with `toolOptionsOpen`
  false on load, so the filter does not open (R3). Limit
  `GmToolRail/SelectionFilterMenu.tsx` kinds to permitted tools (FR-006).
- [ ] T029 [US3] `apps/demo/src/DemoNotice.tsx`: on phone, a compact chip
  in the top-right of `phone-top-stack` (portal target or a phone prop)
  whose view switch stays tappable; desktop unchanged
  (`demo-notice.spec.ts` passes).
- [ ] T030 [US3] Table feed (MapCredit, OfferPrompt, AttackLog) sits above
  the row on phone; `PlacedItemPrompt` and `SelectionBar` checked against
  the scan; fix whatever T027 reports.

## Phase 6: User Story 4 — Panels leave the table in view (P2)

**Independent test**: each sheet opens half (≤ 60% height), expands, closes
by swipe and back, and chat stays usable with the keyboard up.

- [ ] T031 [P] [US4] Extend `apps/demo/e2e/phone-board.spec.ts`: every row
  sheet opens `data-size=half` with height ≤ 0.6 × viewport; expand →
  `full`; `page.goBack()` closes it and stays on the board; opening another
  replaces it; rotate to 915×412 with a sheet open keeps content and size
  (FR-003); focus returns to the tile on close. See it fail.
- [ ] T032 [US4] Fix what T031 reports in `pb/BoardSheet.tsx`; check chat's
  input stays above the keyboard on the owner's phone (R5, fallback
  `visualViewport` only if Firefox fails).

## Phase 7: User Story 6 — The board as an app on the phone (P2)

**Independent test**: manifest served and valid; full-screen control
toggles; nothing cached.

- [ ] T033 [P] [US6] Write `apps/web/e2e/phone-app.spec.ts`:
  `/manifest.webmanifest` returns 200 with `application/manifest+json`
  and the contract's fields; each icon URL returns an image of its size;
  `index.html` links it; `sw.js` registers no fetch handler and
  `caches.keys()` is empty after a board visit (FR-027); in the phone
  layout `fullscreen-toggle` is in More/Tools and its label flips on
  `fullscreenchange`. See it fail.
- [ ] T034 [P] [US6] Create `scripts/brand-icons.mjs` (Playwright renders
  `apps/web/public/brand-mark.svg` on `#0a1018` at 192, 512 and a 512
  maskable with 20% safe padding) and commit
  `apps/web/public/icons/{icon-192,icon-512,maskable-512}.png`.
- [ ] T035 [US6] Create `apps/web/public/manifest.webmanifest` per the
  contract; link it and `apple-touch-icon` from `apps/web/index.html`.
  Not in `apps/demo/index.html` (FR-029). Verify the Rust
  `static_files` `ServeDir` sends `application/manifest+json`
  (`crates/thunderforge-server/src/static_files/mod.rs`); add an explicit
  content type only if it does not.
- [ ] T036 [P] [US6] Create `pb/FullScreenButton.tsx` (R10): shown when
  `document.fullscreenEnabled` and not `display-mode: fullscreen`;
  `requestFullscreen`/`exitFullscreen`; label from `fullscreenchange`.
  Placed in `phoneRows`' `fullscreen` item.

## Phase 8: User Story 5 — Desktop and tablet unchanged (P3)

- [ ] T037 [US5] Add to `apps/demo/e2e/phone-board.spec.ts`: no `phone-*`
  test id renders at 1280×800 or 820×1180; resizing 1280→412→1280 keeps
  the open dock section / armed tool (FR-003).
- [ ] T038 [US5] Run the neighbours at desktop size:
  `pnpm e2e:which --diff`, then each named spec with `--only`, plus the
  demo's `board-panels` and `demo-notice`. All pass unchanged.

## Phase 9: Polish & cross-cutting

- [ ] T039 Telemetry (Principle VII): add `layout` (`phone`|`wide`) and
  `display` (`browser`|`standalone`|`fullscreen`) labels to the existing
  board-load timing and to the allow-list in `packages/telemetry/src`.
- [ ] T040 WCAG pass (FR-030): 200% text at 412 wide (row wraps to labels
  hidden visually but kept as accessible names only if it overflows),
  focus visible on every tile, sheet named, contrast of tile text and icon
  ≥ 4.5:1 / 3:1 on a white and a black map (FR-002a).
- [ ] T041 Run impeccable `detect --json` on `pb/` and the edited files;
  fix findings. `pnpm -F @thunderforge/web check`, `pnpm -F
  @thunderforge/web test`, both builds.
- [ ] T042 Run `pnpm e2e:phone` (standalone + integration) and record the
  measured map shares in this file.
- [ ] T043 Owner's phone session per quickstart.md (GM, player, rotation,
  keyboard, full screen, installed app) — SC-009. **No push, no ship until
  the owner greenlights.**

## Dependencies

- Phase 1 → Phase 2 → stories. T017 (try-out) gates Phase 4 onward.
- US1 and US2 share `PhoneBoard`/`BottomRow`; do US1 first.
- US3, US4 build on US1+US2 surfaces; US6 is independent after Phase 2
  (T033–T036 can run alongside US3/US4). US5 is last before polish.

## Parallel opportunities

- T002, T003; T004/T006 (different test files).
- US1: T011, T012, T018 together.
- US2: T021, T022, T023 together.
- US6: T033, T034, T036 together.

## Implementation strategy

MVP is US1 in the demo, shown to the owner through the tunnel (T017). Then
US2, US3, US4, US6, US5, polish, and the owner's phone session. Commit
signed after each task or checkpoint, explicit paths only.
