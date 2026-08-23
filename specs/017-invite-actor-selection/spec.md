# Feature Specification: Player Onboarding — Invite-to-Actor Selection

**Feature Branch**: `017-invite-actor-selection`

**Created**: 2026-08-23

**Status**: Draft

**Input**: User description: "When a GM shares their world's invite link, a non-GM who follows that link and joins the world should land on a dedicated Actor Selection screen instead of going straight to the world dashboard/staging page empty-handed. On that screen, the joining player can either (a) pick an existing Actor in that world that the GM has designated as an available, unclaimed player character, or (b) create a brand-new Actor of their own to play as. Once picked/created, that Actor becomes 'claimed' by that player. Option (b) must be gated by a new per-world setting the GM controls: 'Allow players to create their own actors'. The Session Setup page should also surface the same invite URL for the GM to copy/distribute."

## Clarifications

### Session 2026-08-23

- Q: Should "Allow players to create their own actors" default to on or off for a newly-created world? → A: Off by default — a GM must explicitly opt in before a joining player can create their own character; the safer default for a GM who hasn't thought about it yet, consistent with this app's existing pattern of DM-gated creation (actors, items, lore are all DM-only-create by default elsewhere).
- Q: When the setting is off and there are zero GM-designated available characters, what does the joining player see? → A: A clear "ask your GM" wait state — the player sees a message explaining no characters are ready yet and that the GM needs to either designate one as available or turn on player-created characters, with no dead-end error and no silent redirect. The player remains a full world member in the meantime (can chat/view compendium, etc. per whatever the world's existing default-Viewer access already allows) — only the "which character am I" step is blocked.
- Q: Can a GM un-claim / reassign an already-claimed character (e.g., a player leaves, or the GM made a mistake)? → A: Yes — a GM can un-claim any character at any time (their existing Owner-level authority over every actor in their world, per spec 010's DM-always-full-control rule, already covers this; no new permission concept is needed), making it available again for another joining player to claim. The un-claimed player keeps their world membership; they just return to the "no character selected" state until they claim another.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - A joining player picks a GM-designated character (Priority: P1)

A GM has already created one or more player-character Actors and marked them as available for a joining player to claim. A new player follows the GM's invite link, registers or logs in, and joins the world. Instead of landing on the world dashboard with no context, they land on an Actor Selection screen listing every available (unclaimed) character. They pick one, and from that point on they're recognized as that character for play.

**Why this priority**: This is the core value of the feature — turning "you're in the world now, figure it out" into a guided first step. It's the simplest, safest path (GM has already prepared characters) and doesn't depend on the create-your-own setting at all, so it can ship and be tested independently of User Story 2.

**Independent Test**: As a GM, create two PC-classified Actors and mark both as available; generate an invite link; as a new user, follow the link, join, and confirm you land on an Actor Selection screen listing exactly those two characters (and no others); select one and confirm you're now recognized as that character; confirm the second character is no longer offered to a second joining player once the first is claimed... covered further in User Story 3.

**Acceptance Scenarios**:

1. **Given** a GM has marked one or more Actors as available for a joining player to claim, **When** a new member joins the world via an invite link, **Then** they land on an Actor Selection screen (not the world dashboard) listing every currently-available (unclaimed) character in that world.
2. **Given** a joining player is on the Actor Selection screen, **When** they select an available character, **Then** that character becomes claimed by them, they are recognized as that character going forward, and the character no longer appears as available to anyone else.
3. **Given** a world has zero available characters and "Allow players to create their own actors" is off, **When** a new member joins, **Then** they see a clear "ask your GM" wait state (per Clarifications) rather than an error, a dead end, or a silent redirect — and they remain a full world member while waiting.
4. **Given** a player has already claimed a character, **When** they revisit the world (e.g., log back in later), **Then** they are not shown the Actor Selection screen again — they go straight to the world dashboard/staging page as their claimed character.

---

### User Story 2 - A joining player creates their own character (Priority: P2)

A GM has turned on "Allow players to create their own actors" for their world. A new player follows the invite link, joins, and on the Actor Selection screen sees the option to create a brand-new character in addition to (or instead of) any GM-designated available ones. They create their own and are recognized as that character going forward.

**Why this priority**: This is the more flexible, opt-in path — valuable for GMs who prefer players to build their own characters, but it's additive to User Story 1's simpler flow and depends on the new world setting existing, so it's sequenced second.

**Independent Test**: As a GM, turn on "Allow players to create their own actors" for a world with zero available characters; as a new joining player, confirm the Actor Selection screen offers a "create your own" option; create a character and confirm you're recognized as it afterward; as a GM with the setting off, confirm the same option is absent from another player's Actor Selection screen.

**Acceptance Scenarios**:

1. **Given** "Allow players to create their own actors" is on for a world, **When** a new member reaches the Actor Selection screen, **Then** they see an option to create their own new character, alongside any GM-designated available characters (if any exist).
2. **Given** a joining player creates their own character on the Actor Selection screen, **When** creation completes, **Then** the new character is automatically claimed by that player (no separate claim step) and they are recognized as it going forward.
3. **Given** "Allow players to create their own actors" is off for a world, **When** a new member reaches the Actor Selection screen, **Then** no "create your own" option is shown, regardless of how many (or how few) GM-designated available characters exist.
4. **Given** a GM changes the setting from off to on (or on to off), **When** a player who has not yet claimed a character next views the Actor Selection screen, **Then** the screen reflects the current setting value, not whatever it was when they first joined.

---

### User Story 3 - The GM manages which characters are available and by whom they're claimed (Priority: P2)

A GM, from within the world's existing Actor management surfaces (the Compendium's NPC/Actor catalog, or an actor's detail view), marks a specific PC-classified Actor as "available for a joining player to claim," and can later un-mark it, or un-claim a character that's already been claimed (e.g., a player left, or a mistake was made), making it available again.

**Why this priority**: This is the GM-facing control surface that makes User Story 1 possible at all — a GM needs a way to actually designate and manage available characters, not just have the joining-player experience assume they exist. Priority P2 (not P1) because a GM could, in principle, manually communicate character assignments out-of-band for a first release, but this is clearly needed for the feature to be self-service.

**Independent Test**: As a GM, open an existing PC-classified Actor's detail view, mark it as available for claiming, confirm it now appears on the Actor Selection screen for a new joining player; after a player claims it, confirm the GM can see who claimed it and can un-claim it, after which it becomes available again.

**Acceptance Scenarios**:

1. **Given** a GM is viewing a PC-classified Actor they have Owner-level access to, **When** they mark it as available for a joining player to claim, **Then** it appears on the Actor Selection screen for any world member who has not yet claimed a character.
2. **Given** a GM is viewing a claimed character, **When** they look at it, **Then** they can see which world member currently has it claimed.
3. **Given** a GM un-claims an already-claimed character, **When** the un-claim completes, **Then** the character becomes available again on the Actor Selection screen, and the player who previously had it claimed returns to the "no character selected" state (per Clarifications) rather than being removed from the world.
4. **Given** a GM un-marks a character as available (without it being claimed), **When** the un-mark completes, **Then** it no longer appears on the Actor Selection screen but is otherwise unaffected (still exists, still editable, per the existing Actor system).

---

### Edge Cases

- What happens when the same player follows the invite link a second time after already having claimed a character? They should not be offered the Actor Selection screen again or be able to claim a second character — they go straight to the world (per User Story 1, Acceptance Scenario 4).
- What happens when two joining players try to claim the same available character at nearly the same instant? Exactly one claim succeeds; the other sees a clear "this character was just claimed" message and is returned to the (now-updated) Actor Selection screen rather than silently failing or double-claiming.
- What happens when a GM deletes a character that a player has already claimed? This follows the existing Actor deletion behavior (spec 010) — the spec does not introduce a special case; the player's "claimed character" association is naturally gone along with the deleted Actor, and they return to the "no character selected" state.
- What happens when the GM themself follows their own invite link? They are never routed to Actor Selection — the DM/GM role is unaffected by this feature entirely (per the request's explicit "a non-GM who follows that link").
- What happens on the Actor Selection screen when a player has Editor/Owner access (via an explicit ownership-block grant) to an Actor that is NOT marked available? It is not offered on the Actor Selection screen — "available for claiming" is a distinct, explicit GM action, not derived from existing ownership-block permissions (see Assumptions).

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The system MUST route a world member who has not yet claimed a character in that world to a dedicated Actor Selection screen, rather than the world dashboard or staging page, the first time they access that world after joining.
- **FR-002**: A world member who has already claimed a character in a world MUST NOT be shown the Actor Selection screen again for that world — they MUST be taken directly to the world as normal.
- **FR-003**: The Actor Selection screen MUST NEVER be shown to the world's DM/GM (Owner or GM role) — this feature applies to non-GM members only.
- **FR-004**: A GM (a world member holding the Owner or GM role, per the precedent in spec 010) MUST be able to mark any PC-classified Actor they have Owner-level access to as "available for a joining player to claim," and MUST be able to later un-mark it.
- **FR-005**: The Actor Selection screen MUST list every currently-available (marked-available, unclaimed) character in the world.
- **FR-006**: Selecting an available character on the Actor Selection screen MUST claim it for the selecting player, MUST remove it from the available list for everyone else, and MUST be atomic — if two members attempt to claim the same character concurrently, exactly one MUST succeed and the other MUST see a clear "already claimed" outcome (never a silent double-claim).
- **FR-007**: Every world MUST have an "Allow players to create their own actors" setting, controlled only by the GM, defaulting to off for newly-created worlds (per Clarifications).
- **FR-008**: The Actor Selection screen MUST offer a "create your own character" option if and only if the world's "Allow players to create their own actors" setting is currently on, evaluated at the time the screen is viewed (not cached from when the player first joined).
- **FR-009**: Creating a character via the Actor Selection screen's "create your own" option MUST automatically claim that new character for the creating player — no separate claim step.
- **FR-010**: When a joining player has zero available characters to choose from and the create-your-own option is unavailable, the Actor Selection screen MUST show a clear "ask your GM" wait state (per Clarifications) — never an error, a dead end, or a silent redirect elsewhere.
- **FR-011**: A world member in the "no character selected" wait state (per FR-010) MUST retain whatever baseline world access non-claimed members already have today (e.g., default-Viewer visibility into world content) — this feature MUST NOT reduce their existing access while they wait.
- **FR-012**: A GM MUST be able to see, for any claimed character, which world member currently has it claimed.
- **FR-013**: A GM MUST be able to un-claim any character in their world at any time (their existing Owner-level authority, per spec 010, already covers this — no new permission concept is introduced). Un-claiming MUST make the character available again and MUST return its previous claimant to the "no character selected" state, without removing them from the world.
- **FR-014**: A member's claimed-character association MUST be world-scoped — claiming a character in one world MUST have no effect on that member's status in any other world.
- **FR-015**: The Session Setup page (`/world/:id/staging`) MUST surface the same shareable invite URL already available from the world dashboard's "Generate Join Link" control, so a GM does not need to leave Session Setup to copy/distribute it.
- **FR-016**: "Available for claiming" MUST be a distinct, explicit GM action on an Actor, independent of that Actor's existing ownership-block permissions (Viewer/Editor/Owner) — a member with Editor/Owner access to an Actor that has not been explicitly marked available MUST NOT see it offered on the Actor Selection screen.

### Key Entities *(include if feature involves data)*

- **Actor Claim** (new): A world-scoped, one-to-one association between a claimed PC-classified Actor and the world member who claimed it. At most one active claim per Actor; at most one active claim per (world, member) pair. Created by selecting an available character or by creating a new one on the Actor Selection screen; removed by a GM un-claim action or by the Actor's deletion.
- **Actor Availability** (new): A GM-controlled flag on a PC-classified Actor marking it as offered to joining players on the Actor Selection screen. Independent of the Actor's ownership-block permissions (FR-016) and of whether it is currently claimed — an available-but-claimed Actor is not shown as an option; only available-and-unclaimed Actors are.
- **World Setting: Allow Player-Created Actors** (new): A single per-world boolean, GM-controlled, defaulting to off, gating whether the Actor Selection screen's "create your own" option is shown.
- **World Member** (existing, reused from spec 010): Gains an association with at most one claimed Actor per world; no other change to the World Member entity itself.
- **Actor** (existing, reused from spec 010): Gains the new Actor Availability flag and (indirectly, via Actor Claim) a notion of "who is playing this character," but no change to its existing fields, PC/NPC classification, or ownership-block model.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: 100% of new (non-GM) world members who join via an invite link land on the Actor Selection screen on their first visit, and 0% are shown it again after claiming a character.
- **SC-002**: A joining player can go from "just joined" to "recognized as a specific character" in 2 actions or fewer when at least one available character exists (open Actor Selection, select a character).
- **SC-003**: 100% of concurrent-claim attempts on the same character resolve to exactly one successful claim — zero instances of two members simultaneously recognized as the same character.
- **SC-004**: 100% of joining players who hit the zero-available/create-off state see the defined wait state, with zero instances of an error page, blank screen, or unexplained redirect in that situation.
- **SC-005**: A GM can generate and copy their world's invite link from Session Setup without navigating to the world dashboard, in 1 action.
- **SC-006**: A GM can un-claim a character and see it become available again for the next joining player in under 3 actions.

## Assumptions

- "Available for claiming" is intentionally a new, explicit flag rather than derived from the existing ownership-block model (Viewer/Editor/Owner) — those permissions answer "who can edit this Actor," a different question from "is this Actor currently offered as an unclaimed character to new joiners." Reusing ownership-block semantics for this would conflate the two and make it impossible for a GM to grant a co-GM Editor access to a PC without also offering it up for claiming.
- Only PC-classified Actors can be marked available or claimed — NPCs are out of scope for this feature, consistent with the existing PC/NPC classification's purpose (spec 010).
- This spec does not change anything about the GM's own experience following their own invite link, nor does it change the existing invite/join mechanics themselves (invite codes, expiry, usage caps) beyond exposing the same invite URL on one additional page (Session Setup, FR-015).
- The Actor Selection screen is a one-time-per-member-per-world gate (until un-claimed by a GM), not a repeatable "switch characters" self-service feature — a player who wants to play a different character needs the GM to un-claim their current one first (per FR-013), matching the request's framing of this as an onboarding step, not an ongoing character-switching system.
- No new notification/messaging system is introduced for the "ask your GM" wait state (FR-010) — the player sees the state when they view the Actor Selection screen; proactively notifying the GM that someone is waiting is not required by this spec and is left as potential future work.
