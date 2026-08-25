# Feature Specification: Players Section

**Feature Branch**: `023-players-section`

**Created**: 2026-08-25

**Status**: Draft

**Input**: User description: "Move the world's 'Players' roster out of the Overview page into its own dedicated section in the world sidebar nav (alongside Scenes, NPCs, Lore, Items, Abilities). This new Players section should let every world member view the other players in the world as their 'characters' — i.e. browse who's playing what character, not just a bare username list. From a GM's perspective, this same section is also a moderation view: the GM needs GM-specific capabilities here (e.g. managing member roles/removal, seeing moderation-relevant info) in addition to the same character-browsing view everyone else gets."

## Clarifications

### Session 2026-08-25

- Q: Once the Players section has role-change and remove-member controls, should the world dashboard's existing Campaign Settings panel drop those same controls, or keep them as a second place to do the same thing? → A: Supersede — remove role-change/remove-member from the dashboard's Campaign Settings panel; the Players section becomes the only place to do this (invite-link generation stays on the dashboard).

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Every member browses the roster as characters, not usernames (Priority: P1)

A player wants to know who else is at the table and, more importantly, *who they're playing* — not just a list of account names. They open the world's new "Players" section (alongside Scenes, NPCs, Lore, Items, Abilities in the sidebar) and see every member paired with the character they've claimed, if any.

**Why this priority**: This is the foundational slice — a real, useful roster view is the whole point of giving Players its own section instead of a bare list buried in Overview. Every other capability in this feature builds on this same list.

**Independent Test**: Can be fully tested by a non-GM world member opening the Players section and seeing every member listed alongside the character each has claimed (or "no character claimed" for a member who hasn't picked one).

**Acceptance Scenarios**:

1. **Given** a world with several members, some with a claimed character and some without, **When** any member opens the Players section, **Then** they see every member listed, each paired with their claimed character's name when one exists.
2. **Given** a member who hasn't claimed a character yet, **When** the roster is displayed, **Then** that member still appears, clearly marked as not yet playing a character (not omitted, not shown as an error).
3. **Given** the Overview page (formerly hosting the roster), **When** any member views it after this feature ships, **Then** no player roster remains there — only a link to the new Players section if a quick reference is still useful.
4. **Given** a member clicks on another member's listed character, **When** they're a member with view access to that character, **Then** they reach that character's existing detail view.

---

### User Story 2 - GM sees the same roster as a moderation view, with member-management actions (Priority: P2)

A GM opens the same Players section a regular member would, but as the GM they additionally need to manage who's in the world: change a member's role, remove a member, and see anything relevant to keeping the table healthy (e.g. who hasn't claimed a character yet, so the GM can follow up).

**Why this priority**: This is the section's second core purpose per the request ("from a GM's perspective it's more of a moderation view") — valuable on its own once the base roster (User Story 1) exists, but a GM can still run a game without it (the equivalent management actions already exist elsewhere today).

**Independent Test**: Can be fully tested by a GM opening the Players section and successfully changing a member's role and removing a member, while a non-GM member in the same world sees neither control.

**Acceptance Scenarios**:

1. **Given** a GM viewing the Players section, **When** the page renders, **Then** they see the same member+character roster every other member sees, plus GM-only controls not shown to anyone else.
2. **Given** a GM viewing the Players section, **When** they change a member's role, **Then** the change takes effect and is reflected immediately in that member's roster entry.
3. **Given** a GM viewing the Players section, **When** they remove a member from the world, **Then** that member no longer appears in the roster and loses access to the world, consistent with how member removal already works elsewhere in the product today.
4. **Given** a non-GM member viewing the Players section, **When** the page renders, **Then** no role-change or removal controls are present anywhere in the UI, and attempting the equivalent action directly is rejected the same way every other GM-only action in this product already is.

---

### Edge Cases

- What happens when a GM removes themselves? Rejected — consistent with this product's existing rule that a world's DM/Owner-role member cannot be removed by member-management actions.
- What happens when a member has claimed more than one character (if that's ever possible)? Out of scope for this feature — the roster shows whatever the existing character-claim relationship already models (see Assumptions); this feature doesn't change claiming rules.
- What happens when a non-GM member navigates directly to a GM-only action's URL/control? Rejected server-side, same as every other GM-only mutation in this product — the UI simply never shows the control to begin with.
- What happens to a member's roster entry the moment their claimed character changes? The Players section reflects the current claim on next load; it is not required to update live for members already viewing the page when someone else's claim changes.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST provide a dedicated "Players" section for each world, reachable from the same world navigation that already links to Scenes, NPCs, Lore, Items, and Abilities.
- **FR-002**: System MUST remove the player roster from the Overview page once the Players section exists.
- **FR-003**: Every world member MUST be able to view the Players section, listing every member of the world.
- **FR-004**: For each member with a claimed character, the Players section MUST show that character's name/identity alongside the member, not just their account username.
- **FR-005**: For each member without a claimed character, the Players section MUST clearly indicate that, rather than omitting the member or showing a broken/empty entry.
- **FR-006**: System MUST allow a member to open a listed character's detail view from the Players section when they already have viewing access to that character.
- **FR-007**: GM/Owner members MUST see additional, GM-only controls in the Players section not shown to any other member: at minimum, changing a member's role and removing a member from the world.
- **FR-008**: System MUST reject role-change and member-removal actions from anyone who is not a GM/Owner of the world, regardless of entry point.
- **FR-009**: System MUST reject an attempt to remove the world's Owner/creator via this section's controls.
- **FR-010**: Role changes and member removal performed from the Players section MUST have the same effect as the equivalent actions already available elsewhere in the product (no divergent behavior between the two entry points).
- **FR-011**: System MUST remove the role-change and remove-member controls from the world dashboard's Campaign Settings panel once the Players section provides them — the Players section is the sole place to perform these actions going forward. The dashboard's invite-link generation is unaffected and stays where it is.

### Key Entities

- **World Member**: An existing concept (a world's `world_members` row) — this feature adds no new fields to it, just a new place to view and act on it.
- **Character claim**: An existing relationship between a world member and the character (actor) they've claimed to play, established during actor selection. This feature reads that existing relationship; it does not change how claiming works.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Any world member can identify which character every other member is playing in one view, without leaving the Players section.
- **SC-002**: A GM can change a member's role or remove a member in under 30 seconds from opening the Players section, with no need to visit a separate page.
- **SC-003**: Zero non-GM members can see or successfully trigger a role-change or removal action, verified across every entry point (UI and direct action attempts).
- **SC-004**: The Overview page's line count/scope shrinks measurably (roster removed) with no loss of information — everything it showed is still reachable, just relocated. The world dashboard's Campaign Settings panel likewise loses its role-change/remove-member controls (FR-011) with no loss of capability — the same actions are still available, just from the Players section only.

## Assumptions

- "Characters" means the existing actor-claim relationship established during Actor Selection (spec 017) — a member is shown paired with whichever actor they've claimed, if any. This feature does not introduce a new character concept or change claiming rules.
- "Moderation-relevant info" (from the request) is interpreted as: role, and whether a member has claimed a character yet — not the separate content-moderation/takedown system (an unrelated, already-shipped feature for copyright complaints). This feature does not touch that system.
- Role-change and remove-member capabilities already exist today (on the world dashboard's Campaign Settings panel) — this feature relocates that capability into the new Players section, which becomes the sole place to perform it (FR-011); it does not need to invent new authorization rules, only reuse the existing ones.
- "GM" throughout means the existing Owner-or-GM distinction (`isGm`) already used consistently elsewhere in this product (Compendium, Scenes, System Settings).
