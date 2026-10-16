# Research: A Board That Fits a Phone

Every open question from the plan's Technical Context is resolved here. The
first section is impeccable's `shape` brief (Operate mode, PRODUCT.md loaded,
no DESIGN.md: the incumbent world is preserved, not replaced). Discovery was
the owner's answers on 2026-10-16, so no further interview round was needed.

## Design brief

**Job and audience.** A player at a table, or on a couch, holding a phone in
one hand, mid-session. A Game Master running the same session from a phone
because they are away from their desk. Both are in a task; the map is the
thing they are looking at. Operate mode: the tool disappears into the task.

**Outcome and proof.** A player moves their token, rolls, reads their sheet
and follows chat and the turn without hunting. A Game Master reaches every
tool. Proof is measured, not felt: map share at 412 × 915 (≥ 85% player,
≥ 80% GM), zero overlapping controls, every control ≥ 44 × 44, and the
owner's own hands on their phone.

**Selected direction.** *The map is the screen; the chrome is a handful of
stones on it.* Inside the incumbent world (dark `bg-background/95` panels,
`FantasyIcon` glyphs, restrained colour, the primary colour reserved for the
armed or open thing):

- No bars. Each control is its own floating tile: 56 × 52, rounded-2xl,
  `bg-background/85` with backdrop blur, a 1px `border-border` edge and a
  soft shadow, icon over an 11px label. The tile carries its own contrast, so
  it reads on a white parchment map and a black dungeon alike (FR-002a).
- One row of five tiles at the bottom, centred, gapped, above the safe-area
  inset: the thumb zone. Labels under icons, because five unlabelled glyphs
  are a guessing game on first use and the row has the width for them.
- Panels are sheets from the bottom: half height by default, a grab handle
  and an expand control for full height, the live map visible above. One
  sheet at a time.
- The armed tool is a small chip at top-left ("Walls ▾ ✕"); the options are
  a tap on the chip, leaving is a tap on ✕. Select needs no chip: it is the
  resting state.
- The turn, in combat, is one quiet line above the row ("Round 2 ·
  Boblin's turn"), tappable into Combat.

Focal moment: opening the board and seeing *only the map and five stones*.
Implementation consequence: the phone chrome is a separate arrangement
component fed by the page's existing state, not a pile of media queries on
the desktop rails.

**Scope and boundaries.** The `/world/:id/play` page, in the app and the
demo, below the phone threshold. Untouched: desktop and tablet layout, the
canvas, gestures, the world's other pages, panel *contents* (Chat, Combat …
render inside a sheet as they render inside the dock). Anti-goals: a
hamburger menu; a full-width black tab bar (the owner's "black bars"); icon
buttons with no names; a modal that blanks the map; motion for its own sake.

**States and ranges.** Nothing open; a sheet half; a sheet full; a tool
armed; combat running; a request waiting (badge); paused play; connection
lost; engine loading or stopped; the demo notice; on-screen keyboard open;
landscape (row moves to the right edge as a column, still in thumb reach of
the right hand); text at 200%. A player with zero, one or several characters;
a player granted authoring tools; a GM with every tool.

**Interaction and layout.** Tap a tile → its sheet opens at half (tap again
or swipe down → closed). Drag the handle up or tap expand → full. System
back → sheet closes first. Tiles show pressed/open state with the primary
colour; 150–200 ms transitions, none under `prefers-reduced-motion`.

**Constraints and open decisions.** WCAG 2.2 AA; Chromium the gate. The
owner's try-out of the first slice may change: tiles vs. one connected pill,
label on/off, which five buttons a GM gets. The builder must not invent
further buttons or move a GM tool out of reach.

## Decisions

### R1. Which layout: size, read live

- **Decision**: Phone layout when the viewport's shorter side is under 600
  CSS px: `matchMedia("(max-width: 599px), (max-height: 599px)")`, read
  through `useSyncExternalStore` in `usePhoneLayout()`, so a rotation or a
  resized window switches at once.
- **Rationale**: The problem is room, not input. Every phone in either
  orientation is under 600; the smallest common tablet portrait (744) is
  over. Touch-specific sizing does not hang off this: tiles are 44+ px in the
  phone layout whatever the pointer.
- **Alternatives**: `pointer: coarse` alone (impeccable's input-not-size
  rule) would give a touch laptop the phone layout and a phone with a mouse
  the desktop one; rejected for choosing the *layout*, kept in mind for
  hover-only affordances (none are added). User-agent sniffing: rejected.

### R2. One state, two arrangements

- **Decision**: `WorldPage` keeps building the same `tools` (via
  `railTools`) and `dockSections` (via `useDockSections`) and the same
  `openGmToolId` / `openDockSectionId` state, then renders either the rails
  and dock or `<PhoneBoard>` with those props.
- **Rationale**: FR-003 for free: crossing the threshold changes the
  arrangement, never the state. Permissions are applied once, upstream, so
  the phone cannot show something the rail would not (Principle III).
- **Alternatives**: CSS-only responsive rails (cannot become a bottom row
  plus sheets without duplicating markup anyway); a separate phone page
  (two information architectures, forbidden by impeccable's adapt rules).

### R3. Armed tool vs. its options

- **Decision**: On the phone, "which tool is armed" stays the page's
  `effectiveGmToolId`; "are its options showing" is a PhoneBoard-local
  boolean. Arming a tool from the Tools sheet opens its options; closing the
  options leaves the tool armed and the chip showing. Select is armed with
  options closed on load (FR-016).
- **Rationale**: Today one id means both, which is why the Select filter
  opens on load (`reconcileOpenTool` falls back to the first permitted tool).
  Desktop behaviour is untouched.

### R4. The sheet

- **Decision**: `BoardSheet` on the existing `radix-ui` Dialog with
  `modal={false}`: focus moves into it on open and returns to its tile on
  close (FR-030), Escape closes, but the map above stays interactive and
  live. Two sizes, `half` (≤ 60% of the height, FR-012) and `full`, switched
  by the handle (pointer drag with a threshold) or an expand button. Opening
  pushes one `history` entry; `popstate` closes the sheet (FR-014).
- **Rationale**: No new dependency; Radix already gives focus return and
  Escape, and the app's `sheet.tsx` shows the pattern.
- **Alternatives**: `vaul` (adds a dependency for snap points we need two
  of); the app's modal `Sheet` (blanks and blocks the map, against FR-012).

### R5. The keyboard and the browser's own chrome

- **Decision**: Viewport meta gains `viewport-fit=cover,
  interactive-widget=resizes-content` (both `index.html`s). The board uses
  `h-dvh` instead of `h-screen`, and tiles and sheets pad by
  `env(safe-area-inset-*)`.
- **Rationale**: `resizes-content` makes Chromium shrink the layout viewport
  when the keyboard opens, so a bottom-anchored sheet rises with it
  (FR-015) without script. `dvh` stops the row from sitting under a
  collapsing address bar. `viewport-fit=cover` is what makes the safe-area
  insets non-zero (FR-019). None of these change desktop rendering.
- **Alternatives**: a `visualViewport` listener moving the sheet by hand
  (kept as a fallback only if Firefox fails the owner's check).

### R6. Which buttons for whom

- **Decision**: A pure `phoneRows(role, permittedTools, sections)` returns
  the row and the grouped sheets:
  - Player row: **Me**, **Dice**, **Sheet**, **Chat**, **More**. More holds
    the remaining permitted sections (Actors, Combat, the system's own,
    Settings, About & feedback), any authoring tools the GM granted, and
    full screen.
  - GM row: **Tools**, **Dice**, **Actors**, **Chat**, **Combat**. Tools
    holds *Board* (Select, Walls, Lights, Shapes, Tokens, Interactions,
    Levels, the token tray) and *Table* (Player requests, Settings, the
    system's own, About & feedback, full screen). Requests badge the Tools
    tile (FR-011).
  - Trusted Player = player.
- **Rationale**: Unit-testable without a browser; one place to change when
  the owner's try-out moves a button. A GM has no single "my sheet", so
  Actors takes that slot.

### R7. Me, Sheet, and the turn

- **Decision**: **Me** finds the claimed actor's token on this scene
  (`useScenePlay`'s `claimedActorId` → the scene's tokens by `actorId`),
  selects it and calls `lookAtToken`; several characters → a small chooser
  sheet; none on this scene → the button opens the Sheet instead and says
  so. **Sheet** renders `InPaneCharacterSheet` for the claimed actor in a
  `BoardSheet`. **TurnLine** reads a new `useActiveCombat(worldId)` hook,
  lifted out of `CombatPanel` (`getActiveCombat` + `subscribeToWorldEvents`)
  so the line stays live while the Combat panel is closed.

### R8. Dice, token tray, levels, banners, demo notice

- **Decision**:
  - The Dice tile opens `DiceRollerPanel` inside a half sheet; results show
    as they do today.
  - The token tray toggle moves into Tools (GM) / More (player, when the
    tray is theirs to use).
  - Level tabs: a compact current-level chip at top-centre when the scene
    has more than one level; managing levels is in Tools.
  - Every top-of-screen notice (taken-over, revoked tool, paused, connection)
    and the chips render in one top stack (a flex column under the safe
    area), so none can lie on another (FR-017, FR-020); the table feed sits
    above the row.
  - The demo notice becomes a small chip in that top stack on a phone; its
    view switch works as before.

### R9. Installable, no offline

- **Decision**: `apps/web/public/manifest.webmanifest` — `name`
  "ThunderForge", `short_name` "ThunderForge", `start_url` "/", `scope` "/",
  `display` "standalone", `background_color` / `theme_color` "#0a1018"
  (matches the existing `theme-color` meta), icons 192 and 512 PNG plus a
  512 maskable and the SVG. Linked from `apps/web/index.html` with
  `apple-touch-icon`. PNGs are rendered once from `brand-mark.svg` by
  `scripts/brand-icons.mjs` (Playwright, already installed; no image tool is
  on the machine) and committed. The demo gets no manifest (assumption).
- **Rationale**: Chromium's install criteria are https, a manifest with
  name, start_url, display and 192/512 icons; a fetch handler is no longer
  required. `public/sw.js` stays the tombstone it is: it caches nothing, and
  FR-027 keeps it that way.
- **To verify in tasks**: the Rust server serves `.webmanifest` with
  `application/manifest+json`; no CSP on the app blocks `manifest-src`.

### R10. Full screen

- **Decision**: `FullScreenButton` shows when `document.fullscreenEnabled`
  and the display mode is not already `fullscreen`; it calls
  `document.documentElement.requestFullscreen()` and toggles its label on
  `fullscreenchange`. It lives in More / Tools. iPhone Safari reports
  `fullscreenEnabled` false, so it hides there (FR-028).

### R11. Not loading phone code on desktop

- **Decision**: `PhoneBoard` is `React.lazy`-loaded the first time the phone
  layout is chosen. The rails are not lazy (they are the desktop default).
