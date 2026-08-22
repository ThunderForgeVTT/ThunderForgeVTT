# Feature Specification: World Compendium

**Feature Branch**: `011-world-compendium`

**Created**: 2026-08-22

**Status**: Draft

**Input**: User description: "Maybe we do Compendium as a whole section and have a tabbed layout where its NPCs, Items, Abilities, TBD. lets do that then on row select on the right side of whre the table is we pull up a little viewing window? and lets lets move thisto its own page http://localhost:5173/world/01a0272b-c399-74c2-b509-d013b3aa38e8/compendium so it all works together then in the Session Setup you hafe Play, Players, Last Session Notes that way lore, npcs, all of that goes under. This will give us a whole portal for managing world artifacts without having to load the */play seciotn and lets the game master curate their world"

## Clarifications

*(No clarification session held — the request was specific enough to fill remaining gaps with the reasonable defaults recorded in the Assumptions section below.)*

## User Scenarios & Testing *(mandatory)*

### User Story 1 - DM browses and manages NPCs in a dedicated Compendium (Priority: P1)

A DM navigates to their world's Compendium (`/world/:id/compendium`), lands on the NPCs tab, and sees the world's full NPC roster in a searchable table. Selecting a row opens a preview panel docked to the right of the table showing that NPC's details, without leaving the table or losing their place in it. From the preview panel the DM can jump to the full edit screen when they need to change more than a quick glance allows.

**Why this priority**: This is the feature's core value — it's the direct replacement for spec 010's NPC catalog and the reason the Compendium exists at all. Every other tab depends on the same shell this story builds.

**Independent Test**: As a world's DM, navigate to `/world/:id/compendium`, confirm the NPCs tab is selected by default and shows the real roster; search for an NPC by name or description and confirm the table filters; click a row and confirm a preview panel appears to the right showing that NPC's name, description, and classification; click "Edit" in the preview panel and confirm arrival at that actor's edit screen.

**Acceptance Scenarios**:

1. **Given** a DM opens `/world/:id/compendium`, **When** the page loads, **Then** it renders inside the standard app header/navigation (not the full-screen canvas) with a tabbed layout defaulting to the NPCs tab.
2. **Given** the DM is on the NPCs tab, **When** they view the table, **Then** every NPC in the world appears with its name and description, sourced from real data.
3. **Given** the DM types into the search field, **When** the query matches an NPC's name or description, **Then** the table narrows to matching rows only, updating as they type.
4. **Given** the DM clicks a row in the table, **When** the row is selected, **Then** a preview panel appears docked to the right of the table showing that NPC's full detail (name, description, classification, type), and the table remains visible and scrollable at the same time.
5. **Given** an NPC is selected and its preview panel is open, **When** the DM clicks a "View full detail" or "Edit" action in the panel, **Then** they're taken to that actor's existing `/world/:id/actor/:id/view` or `/edit` route.
6. **Given** the DM has Editor-or-Owner access to the selected NPC, **When** they view its preview panel, **Then** an "Edit" action is available; **Given** they only have Viewer access, **When** they view the panel, **Then** only a "View" action is available (no Edit).
7. **Given** the DM adds a new NPC from the Compendium's NPCs tab, **When** the creation succeeds, **Then** the new NPC appears in the table without a full page reload.

---

### User Story 2 - Any world member can browse the Compendium read-only (Priority: P2)

A player (non-DM world member) opens the same Compendium route and can browse and search the NPC roster and preview any NPC's detail, exactly as the DM does, but without any create/edit affordances they don't already have permission for.

**Why this priority**: The Compendium is a shared world-reference surface, not a DM-only tool — players benefit from being able to look up an NPC's description mid-session-prep without pinging the DM. This depends on User Story 1's shell already existing.

**Independent Test**: As a non-DM world member, navigate to `/world/:id/compendium`, confirm the NPCs tab, table, search, and row-preview all work identically to the DM's view, but the "Add NPC" control and any Edit actions on NPCs the player doesn't have Editor/Owner access to are absent.

**Acceptance Scenarios**:

1. **Given** a Player (non-DM) world member opens the Compendium, **When** the page loads, **Then** they see the same NPCs tab, table, and search as the DM, populated with real data.
2. **Given** a Player selects an NPC row, **When** the preview panel opens, **Then** they see the same detail a DM would see, with a "View" action always available and an "Edit" action only if their effective permission on that actor is Editor or Owner.
3. **Given** a Player is on the Compendium's NPCs tab, **When** they look for an "Add NPC" control, **Then** it is not present (DM/GM-only, matching spec 010's actor-creation rule).

---

### User Story 3 - Session Setup is simplified to launch-only concerns (Priority: P2)

A DM or player who lands on the world's Session Setup screen (`/world/:id/staging`) now sees only three things: the "Play" action, the player roster, and a "Last Session Notes" panel showing a short freeform recap the DM can update between sessions. The NPC catalog and the "Lore — coming soon" placeholder that used to live here are gone — the Compendium (User Story 1/2) is where that content now lives.

**Why this priority**: This is a direct, low-risk consequence of Users Stories 1-2 landing (the content has to move out of staging once the Compendium exists), and depends on the Compendium existing first, but it's the second most user-visible half of this feature and closes the loop the user asked for.

**Independent Test**: As a DM, load `/world/:id/staging` and confirm only Play, Players, and Last Session Notes are present (no NPC list, no Lore placeholder); edit the session notes and confirm the change persists across a reload; as a Player, load the same screen and confirm the notes are visible but not editable.

**Acceptance Scenarios**:

1. **Given** a DM loads `/world/:id/staging`, **When** the page renders, **Then** it shows the Play action, the player roster, and a "Last Session Notes" panel, and does not show an NPC list or a Lore placeholder.
2. **Given** a DM edits the Last Session Notes text and saves, **When** the page is reloaded (by anyone in the world), **Then** the updated notes are shown.
3. **Given** a Player (non-DM) loads Session Setup, **When** they view Last Session Notes, **Then** the text is visible but read-only (no save control).
4. **Given** a DM wants to manage NPCs from Session Setup, **When** they look for that capability, **Then** they find a clearly-labeled link/action to the Compendium instead of an inline catalog.

---

### Edge Cases

- A world with zero NPCs: the Compendium's NPCs tab shows a genuine empty state ("No NPCs yet" plus, for a DM, the add-NPC control), never placeholder/lorem-ipsum content.
- Selecting a row, then using search to filter it out of view: the preview panel for the previously-selected row stays open (it doesn't disappear just because the row is temporarily filtered out of the table), until the user selects a different row or explicitly closes it.
- A non-DM world member with an explicit Viewer-level (or no-explicit-row, default-Viewer) grant on a specific NPC previews it fine but sees no Edit action for that one row, even if they have Editor/Owner on other NPCs in the same table.
- Last Session Notes has never been set for a brand-new world: Session Setup shows an empty/placeholder-invitation state ("No notes yet" for players; an empty editable field for the DM), not an error.
- A DM clears the Last Session Notes text entirely and saves: this is a valid save (an empty recap), not treated as "no change."
- Opening the Items or Abilities tab: shows a clearly-labeled "coming soon" state — no table, no fabricated rows, no broken search box.
- Deep-linking directly to `/world/:id/compendium` (not arriving via a link from Session Setup): works the same as navigating there normally, subject to the same world-visibility rule as every other world-scoped route (a non-member/non-owner is rejected).

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The system MUST provide a Compendium page at a dedicated route scoped to a world, rendered inside the standard application chrome (header/navigation), not the full-screen play canvas.
- **FR-002**: The Compendium MUST present its content in a tabbed layout with at least three tabs — NPCs, Items, Abilities — and MUST be structured so that additional content-type tabs can be added later without restructuring the existing ones.
- **FR-003**: The NPCs tab MUST show a searchable table/list of the world's NPCs, each row showing at minimum its name and description, matching the search/filter capability already established for the NPC catalog (name and description, instant-as-you-type).
- **FR-004**: Selecting a row in the NPCs tab MUST open a preview panel docked to the right side of the table (a split view), showing that NPC's detail (name, description, classification, type), without navigating away from the table.
- **FR-005**: The preview panel MUST offer a way to reach that actor's full view/edit route, and MUST only offer an edit action when the current user's effective permission on that actor is Editor or Owner (view is always available to anyone who can see the row at all).
- **FR-006**: A DM/GM MUST be able to create a new NPC directly from the Compendium's NPCs tab, with the new NPC appearing in the table without a full page reload.
- **FR-007**: Any world member (DM or player) MUST be able to open the Compendium and browse/search/preview NPCs; create and edit affordances remain gated by the existing DM-only-create and per-actor-permission rules (unchanged from spec 010).
- **FR-008**: The Items and Abilities tabs MUST render a clearly-labeled "coming soon" placeholder in this pass — no real data, no fabricated example rows, and no functioning search/create controls for those tabs.
- **FR-009**: The Session Setup screen MUST be reduced to exactly three sections: the Play action, the player roster, and a "Last Session Notes" panel. The NPC catalog and the "Lore — coming soon" placeholder previously on this screen MUST be removed from it.
- **FR-010**: Session Setup MUST provide a clearly-labeled way to reach the Compendium (a link or button), since NPC management no longer lives on this screen.
- **FR-011**: The system MUST persist a single freeform "Last Session Notes" text per world, visible to every world member on Session Setup.
- **FR-012**: Only a DM/GM MUST be able to edit Last Session Notes; a Player MUST see the current text read-only.
- **FR-013**: Saving Last Session Notes (including saving an empty value) MUST persist and be visible to any world member who subsequently loads Session Setup.
- **FR-014**: World-visibility rules for the Compendium route MUST match every other world-scoped route in the app (a user with no ownership/membership relationship to the world is rejected).

### Key Entities

- **World Session Notes**: A single freeform text field scoped to one world (not per-scene, not a historical log of every session — the "last" recap only), holding the DM's most recent between-sessions summary. Replaces the prior "Lore — coming soon" placeholder's slot on Session Setup for this pass; a fuller per-session log or rich lore system remains future work.
- **Compendium Tab**: A named, orderable section of the Compendium page mapping to one content type (NPCs in this pass; Items and Abilities as placeholders; further types expected later). Existing entities (`Actor` from spec 010) are reused as-is for the NPCs tab — no new actor data model is introduced by this feature.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A DM can find a specific NPC's description (search-to-visible-preview) in under 10 seconds, without ever entering the full-screen play canvas.
- **SC-002**: 100% of NPC-management actions previously available on Session Setup (browse, search, add, view, edit) remain available, unchanged in capability, after relocating to the Compendium.
- **SC-003**: Session Setup's visual/informational footprint is reduced to exactly the three specified sections (Play, Players, Last Session Notes) with no residual NPC or Lore content.
- **SC-004**: A DM can update and persist a between-session recap in under 30 seconds from landing on Session Setup.
- **SC-005**: Adding a new top-level content type to the Compendium in the future requires adding a new tab, not restructuring the NPCs tab, the routing, or the preview-panel pattern.

## Assumptions

- The Compendium route is `/world/:id/compendium`, matching the user's stated URL and the existing `/world/:id/staging` / `/world/:id/play` sibling-route pattern.
- The NPCs tab's table, search, and add-NPC behavior are a direct relocation of spec 010's existing NPC catalog capability (search-as-you-type over name/description, DM-only creation) — no new NPC-specific capability is introduced, only a new surface (tabbed page + split-view preview) for the same capability.
- The row-preview panel shows a compact summary (name, description, classification, type) rather than duplicating the entire edit form inline; deeper editing still happens on the existing `/world/:id/actor/:id/edit` route reached via an action in the panel. This keeps the panel simple and reusable for future tabs (Items/Abilities) whose detail shape isn't decided yet.
- Items and Abilities have no data model yet; their tabs are placeholder-only in this pass, consistent with the user's explicit "TBD" framing and instruction that only the NPCs tab needs to be fully real.
- "Last Session Notes" is a single per-world freeform text value (the latest recap), not a per-session historical log — the user's phrasing ("Last Session Notes") describes one current note, not an archive. A full per-session log is out of scope and left as future work alongside the deferred Lore system.
- Last Session Notes is visible to every world member but editable only by a DM/GM, mirroring the read/write split already established for other DM-curated world content (e.g. the NPC catalog's add/edit rights).
- The Compendium is reachable via a link from Session Setup, and is also directly navigable by URL (subject to the same world-visibility check as every other world-scoped route) — it does not require passing through Session Setup first.
- No new permission concept is introduced: the Compendium's NPCs tab reuses spec 010's existing per-actor ownership-block permissions (Viewer/Editor/Owner) unchanged.
