# Implementation Plan: The Web App, Played Without a Pointer

**Branch**: `092-web-app-accessibility` | **Date**: 2026-10-13 | **Spec**: [spec.md](spec.md)

## Summary

Six slices of work, in this order, each proven by its own `a11y-` spec:

1. **The forwarder yields** (US2). `engine/canvasKeyboard.ts` gains a
   `yieldsKeys(target)` guard and forwards only when the target is the
   body, the canvas, or neither interactive nor scrollable; `preventDefault`
   only after forwarding. Unit tests first. This goes first because US1's
   rows handle the arrows themselves and must not be double-served.
2. **The Board list and the named canvas** (US1). A new
   `PlayDock/BoardPanel.tsx` lists the scene's tokens from the store,
   selects through `facets.selection.selectOne`, moves through
   `facets.selection.moveBy`, and announces results in a live region;
   `useEngineCanvas.ts` gives the canvas a role, name, description and
   `tabindex`; `HelpPanel.tsx` and a new guide list the keys.
3. **Log and alerts** (US3). `ChatPanel.tsx` becomes a `role="log"`;
   `StatusBadge` learns `role`; the two auth pages mark fields invalid and
   move focus.
4. **Landmarks, skip link, titles, tables** (US4). `MainLayout` gets the
   skip link; the pages' own `<main>` become `div`; the Play view gets a
   `main` and an `h1`; five pages get `SEO`; three compendium tables get
   captions, `scope` and in-row controls.
5. **Controls** (US5). Labels, focus rings (including forced colours),
   new-tab links, reduced motion, the contrast pair, the target sizes, the
   wall tool's title.
6. **The demo** (US6). The notice yields to focus; the demo gets an axe
   check and a keyboard sitting as the slice's standalone half.

The `accessibility` slice is declared first (slice entry and root scripts),
so each phase's spec can be run with `pnpm e2e:accessibility` as it lands.

## Technical Context

**Language/Version**: TypeScript 5 / React 19 / Vite (apps/web, apps/demo);
Playwright 1.62 with `@axe-core/playwright` 4.13.0 (already in apps/web).
No Rust changes.
**Primary Dependencies**: the world store and its selection facet
(`apps/web/src/engine/world/{store.ts,facets/selection.ts}`), Radix
primitives under `components/ui/**`, the SEO component, the e2e fixtures.
**Storage**: none new. No schema change.
**Testing**: Vitest for `canvasKeyboard.ts` and `BoardPanel`'s pure helpers
(`squareOf`, `announcementFor`); Playwright for everything the user sees;
axe through the existing fixture, extended with an opt-in best-practice tag.
**Target Platform**: Chromium (constitution); `page.emulateMedia` for
`forced-colors` and `prefers-reduced-motion`.
**Project Type**: web app + static demo, pnpm workspace.
**Performance Goals**: the Board list re-renders only when a token a panel
reads changes (`useSceneTokens` already guarantees this); the live region
collapses in-flight results so a held key produces one announcement per
settled result, not one per frame.
**Constraints**: no new dependency; no engine semantic change; every file
under 1000 lines; components dispatch to the store, never the GraphQL
client; the engine is the source of truth for selection and position.
**Scale/Scope**: ~30 source files touched, 6 new e2e specs, 1 slice entry,
1 guide.

## Constitution and AGENTS.md check

- **I. ECS owns simulation, React owns chrome.** Selection and position
  stay in the engine. The Board list reads `selectedTokenIds` and token
  positions from the store the engine writes, and asks for changes
  through the selection facet (`select_token`, `moveBy`). It never writes
  a position or a selection of its own. The canvas's name and description
  are chrome around the board, not state in it. Pass.
- **II. Plugin-modular engine.** No engine change. Pass.
- **III. Authorisation at the data boundary.** A keyboard move is the same
  move intent as a drag; the server refuses what the user may not move,
  and the refusal is what gets announced (FR-005, SC-003). Pass.
- **IV. ADRs and specs first.** This plan. No ADR: no architectural
  decision is new; the Board list follows ActorsPanel's existing pattern.
- **V. Verify before done.** Each phase ends by running its spec; the last
  task runs the slice alone and records the wall time.
- **VI. Every feature is proven by its own slice.** Slice: `accessibility`.
  Own: the `a11y-` prefix in `apps/web/e2e`. Standalone half:
  `pnpm -F @thunderforge/demo e2e a11y`. Neighbours (spec → seam):
  `token-keyboard-move.spec.ts` (the forwarder still feeds the engine when
  focus is on the body), `token-stack-picker.spec.ts` (selecting one of a
  stack from the DOM), `canvas-context-menu.spec.ts` (the menu opens on a
  keyboard-made selection), `combat-accessibility.spec.ts` (the dock's
  keyboard paths keep working under the new forwarder),
  `chat-panel.spec.ts` (the feed's role change), `play-panels-by-role.spec.ts`
  (the dock gains a panel), `scene-management.spec.ts` (the level tabs'
  arrow keys), `players-section.spec.ts` (the role select's label),
  `worlds-list.spec.ts` (the skip link and single `main`). Paths: the files
  named under Project Structure. `pnpm e2e:which --diff` will say FULL
  SUITE for `components/ui/**`, `styles/**` and `layouts/main-layout/**`;
  by the owner's standing decision the proof is this slice plus the
  neighbour slices `tokens`, `canvas`, `combat`, `companion`, `accounts`,
  `worlds`, `scenes` and `actors`, each run once at the end (SC-010).
- **VII. Telemetry on, anonymous.** Nothing here emits telemetry. Pass.
- **AGENTS.md: React isolated from the network.** `BoardPanel` takes the
  store and the facet, never the GraphQL client. Optimistic state: none —
  the list shows the store's position, and the store already applies the
  engine's optimistic move and the server's rollback; the live region reads
  the intent result. Pass.

## Decisions

- **Why a list, not focusable tokens on the canvas.** winit owns the
  canvas; there is no DOM per token to focus, and painting an overlay of
  invisible buttons over the board would have to track the camera every
  frame. A list in the dock costs nothing per frame, is where a reader
  looks, and ActorsPanel already proves the pattern.
- **Why the rows move through `moveBy`, not by forwarding arrows to the
  engine.** The engine's arrow handler moves the selection it holds, which
  is right once a token is selected. But the announcement needs the
  result, and the only place the result comes back to React is the intent
  result `moveBy` returns. The canvas, when focused, still uses the
  engine's own handler (FR-008); both paths end in the same adjudication.
- **Why the forwarder tests the target rather than a list of safe
  containers.** A list of containers (the dock, dialogs) would have to be
  extended every time a panel is added. "Interactive or scrollable keeps
  its keys" is a rule, not a list, and it is what the browser itself does.
- **Why `main` stays in the layout, not the page.** One mechanism applied
  everywhere beats twenty-five judgement calls; the layout already renders
  it, and the skip link needs one target it can name.
- **Why the demo imports the web app's axe fixture.** The fixture is the
  rule (tags, scope, the message format); copying it would fork the rule.
  The import is a workspace-relative path in a test file, not a package
  dependency.
- **Best-practice rules only on named pages.** The existing fixture's
  callers deliberately exclude advice; turning it on globally would fail
  27 specs on findings that are not theirs. The new option is opt-in.

## Project Structure

### Documentation (this feature)

```text
specs/092-web-app-accessibility/
├── spec.md
├── plan.md
├── tasks.md
└── discovery.md          # the repository-wide report, as delivered
```

### Source Code (files touched; new files marked `+`; line budget in brackets)

```text
apps/web/src/
├── engine/
│   ├── canvasKeyboard.ts                        # yieldsKeys guard; preventDefault after forward [<200]
│   ├── canvasKeyboard.test.ts                   # + guard cases [<200]
│   └── world/
│       └── useSceneTokens.ts                    # unchanged; read by BoardPanel
├── components/
│   ├── world/
│   │   ├── PlayDock/
│   │   │   ├── BoardPanel.tsx                   # + the Board list, rows, live region [<300]
│   │   │   ├── boardSquares.ts                  # + squareOf(token, gridSize), announcementFor(result) [<120]
│   │   │   ├── __tests__/boardSquares.test.ts   # + [<150]
│   │   │   ├── WorldDock.tsx                    # a "board" section
│   │   │   ├── HelpPanel.tsx                    # the keys
│   │   │   └── ChatPanel.tsx                    # role=log, aria-live
│   │   ├── LevelTabs/LevelTabs.tsx              # roving tabindex
│   │   ├── RollAdvantage/AdvantagePicker.tsx    # roving tabindex; 24px targets
│   │   ├── MapCredit.tsx                        # new-tab names
│   │   ├── SessionSetupInviteLink.tsx           # label
│   │   ├── SystemLegalNotice.tsx                # new-tab name
│   │   └── DiceRollerPanel/DiceRollerPanel.tsx  # label
│   ├── canvas-tools/
│   │   ├── WallTool/WallTool.tsx                # roving radios; title into name
│   │   └── TokenPanel/TokenPanel.scss           # visible ring; contrast pair
│   ├── ui/
│   │   ├── status-badge/StatusBadge.tsx         # role prop
│   │   ├── dialog.tsx                           # reduced motion
│   │   ├── skeleton.tsx                         # reduced motion
│   │   ├── button.tsx                           # target-size where sole control
│   │   └── new-tab-link/NewTabLink.tsx          # + <a target=_blank> with the suffix [<60]
│   ├── about/AboutInstance.tsx                  # NewTabLink
│   └── seo/SEO.tsx                              # unchanged; used by five more pages
├── layouts/
│   ├── main-layout/MainLayout.tsx               # skip link; id on main
│   └── world-layout/WorldLayout.tsx             # main + h1 for Play
├── pages/
│   ├── world/
│   │   ├── useEngineCanvas.ts                   # role, label, describedby, tabindex on the canvas
│   │   ├── WorldPage.tsx                        # the board summary element; passes facet to the dock
│   │   ├── {WorldListPage,WorldDashboardPage,WorldCompendiumPage,CreateWorldPage}.tsx  # <main> → <div>
│   │   ├── scenes/{ScenesPage,SceneDetailPage}.tsx   # SEO; label; <main> → <div>
│   │   ├── players/PlayersPage.tsx              # SEO; role select label
│   │   ├── compendium/{AbilityCompendiumTab,NpcCompendiumTab,ItemCompendiumTab}.tsx  # table semantics
│   │   ├── compendium/{AbilityEffectEditor,ItemEffectEditor,LoreCompendiumTab,BookListTab}.tsx  # labels; rings
│   │   └── actor/{ActorInventoryPanel,ActorLorePanel}.tsx  # rings
│   ├── auth/{RegisterPage,LoginView,SignUpPage}.tsx    # alert, aria-invalid, describedby, focus; SEO
│   ├── legal/LegalDocumentPage.tsx              # <main> → <div>
│   ├── feedback/MySubmissions.tsx               # NewTabLink
│   └── admin/TelemetryPanel.tsx                 # NewTabLink
└── styles/mixins.scss                           # outline beside box-shadow

apps/web/e2e/
├── fixtures/axe.ts                              # { bestPractice?: boolean } option
├── fixtures/a11y.ts                             # + page list, skip-link and landmark helpers [<200]
├── a11y-board-keyboard.spec.ts                  # + US1 [<400]
├── a11y-play-dock-keys.spec.ts                  # + US2 [<300]
├── a11y-chat-and-forms.spec.ts                  # + US3 [<300]
├── a11y-pages.spec.ts                           # + US4 [<300]
└── a11y-controls.spec.ts                        # + US5 [<400]

apps/demo/
├── src/DemoNotice.tsx                           # yields to focus; NewTabLink
└── e2e/a11y.spec.ts                             # + US6, the standalone half [<300]

docs/guides/playing-by-keyboard.md               # + the keys, in the guides' voice
scripts/e2e/slices.json                          # + accessibility slice
package.json                                     # + e2e:accessibility{,:standalone,:integration}
```

The remaining pages that nest a `<main>` (about twenty beyond the five
named) are found by `grep -rln "<main" apps/web/src/pages` at T030 and
listed in the task's ledger note; each is the same one-word change.

**Slice**: `scripts/e2e/slices.json` gains this feature's entry:

```json
{
  "name": "accessibility",
  "summary": "the web app and the demo by keyboard and screen reader: the Board list, the forwarder, landmarks, forms, the demo notice",
  "own": ["a11y-"],
  "standalone": "pnpm -F @thunderforge/demo e2e a11y",
  "neighbours": [
    { "spec": "token-keyboard-move.spec.ts", "seam": "the forwarder still feeds the engine from the body" },
    { "spec": "token-stack-picker.spec.ts", "seam": "selecting one of a stack from the DOM" },
    { "spec": "canvas-context-menu.spec.ts", "seam": "the menu opens on a keyboard-made selection" },
    { "spec": "combat-accessibility.spec.ts", "seam": "the dock's keyboard paths under the new forwarder" },
    { "spec": "chat-panel.spec.ts", "seam": "the feed is a log" },
    { "spec": "play-panels-by-role.spec.ts", "seam": "the dock gains a Board panel" },
    { "spec": "scene-management.spec.ts", "seam": "level tabs move with the arrows" },
    { "spec": "players-section.spec.ts", "seam": "the role select is labelled" },
    { "spec": "worlds-list.spec.ts", "seam": "skip link and one main on a hub page" }
  ],
  "paths": [
    "apps/web/src/components/world/PlayDock/{BoardPanel.tsx,boardSquares.ts}",
    "apps/web/src/components/world/PlayDock/__tests__/boardSquares.test.ts",
    "apps/web/src/components/ui/new-tab-link/**",
    "apps/web/e2e/fixtures/a11y.ts",
    "apps/demo/src/DemoNotice.tsx",
    "apps/demo/e2e/a11y.spec.ts",
    "docs/guides/playing-by-keyboard.md"
  ]
}
```

Files this spec edits that another slice already owns stay with that
slice (`canvasKeyboard.ts` with `tokens`, `ChatPanel.tsx` with
`companion`, the auth pages with `accounts`, and so on); `pnpm e2e:which`
then names both, which is the point.

Root `package.json`:

```json
"e2e:accessibility": "pnpm run e2e:accessibility:standalone && pnpm run e2e:accessibility:integration",
"e2e:accessibility:standalone": "pnpm -F @thunderforge/demo e2e a11y",
"e2e:accessibility:integration": "node ./scripts/e2e-parallel.mjs --shards=1 --slice=accessibility"
```

## Risks

- **The forwarder guard is too eager.** If `yieldsKeys` treats the dock's
  scrolling container as "scrollable", keys pressed with focus on a plain
  panel never reach the engine. Mitigation: scrollable counts only when the
  target itself is focused (it has a tabindex) and has overflow to scroll;
  `token-keyboard-move.spec.ts` and `combat-accessibility.spec.ts` are the
  neighbours that catch each direction of the mistake.
- **`moveBy`'s result may not carry a reason.** Then FR-005 announces
  "refused" and a follow-up in the `tokens` slice adds the reason to the
  intent result. The spec's assumption says so.
- **Twenty-five `<main>` edits across slices.** Each is one word, but
  `pnpm e2e:which --diff` will name most slices. The owner's decision
  (slices, not the suite) is recorded; the neighbour slices in SC-010 are
  the bound.
- **Best-practice rules find more than this spec lists.** The page list's
  axe run is written first and seen failing; whatever it finds beyond the
  discovery is fixed if it is on a listed page, or recorded in the ledger
  with its rule id if it belongs to another slice's file.
- **The demo notice's yield-to-focus fights the sitting specs.** They
  press the notice's buttons; if the notice moves on focus, their locators
  must still find it. The yield is a translate, not a removal.
