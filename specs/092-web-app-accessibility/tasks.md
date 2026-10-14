# Tasks: The Web App, Played Without a Pointer

**Input**: Design documents from `specs/092-web-app-accessibility/`
**Prerequisites**: spec.md, plan.md, discovery.md

**Tests**: The forwarder's guard (T011) and the Board list's pure helpers
(T021) have unit tests written and seen failing before the code. Every
e2e is written against the spec's scenarios and seen failing before the
change it proves; the page list's axe run (T040) is written first so that
what best-practice rules find is known before any page is edited.

**Proof**: The `accessibility` slice is the proof (T080). Its standalone
half is `pnpm -F @thunderforge/demo e2e a11y`; its integration half is
the `a11y-` specs. The neighbour slices named in SC-010 are run once each
(T081). The full suite is never used as a gate.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel with the other tasks marked [P] (a
  different file, and no unfinished dependency).
- **[Story]**: one of:
  - US1: pick a token and walk it, by keyboard;
  - US2: the dock's own keys stay its own;
  - US3: what changes is announced, and what fails is said;
  - US4: one main, one heading, a way past the header;
  - US5: forms, focus and targets;
  - US6: the demo, by keyboard and axe.

---

## Phase 1: Setup (the slice)

- [ ] T001 `scripts/e2e/slices.json`: add the `accessibility` entry from
  plan.md (own `a11y-`, standalone, nine neighbours, paths). Run
  `pnpm e2e:slices` and check it lists the slice with no own specs yet.
- [ ] T002 [P] Root `package.json`: `e2e:accessibility`,
  `e2e:accessibility:standalone`, `e2e:accessibility:integration` as
  plan.md gives them.
- [ ] T003 [P] `apps/web/e2e/fixtures/axe.ts`: an options argument
  `{ selector?, bestPractice? }` beside the existing positional selector,
  adding the `best-practice` tag when set. The 27 callers are untouched;
  `pnpm -F @thunderforge/web test` and one existing axe spec
  (`status-display.spec.ts`) still pass. (FR-035)
- [ ] T004 [P] `apps/web/e2e/fixtures/a11y.ts`: the page list (route,
  expected title fragment, needs-world, needs-admin), `landmarks(page)`
  returning counts of `main` and `h1`, `skipLink(page)` that presses Tab
  from the top and returns the focused element's text, and
  `focusedIsUnder(page, selector)` for the demo notice.

**Checkpoint**: `pnpm e2e:accessibility` runs (the demo half fails for
want of `a11y.spec.ts`; that is T070).

---

## Phase 2: Foundational — the forwarder yields (US2)

- [ ] T010 [US2] `apps/web/e2e/a11y-play-dock-keys.spec.ts`: scenarios
  US2.1–US2.4. A Game Master with one token selected (through the stack
  picker, the one DOM path today): ArrowRight on a focused dock tab moves
  focus to the next tab and the token's server position is unchanged;
  Space on the "Roll" button in the dice panel rolls; PageDown on the chat
  feed (given `tabindex=0`) scrolls it; an arrow with focus on the body
  still moves the token. Seen failing: the first three fail today.
- [ ] T011 [US2] `apps/web/src/engine/canvasKeyboard.test.ts`: cases for
  `yieldsKeys`: a `button`, an `a[href]`, a `[role=tab]`, a
  `[role=slider]`, an element with `tabindex=0` and `overflow: auto` and
  content taller than itself, a `div` with no tabindex (does not yield),
  the body (does not yield), the canvas (does not yield). Seen failing.
- [ ] T012 [US2] `apps/web/src/engine/canvasKeyboard.ts`: export
  `yieldsKeys(target)`; the forwarder returns early when it is true;
  `preventDefault` moves after `dispatchEvent`. Header comment says why.
  (FR-011, FR-012, FR-013)
- [ ] T013 [P] [US2] `apps/web/src/components/world/LevelTabs/LevelTabs.tsx`:
  roving `tabindex` with ArrowLeft/Right (and Home/End) on the `tablist`;
  the chosen tab is the focused one. (FR-014)
- [ ] T014 [P] [US2] `apps/web/src/components/canvas-tools/WallTool/WallTool.tsx`:
  the `radiogroup` roves with the arrows; the `title` text at line 131
  joins the control's `aria-label`. (FR-014, FR-030)
- [ ] T015 [P] [US2] `apps/web/src/components/world/RollAdvantage/AdvantagePicker.tsx`:
  roving arrows; each target at least 24×24 CSS px. (FR-014, FR-029)
- [ ] T016 [US2] Add US2.5 to `a11y-play-dock-keys.spec.ts`: the level
  tabs, the wall tool and the advantage picker move with the arrows and
  Tab leaves each group. Run the file; run `pnpm e2e:tokens` once (the
  forwarder is its file).

**Checkpoint**: `a11y-play-dock-keys.spec.ts` green;
`token-keyboard-move.spec.ts` and `combat-accessibility.spec.ts` green.

---

## Phase 3: User Story 1 — the Board list and the named canvas (US1)

- [ ] T020 [US1] `apps/web/e2e/a11y-board-keyboard.spec.ts`: scenarios
  US1.1–US1.7 against the fixtures `token-keyboard-move.spec.ts` uses
  (`registerAndCreateWorld`, `launchSceneByName`, `waitForEngineReady`,
  `serverTokenPosition`, `inviteAndJoinAsPlayer`): rows named with label
  and square; Enter selects and `selection_state()` agrees; each arrow
  moves one square on the server and on a second client; a wall refuses
  and is announced; a player's arrow on the Game Master's token is refused
  and announced; Shift+F10 on a row opens the context menu; Tab reaches the
  canvas, which has a name, a description and a focus ring, and ArrowRight
  there moves the selected token. Seen failing: no Board list exists.
- [ ] T021 [US1] `apps/web/src/components/world/PlayDock/__tests__/boardSquares.test.ts`:
  `squareOf({x, y}, gridSize)` gives 1-based column and row; a token
  between squares rounds to nearest; `announcementFor(label, result)` gives
  "{label}, column c, row r" on success and "{reason}; {label} stays at
  column c, row r" on refusal, with "refused" when no reason is carried.
  Seen failing.
- [ ] T022 [US1] `apps/web/src/components/world/PlayDock/boardSquares.ts`:
  the two helpers. Under 120 lines.
- [ ] T023 [US1] `apps/web/src/components/world/PlayDock/BoardPanel.tsx`:
  `BoardPanel({ store, selection, sceneId, gridSize })`. A heading
  "Tokens on the board", a `ul` of rows from `useSceneTokens`, each a
  `button` named by `squareOf`, `aria-pressed` from `useSelectedTokenIds`;
  Enter/Space → `selection.selectOne(id)`; an arrow on a selected row →
  `selection.moveBy({x, y})` with the result announced in a
  `aria-live="polite"` `p`, collapsing in-flight results to the latest;
  an arrow on an unselected row selects only; Shift+F10 / ContextMenu
  dispatch the same event the canvas's menu listens for; focus falls back
  to the heading when the focused row vanishes. No GraphQL import. Under
  300 lines. (FR-001–FR-007, FR-036)
- [ ] T024 [US1] `apps/web/src/components/world/PlayDock/WorldDock.tsx`:
  a `board` section first in the tab order of the dock's panels;
  `WorldPage.tsx` passes the selection facet, scene id and grid size.
  `play-panels-by-role.spec.ts` still passes (every role sees the Board
  panel; it moves only what the server allows).
- [ ] T025 [US1] `apps/web/src/pages/world/useEngineCanvas.ts`: in
  `applyCanvasVisibility`, set `tabIndex=0`, `role="application"`,
  `aria-label` ("Board: {scene name}") and `aria-describedby` on the
  canvas; `WorldPage.tsx` renders the visually hidden summary element
  (tokens in scene, selected, the keys) from the store. A focus ring on
  `canvas:focus-visible` in the play styles, with an `outline` for forced
  colours. (FR-008, FR-009)
- [ ] T026 [P] [US1] `apps/web/src/components/world/PlayDock/HelpPanel.tsx`:
  a "Keys" section: Tab to the Board list, Enter to select, arrows to
  move, O to open a door, Shift+F10 for the menu, Escape to clear.
- [ ] T027 [P] [US1] `docs/guides/playing-by-keyboard.md`: the same keys in
  the guides' voice, with the Board list and the canvas each explained in
  a paragraph; linked from `docs/guides/your-first-world.md` where it
  describes moving a token. (FR-010)
- [ ] T028 [US1] Add US1.8 to `a11y-board-keyboard.spec.ts`: the Help
  panel's key list and the guide name the same keys. Run the file; run
  `canvas-context-menu.spec.ts` and `token-stack-picker.spec.ts`.

**Checkpoint**: US1 scenarios green; `pnpm e2e:tokens` and
`pnpm e2e:canvas` green.

---

## Phase 4: User Story 3 — log and alerts (US3)

- [ ] T030 [US3] `apps/web/e2e/a11y-chat-and-forms.spec.ts`: US3.1–US3.4.
  A second client posts a message and it appears inside `[role=log]`; a
  sign-up with a taken email shows `[role=alert]`, the email field is
  `aria-invalid` and described by the alert, and is focused; the same for
  a wrong password on sign-in; axe with `bestPractice` on both forms.
  Seen failing.
- [ ] T031 [P] [US3] `apps/web/src/components/world/PlayDock/ChatPanel.tsx`:
  the feed `div` gets `role="log"`, `aria-live="polite"`,
  `aria-relevant="additions"`, `aria-label="Chat"`, `tabindex=0`.
  `chat-panel.spec.ts` still passes. (FR-015)
- [ ] T032 [P] [US3] `apps/web/src/components/ui/status-badge/StatusBadge.tsx`:
  a `role` prop passed through; `tone="error"` defaults it to `alert`.
  (FR-017)
- [ ] T033 [US3] `apps/web/src/pages/auth/RegisterPage.tsx` and
  `apps/web/src/pages/auth/LoginView.tsx`: errors through the badge as an
  alert with an id; the failing field `aria-invalid` and
  `aria-describedby`; focus to the first invalid field on submit.
  (FR-016)
- [ ] T034 [US3] Run the file; run `pnpm e2e:accounts` and
  `pnpm e2e:companion` once.

---

## Phase 5: User Story 4 — landmarks, skip link, titles, tables (US4)

- [ ] T040 [US4] `apps/web/e2e/a11y-pages.spec.ts`: for each page in the
  fixture's list — worlds list, create world, a world's dashboard,
  scenes, a scene's detail, players, compendium (each tab), a legal page,
  sign up, sign in, the Play view — count landmarks, read the title,
  press Tab and follow the skip link, and run axe with `bestPractice`.
  Seen failing; record in the ledger every rule id it reports per page,
  so the ledger says what the best-practice run found before any fix.
- [ ] T041 [US4] `apps/web/src/layouts/main-layout/MainLayout.tsx`: a
  "Skip to content" link first in the tree, visually hidden until focused,
  `href="#main"`; `id="main"` and `tabIndex=-1` on the layout's `main`.
  (FR-018)
- [ ] T042 [US4] Every page that renders its own `<main>` inside
  `MainLayout` (the five named in spec.md and the rest found by
  `grep -rln "<main" apps/web/src/pages`): `<main>` becomes `<div>`
  with the same classes. List every file in this task's ledger note.
  (FR-019)
- [ ] T043 [US4] `apps/web/src/layouts/world-layout/WorldLayout.tsx` and
  `WorldPage.tsx`: a `main` wrapping the board region and the dock, an
  `h1` naming the scene (visually hidden), and the skip link's target is
  the Board list's heading in Play. (FR-020)
- [ ] T044 [P] [US4] `SEO` on `pages/world/scenes/ScenesPage.tsx`,
  `pages/world/scenes/SceneDetailPage.tsx`, `pages/world/players/PlayersPage.tsx`,
  `pages/world/WorldCompendiumPage.tsx`, `pages/auth/SignUpPage.tsx`:
  "{Page} · {World} · ThunderForge" as the others do. (FR-021)
- [ ] T045 [P] [US4] `pages/world/compendium/{AbilityCompendiumTab,NpcCompendiumTab,ItemCompendiumTab}.tsx`:
  `caption` (or `aria-label` on the table), `scope="col"` on headers,
  `aria-selected` removed from `<tr>`, a button inside the first cell that
  opens the row; the `onClick` on the row stays. Each file under 1000
  lines after the change. (FR-022)
- [ ] T046 [US4] Fix whatever T040's best-practice run reported on the
  listed pages that the discovery did not name; for a finding in a file
  another slice owns and this spec does not list, record the rule id and
  the file in the ledger and leave it. Run the file; run
  `pnpm e2e:worlds` and `pnpm e2e:scenes` once.

---

## Phase 6: User Story 5 — controls (US5)

- [ ] T050 [US5] `apps/web/e2e/a11y-controls.spec.ts`: US5.1–US5.7. Open
  the dice roller, the ability and item effect editors, the scenes page,
  the lore tab, the book list, the invite link, the players page, the
  actor inventory and lore panels, the token panel, the wall tool, the
  advantage picker, the map credit; check accessible names, `:focus-visible`
  styles (and an outline under `emulateMedia({ forcedColors: "active" })`),
  new-tab names, no running animation on a dialog and a skeleton under
  `reducedMotion: "reduce"`, and axe's `color-contrast` and `target-size`
  rules. Seen failing.
- [ ] T051 [P] [US5] Labels: `components/world/DiceRollerPanel/DiceRollerPanel.tsx:94`,
  `pages/world/compendium/AbilityEffectEditor.tsx:252,259`,
  `pages/world/scenes/ScenesPage.tsx:187`,
  `pages/world/compendium/LoreCompendiumTab.tsx:189`,
  `pages/world/compendium/ItemEffectEditor.tsx:211,218`,
  `pages/world/compendium/BookListTab.tsx:884,891,898`,
  `components/world/SessionSetupInviteLink.tsx:63`,
  `pages/world/players/PlayersPage.tsx:458`: a `<label>` or `aria-label`
  each. (FR-023)
- [ ] T052 [P] [US5] Focus rings: `pages/world/actor/ActorInventoryPanel.tsx:290`,
  `pages/world/actor/ActorLorePanel.tsx:268`,
  `pages/world/compendium/ItemEffectEditor.tsx:135,173,202,230` stop
  removing the outline; `components/canvas-tools/TokenPanel/TokenPanel.scss:363-366`
  draws a visible ring; `styles/mixins.scss:3-9` adds
  `outline: 2px solid transparent` beside the shadow. (FR-024, FR-025)
- [ ] T053 [P] [US5] `components/ui/new-tab-link/NewTabLink.tsx`: an
  anchor with `target="_blank" rel="noopener noreferrer"` and a visually
  hidden " (opens in a new tab)"; used by `components/world/MapCredit.tsx:25,35,45`,
  `PlayDock/ActorsPanel.tsx:451,576`, `components/world/SystemLegalNotice.tsx:68`,
  `pages/feedback/MySubmissions.tsx:101`, `components/about/AboutInstance.tsx:43`,
  `pages/admin/TelemetryPanel.tsx:115`. (FR-026)
- [ ] T054 [P] [US5] `components/ui/dialog.tsx:62` and
  `components/ui/skeleton.tsx:7`: the animation classes under
  `motion-safe:` (or a `prefers-reduced-motion` query in the stylesheet).
  (FR-027)
- [ ] T055 [P] [US5] `components/canvas-tools/TokenPanel/TokenPanel.scss`:
  the `#a89968` on `#fffaee` pair replaced by one at or above 4.5:1
  (`#6f6340` on `#fffaee` is 6.1:1). (FR-028)
- [ ] T056 [US5] `components/ui/button.tsx:23-34`: where a 24px size is a
  control's only hit target, pad to 24px of spacing as 2.5.8 allows, or
  size it up; confirmed by T050's `target-size` run. (FR-029)
- [ ] T057 [US5] Run the file; run `pnpm e2e:combat` and `pnpm e2e:actors`
  once.

---

## Phase 7: User Story 6 — the demo (US6)

- [ ] T060 [US6] `apps/demo/e2e/a11y.spec.ts`, using `openSitting` and the
  `sitting` helpers: US6.1–US6.4. Tab to the Board list, select the
  fighter, press ArrowRight and check `cast()` against `FIGHTER_START`
  moved one square; focus each control on the page and assert its box
  does not intersect the notice; axe with WCAG and `bestPractice` tags
  on the board and each page (`demo-pages.spec.ts` lists the routes);
  the notice's three links say they open a new tab. Imports the web
  app's `fixtures/axe.ts` by workspace path. Seen failing.
- [ ] T061 [US6] `apps/demo/src/DemoNotice.tsx`: a `focusin` listener on
  the document translates the notice off the focused element's box when
  they intersect (and back on `focusout`); the links through
  `NewTabLink`; the notice no taller than a third of a 320px-wide
  viewport. The sitting specs (`demo-board`, `demo-pages`, `demo-limits`)
  still pass. (FR-031, FR-026)
- [ ] T062 [US6] Run `pnpm -F @thunderforge/demo e2e a11y`; then
  `pnpm e2e:accessibility:standalone`. (FR-032, FR-033)

---

## Phase 8: Polish and proof

- [ ] T070 [P] `pnpm verify` limited to the files this spec touched
  (`pnpm -F @thunderforge/web lint`, `typecheck`, `test`;
  `pnpm -F @thunderforge/demo lint`, `typecheck`); every touched file
  under 1000 lines (`wc -l` on the list in plan.md's tree). (SC-011)
- [ ] T071 [P] `discovery.md` section 4 ("Needs runtime check"): for each
  of the ten items, one line in this ledger saying which task settled it
  (1, 2, 5, 6, 7, 8, 9, 10) or that it stays open (3 theme contrast over
  bright maps; 4 the dock at 200% zoom and 320px), with the open ones
  carried to spec.md's Out of scope.
- [ ] T080 Proof: run `pnpm e2e:accessibility` alone with
  `--record-durations`, grep the log for `✘`, and record the result and
  the wall time here. (SC-010)
- [ ] T081 Proof: run `pnpm e2e:tokens`, `pnpm e2e:canvas`,
  `pnpm e2e:combat`, `pnpm e2e:companion`, `pnpm e2e:accounts`,
  `pnpm e2e:worlds`, `pnpm e2e:scenes` and `pnpm e2e:actors` once each,
  two at a time (the per-checkout stack allows two); record each result
  here. `pnpm e2e:which --diff` will say FULL SUITE for the three
  cross-cutting paths; by the owner's decision these slices are the
  proof and the full suite is not run. (SC-010)
- [ ] T082 Commit each phase as it lands (signed, through gitops), no
  session links; this ledger updated with results.

---

## Dependencies

- Phase 1 before everything (the slice must exist to run a phase's spec).
- Phase 2 before Phase 3: the Board rows handle arrows themselves, which
  only works once the forwarder yields to a focused button.
- Phase 3 before Phase 7: the demo's keyboard sitting uses the Board list.
- Phases 4, 5 and 6 are independent of each other and of Phase 3; T041
  and T043 touch the same layout tree and go in order.
- T040 before T042–T046: the best-practice run's findings are recorded
  before any page is edited.
- Phase 8 last.

## Ledger

(Results, wall times and the T042 file list are recorded here as tasks
close.)
