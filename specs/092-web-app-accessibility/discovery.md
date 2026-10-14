# ThunderForgeVTT accessibility discovery pass (A11Y Project checklist)

Date: 2026-10-13. Checkout: `.claude/worktrees/keen-oak-44oe` (branch `token-store-on-main`).
Method: static reading only (no build, browser, or test run, because an e2e run was in progress). Checklist items come from https://www.a11yproject.com/checklist/, fetched for this pass.
Scope: `apps/web` (the main VTT; `apps/demo` renders the same `apps/web/src`), `apps/landing`, `apps/demo`, `apps/hero-builder` (with `packages/hero-builder` and `packages/heroes`, which it renders). `packs/interface/*` holds colour tokens only (JSON, no markup), so it matters here only for contrast.

Paths below are relative to the repo root.

Legend: **Pass** = static evidence it is handled. **Gap** = static evidence it is not. **Unknown** = needs a runtime check. **N/A** = the app has no such content.

---

## 1. Checklist × app matrix

| Checklist item | web | landing | demo (= web + notice) | hero-builder |
|---|---|---|---|---|
| **Content** plain language | Pass (copy is plain throughout) | Pass | Pass | Pass |
| Unique, descriptive button/link/label text | Unknown (many "View"/"Edit" per row, e.g. compendium tabs; check names in context) | Pass | Unknown (as web) | Pass |
| Left-aligned LTR text | Pass | Pass | Pass | Pass |
| **Global** valid HTML | Gap: nested `<main>`, `MainLayout.tsx:15` around ~25 pages that render their own `<main>` (e.g. `pages/world/WorldListPage.tsx:145`, `WorldDashboardPage.tsx:147`, `compendium/WorldCompendiumPage.tsx:152`, `legal/LegalDocumentPage.tsx:63`); `aria-selected` on a plain `<tr>` (`compendium/AbilityCompendiumTab.tsx:402`) | Pass | Gap (as web) | Pass |
| `lang` on `<html>` | Pass (`apps/web/index.html:2`) | Pass | Pass | Pass |
| Unique page title | Gap: no `<SEO>` on `pages/world/scenes/ScenesPage.tsx`, `SceneDetailPage.tsx`, `players/PlayersPage.tsx`, `compendium/WorldCompendiumPage.tsx`, `auth/SignUpPage.tsx`. The other ~48 pages set one through `components/seo/SEO.tsx` | Pass (single page) | Gap (as web) | Pass (single view) |
| Viewport zoom not disabled | Pass (`index.html:5`) | Pass | Pass | Pass |
| Landmarks | Gap: Play view (`pages/world/WorldPage.tsx:494`, `layouts/world-layout/WorldLayout.tsx:37`) has no `<main>`, and the board canvas sits outside every landmark at the end of `<body>` (`pages/world/useEngineCanvas.ts:48-62`); nested `<main>` elsewhere. The dock and tool rail do use `<nav aria-label>` | Pass (`Nav.tsx:7`, `App.tsx:22`, labelled sections) | Gap (as web) | Pass (`App.tsx:72,97`) |
| Linear content flow | Unknown (Play overlays are absolutely positioned; DOM vs visual order of dock panel vs rail, `WorldDock.tsx:73-102`) | Pass | Unknown | Pass |
| Avoid `autofocus` | Pass (5 uses, all inside dialogs or menus that just opened: `CanvasContextMenu.tsx:784,888`, `SceneSwitcher.tsx:135`, `ShapeTool.tsx:297`, `QuickNpcDialog.tsx:137`) | Pass | Pass | Pass |
| Extendable session timeouts | Pass (sessions last days: `crates/thunderforge-server/src/auth/sessions.rs:321`; a 401 sends you to sign-in, `hooks/useAuth.ts:125`) | N/A | N/A (local) | N/A |
| Remove `title` tooltips | Gap (minor): ~69 `title=` attributes. Most duplicate an `aria-label`, but some carry the only hint, e.g. `canvas-tools/WallTool/WallTool.tsx:131` | Pass | Gap (as web) | Pass |
| **Keyboard** visible focus | Gap: `outline-none` with no replacement ring on native selects at `actor/ActorInventoryPanel.tsx:290`, `actor/ActorLorePanel.tsx:268`, `item/ItemEffectEditor.tsx:135,173,202,230`; a ring at 0.1 opacity in `styles/TokenPanel.scss:363-366`. The global ring (`styles/globals.css:136-143`) is otherwise solid | Pass (`styles.css:90`) | Gap (as web) | Pass (`styles.ts:82`, `App.tsx:221`) |
| Focus order matches layout | Unknown | Pass | Unknown | Pass |
| No invisible focusables | Unknown (the dock is hidden with `display:none` in staging, `WorldPage.tsx:~589`, which is correct; check the hidden file inputs) | Pass | Unknown | Pass (`tfhb-app-hidden-input` sits inside a visible label) |
| **Keyboard operability of core task** (A11Y Project lists this under Keyboard; WCAG 2.1.1) | **Gap (blocker)**: no keyboard way to select a token on the board. See finding B1 | Pass: hero tokens are buttons that arrow keys move, with a live announcement (`sections/Hero.tsx:479-519`) | Gap (as web) | Pass |
| **Images** `alt` on every `img` | Pass (the heuristic scan found no `<img>` without `alt`) | Pass | Pass | Pass (SVG preview has `role="img"` plus `<title>`: `packages/heroes/src/render.ts:104-126`) |
| Decorative images use `alt=""` | Pass | Pass | Pass | Pass |
| Complex images (maps, charts) | Gap: the board canvas has no role, no name, and no text alternative (`useEngineCanvas.ts`; no `aria`/`tabindex` anywhere in the engine mount) | Pass (`MapLegend.tsx:100` `role="img"` with a descriptive label; StarChart nodes are labelled) | Gap (as web) | Pass |
| Text in images | Pass | Pass | Pass | N/A |
| **Headings** used to introduce content | Pass on hub pages | Pass | Pass | Pass |
| One `h1` | Gap: Play view has none (dock panels start at `h2`, `WorldDock.tsx:79`) | Pass (`Hero.tsx:374`) | Gap (as web) | Pass |
| Logical sequence, no skipped levels | Unknown (sampled pages go h1, then h2; HelpPanel uses h3 under the dock's h2, which is fine) | Pass | Unknown | Pass |
| **Lists** use `ul/ol/dl` | Pass (sampled) | Pass | Pass | Pass |
| **Controls** `a` for links | Pass | Pass | Pass | Pass |
| Links recognisable without colour | Unknown (most inline links are underlined, e.g. `MapCredit`; check the themed `text-primary` links) | Pass | Pass (notice links underlined) | N/A |
| Controls have `:focus` | Gap (see Keyboard) | Pass | Gap | Pass |
| `button` for buttons | Gap (minor): clickable `<tr onClick>` with no key handler (`AbilityCompendiumTab.tsx:394-403`; same pattern in `NpcCompendiumTab.tsx`, `ItemCompendiumTab.tsx`). Each row also has a View/Edit link, so a keyboard path exists | Pass | Gap (as web) | Pass |
| Skip link | Gap: none anywhere (`layouts/main-layout/MainLayout.tsx`) | Gap: Nav plus ~6 hero token buttons come before `<main>` (`App.tsx:21-22`, `Hero.tsx:479`) | Gap | Pass (short page; not needed) |
| New-tab links identified | Gap: `components/world/MapCredit.tsx:25,35,45`, `PlayDock/ActorsPanel.tsx:451,576`, `game-systems/legal/SystemLegalNotice.tsx:68`, `feedback/MySubmissions.tsx:101`, `navigation/AboutInstance.tsx:43`, `pages/admin/TelemetryPanel.tsx:115` | Pass (no `_blank`) | Gap: also `apps/demo/src/DemoNotice.tsx:98,107,116` | N/A |
| **Tables** `table` for tabular data | Pass | N/A | Pass | N/A |
| `th` with `scope` | Gap: compendium and scene tables (`ScenesPage.tsx`, `compendium/{Ability,Item,Lore,Npc}CompendiumTab.tsx`) use `th` without `scope`. `components/ui/data-table/DataTable.tsx:82-88` and the admin tables do it right | N/A | Gap | N/A |
| `caption` | Gap: same tables; only `DataTable.tsx:82` has an sr-only caption | N/A | Gap | N/A |
| **Forms** every input labelled | Gap: placeholder-only inputs at `components/world/DiceRollerPanel/DiceRollerPanel.tsx:94` (Play), `pages/world/ability/AbilityEffectEditor.tsx:252,259`, `scenes/ScenesPage.tsx:187`, `compendium/LoreCompendiumTab.tsx:189`, `item/ItemEffectEditor.tsx:211,218`, `compendium/BookListTab.tsx:884,891,898`, `components/world/SessionSetupInviteLink.tsx:63`; unlabelled per-member role `<select>` at `players/PlayersPage.tsx:458` | Pass (`DiceTray.tsx:127`) | Gap (as web) | Pass (wrapping labels) |
| `fieldset`/`legend` | Unknown (5 uses; role-based radiogroups are used instead in the Play tools) | N/A | Unknown | Pass |
| `autocomplete` | Pass (auth: `LoginView.tsx:554,574,661`, `RegisterPage.tsx:138-181`) | Pass (`autoComplete="off"` on the formula field) | Pass | Pass |
| Errors listed after submit | Gap: Register shows its error as an info-styled `StatusBadge` after the submit button, with no alert role (`auth/RegisterPage.tsx:72,98-100,200`) | N/A | Gap | Pass (`role="alert"` list, `App.tsx:181`; `HeroBuilder.tsx:260`) |
| Errors tied to inputs | Gap (same, and generally: `aria-invalid` appears 9 times and `aria-describedby` 7 times across the app) | N/A | Gap | Unknown (problems carry `data-field` but no `aria-describedby`) |
| Status not by colour alone | Unknown (StatusBadge variants include text; check HP bars and condition chips) | Pass | Unknown | Pass |
| **Media** no autoplay / controls / pausable | N/A (no `<video>`/`<audio>` found) | N/A | N/A | N/A |
| **Video** captions / seizure triggers | N/A; the board's own flashes (light flicker exists in the engine) are Unknown | N/A | N/A | N/A |
| **Audio** transcripts | N/A | N/A | N/A | N/A |
| **Appearance** specialised modes (forced colours) | Unknown (no `forced-colors` handling found in any app) | Unknown | Unknown | Unknown |
| 200% text | Unknown (Play's fixed 22rem dock with `max-w-[85vw]`, `WorldDock.tsx:74`; demo's fixed bottom notice) | Unknown | Unknown | Unknown |
| Proximity of related content | Pass (sampled) | Pass | Pass | Pass |
| Not colour alone | Unknown (board token state is canvas-only) | Pass | Unknown | Pass |
| Instructions not visual-only | Gap: no keyboard guide for the board. `docs/guides/playing-on-a-touchscreen.md` covers touch; `PlayDock/HelpPanel.tsx` lists no keys; the engine binds arrows plus O, C, Delete (`crates/thunderforge-engine/src/systems/{selection,wall,shape}.rs`) | Pass (token labels say "Arrow keys move one square") | Gap | Pass |
| Simple, consistent layout | Pass | Pass | Pass | Pass |
| **Animation** subtle, no flashing | Unknown (board) | Pass | Unknown | Pass |
| Pause background video | N/A | N/A | N/A | N/A |
| Honour `prefers-reduced-motion` | Pass, with gaps: honoured by dice (`engine/bevy/diceThrow.ts:79`), camera (`engine/lookAt.ts:36`, engine `SetReducedMotion` in `crates/thunderforge-engine/src/sdk.rs:345`), `BoardLoading.tsx:98`, `EngineLoader.tsx:144`. Not guarded: Radix dialog `animate-in/zoom-in` (`components/ui/dialog.tsx:62`), skeleton `animate-pulse` (`components/ui/skeleton.tsx:7`, `StatusPage.tsx:270`) | Pass (`styles.css:56,389,796,1100`) | as web | Unknown (no motion found) |
| **Contrast** text 4.5:1 / large text 3:1 | Pass for theme tokens: enforced by `crates/thunderforge-pack-system-spec/src/contrast.rs` (spec 032 FR-012a) on interface packs and, per `globals.css:136-142`, on the base theme. Gap for hard-coded colours: `TokenPanel.scss` placeholder `#a89968` on `#fffaee` = 2.72:1 | Pass (spot checks: `--text-soft` 6.42:1, `--ink-red` 4.62:1) | as web | Pass (muted 7.75:1, danger 7.48:1) |
| Icons 3:1 | Unknown (`text-muted-foreground` icons on `bg-background/95` over the canvas) | Pass | Unknown | Pass |
| Input borders 3:1 | Gap: `TokenPanel.scss:50-60` border `#a89968` on `#fffaee` = 2.72:1. Theme `--input` is Unknown | Pass | as web | Gap: `--tfhb-line #4a4257` = 1.80:1 against the page and 1.57:1 against the field (`packages/hero-builder/src/styles.ts:13`, `apps/hero-builder/src/App.tsx:214,219`) |
| Text over images/video | Unknown (Play overlays use `bg-background/95` with blur over the map; probably fine) | Pass | Unknown | N/A |
| `::selection` contrast | Pass (no custom selection) | Pass (`styles.css:85`, ink-black on a light blue tint) | Pass | Pass |
| **Mobile** any orientation | Pass (no orientation lock found) | Pass | Pass | Pass |
| No horizontal scroll | Pass, mostly: the world hub was fixed deliberately (`WorldSectionShell.tsx` comment); Play is full-screen. Unknown elsewhere | Pass (`overflow-x:hidden`) | Unknown | Pass (`flex-wrap`) |
| Easy-to-activate icon controls | Gap: button sizes default 32px, sm 28px, xs/icon-xs 24px, with no coarse-pointer bump (`components/ui/button.tsx:23-34`). `RollAdvantage/AdvantagePicker.tsx:31-38` is about 20px tall with adjacent targets, which fails WCAG 2.5.8 AA. Dock and rail buttons are 36px | Pass | Gap (as web) | Pass (44px minimum, `App.tsx:213,218`) |
| Space between interactive items | Gap (same AdvantagePicker; `gap-1` rails) | Pass | Gap | Pass |

---

## 2. Findings ranked by user impact

### Blocker

**B1. web/demo: you cannot select a token on the board with the keyboard, so you cannot play with it.** Checklist: Keyboard (2.1.1).
- `crates/thunderforge-engine/src/systems/selection.rs:181-200`: arrow-key movement needs a token that is already selected, and selection is pointer-only. The only DOM path that selects a token is the stack picker, `apps/web/src/pages/world/WorldPage.tsx:557`, which itself opens from a canvas click. The Shift+F10 / ContextMenu-key menu opens on the selected token (`components/world/CanvasContextMenu/useCanvasContextMenu.ts:97-108`), so it is also out of reach. The combat tracker and attack flow do work by keyboard (credit: `e2e/combat-accessibility.spec.ts`).
- Fix: add a "Tokens in this scene" list (dock section or roving listbox) that calls `facets.selection.selectOne`, and/or Tab/`[`/`]` cycling in the engine. The landing hero (`apps/landing/src/sections/Hero.tsx:479-519`) is already a working model.

### Serious

**S1. web/demo Play: the window-level key forwarder swallows Space, arrows, PageUp/PageDown, Home and End for the whole dock.** Checklist: Keyboard / Controls (2.1.1, 2.4.7).
- `apps/web/src/engine/canvasKeyboard.ts` (`SCROLL_KEYS` and `startCanvasKeyboardRouting`) listens in the capture phase on `window`. It exempts only text fields and menus/dialogs, then calls `preventDefault()` and re-dispatches the key to the canvas. Consequences: Radix arrow-key navigation (tabs, radio and toggle groups, which skip their handlers when `defaultPrevented`) stops working; the dock panel and chat feed cannot be scrolled with keys; Space on a focused dock button probably does not activate it (confirm at runtime); and each such key also moves the selected token.
- Fix: forward only when `document.activeElement` is `body` or the canvas, or also exempt interactive targets (`button, a, [role=tab|radio|option|slider], [tabindex]`, scroll containers).

**S2. web/demo: the board canvas has no accessible name, no role, sits outside every landmark, and has no text alternative.** Checklist: Images, complex images (1.1.1); Global, landmarks.
- Bevy inserts `<canvas>` as the last child of `<body>`, and `pages/world/useEngineCanvas.ts:48-62,95-130` sets only its style. Nothing tells a screen-reader user what scene they are on, what tokens are in it, or where they are. The dock's Actors panel is a partial alternative.
- Fix: when the canvas appears, give it `role="application"` (or `img`) plus `aria-label="Board: <scene>"` and `aria-describedby` pointing at an sr-only DOM summary (scene, selected token, its square, nearby tokens), updated from the world store.

**S3. web/demo Play: chat messages and dice rolls are not announced.** Checklist: Forms/Controls status messages (4.1.3).
- `components/world/PlayDock/ChatPanel.tsx:180-183`: the feed is a plain scrolling `div` with no `role="log"` or `aria-live`. By contrast, the attack log and offers do announce (`AttackLog.tsx:92`, `OfferPrompt.tsx:90`).
- Fix: `role="log" aria-live="polite" aria-label="Chat"` on the feed, or an sr-only announcer for new entries.

**S4. web/demo: Register form errors are invisible to assistive tech and not tied to their fields.** Checklist: Forms, errors (3.3.1).
- `pages/auth/RegisterPage.tsx:72` ("Passwords do not match.") and `:98-100` render through `StatusBadge` at `:200` in its default `info` variant, after the button, with no `role="alert"`, `aria-invalid` or `aria-describedby`. (The login view has a live region, `LoginView.tsx:854`.)
- Fix: render the status with `role="alert"` and the danger variant, and set `aria-invalid` plus `aria-describedby` on the confirm-password field.

### Moderate

**M1. web/demo/landing: no skip link.** Controls (2.4.1). `layouts/main-layout/MainLayout.tsx:13-15`; in landing, Nav plus about 6 hero token buttons come before `<main>` (`apps/landing/src/App.tsx:21-22`). Fix: add a "Skip to content" link that shows on focus, targeting `<main id>`.

**M2. web/demo: nested `<main>` landmarks on about 25 pages.** Global, valid HTML / landmarks. `MainLayout.tsx:15` wraps pages that render their own `<main>` (`WorldListPage.tsx:145`, `WorldDashboardPage.tsx:147`, `WorldCompendiumPage.tsx:152`, `LegalDocumentPage.tsx:63`, `CreateWorldPage.tsx:224`, …). The axe fixture leaves out best-practice rules (`apps/web/e2e/fixtures/axe.ts:31-37`), so `landmark-no-duplicate-main` is never run. Fix: make the page wrappers `<div>`, or make MainLayout's a `<div>`.

**M3. web/demo: the Play view has no `h1` and no `main`.** Headings / landmarks. `pages/world/WorldPage.tsx:494-`, `layouts/world-layout/WorldLayout.tsx:37`. Fix: an sr-only `<h1>{world} · {scene}</h1>`, with the canvas host and overlays inside `<main>`.

**M4. web/demo: inputs labelled only by a placeholder.** Forms (1.3.1, 3.3.2). `DiceRollerPanel.tsx:94` (in Play), `AbilityEffectEditor.tsx:252,259`, `ScenesPage.tsx:187`, `LoreCompendiumTab.tsx:189`, `ItemEffectEditor.tsx:211,218`, `BookListTab.tsx:884,891,898`, `SessionSetupInviteLink.tsx:63`; unlabelled role select at `PlayersPage.tsx:458`. Fix: add `aria-label` or a visible `<label>`.

**M5. web/demo: focus indicator removed on native selects.** Keyboard (2.4.7). `ActorInventoryPanel.tsx:290`, `ActorLorePanel.tsx:268`, `ItemEffectEditor.tsx:135,173,202,230`, and the 0.1-opacity ring in `TokenPanel.scss:363-366`. Fix: copy the `focus-visible:ring-3 focus-visible:ring-ring/50` classes from `PlayersPage.tsx:464`.

**M6. web/demo: small touch targets.** Mobile (2.5.8 AA / 2.5.5). `components/ui/button.tsx:23-34` (24–32px, no `pointer: coarse` bump); `RollAdvantage/AdvantagePicker.tsx:31-38` is about 20px tall with targets touching (fails AA). Fix: give the picker a 24px minimum height now; use `@media (pointer: coarse)` to raise buttons to 44px.

**M7. web/demo: hub routes have no unique title.** Global (2.4.2). `ScenesPage.tsx`, `SceneDetailPage.tsx`, `PlayersPage.tsx`, `WorldCompendiumPage.tsx`, `SignUpPage.tsx` have no `<SEO>`. Fix: add `<SEO title=…>`.

**M8. landing: the GitHub "Star" link can lose its name on narrow screens.** Controls (4.1.2). `apps/landing/src/Nav.tsx:27-35` with `styles.css:293-296`: under 900px the "Star" text is `display:none`, the icon is `aria-hidden`, and the name then rests on an `aria-label` on a `<span>` that exists only once the star count has loaded. If the fetch fails, the link has no name. Fix: `aria-label="ThunderForge on GitHub"` on the `<a>`.

**M9. hero-builder: control borders fail non-text contrast.** Contrast (1.4.11). `--tfhb-line #4a4257` is 1.80:1 against the page and 1.57:1 against the field (`packages/hero-builder/src/styles.ts:13`; `apps/hero-builder/src/App.tsx:214,219`). Fix: lighten the line colour to about `#7a7088` (≈3.4:1 against `#1d1a24`).

**M10. demo: the fixed bottom notice may cover focused controls.** Keyboard (2.4.11 Focus Not Obscured, AA). `apps/demo/src/DemoNotice.tsx` `<aside>` is `fixed bottom-2 z-50`, 12px text, and wraps at zoom. Fix: reserve bottom padding equal to its height, or make it collapsible; confirm at runtime.

### Minor

- **m1 web:** clickable `<tr>` rows with no key handler and invalid `aria-selected` (`AbilityCompendiumTab.tsx:394-403`, and the NPC and Item tabs); compendium and scene tables lack `scope` and `caption`. Fix: use the existing `DataTable` (which has a caption and `scope`), or drop the row click and keep the link.
- **m2 web/demo:** new-tab links are not identified (files listed in the matrix). Fix: an sr-only "(opens in a new tab)" and an icon.
- **m3 web:** custom `tablist` and `radiogroup` widgets have no arrow-key roving (`LevelTabs/LevelTabs.tsx:75-90`, `WallTool.tsx:118-131`, `AdvantagePicker.tsx:22-31`). They still work with Tab and Enter. Fix: roving `tabIndex` plus arrow handlers, or plain `aria-pressed` buttons.
- **m4 web:** information carried only in a `title` tooltip (`WallTool.tsx:131` hint; GM-only badge `title`). Fix: visible text or `aria-describedby`.
- **m5 web/docs:** board keyboard controls are not documented anywhere (no keyboard guide in `docs/guides/`, nothing in `HelpPanel.tsx`). Fix: `docs/guides/playing-with-a-keyboard.md` plus a "Keys" list in Help.
- **m6 web:** `TokenPanel.scss` placeholder and border `#a89968` on `#fffaee` = 2.72:1. Fix: darken to about `#7d6f45`.
- **m7 web:** dialog zoom/fade and skeleton pulse ignore reduced motion (`ui/dialog.tsx:62`, `ui/skeleton.tsx:7`). Fix: `motion-safe:` prefixes or a global reduced-motion rule.
- **m8 hero-builder:** colour swatches are named by hex value (`controls/ColorControl.tsx:59`, "Skin #a87b5d"). Fix: add a human colour name.

### Severity counts per app

| App | Blocker | Serious | Moderate | Minor |
|---|---|---|---|---|
| web | 1 | 4 | 7 (M1–M7) | 7 (m1–m7) |
| demo (inherits web, plus M10; m2 also covers its own links) | 1 | 4 | 8 | 7 |
| landing | 0 | 0 | 2 (M1, M8) | 0 |
| hero-builder | 0 | 0 | 1 (M9) | 1 (m8) |

---

## 3. What existing tooling guards, and what it cannot see

**Already in place (credit):**
- **axe-core in Playwright**: `apps/web/e2e/fixtures/axe.ts` checks WCAG 2.0/2.1/2.2 A+AA tags and waits for animations to finish. It is called in 27 spec files with a scope selector: sheets (`dnd5e-sheet`, `genie-sheet-layout`, `system-roll-for-shoes`), `worlds-list` (`main`), `world-dashboard-statistics`, `actor-art`, `abilities-vocabulary`, `board-loading`, play-pause notices and dialogs, `combat-accessibility`, `canvas-context-menu`, `look-at-and-follow`, `players-section`, `token-links`, `npc-visibility`, `system-conditions`, `system-settings`, `admin-instance-panels`, and three journeys.
- **Keyboard-only combat e2e**: `apps/web/e2e/combat-accessibility.spec.ts` drives the fight's controls by keys, checks focus return and `:focus-visible` rings.
- **Contrast floor for themes**: `crates/thunderforge-pack-system-spec/src/contrast.rs` rejects interface packs (`packs/interface/*/interface.json`) below 4.5:1 for text and 3:1 for UI. According to `styles/globals.css:136-142` it also caught the base focus ring.
- **Reduced motion**: dice, camera fly-to and the engine's `SetReducedMotion` (`crates/thunderforge-engine/src/sdk.rs:345`), loaders, and four landing CSS blocks.
- **Canvas keyboard support that exists**: arrows move the selected token (`selection.rs:181`), Shift+F10 / ContextMenu opens the token menu, the stack picker is a `listbox`, and selection count and connection status are live regions (`SelectionCount.tsx:26`, `ConnectionStatus.tsx:125`, `SelectionBar.tsx:68`).
- **Touch**: `docs/guides/playing-on-a-touchscreen.md` and the `limited-device` demo spec.

**Blind spots:**
- **No `eslint-plugin-jsx-a11y`** in `apps/web/eslint.config.mjs`, and no linting in landing or hero-builder. The placeholder labels, clickable rows and missing `scope` would all have been flagged at lint time.
- **axe runs only on scoped regions**, so most hub pages (Scenes, Players, Compendium tabs, item/ability editors, auth) and the Play dock as a whole are never scanned. That is exactly where M4 and M5 live.
- **Best-practice rules are excluded** on purpose (`axe.ts:12-15`), so duplicate or nested `main`, missing `h1`, `region` and skip-link rules never fire.
- **No axe in `apps/landing/e2e`, `apps/demo/e2e` or `apps/hero-builder/e2e`.**
- **axe cannot see the canvas.** Token names, positions, HP and walls exist only as pixels, and no automated tool covers B1 or S2.
- **axe cannot catch the key-forwarder problem (S1).** It needs a keyboard e2e inside Play that is not about combat (Tab into the dock, arrow through a radiogroup, Space on a button, then check the token did not move).
- **The contrast checker covers theme tokens only.** Hard-coded hex values in SCSS, inline styles (`DiceRollerPanel.tsx` uses inline styles) and the hero-builder palette are outside it, and so are alpha-composited surfaces (`bg-background/95` over the map).
- **Forced-colors and 200% zoom** are not covered by any test.

---

## 4. Needs a runtime / browser check

1. S1: in Play, focus a dock button and press Space; focus a Radix tabs or radio group in the dock and press the arrows. Does it activate or move, and does the selected token move?
2. Whether winit gives the canvas a `tabindex` and what a screen reader announces on reaching it (NVDA/VoiceOver).
3. Theme contrast of `text-muted-foreground`, `--input` borders and icons on `bg-background/95` composited over a bright map, in light and dark themes and with each interface pack.
4. 200% text zoom and a 320px-wide reflow for the Play dock (22rem panel), the demo notice, and the world hub sidebar strip.
5. The demo notice covering the focused element at the bottom of the viewport (2.4.11).
6. Windows High Contrast / `forced-colors`: do the focus rings drawn as `box-shadow` (`styles/mixins.scss:3-9`, `TokenPanel.scss`) disappear? They will under forced colours unless an outline is kept.
7. Board animation: does light flicker or any engine effect exceed three flashes per second, and does reduced motion also calm token-move tweens and light flicker?
8. Heading order and the accessible names of repeated "View"/"Edit" links in compendium rows.
9. Whether any role or status chips (HP bars, conditions, connection) rely on colour alone.
10. A full-page axe run, with best-practice rules on, over Scenes, Players, Compendium, Register, landing and demo, to turn the Unknowns in the matrix into Pass or Gap.
