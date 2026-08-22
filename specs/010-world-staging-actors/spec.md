# Feature Specification: World Staging Route and Actor Ownership

**Feature Branch**: `010-world-staging-actors`

**Created**: 2026-08-22

**Status**: Draft

**Input**: User description: "on the welcome screen when you click enter for a world can it take you to a dedicated /world/:id/staging route? that includes the normal app header and lets this page become a real world manager screen for the dm and a nice landing page for the players? for the dm its a place to catalog and add npcs which should be like /world/:id/actor/:id/view or edit and give dedicated routes for that. ALL players and non player characters should be classified as actors, the difference is permissions and control so we want to build a generalized base and if an actor is flagged as a Player Character vs Non-Player Character it should allow the dm to assign ownership otherwise the dm owns all non player characters. UNLESS its designated as a specific player has control over it via the ownership block which should show all players plus the dm and allow the dm to set Viewer, Editor, Owner permissions over any actor. this is a big feature. when the user hits play it takes to the real /play route for the game where the canvas is that full screen deal"

## Clarifications

### Session 2026-08-22

- Q: Both Owner and Editor can edit an actor's fields — what extra capability (if any) should Owner have that Editor doesn't? → A: Live-play token control — only the Owner-level assignee can move/control the actor's token on the canvas during a session; an Editor can edit the sheet/fields but cannot act as the actor in play.
- Q: Who can create a brand-new actor (PC or NPC)? → A: DM-only — the DM creates every actor, then assigns a player as Owner of a new PC (or grants any level on any actor) via the ownership block.
- Q: If an actor has no explicit ownership-block entry for a given world member, what do they see by default? → A: Default Viewer for all — every world member gets default read-only access to any actor lacking an explicit entry naming them; the DM always additionally has implicit, un-removable Owner-equivalent control over every actor regardless of the block's contents.
- Q: Do the spec's "DM-only" controls (creating actors, editing an actor's ownership block) apply to just the world's single Owner, or to both Owner and GM role holders? → A: Owner and GM both — any member holding either role has full DM-level control over actors and ownership blocks, matching the precedent already established in spec 009 (Owner/GM treated equally as "the DM" for authorization).
- Q: Can more than one non-DM world member simultaneously hold "Owner" level on the same actor? → A: Yes — Owner is uncapped, like Editor/Viewer; if more than one Owner-level member acts on the actor's token in a live session, the most recent action wins (no locking/turn-taking is specified).
- Q: When a world member holding an ownership-block entry (Viewer/Editor/Owner) on some actor is removed from the world entirely, what happens to that entry? → A: Cascade-delete — all of that member's ownership entries across every actor in the world are deleted automatically when their world membership ends; an actor left without an explicit Owner simply falls back to DM-only control until the DM assigns a new one.
- Q: Who should be allowed to generate a share link for an actor? → A: Anyone with Owner-level access to that actor, including the DM's always-implicit access — matches the existing edit-rights pattern, no new permission concept needed.
- Q: When someone copies a shared actor into their own world, what happens to the copy's ownership-block entries? → A: Reset to empty — the copy starts with no explicit ownership entries (destination world's DM has implicit full control, same as any newly-created actor); source-world assignments never carry over since they'd reference members outside the destination world.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - DM lands on a real world-manager staging screen (Priority: P1)

A DM who owns or runs a world clicks "Enter" for that world from the welcome screen and lands on a dedicated staging screen (not the play canvas, and not today's placeholder). This screen renders inside the normal app header/navigation and gives the DM a real place to catalog the world's actors: see the existing roster (both NPCs and player characters), open any actor's detail, and add a new NPC. A single "Play" action from here takes the DM into the existing full-screen canvas.

**Why this priority**: This is the entry point every DM hits before every session; without it, none of the actor-management value below is reachable.

**Independent Test**: As a world's DM/owner, click "Enter" on that world from `/welcome`; confirm landing on `/world/:id/staging` (app header visible, not full-screen canvas) showing the world's real actor roster; click "Play" and confirm arrival at the full-screen canvas.

**Acceptance Scenarios**:

1. **Given** a DM is on `/welcome`, **When** they click "Enter" for a world they own/run, **Then** they land on `/world/:id/staging`, rendered inside the standard app header/navigation.
2. **Given** a DM is on the staging screen, **When** they view the actor roster, **Then** they see every actor in the world (both NPCs and player characters), each showing its name and PC/NPC classification, sourced from real data — no placeholder content.
3. **Given** a DM is on the staging screen, **When** they choose to add a new NPC, **Then** a new NPC actor is created in the world and appears in the roster.
4. **Given** a DM is on the staging screen, **When** they click "Play", **Then** they arrive at the world's full-screen canvas (`/world/:id/play`) exactly as it behaves today.

---

### User Story 2 - Player lands on a landing page for the world (Priority: P1)

A player (non-DM world member) clicks "Enter" for a world from the welcome screen and lands on the same `/world/:id/staging` route, but sees a landing-page experience appropriate to a player: world/session information and the actors visible to them, without any DM-only management controls (no "add NPC," no editing another player's or the DM's actors beyond what their own permissions allow). A "Play" action takes them into the same full-screen canvas.

**Why this priority**: Extends the entry-point fix to the majority of a world's users; independently valuable and testable from the DM experience.

**Independent Test**: As a non-DM world member, click "Enter" for a world from `/welcome`; confirm landing on `/world/:id/staging` with no DM-only actor-management controls visible; click "Play" and confirm arrival at the same full-screen canvas.

**Acceptance Scenarios**:

1. **Given** a player is on `/welcome`, **When** they click "Enter" for a world they belong to, **Then** they land on `/world/:id/staging` with no NPC-catalog-editing or actor-creation controls visible.
2. **Given** a player is on the staging screen, **When** they view the actor roster, **Then** they see every actor they have at least Viewer access to (which, by default, is every actor in the world), with edit controls shown only for actors where their permission level allows editing.
3. **Given** a player is on the staging screen, **When** they click "Play", **Then** they enter the same full-screen canvas as any other member, independent of what the DM or other players are currently doing.

---

### User Story 3 - DM manages an actor's ownership/permissions (Priority: P1)

From an actor's detail screen, the DM opens that actor's ownership block — a list of every world member (all players plus the DM) paired with a permission level (Viewer, Editor, or Owner) — and changes who has what level of access to that specific actor. This works identically whether the actor is an NPC or a player character: the DM can grant a player Owner of their own PC, hand out Editor/Viewer access on an NPC to a player who's temporarily controlling it, or reassign any of this at any time.

**Why this priority**: This is the mechanism that makes the PC/NPC distinction meaningful (permissions and control, not a different data model) and is the direct fast-follow to the player-character-assignment gap left open by the prior staging-page work; without it, the actor system is just a read-only roster.

**Independent Test**: As a DM, open any actor's detail screen, open its ownership block, assign a player Owner/Editor/Viewer, and confirm that player's access to that actor changes accordingly; confirm a non-DM member cannot open or change the ownership block on any actor, including one they themselves own.

**Acceptance Scenarios**:

1. **Given** a DM is viewing an actor's detail screen, **When** they open its ownership block, **Then** they see every world member (all players plus the DM) and, for each, either an explicit assigned permission level or an indication that the member has the default (Viewer) level.
2. **Given** a DM is in an actor's ownership block, **When** they assign a player "Owner" of a PC actor, **Then** that player gains full edit rights and live-play token control over that actor, in addition to their existing default Viewer access to every other actor. More than one player may hold Owner on the same actor at once; if this happens, whichever Owner-level member most recently acted on the actor's token controls it.
3. **Given** a DM is in an actor's ownership block, **When** they assign a player "Editor" on an NPC, **Then** that player can edit the NPC's fields but cannot control its token during play and cannot open or change its ownership block.
4. **Given** a player who owns their own PC actor is viewing that actor's detail screen, **When** they look for ownership-block controls, **Then** none are shown or editable — only the DM can change who has access to an actor.
5. **Given** an actor (NPC or PC) has no explicit ownership-block entry for a particular player, **When** that player views the actor, **Then** they have default Viewer access (read-only) regardless of the actor's PC/NPC classification, and the DM always has full control over the actor regardless of the block's contents.

---

### User Story 4 - Dedicated actor view/edit routes (Priority: P2)

Any world member can navigate directly to `/world/:id/actor/:actorId/view` to see an actor's full detail, and — if their permission level on that actor allows editing — to `/world/:id/actor/:actorId/edit` to change its fields. These are real, linkable, bookmarkable routes reached both from the staging screen's roster and by direct URL.

**Why this priority**: Independently useful (deep-linkable actor sheets) but secondary to the staging screen and ownership mechanism landing first, since those are what make the roster and permissions reachable in the first place.

**Independent Test**: As a world member with at least Viewer access to an actor, navigate directly to `/world/:id/actor/:actorId/view`; confirm the actor's detail renders. Attempt `/world/:id/actor/:actorId/edit` for an actor where the member has only Viewer access; confirm editing is blocked or the route redirects to the view page.

**Acceptance Scenarios**:

1. **Given** a world member has at least Viewer access to an actor, **When** they navigate to that actor's `/view` route, **Then** the actor's detail renders with real data.
2. **Given** a world member has Editor or Owner access to an actor, **When** they navigate to that actor's `/edit` route, **Then** they can change and save the actor's fields.
3. **Given** a world member has only Viewer access (default or explicit) to an actor, **When** they navigate to that actor's `/edit` route, **Then** the system blocks the edit and directs them to the read-only `/view` route instead.
4. **Given** a user who is not a member of the world (and not its owner/DM) attempts either route for an actor in that world, **When** the route loads, **Then** they are denied access consistent with existing world-visibility rules.

---

### User Story 5 - Share an actor and copy it into another world (Priority: P2)

A world member with Owner-level access to an actor (including the DM, who always has implicit Owner-level access) can generate a shareable link for that actor. Anyone who opens that link — including someone in a completely different world or campaign — sees a read-only preview of the actor. If they're logged in, they can choose "Copy to World," pick one of their own worlds where they hold DM-level access, confirm, and receive a deep, fully independent copy of the actor in that world: a brand-new actor record with its own identity, including cloned copies of all of its cascaded data (abilities, items, actor-specific lore, and any other actor sub-data). The copy is never referentially linked back to the source — future edits to either one never affect the other.

**Why this priority**: This is the feature the requester considers the biggest long-term value — building a reusable library of NPCs/characters that can move between worlds — but it depends on the Actor entity, routes, and permission model from User Stories 1, 3, and 4 already existing, so it's sequenced after them.

**Independent Test**: As a member with Owner-level access to an actor, generate a share link; open that link as a different, unrelated user; confirm a read-only preview renders with no edit controls. Click "Copy to World," pick a destination world, confirm, and verify a new, fully independent actor (with cloned abilities/items/lore) appears in that world. Edit either copy afterward and confirm the other is unaffected.

**Acceptance Scenarios**:

1. **Given** a member with Owner-level access to an actor, **When** they generate a share link, **Then** the system produces a stable, shareable URL for that specific actor.
2. **Given** any user opens a valid share link, **When** the page loads, **Then** they see a read-only preview of the actor's data (name, classification, abilities, items, lore) with no edit controls and no ownership-block visibility.
3. **Given** a logged-in user viewing a shared actor, **When** they click "Copy to World," **Then** they are shown a list of their own worlds where they hold DM-level access and must pick one and confirm before anything is copied.
4. **Given** a user confirms copying a shared actor into one of their worlds, **When** the copy completes, **Then** a brand-new actor (with a new identity, no reference back to the source, and an empty ownership block) appears in that world's roster, including independent copies of its abilities, items, and actor-specific lore, and the user sees a clear confirmation that the copy succeeded.
5. **Given** a copied actor exists in a destination world, **When** either the original or the copy is later edited, **Then** the other is completely unaffected — they share no live data or synchronization.
6. **Given** the actor's Owner-level member revokes a previously generated share link, **When** anyone attempts to open that link afterward, **Then** they see a clear "no longer available" state instead of the actor's data.

---

### Edge Cases

- What happens when a world has zero actors yet? The staging screen's roster shows a genuine empty state, and the DM's "add NPC" action is the way to populate it — no placeholder/lorem-ipsum content.
- What happens when the DM navigates to an actor's `/edit` route for an actor that has since been deleted by another concurrent DM session? The system shows a not-found/removed state rather than a blank or crashing page.
- What happens when a player who is the assigned Owner of their PC tries to view or reach the ownership-block UI directly (e.g., by URL)? It remains inaccessible/read-only to them — ownership-block changes are DM-only regardless of the requester's own permission level on that actor.
- What happens when the DM changes an actor's PC/NPC classification (if that's ever editable) while a player currently holds Owner on it? The ownership block's existing entries are preserved as-is; classification is independent of the permission entries themselves.
- What happens on a very small viewport? The staging screen and actor detail/edit routes must remain usable — no dedicated mobile layout is required, but nothing should become totally inaccessible.
- What happens when a user directly visits `/world/:id/play` without having gone through `/staging` first? Per the migration note below, they land straight in the full-screen canvas — staging is no longer a gate `/play` enforces, only an earlier step in the normal navigation flow from `/welcome`.
- What happens when more than one member holds Owner-level access on the same actor and both try to control its token in the same live session? The most recently-acting Owner-level member's action takes effect — no locking, queueing, or turn-taking is specified for this conflict.
- What happens when a world member holding an ownership-block entry on one or more actors is removed from the world (kicked or leaves)? Their entries across every actor in that world are deleted automatically; any actor left without an explicit Owner falls back to DM-only control until the DM assigns a new one.
- What happens when the source actor is deleted after being shared? The share link shows a clear "no longer available" state instead of erroring.
- What happens when a user with no world where they hold DM-level access tries to copy a shared actor? The "Copy to World" flow shows no eligible destination worlds and explains that DM-level access in a world is required to copy into it.
- What happens if the same shared actor is copied into the same destination world more than once? Each copy is created independently — no deduplication is performed; copying is a repeatable, additive action.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The system MUST provide a dedicated route, `/world/:id/staging`, that renders inside the standard app header/navigation chrome (not the full-screen canvas shell).
- **FR-002**: Clicking "Enter" for a world from the welcome screen MUST navigate to that world's `/world/:id/staging` route.
- **FR-003**: The staging route MUST show the world's real actor roster (both NPC and player-character actors) to every world member, with each entry indicating at least the actor's name and PC/NPC classification — no placeholder/lorem-ipsum content, and a genuine empty state when the roster is empty.
- **FR-004**: The staging route MUST provide the DM a control to create a new NPC actor in the world, which then appears in the roster.
- **FR-005**: The staging route MUST NOT show NPC-creation or other DM-only management controls to non-DM world members.
- **FR-006**: The staging route MUST provide a single, prominent "Play" action that navigates the current user to the world's full-screen canvas at `/world/:id/play`.
- **FR-007**: `/world/:id/play` MUST continue to render the existing full-screen canvas experience directly (no embedded staging step), since staging now happens earlier in the flow at `/world/:id/staging`.
- **FR-008**: The system MUST treat player characters and non-player characters as the same underlying "actor" concept, distinguished only by a PC/NPC classification flag — not by separate data models or separate route structures.
- **FR-009**: The system MUST provide a dedicated route, `/world/:id/actor/:actorId/view`, showing a single actor's full detail to any world member with at least Viewer access to that actor.
- **FR-010**: The system MUST provide a dedicated route, `/world/:id/actor/:actorId/edit`, allowing a world member with Editor or Owner access to that actor to change and save its fields.
- **FR-011**: The system MUST block editing (redirecting to the view route or otherwise refusing the change) when a member with only Viewer access to an actor reaches that actor's edit route.
- **FR-012**: The system MUST deny both actor routes to a user who is not a member of the world and not its DM/owner, consistent with existing world-visibility rules.
- **FR-013**: Every actor MUST have an associated "ownership block": a per-world-member record of that member's permission level (Viewer, Editor, or Owner) on that specific actor.
- **FR-014**: Only the DM MUST be able to view and change an actor's ownership block; no other permission level (including a member holding Owner on that same actor) grants access to change it.
- **FR-015**: The ownership-block editing UI MUST list every world member — all players plus the DM — as an assignable subject, and MUST let the DM set that subject's permission level (Viewer, Editor, or Owner) for the actor being edited.
- **FR-016**: For any world member with no explicit ownership-block entry on a given actor, the system MUST treat that member as having Viewer (read-only) access to that actor by default.
- **FR-017**: The DM MUST always retain full (Owner-equivalent) control over every actor in their world regardless of that actor's ownership-block contents — the block is additive delegation, never a way to reduce or remove the DM's own access.
- **FR-018**: Only a world member holding actor-level Owner (or the DM, per FR-017) MUST be able to control/move that actor's token during a live play session; an Editor may change the actor's fields/sheet but not act as it in play. An actor MAY have more than one Owner-level member simultaneously; when more than one acts on the actor's token in the same session, the most recent action MUST take effect (no locking/turn-taking required).
- **FR-019**: Only the DM MUST be able to create a new actor (NPC or player character); the DM subsequently assigns a player Owner (or any other level) of a newly created actor via the ownership block.
- **FR-020**: The staging route MUST include a clearly-labeled extension point for future world-manager sections beyond the actor catalog (e.g., lore, session trackers), without building that additional content in this pass.
- **FR-021**: For authorization purposes throughout this feature, "DM" MUST mean any world member holding the existing Owner or GM role — both roles carry identical, full DM-level control over actor creation and ownership blocks; no distinction between Owner and GM is introduced by this feature.
- **FR-022**: When a world member is removed from a world, the system MUST automatically delete every ownership-block entry that named them, across all of that world's actors — no orphaned entries referencing a non-member MUST remain.
- **FR-023**: The system MUST let any world member with Owner-level access to an actor (including the DM's implicit access, per FR-017) generate a shareable link for that specific actor.
- **FR-024**: Opening a valid, non-revoked share link MUST show a read-only preview of the actor's full data (fields, abilities, items, actor-specific lore) without exposing edit controls or the actor's ownership block, regardless of the viewer's own world membership.
- **FR-025**: A logged-in viewer of a shared actor MUST be able to choose "Copy to World," see a list of their own worlds where they hold DM-level access, and select one as the destination before anything is copied.
- **FR-026**: Confirming a copy MUST create a new, independent actor record in the destination world — a full deep copy of the source actor's data and all cascaded sub-data (abilities, items, actor-specific lore, and any other actor sub-data), with a new identity that has no live or referential link back to the source.
- **FR-027**: After copying, edits to the source actor MUST NOT affect the copy, and edits to the copy MUST NOT affect the source — the two are fully independent records from the moment of copy.
- **FR-028**: The system MUST clearly notify the user when a copy operation completes successfully.
- **FR-029**: The actor-level Owner (or DM) who generated a share link MUST be able to revoke it; a revoked link MUST show a clear "no longer available" state to anyone who opens it afterward, rather than the actor's data.
- **FR-030**: A copied actor's ownership-block entries MUST start empty in the destination world (destination DM has implicit full control, same as any newly-created actor) — source-world ownership assignments MUST NOT be carried over, since they may reference members who are not part of the destination world.

### Key Entities *(include if feature involves data)*

- **Actor** (generalizes the existing NPC/PC concept): A world- and scene-scoped entity representing either a non-player character or a player character, distinguished by a PC/NPC classification flag. Has a name/label and a set of sheet fields (existing, unchanged by this feature) plus a new associated ownership block.
- **Actor Ownership Entry**: A (actor, world member, permission level) association. Permission level is one of Viewer, Editor, or Owner; an actor may have any number of entries at any level, including more than one Owner-level entry at once. Any world member without an explicit entry defaults to Viewer on that actor. The DM's access to an actor is never governed by this entry set — it is always implicitly full. An entry is automatically deleted when the world member it names is removed from the world.
- **World Member** (existing): A user's membership and role (Owner, GM, or Player) within a world; reused as the pool of subjects assignable in an actor's ownership block. Owner and GM are both treated as "the DM" for this feature's authorization rules (FR-021).
- **World** (existing): Reused as the scope for the staging route and for resolving "who is the DM" for permission purposes.
- **Actor Share Link**: A stable, shareable reference to one specific actor, created by a member with Owner-level access to it. Carries a revoked/active state (set by its creator) but no usage cap or expiration by default. Viewing it never grants access to anything beyond that one actor's read-only data. A "Copy to World" action performed through it produces a brand-new Actor (and cloned copies of all of that actor's cascaded sub-data) in a destination world of the viewer's choosing — a one-time deep copy, not an ongoing link between the two actors.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: 100% of "Enter" clicks from the welcome screen land on the world's dedicated staging route, never directly on the full-screen canvas and never on the prior placeholder experience.
- **SC-002**: A DM can go from the staging screen to a usable full-screen canvas in one click ("Play"), with no intermediate forced steps.
- **SC-003**: A DM can create a new NPC and see it appear in the roster without a page reload, in under 3 actions (open creation control, provide a name, confirm).
- **SC-004**: A DM can change any actor's ownership so that a specific player gains Editor or Owner access, and that player's access reflects the change without needing to leave and re-enter the world.
- **SC-005**: 100% of attempts by a non-DM member to view or change an actor's ownership block are blocked, regardless of that member's own permission level on the actor in question.
- **SC-006**: 100% of attempts to edit an actor by a member with only Viewer-level access (default or explicit) are blocked and redirected to the read-only view.
- **SC-007**: Every actor in a world is reachable via its own direct, bookmarkable view URL by any member with at least Viewer access to it.
- **SC-008**: 100% of a removed world member's ownership-block entries are gone from every actor in that world immediately after their removal — none remain reachable or visible in any ownership-block listing.
- **SC-009**: A member with Owner-level access can generate a share link for an actor in 2 actions or fewer.
- **SC-010**: 100% of copy operations produce a fully independent actor in the destination world — zero copies retain any live reference to the source actor or its data after creation completes.
- **SC-011**: 100% of copied actors' ownership-block entries start empty in the destination world, regardless of the source actor's own ownership-block contents.

## Assumptions

- This feature deliberately changes the route shape established by the prior GM-staging-page work, which treated "staging vs. playing" as a client-side UI state inside `/world/:id/play` rather than a separate URL. That prior route continues to serve the full-screen canvas unchanged; only the entry point changes — staging now happens first, at its own `/world/:id/staging` route, reached from the welcome screen, rather than as an initial state inside `/play`.
- The staging screen's non-actor-catalog sections (world/session landing-page content for players, and any future large features called out as "more later" — e.g., lore, trackers) are explicitly out of scope for this pass beyond providing a labeled extension point; only the actor catalog and its "add NPC" action need to be real and functional here.
- Actor sheet fields themselves (whatever attributes an NPC or PC has beyond name/classification) are assumed to already exist or to be handled by existing actor data structures; this feature does not redesign sheet content, only the routes, roster, and ownership/permission model around it.
- Scene-level actor placement/visibility (which scene an actor currently appears in on the canvas) is unchanged by this feature; the staging roster and actor detail routes are world-scoped, not scene-scoped.
- A player who loses their Owner/Editor grant on an actor they were previously controlling in a live session is not specified here to be force-disconnected from mid-session control; enforcement takes effect on next access/action, consistent with how permission changes are typically applied in this system.
- Share links are persistent (non-expiring) and uncapped in number of views/copies by default, unlike world invite codes which are usage-capped — sharing an actor is meant to function like a public showcase link, not a one-time invite. Its Owner-level creator (or the DM) may revoke it at any time (FR-029).
- Viewing a shared actor requires the viewer to be logged into the application (consistent with how the rest of the app handles content) but does NOT require the viewer to be a member of the source actor's world.
- "DM-level access" for choosing a copy destination world means the same Owner-or-GM role established in FR-021 — the viewer must be able to create actors in the destination world (per FR-019) for it to be offered as a valid destination.
- **Follow-up (out of scope for this spec)**: This share-and-copy mechanism is being built actor-first, but the underlying pattern — generate a link → show a read-only independent preview → let the viewer copy a fully-cloned, non-referential version into their own space — is expected to generalize to other content types (candidates: scenes/maps, items, game-system templates, or whole world templates). A follow-up spec should define that generalized "share or view independently" engine once this actor-specific version has shipped and proven the pattern; this spec does not build any shared/abstracted sharing infrastructure beyond what actors need.
