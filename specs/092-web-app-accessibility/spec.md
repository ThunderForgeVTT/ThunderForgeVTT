# Feature Specification: The Web App, Played Without a Pointer

**Feature Branch**: `092-web-app-accessibility`
**Created**: 2026-10-13
**Status**: Draft
**Input**: The owner, 2026-10-13: the landing page was taken through an
accessibility pass in specs 090 and 091 and holds; the web app and the demo
have not been. An accessibility discovery of the whole repository
([discovery.md](discovery.md)) found one blocker, four serious findings
and a long tail. This spec is the web app's and the demo's share of that
report, with the blocker's fix designed rather than named.

## Why

A Game Master who cannot use a mouse can today sign up, make a world, build
a character, invite players, run a fight from the tracker and roll dice —
every one of those was proven by keyboard in spec 046
(`apps/web/e2e/combat-accessibility.spec.ts`). What they cannot do is pick
a token on the board. Selection on the board is pointer-only: the engine's
own arrow-key movement (`crates/thunderforge-engine/src/systems/selection.rs:181-200`)
moves the token that is already selected, and the only way to select one is
to click it. Everything downstream of selection — the context menu, Look at,
the properties panels — is therefore closed to a keyboard user, which makes
the whole Play view a thing they can watch but not play.

The rest of the report is the ordinary debt of an app that grew fast: a
global key forwarder that eats Space and the arrows from the dock's own
controls, a chat feed no screen reader hears, sign-up errors nobody is told
about, pages with two `<main>` landmarks and no way to skip the header,
inputs labelled only by their placeholder, focus rings removed from selects,
and a demo notice that can sit on top of the thing you just focused.

None of this is exotic. Each item has a known shape and a cheap proof. What
the repository lacks is one place that says what the web app promises a
keyboard or screen-reader user, and the Playwright specs that keep it true.

## What exists

Read on 2026-10-13 against main at `da60b0fa`. Paths are under `apps/web/src`
unless they say otherwise.

**The board and its keys.**

- The engine's canvas is inserted by winit as the last child of `<body>`,
  pinned with `position: fixed; inset: 0`, and hidden with `display: none`
  when the page is not in Play (`pages/world/useEngineCanvas.ts:48-62,95-130`).
  It has no `role`, no accessible name, no description and no `tabindex`
  that the app sets.
- `engine/canvasKeyboard.ts` forwards every window `keydown`/`keyup` to the
  canvas in the capture phase (lines 129-134). It exempts text entry
  (`isTextEntry`, line 54: `INPUT`, `TEXTAREA`, `SELECT`, contenteditable)
  and open menus or dialogs (`isInOverlay`, line 71), nothing else. For
  `SCROLL_KEYS` (line 35: the arrows, Space, PageUp/Down, Home, End) it
  calls `preventDefault` (line 123). A focused button in the dock, a Radix
  tab list, a slider or a scrolling panel is none of the exemptions, so
  Space on a focused button and the arrows on a tab list are taken by the
  forwarder first.
- Selection is engine state (`SelectedToken` in
  `crates/thunderforge-engine/src/resources/selection.rs`, topmost first).
  The engine mirrors it into the world store as `select_token` /
  `select_tokens` (`engine/world/store.ts`), and the store forwards a UI
  `select_token` to the engine (`engine/world/facets/selection.ts:126-128`,
  `selectOne`). The engine SDK parses both (`crates/thunderforge-engine/src/sdk.rs:272-278`)
  and exposes `selection_state()` (`sdk.rs:478`) for tests.
- `facets/selection.ts:134` `moveBy(delta)` moves every selected token by a
  delta as an intent, returning one `IntentResult<MoveIntent>` per token:
  the engine and server adjudicate, and a refused move comes back refused.
- `engine/world/useSceneTokens.ts` returns the scene's tokens from the
  store, stable until something a panel reads changes.
- The only DOM path that selects a token is the stack picker
  (`pages/world/WorldPage.tsx:553-561`, `TokenStackPicker` →
  `facets.selection.selectOne(tokenId)`), and it opens on a canvas click.
  The context menu opens on Shift+F10 or the Menu key
  (`pages/world/useCanvasContextMenu.ts:97-108`) but needs a selection.
- The dock (`components/world/PlayDock/WorldDock.tsx:73-102`) is a
  `<section>` of `w-[22rem] max-w-[85vw]` with an `h2` and a
  `nav aria-label="Play panels"`; its panels are Actors, Combat, Chat,
  Settings, Help and the system's own panel. `PlayDock/ActorsPanel.tsx`
  (649 lines) lists actors with `aria-pressed` controls;
  `PlayDock/useSelectedTokenIds.ts` reads the engine's selection from the
  store; `PlayDock/LookAtButton.tsx` pans the camera to a token;
  `components/world/SelectionCount/SelectionCount.tsx` is a live region
  that says how many are selected.
- `PlayDock/HelpPanel.tsx` lists no keys. The engine binds the arrows, O
  (open door), C and Delete in `systems/{selection,wall,shape}.rs`.
  `docs/guides/` has no keyboard guide.

**What already announces.** `PlayDock/AttackLog/AttackLog.tsx` and
`PlayDock/OfferPrompt/OfferPrompt.tsx` have live regions; `SelectionCount`
does; `components/world/ConnectionStatus.tsx` does. `PlayDock/ChatPanel.tsx:179-182`
is a plain scrolling `div`: no `role="log"`, no `aria-live`.

**Forms and pages.**

- `pages/auth/RegisterPage.tsx:72,202` shows errors through the default
  `components/ui/status-badge/StatusBadge.tsx`, which has no `role="alert"`;
  the fields are not marked `aria-invalid` and do not point at the message.
- `layouts/main-layout/MainLayout.tsx:15` renders `<main className="pt-6 pb-12">`;
  pages render their own `<main>` inside it (`pages/world/WorldListPage.tsx:145`,
  `pages/world/WorldDashboardPage.tsx:147`, `pages/world/WorldCompendiumPage.tsx:152`,
  `pages/legal/LegalDocumentPage.tsx:63`, `pages/world/CreateWorldPage.tsx:224`,
  and about twenty more). There is no skip link (`MainLayout.tsx:13-15`).
- The Play view (`pages/world/WorldPage.tsx:494-`, inside
  `layouts/world-layout/WorldLayout.tsx:37`) has no `h1` and no `main`.
- `components/seo/SEO.tsx` sets a unique title on most pages;
  `pages/world/scenes/ScenesPage.tsx`, `pages/world/scenes/SceneDetailPage.tsx`,
  `pages/world/players/PlayersPage.tsx`, `pages/world/WorldCompendiumPage.tsx`
  and `pages/auth/SignUpPage.tsx` do not use it.
- Placeholder-only inputs: `components/world/DiceRollerPanel/DiceRollerPanel.tsx:94`,
  `pages/world/compendium/AbilityEffectEditor.tsx:252,259`,
  `pages/world/scenes/ScenesPage.tsx:187`,
  `pages/world/compendium/LoreCompendiumTab.tsx:189`,
  `pages/world/compendium/ItemEffectEditor.tsx:211,218`,
  `pages/world/compendium/BookListTab.tsx:884,891,898`,
  `components/world/SessionSetupInviteLink.tsx:63`; an unlabelled role
  `<select>` at `pages/world/players/PlayersPage.tsx:458`.
- Focus ring removed on native selects: `pages/world/actor/ActorInventoryPanel.tsx:290`,
  `pages/world/actor/ActorLorePanel.tsx:268`,
  `pages/world/compendium/ItemEffectEditor.tsx:135,173,202,230`; a
  0.1-opacity ring in `components/canvas-tools/TokenPanel/TokenPanel.scss:363-366`.
  `styles/mixins.scss:3-9` draws focus rings as `box-shadow`, which
  forced-colours mode discards.
- Small targets: `components/ui/button.tsx:23-34` sizes 24–32px;
  `components/world/RollAdvantage/AdvantagePicker.tsx:31-38` about 20px.
- Tables: `components/ui/data-table/DataTable.tsx:82-88` sets `scope`;
  the Ability, NPC and Item compendium tabs use clickable `<tr onClick>`
  rows with `aria-selected` on a `<tr>` (not valid there) and no
  `scope` or `caption`.
- Links that open a new tab without saying so: `components/world/MapCredit.tsx:25,35,45`,
  `PlayDock/ActorsPanel.tsx:451,576`, `components/world/SystemLegalNotice.tsx:68`,
  `pages/feedback/MySubmissions.tsx:101`, `components/about/AboutInstance.tsx:43`,
  `pages/admin/TelemetryPanel.tsx:115`.
- Groups without arrow movement: `components/world/LevelTabs/LevelTabs.tsx:75-90`
  (a `tablist`), `components/canvas-tools/WallTool/WallTool.tsx:118-131`
  (a `radiogroup`), `RollAdvantage/AdvantagePicker.tsx:22-31`. `WallTool.tsx:131`
  carries information only in a `title`.
- `TokenPanel.scss` sets `#a89968` on `#fffaee` (2.72:1). `components/ui/dialog.tsx:62`
  and `components/ui/skeleton.tsx:7` animate regardless of
  `prefers-reduced-motion`.

**The demo.** `apps/demo` renders the same source as the web app against a
static, in-page backend. `apps/demo/src/DemoNotice.tsx` is a fixed notice
at the bottom of the viewport with three `_blank` links (lines 98, 107,
116); it can cover a focused control near the bottom. Its e2e
(`apps/demo/e2e/`: `demo-board`, `demo-pages`, `demo-limits`, `tokens`,
`scenes`, `fight`, `dice`, `lighting`, `shapes-by-players`, `uploads`,
`telemetry`, `limited-device`, `rolls-across-tabs`, with `support.ts` and
`sitting.ts`) run no axe check; `@axe-core/playwright` is not in
`apps/demo/package.json`.

**Tooling.** `apps/web/package.json:22` has `@axe-core/playwright` 4.13.0.
`apps/web/e2e/fixtures/axe.ts` runs axe with the tags `wcag2a wcag2aa
wcag21a wcag21aa wcag22aa`, deliberately not best-practice, scoped by a
selector when given; 27 web specs call it. There is no
`eslint-plugin-jsx-a11y`. `crates/thunderforge-pack-system-spec/src/contrast.rs`
holds a contrast floor for system themes.

**Slices.** `scripts/e2e/slices.json` declares each slice's `own` (spec
file names or prefixes), `neighbours` (spec + seam) and `paths`; a slice
may have a `standalone` command. `tokens` owns the `token-` prefix,
`look-at-and-follow.spec.ts`, `engine/canvasKeyboard.ts`, the selection
facets and `PlayDock/{LookAtButton.tsx,useSelectedTokenIds.ts}`;
`canvas` owns `canvas-`, `components/engine/**`, the context menu,
`WallTool` and `ShapeTool`; `combat` owns `combat-` (so
`combat-accessibility.spec.ts`); `companion` owns `ChatPanel.tsx`;
`accounts` owns `pages/auth/**`; `worlds` owns `WorldDock`, `HelpPanel`,
`SettingsPanel`, `MapCredit`, `SessionSetupInviteLink`, `components/seo/**`
and the world layouts; `scenes` owns `LevelTabs/**` and the scene pages;
`actors` owns `ActorsPanel.tsx` and the players pages. `components/ui/**`,
`styles/**` and `layouts/main-layout/**` are `crossCutting`.

## Decisions

### Made by the owner

- The landing was done in 090 and 091 and is not reopened here. The
  hero-builder app is out of scope.
- A change is proven by its slice, never by the full suite, even when it
  touches a cross-cutting path. This spec touches three (`components/ui/**`,
  `styles/**`, `layouts/main-layout/**`); its proof is the `accessibility`
  slice plus the neighbour slices named in `plan.md`.
- No new dependency. `@axe-core/playwright` is already in `apps/web`; the
  demo's axe check imports it through the workspace rather than adding it
  to `apps/demo/package.json`. Whether to add `eslint-plugin-jsx-a11y` is
  an open question for the owner, not a step in this spec.

### Made by this spec

- **Keyboard selection lives in the dock, as a list of the scene's tokens.**
  The dock already reads the engine's selection from the store
  (`useSelectedTokenIds`) and already lists actors with `aria-pressed`
  controls. A "Board" list of the tokens in the scene, one button per
  token, is the natural place to select one; it is where a screen reader
  user will look, and it works when the canvas is covered or small.
- **Selection and movement both go through the store, and the engine stays
  the source of truth.** Choosing a row dispatches `select_token` through
  `facets.selection.selectOne`; the row shows as selected only when the
  store's `selectedTokenIds`, which the engine writes, says so. An arrow
  key on a selected row dispatches a move through
  `facets.selection.moveBy`, which the engine and server adjudicate; the
  announcement reads the result they return. No component talks to the
  GraphQL client, and nothing in the list keeps a position of its own.
- **The forwarder yields to focused controls.** `canvasKeyboard.ts` forwards
  only when the key's target is the body, the canvas, or an element that
  is neither interactive nor scrollable. A focused button, link, tab,
  slider, radio or scrollable panel keeps its keys.
- **The canvas gets a name, a description and focus.** `role="application"`,
  `aria-label` naming the scene, `aria-describedby` pointing at a visually
  hidden summary built from the store (how many tokens, what is selected,
  the keys), `tabindex="0"`, and a visible focus ring. With the canvas
  focused, the engine's own arrow movement works as it does today.
- **One slice, `accessibility`, owns the proof.** Its web specs are the
  `a11y-` prefix; its standalone half is the demo's `a11y` spec. Existing
  keyboard and axe assertions stay where they are and are named as
  neighbours.
- **Honest claim.** Pages are checked against axe's WCAG 2.2 A/AA rule
  tags (the existing fixture) and, on the pages this spec names, axe's
  best-practice rules as well. Keyboard paths are proven by Playwright.
  This is not a WCAG conformance claim; a screen-reader pass with NVDA or
  VoiceOver is listed for the owner and is not something Playwright proves.

## User Scenarios & Testing

### User Story 1 – Pick a token and walk it, by keyboard (Priority: P1)

A Game Master who does not use a mouse opens Play, tabs into the dock's
Board list, hears "Goblin, column 12, row 7, not selected", presses Enter
and hears it is selected, then presses the right arrow and hears "Goblin,
column 13, row 7". Pressing into a wall, they hear "Wall in the way; Goblin
stays at column 13, row 7". The token on the board is the one that moved.

**Why this priority**: it is the blocker. Without it the Play view is
closed to a keyboard user, and everything in the other stories is
decoration on a page they cannot use.

**Independent Test**: `apps/web/e2e/a11y-board-keyboard.spec.ts` — a Game
Master places two tokens, tabs to the Board list, selects one, presses the
arrows, and the spec reads the engine's `selection_state()`, the store's
`selectedTokenIds`, the server's stored position and the live region.

**Acceptance Scenarios**:

1. **Given** a scene with tokens, **When** the Game Master tabs to the dock's
   Board list, **Then** each token is a button named with the token's label
   and its square, with `aria-pressed` reflecting the engine's selection.
2. **Given** a row is focused, **When** Enter or Space is pressed, **Then**
   the engine reports that token selected (`selection_state()`), the store's
   `selectedTokenIds` matches, and the selection count live region says so.
3. **Given** a selected row is focused, **When** an arrow key is pressed,
   **Then** the token's server position changes by one grid square in that
   direction, a second client's board shows it, and the live region says the
   new square.
4. **Given** a selected row is focused and a wall stands in the way,
   **When** the arrow toward it is pressed, **Then** the server position does
   not change and the live region says the move was refused.
5. **Given** a row the user may not move (a player on another player's
   token), **When** an arrow is pressed, **Then** nothing moves and the live
   region says the token is not theirs to move.
6. **Given** a token is selected from the list, **When** Shift+F10 is
   pressed with the row focused, **Then** the canvas context menu opens for
   that token, as it does today after a click.
7. **Given** the canvas, **When** the user tabs to it, **Then** it is
   focusable, named for the scene, described by the summary, shows a focus
   ring, and the arrows move the selected token through the engine.
8. **Given** the Help panel, **When** it is opened, **Then** it lists the
   board's keys, and `docs/guides/playing-by-keyboard.md` exists and says
   the same.

---

### User Story 2 – The dock's own keys stay its own (Priority: P1)

A player tabs to the dock, moves between its tabs with the arrows, presses
Space on "Roll", and scrolls the chat with PageDown. None of those keys
reaches the board, and none moves a token.

**Why this priority**: it is the serious finding that breaks the parts of
the app that already work by keyboard. Fixing US1 without this would make
the arrows do two things at once.

**Independent Test**: `apps/web/e2e/a11y-play-dock-keys.spec.ts` — with a
token selected, focus a dock tab and press ArrowRight; focus a button and
press Space; focus the chat feed and press PageDown. The spec reads the
token's position before and after and the focused element.

**Acceptance Scenarios**:

1. **Given** a token is selected and a dock tab is focused, **When**
   ArrowRight is pressed, **Then** the next tab is focused and the token has
   not moved.
2. **Given** a focused button in the dock, **When** Space is pressed,
   **Then** the button activates and the key's default is not prevented
   elsewhere.
3. **Given** the chat feed is focused, **When** PageDown is pressed,
   **Then** the feed scrolls and the token has not moved.
4. **Given** focus is on the body or the canvas, **When** an arrow is
   pressed, **Then** the engine still receives it, as it does today
   (`token-keyboard-move.spec.ts` keeps passing).
5. **Given** the level tabs, the wall tool's radio group and the advantage
   picker, **When** an arrow key is pressed on a focused member, **Then**
   focus moves to the next member and the member is chosen, and Tab leaves
   the group.

---

### User Story 3 – What changes is announced, and what fails is said (Priority: P2)

A screen-reader user hears a new chat message arrive without leaving the
sheet they are reading, and when sign-up fails they hear which field is
wrong and why.

**Why this priority**: both findings are serious but neither blocks play.
They decide whether a screen-reader user can keep up with a table and get
into one.

**Independent Test**: `apps/web/e2e/a11y-chat-and-forms.spec.ts` — a second
client posts a chat message and the spec checks the feed's role and live
attributes; a sign-up with a taken email checks the alert, `aria-invalid`
and `aria-describedby`, and axe on the form.

**Acceptance Scenarios**:

1. **Given** the Chat panel, **When** it renders, **Then** the feed has
   `role="log"` and is polite-live, and a message posted by another client
   appears inside it.
2. **Given** the sign-up form, **When** it is submitted with an error,
   **Then** the message has `role="alert"`, the field is `aria-invalid`,
   the field's `aria-describedby` names the message, and focus moves to the
   first invalid field.
3. **Given** the sign-in form, **When** it fails, **Then** the same holds.
4. **Given** either form, **When** axe runs with the WCAG and best-practice
   tags on it, **Then** there are no violations.

---

### User Story 4 – Every page has one main, one heading and a way past the header (Priority: P2)

A keyboard user lands on any page, presses Tab once, and is offered "Skip
to content". A screen-reader user asks for the page's landmarks and finds
one `main`, one `h1`, and a title that says which page this is.

**Why this priority**: landmarks and titles are how a screen-reader user
knows where they are; the nested `<main>` on about twenty-five pages is a
single-cause defect with a single fix.

**Independent Test**: `apps/web/e2e/a11y-pages.spec.ts` — for each page in
a named list, open it, count `main` and `h1`, read the title, press Tab and
check the skip link, follow it and check focus, then run axe with the
WCAG and best-practice tags on the whole page.

**Acceptance Scenarios**:

1. **Given** any page under `MainLayout`, **When** Tab is pressed once from
   the top, **Then** a visible "Skip to content" link is focused, and
   activating it moves focus into the page's `main`.
2. **Given** any page in the list, **When** its landmarks are counted,
   **Then** there is exactly one `main` and one `h1`.
3. **Given** the Play view, **When** it is open, **Then** it has a `main`
   containing the board and the dock, and an `h1` naming the scene (visually
   hidden is acceptable).
4. **Given** Scenes, a scene's detail, Players, the compendium and Sign up,
   **When** opened, **Then** `document.title` names the page and the world.
5. **Given** the pages in the list, **When** axe runs with the WCAG and
   best-practice tags on the whole page, **Then** there are no violations.
6. **Given** the compendium's Ability, NPC and Item tabs, **When** rendered,
   **Then** each table has a caption or a labelled name, header cells have
   `scope`, and a row is chosen through a control inside it rather than by
   clicking the row.

---

### User Story 5 – Forms, focus and targets (Priority: P3)

Every input has a label; every control shows where focus is, in forced
colours too; links that open a new tab say so; animation respects the
reader's preference; nothing is told only by colour or a tooltip.

**Why this priority**: each is small and none blocks a task, but together
they are what a keyboard or low-vision user meets on every page.

**Independent Test**: `apps/web/e2e/a11y-controls.spec.ts` — open the
pages and panels named, and check labels, focus styles (including under
`forced-colors: active`), link names, `prefers-reduced-motion`, and the
contrast and target rules through axe.

**Acceptance Scenarios**:

1. **Given** the inputs listed in What exists, **When** rendered, **Then**
   each has an accessible name from a label or `aria-label`, not only a
   placeholder.
2. **Given** the selects listed, **When** focused by keyboard, **Then** a
   focus ring is visible, and under `forced-colors: active` an outline is
   drawn.
3. **Given** the links listed, **When** read, **Then** the accessible name
   or a visually hidden suffix says "opens in a new tab".
4. **Given** `prefers-reduced-motion: reduce`, **When** a dialog opens or a
   skeleton shows, **Then** neither animates.
5. **Given** the token panel, **When** axe runs its contrast rule, **Then**
   the `#a89968` on `#fffaee` pairing is gone.
6. **Given** the advantage picker and the smallest buttons, **When** axe
   runs `target-size`, **Then** no violation.
7. **Given** the wall tool, **When** the `title`-only text is read, **Then**
   it is also in the control's accessible name or description.

---

### User Story 6 – The demo, by keyboard and axe (Priority: P3)

A visitor to the demo with a keyboard can do what the demo's sitting does —
select the fighter, walk him, open the door — and the demo notice never
covers what they focused.

**Why this priority**: the demo renders the same source, so it inherits
US1–US5; what is its own is the notice and the absence of any axe check.

**Independent Test**: `apps/demo/e2e/a11y.spec.ts`, the slice's standalone
half: the sitting by keyboard, the notice's position against the focused
element, and axe on the demo's board and pages.

**Acceptance Scenarios**:

1. **Given** the demo's board, **When** the visitor tabs to the Board list
   and presses the arrows, **Then** the fighter moves as the sitting's drag
   moves him.
2. **Given** the demo notice, **When** any control is focused, **Then** the
   focused element's bounding box does not intersect the notice, or the
   notice has moved out of the way.
3. **Given** the demo's board and its pages, **When** axe runs with the
   WCAG and best-practice tags, **Then** there are no violations.
4. **Given** the demo notice's links, **When** read, **Then** each says it
   opens in a new tab.

### Edge Cases

- A token with no label is announced by its actor's name, then by its id's
  short form; never as an empty button.
- A stack: selecting a row that is a member of a stack narrows the
  selection to that token (as the stack picker does), not the stack.
- The scene changes while a row is focused: the list re-renders from the
  store; focus moves to the list's heading rather than vanishing.
- A row is deleted (another client removes the token) while focused: focus
  moves to the next row, or the heading when none is left.
- Arrow on a row that is focused but not selected: selects it and moves it
  in one press? No — it selects it only, and says so; the second press
  moves. One key, one meaning.
- Keyboard repeat: holding an arrow sends repeated moves; each is one
  intent and each result is announced, with the announcement collapsed to
  the last result in flight so the reader is not flooded.
- Focus on the canvas and a dialog opens over it: the forwarder's overlay
  exemption still holds (`isInOverlay`).
- The skip link on a page whose `main` is the Play view: it lands on the
  Board list, not the canvas, so a screen-reader user lands on text.
- `forced-colors: active` with the focus ring: `box-shadow` is discarded,
  so the mixin adds a transparent `outline` that forced colours paints.
- The demo notice at 320px width: it must not take more than a third of
  the viewport height, and the page must scroll past it.

## Requirements

### The board's keys (US1)

- **FR-001**: The Play dock MUST offer a Board list of the tokens in the
  current scene, built from the world store (`useSceneTokens`), one button
  per token, named "{label}, column {c}, row {r}", with `aria-pressed`
  reflecting the store's `selectedTokenIds`.
- **FR-002**: Activating a row MUST dispatch `select_token` through the
  selection facet (`facets.selection.selectOne`); the row MUST NOT hold a
  selected state of its own, and MUST read it back from the store after the
  engine has written it.
- **FR-003**: An arrow key on a focused, selected row MUST dispatch a move
  of one grid square (the scene's `gridSize`) in that direction through
  `facets.selection.moveBy`, and MUST NOT call the GraphQL client, the
  engine bridge or the token API directly.
- **FR-004**: An arrow key on a focused row that is not selected MUST select
  it and MUST NOT move it.
- **FR-005**: The result of each move MUST be announced in a polite live
  region: the new square on success; the reason on refusal (a wall, not the
  user's token, the engine not ready). Repeated results in flight MUST
  collapse to the latest.
- **FR-006**: A refused move MUST leave the token where the server says it
  is; the list MUST show the square the store holds, never one it
  predicted.
- **FR-007**: With a row focused and a token selected, Shift+F10 and the
  Menu key MUST open the canvas context menu for the selection, as a click
  does today.
- **FR-008**: The engine canvas MUST have `tabindex="0"`, `role="application"`,
  an `aria-label` naming the scene, and `aria-describedby` pointing at a
  visually hidden summary that says how many tokens are in the scene, which
  are selected, and the keys. The summary MUST update from the store.
- **FR-009**: The focused canvas MUST show a visible focus ring, in forced
  colours too.
- **FR-010**: The Help panel MUST list the board's keys (select, move, open
  a door, context menu, Escape), and `docs/guides/playing-by-keyboard.md`
  MUST exist and say the same.

### The forwarder (US2)

- **FR-011**: `canvasKeyboard.ts` MUST forward a key only when its target is
  the document body, the canvas, or an element that is neither interactive
  (`button, a[href], input, select, textarea, summary, [contenteditable],
  [tabindex]` with a non-negative tabindex, or any `role` of button, link,
  tab, menuitem, option, radio, checkbox, switch, slider, spinbutton,
  textbox, combobox, listbox, tree, grid, application) nor scrollable
  (overflow auto or scroll with content to scroll). The text-entry and
  overlay exemptions MUST stay.
- **FR-012**: The forwarder MUST call `preventDefault` on a scroll key only
  when it has forwarded the key.
- **FR-013**: `isTextEntry`, `isInOverlay` and the new `yieldsKeys` guard
  MUST be exported and unit-tested; the existing unit tests for the file
  MUST keep passing.
- **FR-014**: `LevelTabs`, the wall tool's radio group and the advantage
  picker MUST move focus and choice with the arrow keys within the group
  (roving `tabindex`), with Tab leaving the group.

### Announcements and errors (US3)

- **FR-015**: The chat feed MUST have `role="log"` and `aria-live="polite"`
  (and `aria-relevant="additions"`), with an accessible name.
- **FR-016**: Sign-up and sign-in errors MUST render in an element with
  `role="alert"`; the failing field MUST be `aria-invalid="true"` and its
  `aria-describedby` MUST reference the message; focus MUST move to the
  first invalid field.
- **FR-017**: `StatusBadge` MUST accept a `role` (or a `tone="error"` that
  sets `role="alert"`) so callers can make a badge an alert without a
  second component.

### Pages (US4)

- **FR-018**: `MainLayout` MUST render a "Skip to content" link as the first
  focusable element, visible on focus, targeting the page's `main`.
- **FR-019**: Every page MUST render exactly one `main` landmark and one
  `h1`. Pages that render their own `<main>` inside `MainLayout` MUST be
  changed so one of the two is a `div` (the layout keeps `main`; the page
  loses it) — one mechanism, applied to every page in the list in What
  exists.
- **FR-020**: The Play view MUST have a `main` containing the board region
  and the dock, and an `h1` naming the scene (visually hidden is
  acceptable).
- **FR-021**: Scenes, scene detail, Players, the compendium and Sign up
  MUST set a unique `document.title` through `components/seo/SEO.tsx`.
- **FR-022**: The compendium's Ability, NPC and Item tables MUST have a
  caption or `aria-label`, `scope` on header cells, no `aria-selected` on
  `<tr>`, and a control inside the row for choosing it; the row's `onClick`
  MAY stay as a pointer convenience.

### Controls (US5)

- **FR-023**: Every input listed in What exists MUST have an accessible
  name from a `<label>` or `aria-label`; the role `<select>` on Players
  MUST be labelled.
- **FR-024**: No stylesheet MUST remove the focus indicator from a control
  without drawing another; the token panel's 0.1-opacity ring MUST be
  replaced with a visible one.
- **FR-025**: The focus mixin in `styles/mixins.scss` MUST add a transparent
  `outline` beside its `box-shadow`, so forced-colours mode draws a ring.
- **FR-026**: Links that open a new tab MUST say so in their accessible name
  (a shared `NewTabLink` or a visually hidden suffix), including the demo
  notice's.
- **FR-027**: `dialog.tsx` and `skeleton.tsx` MUST not animate under
  `prefers-reduced-motion: reduce`.
- **FR-028**: The token panel's `#a89968` on `#fffaee` pairing MUST be
  replaced by one at or above 4.5:1.
- **FR-029**: The advantage picker's targets MUST be at least 24×24 CSS px
  with the spacing WCAG 2.5.8 allows; the `button.tsx` sizes MUST satisfy
  axe's `target-size` rule where they are used as the only control.
- **FR-030**: The wall tool's `title`-only information MUST also be in the
  control's accessible name or description.

### The demo (US6)

- **FR-031**: The demo notice MUST NOT cover a focused element: either it
  is positioned so no focusable element sits under it, or it yields (moves
  or collapses) when focus enters the area it covers.
- **FR-032**: The demo MUST get an axe check (the web app's fixture,
  imported through the workspace, no new dependency) on its board and each
  of its pages, with the WCAG and best-practice tags.
- **FR-033**: The demo's keyboard sitting MUST run as the `accessibility`
  slice's standalone half: `pnpm -F @thunderforge/demo e2e a11y`.

### Proof and shape

- **FR-034**: `scripts/e2e/slices.json` MUST gain an `accessibility` slice
  owning the `a11y-` prefix, with the neighbours and paths `plan.md` names,
  and root `package.json` MUST gain `e2e:accessibility`,
  `e2e:accessibility:standalone` and `e2e:accessibility:integration`.
- **FR-035**: `apps/web/e2e/fixtures/axe.ts` MUST gain an option to add
  axe's `best-practice` tag for the pages this spec names, without changing
  the default the 27 existing callers get.
- **FR-036**: No file this spec adds or changes MAY exceed 1000 lines;
  `ActorsPanel.tsx` (649) and `WorldPage.tsx` MUST NOT be the home of the
  Board list — it is its own file.
- **FR-037**: Nothing in this spec MAY add a dependency, change the GraphQL
  schema, or change the engine's selection or movement semantics; the
  engine is the source of truth for both.

### Key Entities

- **Board list**: the dock's list of the scene's tokens, read from the
  world store, one row per token; a row has a label, a square and a
  selected state the engine owns.
- **Move announcement**: a polite live region holding the last move result
  for the selected token: a square or a refusal.
- **Board summary**: the visually hidden description of the canvas: token
  count, selection, keys.
- **Key guard**: the forwarder's test of whether a key's target keeps its
  keys.
- **Page list**: the pages US4 checks, by route, in the spec's own fixture.

## Success Criteria

- **SC-001**: A Game Master selects a token from the Board list and moves it
  one square in each of four directions by keyboard alone; the server's
  stored position, the engine's `selection_state()` and a second client's
  board agree (`a11y-board-keyboard.spec.ts`).
- **SC-002**: A move into a wall by keyboard leaves the server position
  unchanged and the live region names the refusal within 2 seconds.
- **SC-003**: A player's arrow on another player's token moves nothing and
  is announced as not theirs.
- **SC-004**: With a token selected, ArrowRight on a dock tab, Space on a
  dock button and PageDown on the chat feed each do their own thing and
  the token's position is unchanged (`a11y-play-dock-keys.spec.ts`).
- **SC-005**: `token-keyboard-move.spec.ts`, `token-stack-picker.spec.ts`,
  `canvas-context-menu.spec.ts` and `combat-accessibility.spec.ts` pass
  unchanged after the forwarder change.
- **SC-006**: A message posted by a second client appears inside an element
  with `role="log"`; a failed sign-up shows an alert with the field marked
  invalid and described, and focus on it (`a11y-chat-and-forms.spec.ts`).
- **SC-007**: On every page in the page list: one `main`, one `h1`, a
  skip link first in the tab order that lands inside `main`, a unique
  title, and zero axe violations with the WCAG and best-practice tags
  (`a11y-pages.spec.ts`).
- **SC-008**: On the panels and pages US5 names: every input named, every
  select showing focus, every new-tab link saying so, no animation under
  reduced motion, and zero violations of axe's `color-contrast` and
  `target-size` rules (`a11y-controls.spec.ts`).
- **SC-009**: The demo's fighter is selected and walked by keyboard, no
  focused element is under the notice, and the demo's board and pages pass
  axe with the WCAG and best-practice tags (`apps/demo/e2e/a11y.spec.ts`).
- **SC-010**: `pnpm e2e:accessibility` runs the standalone half then the
  integration half and passes, alone, with the wall time recorded in
  `tasks.md`; the neighbour slices `tokens`, `canvas`, `combat`,
  `companion`, `accounts`, `worlds`, `scenes` and `actors` pass.
- **SC-011**: `pnpm verify` passes on the changed code, and every file this
  spec touches is under 1000 lines.
- **SC-012**: `docs/guides/playing-by-keyboard.md` exists and the Help panel
  lists the same keys (`a11y-board-keyboard.spec.ts` reads both).

### Proof

The `accessibility` slice is the proof: `pnpm e2e:accessibility` runs
`pnpm -F @thunderforge/demo e2e a11y` and then the `a11y-` specs against
the stack. The neighbour slices named in SC-010 are run once each at the
end. The full e2e suite is not a gate.

## Assumptions

- `facets.selection.moveBy` returns one `IntentResult<MoveIntent>` per
  selected token carrying enough to say why a move was refused. If it
  carries only accepted/refused, the announcement says "refused" and the
  reason is added to the intent result in the tokens slice's own code, not
  here.
- The engine's `select_token` from the store narrows a stack to one, as the
  stack picker relies on.
- A grid square is the scene's `gridSize` in world units, as the store
  holds it (`engine/world/types.ts:214`).
- The demo's `openDemo` helper and the web app's `fixtures/axe.ts` can be
  imported together in `apps/demo/e2e` through the workspace without a
  package change; if a path alias is needed it is a `tsconfig` path, not a
  dependency.
- Playwright's Chromium honours `forced-colors` and
  `prefers-reduced-motion` through `page.emulateMedia`.

## Out of scope

- The landing (`apps/landing`) and the hero-builder app.
- A manual screen-reader pass (NVDA, VoiceOver); listed for the owner.
- `eslint-plugin-jsx-a11y`; a dependency decision for the owner.
- A coarse-pointer bump of every button to 44px; the owner decides the
  touch policy (`docs/guides/playing-on-a-touchscreen.md` is its home).
- Engine-side changes to selection or movement; the engine's keys and
  adjudication stay as they are.
- Theme contrast over bright maps and the board's own flash rate; both
  need a runtime check (discovery section 4) and are noted as open.
- Heading order and repeated View/Edit names across hub pages beyond what
  axe's best-practice rules flag on the page list.

## Open questions

- Owner: add `eslint-plugin-jsx-a11y` to `apps/web`'s lint, so the next
  placeholder-only input is caught at lint time rather than by a spec?
- Owner: a 44px minimum on coarse pointers for every control, or only for
  the play surface?
- Owner: schedule a screen-reader pass once US1–US4 land; which reader?
