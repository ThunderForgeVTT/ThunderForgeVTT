# Feature Specification: Scene Management Overhaul

**Feature Branch**: `022-scene-management-overhaul`

**Created**: 2026-08-24

**Status**: Draft

**Input**: User description: "Scene management overhaul: (1) On the world System Settings page, rename the 'Change system'/'Assign a system' card heading to 'System Settings'; the existing game-system picker becomes a labeled field 'Change System' within that card. Add a new 'Default Scene Grid Type' dropdown to that same System Settings page with options None, Squares, Hexagons, persisted at the world level; every newly created scene defaults to the world's configured grid type. (2) Move scene management out of the Session Setup / Staging page entirely into its own dedicated 'Scenes' section (its own tab/nav destination, alongside NPCs/Lore/Items/Abilities in the world sidebar nav). (3) In the new Scenes section, a GM can: create a new scene, import a map from a dd2vtt (DungeonDraft VTT export) file, write a Markdown summary/description for the scene using the same Markdown edit/view editor used for Lore entries, and toggle a 'hidden' switch per scene to control player visibility. (4) Players (non-GM) see a table of all non-hidden scenes; clicking a row opens a detail view showing the scene's Markdown summary and a small (roughly 1/16 scale) thumbnail image of the map — this requires building server-side image thumbnail generation for imported/uploaded scene map images."

## Clarifications

### Session 2026-08-24

- Q: Now that launching a scene from the Scenes section is what starts/switches Play, what should the existing top-right "Play" button do when no scene has been launched yet? → A: Play is always accessible (never disabled/hidden). Which scene is loaded is driven purely by scene selection — a GM can change scenes mid-game either via the existing on-canvas scene controls inside Play, or via the Scenes section's Launch action. When nothing has been explicitly loaded yet, Play opens to an empty/unloaded canvas rather than an error or a disabled button; the actual scene/tokens/walls/lights are loaded and unloaded by the existing canvas asset-loading pipeline in response to which scene is currently selected.
- Q: When a GM creates a new scene, should it start out hidden from players by default, or visible by default? → A: Hidden by default — the GM explicitly un-hides a scene once it's ready to be seen.
- Q: When a scene's grid type is set to "None," should the grid be purely invisible, or also disable snapping/grid-unit measurement? → A: No grid at all — no lines, no snapping, free-form placement; measuring tools fall back to measuring in raw pixels instead of grid units.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - GM manages scenes from a dedicated Scenes section (Priority: P1)

A GM currently manages scenes from a scene-switcher tucked into the Session Setup page, with no room to import a map, describe it, or control who sees it. The GM needs a real home for scene management: its own section in the world, reachable like NPCs/Lore/Items/Abilities, where they can create a scene, import a dd2vtt map file into it, write a Markdown summary of what the scene is, and mark it hidden while it's still being prepared.

The only *play* action available on a scene from this section is "Launch" — functionally identical to the existing Play button, just targeted at that one scene. There is no separate "select active scene" control: launching a scene from the Scenes section is how the GM chooses what's being played, replacing the old Session-Setup scene picker. If the world is already in an active Play session when the GM launches a different scene, that switch takes effect immediately for everyone currently in Play — their view unloads the previous scene and loads the newly launched one, with no separate rejoin step required.

**Why this priority**: This is the foundational slice — every other story (default grid type, player browsing) depends on scenes having a real home with a summary and a visibility flag. Without this, there's nothing for the other stories to build on.

**Independent Test**: Can be fully tested by a GM navigating to the new Scenes section, creating a scene, importing a dd2vtt file into it, writing and saving a Markdown summary, and toggling it hidden/visible — all without touching Session Setup or any other feature.

**Acceptance Scenarios**:

1. **Given** a GM viewing a world, **When** they open the Scenes section from the world's navigation, **Then** they see a list of the world's scenes with the option to create a new one.
2. **Given** the Scenes section, **When** the GM creates a new scene and gives it a name, **Then** the scene is created and appears in the list.
3. **Given** an existing scene, **When** the GM imports a valid dd2vtt map file into it, **Then** the scene's map/background is updated to the imported map.
4. **Given** an existing scene, **When** the GM writes a summary using the Markdown editor and saves, **Then** the rendered summary is shown when viewing that scene thereafter.
5. **Given** an existing scene, **When** the GM toggles the "hidden" switch on, **Then** the scene is marked hidden and (per User Story 2) no longer appears to players.
6. **Given** Session Setup (Staging), **When** a GM or player views it, **Then** no scene-management or scene-selection controls remain there — choosing what's being played happens only by launching a scene from the Scenes section.
7. **Given** the Scenes section with no active Play session underway, **When** the GM clicks "Launch" on a scene, **Then** a Play session starts with that scene loaded, identical to today's Play button.
8. **Given** an active Play session already loaded with scene A and other members currently in Play, **When** the GM clicks "Launch" on scene B, **Then** scene A is unloaded and scene B is loaded for every member currently in Play, without those members needing to manually rejoin or refresh.

---

### User Story 2 - Players browse and preview visible scenes (Priority: P2)

A player wants to know what maps might come up in the next session without spoiling anything the GM has intentionally hidden. They should be able to open the Scenes section, see a clean table of every scene the GM hasn't hidden, and click into one to read its summary and see a small preview image of the map.

**Why this priority**: Delivers the player-facing payoff of User Story 1's data (summary + hidden flag) — it's the second most valuable slice but depends on scenes already having summaries and a hidden flag to filter by.

**Independent Test**: Can be fully tested by a non-GM world member opening the Scenes section, confirming hidden scenes are absent from the table, and clicking a visible row to see its summary and thumbnail.

**Acceptance Scenarios**:

1. **Given** a world with both hidden and visible scenes, **When** a player opens the Scenes section, **Then** the table lists only the non-hidden scenes.
2. **Given** the scenes table, **When** a player clicks a row, **Then** a detail view opens showing that scene's rendered Markdown summary and a small preview image of its map.
3. **Given** a scene with no imported map yet, **When** a player opens its detail view, **Then** the summary is shown without a broken or missing-image error.
4. **Given** a GM toggles a previously-visible scene to hidden, **When** a player who already had that scene's detail view open refreshes or revisits the Scenes section, **Then** the scene no longer appears in their table.

---

### User Story 3 - New scenes default to the world's configured grid type (Priority: P3)

A GM running a grid-free (theater-of-the-mind) or hex-based game is tired of re-selecting the same grid type every time they create a scene. They want to set it once for the world and have every new scene start with that setting.

**Why this priority**: A convenience/consistency improvement on top of scene creation (User Story 1) — valuable but not blocking; a GM can still manually pick a grid type per scene without it.

**Independent Test**: Can be fully tested by setting the world's default scene grid type to a non-default value (e.g. Hexagons), creating a new scene, and confirming the new scene starts with that grid type without the GM selecting it explicitly.

**Acceptance Scenarios**:

1. **Given** a world with no default scene grid type configured, **When** a GM creates a scene, **Then** the scene defaults to the system's standard grid type (Squares), preserving today's behavior.
2. **Given** a GM sets the world's default scene grid type to Hexagons, **When** they create a new scene, **Then** the new scene starts with a Hexagons grid.
3. **Given** a world's default scene grid type is set to None, **When** a GM creates a new scene, **Then** the new scene starts with no grid overlay, no token snapping, and distance-measuring tools report raw pixel distances instead of grid units.
4. **Given** an existing scene created before the default was changed, **When** the world's default scene grid type is later changed, **Then** the existing scene's grid type is unaffected (the default only applies at creation time).

---

### User Story 4 - System Settings page is relabeled and gains the default grid type control (Priority: P4)

The System Settings page's "Change system" card reads as if it's only about the system, when it's really the world's general settings home. A GM needs the heading to reflect that, with the system picker as one labeled control among others — including the new default scene grid type control from User Story 3.

**Why this priority**: Smallest, purely presentational/organizational change; depends on User Story 3 existing to have a second control to place under the renamed heading, so it's last.

**Independent Test**: Can be fully tested by opening System Settings and confirming the card heading reads "System Settings," the system picker has its own "Change System" label, and a "Default Scene Grid Type" control is present alongside it.

**Acceptance Scenarios**:

1. **Given** a GM opens System Settings, **When** the page renders, **Then** the section heading that previously read "Change system" / "Assign a system" now reads "System Settings."
2. **Given** the System Settings page, **When** the GM looks at the system picker, **Then** it is labeled "Change System."
3. **Given** the System Settings page, **When** the GM looks for the grid type control, **Then** they find a "Default Scene Grid Type" control offering None, Squares, and Hexagons.

---

### Edge Cases

- What happens when a GM imports a dd2vtt file that is invalid, corrupted, or not a recognized map format? The import must fail with a clear, actionable message and must not corrupt or partially overwrite the scene's existing map.
- What happens when a GM imports a new dd2vtt map into a scene that already has one? The new map replaces the old one; the scene's summary and hidden state are unaffected.
- What happens when a non-GM member tries to toggle a scene's hidden switch or edit its summary directly (e.g. by guessing a URL)? The action must be rejected — only the GM/Owner may change hidden state or summary.
- What happens when a scene has no summary written yet? Its detail view (for both GM and players) shows an empty/placeholder state rather than an error.
- What happens when a scene's map image can't be reduced to a preview size (e.g. an unusual image format or dimensions)? The detail view shows a graceful placeholder instead of a broken image.
- What happens to a scene actively selected for Play when a GM hides it mid-session? Play continues uninterrupted for the current session; hiding only affects the Scenes section's player-facing table going forward.
- What happens when a GM launches the scene that is already the active one? Nothing changes for anyone in Play; it behaves the same as the existing Play button being clicked again.
- What happens when a non-GM member tries to launch a scene? The action must be rejected — only the GM/Owner may launch/switch the active scene, consistent with Play already being GM-initiated today.
- What happens when a member's connection drops or reconnects during a GM-initiated scene switch? On reconnecting, they must land in the currently-launched scene, not the one that was active when they disconnected.
- What happens when any world member opens Play before a GM has ever launched a scene? Play opens normally to an empty/unloaded canvas state — it is never disabled, hidden, or an error — and starts showing content as soon as a GM launches a scene.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST provide a dedicated "Scenes" section for each world, reachable from the same world navigation that already links to NPCs, Lore, Items, and Abilities.
- **FR-001a**: Every scene MUST be uniquely identified by its own persistent id (the same way worlds and lore entries already are), and MUST have its own dedicated, linkable detail view reachable at a URL built from that id — mirroring the existing per-entity detail-page convention (actors, lore entries, items) — so create/import/summary/hidden/launch all operate on one addressable scene record rather than only through an inline list/modal.
- **FR-002**: System MUST remove all scene creation/management and scene-selection controls from the Session Setup (Staging) page; choosing which scene is active for Play happens exclusively by launching a scene from the Scenes section.
- **FR-002a**: GM/Owner members MUST be able to launch a scene directly from the Scenes section, which starts (or updates) the active Play session with that scene loaded — functionally the same action as the existing Play button, just scene-specific.
- **FR-002b**: When a GM launches a different scene while a Play session is already active, System MUST unload the previously-active scene and load the newly-launched scene for every world member currently in that Play session, without requiring them to manually rejoin.
- **FR-002c**: System MUST reject scene-launch attempts by non-GM/Owner world members.
- **FR-002d**: The Play entry point MUST remain accessible to every world member at all times, regardless of whether a scene has ever been launched; opening it with no scene launched yet MUST show an empty/unloaded canvas state rather than an error or a disabled control.
- **FR-002e**: GM/Owner members MUST be able to change which scene is loaded during an active Play session from within Play itself (existing on-canvas scene controls), in addition to launching a scene from the Scenes section — both are equivalent ways of selecting the active scene.
- **FR-003**: GM/Owner members MUST be able to create a new scene from the Scenes section; a newly created scene MUST start hidden by default, requiring the GM to explicitly un-hide it before it appears to players.
- **FR-004**: GM/Owner members MUST be able to import a dd2vtt map file into a scene, replacing that scene's map/background with the imported one.
- **FR-005**: GM/Owner members MUST be able to write and save a Markdown-formatted summary for a scene, using the same Markdown editing experience already used for Lore entries.
- **FR-006**: System MUST render a saved scene summary as formatted Markdown wherever it is displayed (to both GM and players).
- **FR-007**: GM/Owner members MUST be able to toggle a "hidden" state per scene.
- **FR-008**: System MUST exclude hidden scenes from the scene table shown to non-GM (player) world members.
- **FR-009**: System MUST continue to show all scenes (hidden and visible) to GM/Owner members in the Scenes section.
- **FR-010**: Non-GM (player) world members MUST be able to view a table of all non-hidden scenes in a world.
- **FR-011**: Non-GM (player) world members MUST be able to open a scene's detail view from that table, showing the scene's rendered summary and a reduced-size preview image of its map.
- **FR-012**: System MUST generate a reduced-size preview image (roughly 1/16 the scale of the source map) for a scene's map whenever a map image is imported or uploaded for that scene.
- **FR-013**: System MUST show a graceful placeholder (not a broken image) in the scene detail view when a scene has no map image or no preview image is available.
- **FR-014**: System MUST allow a GM/Owner to set a world-level default scene grid type, offered as None, Squares, or Hexagons.
- **FR-014a**: A scene with grid type None MUST render no grid lines, MUST NOT snap token movement to a grid, and distance-measuring tools on that scene MUST report distances in raw pixels rather than grid units.
- **FR-015**: System MUST apply the world's default scene grid type to every newly created scene in that world, unless the GM explicitly chooses a different grid type at creation time.
- **FR-016**: Changing a world's default scene grid type MUST NOT retroactively change the grid type of scenes already created.
- **FR-017**: System MUST rename the System Settings page's system-assignment section heading from "Change system"/"Assign a system" to "System Settings."
- **FR-018**: System MUST label the existing game-system picker control "Change System" within that renamed section.
- **FR-019**: System MUST reject attempts by non-GM/Owner world members to create scenes, import maps, edit summaries, change a scene's hidden state, or launch a scene, regardless of entry point.
- **FR-020**: System MUST reject a dd2vtt import with a clear error message when the uploaded file is not a valid map export, without altering the scene's existing map, summary, or hidden state.

### Key Entities

- **Scene**: A world's playable map/location. Gains, beyond what exists today (name, description, grid settings, map/background image): a Markdown-formatted summary, and a hidden flag controlling player-facing visibility. Every scene belongs to exactly one world.
- **Scene preview image**: A reduced-size rendition of a scene's map image, generated for use in the player-facing detail view, distinct from the full-resolution map used during Play.
- **World scene settings**: A world-level configuration (alongside the world's assigned game system) holding the default grid type applied to newly created scenes.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A GM can create a scene, import a map, write a summary, and set its visibility entirely from the Scenes section, without visiting Session Setup, in under 2 minutes.
- **SC-002**: Players never see a scene the GM has marked hidden, across 100% of visits to the Scenes section.
- **SC-003**: A player can find and preview any visible scene's map and summary in two clicks or fewer from entering the world (open Scenes section, click the scene row).
- **SC-004**: Once a world's default scene grid type is set, 100% of scenes subsequently created in that world start with that grid type without the GM re-selecting it.
- **SC-005**: Scene preview images load noticeably faster than opening the full map would, keeping the Scenes section table and detail view responsive even for maps with large source images.
- **SC-006**: When a GM launches a different scene during an active Play session, every currently-connected member is viewing the newly-launched scene within seconds, with no manual action required on their part.

## Assumptions

- "Dd2vtt (DungeonDraft VTT export) file" refers to the existing map-import format already supported elsewhere in the product; this feature exposes that same import capability from the new Scenes section rather than defining a new file format.
- "Markdown edit/view editor used for Lore entries" means reusing that same editing/rendering experience for scene summaries, not building a second, different Markdown implementation.
- The three grid type options (None, Squares, Hexagons) are the complete set for this feature; no other grid shapes are in scope.
- "Non-GM (player) world members" means any world member without GM/Owner role, matching the existing GM/Owner-vs-Player distinction used elsewhere in the product (e.g. Compendium, Lore).
- Existing scenes created before this feature ships have no summary and default to not-hidden (visible), so they continue appearing to players exactly as before until a GM chooses to hide them or add a summary.
- "Launch" fully replaces today's separate "pick a scene, then click Play" flow — the Scenes section's Launch action is the single way to choose what's being played going forward; Session Setup keeps no scene-selection control of its own.
- Live-switching everyone already in Play to a newly-launched scene assumes the product's existing multiplayer Play session already has a mechanism for keeping connected members in sync on canvas/scene state (per the project's real-time canvas architecture) that this feature extends to cover "which scene is loaded," rather than this feature needing to invent a new live-sync transport from scratch. This should be confirmed during planning.
- FR-001a's "own persistent id" is already true at the data layer today (scenes already carry their own id, independent of worlds/lore); what's new is giving that id a real, addressable frontend gateway (its own detail route), not inventing a new identity concept.
