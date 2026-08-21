# Feature Specification: Canvas-Native Token Authoring & Scene-Switch Loading Feedback

**Feature Branch**: `004-token-canvas-authoring`

**Created**: 2026-08-20

**Status**: Draft

**Input**: User description: "Canvas-native token authoring & scene-switch loading feedback: Give tokens the same first-class canvas tooling that walls and shapes already have — a GM (and, where permitted, players moving their own token) can click-and-drag a token directly on the canvas to place or reposition it, and use resize/rotate handles to adjust its footprint, instead of relying solely on the existing TokenPanel modal/list (which stays for bulk management, avatar/health-bar editing, and non-canvas token creation). All canvas-driven moves/resizes must go through the same GraphQL move/update mutations and RxDB sync path TokenPanel already uses today, so behavior stays consistent whether a token is moved via the panel or the canvas. Separately, when a GM switches the active scene via the existing SceneSwitcher dropdown, all connected clients (GM and players) should see clear loading feedback (spinner/skeleton state) while the new scene's background image, walls, lights, and tokens load, and a clear error state if loading fails (e.g. background asset unreachable) — replacing whatever currently happens (likely an abrupt/blank transition) during that window. Out of scope for this spec: campaign/world lifecycle (creating worlds, launching campaigns, per-GM join URLs, pausing a world to block player movement) — that is a separate future spec. Also out of scope: token type/visual differentiation (NPC/vehicle/player art) — noted as a known future gap (MVP.md Phase 4) but not part of this spec's scope."

## Clarifications

### Session 2026-08-20

- Q: Can a single player have more than one token assigned to them at once, or is it always exactly one token per player? → A: A player may be granted control of multiple tokens by the GM (e.g. a companion/summon), but each player has exactly one designated **primary token** (their default/profile token). Players cannot create tokens themselves. A player may update their primary token's photo/avatar directly; that field is expected to later tie into a character sheet (out of scope for this spec).
- Q: Should token resizing be constrained to whole grid-cell increments, or allow free continuous resizing? → A: Grid-cell increments only (1×1, 2×2, 3×3, ...), matching standard TTRPG creature-size convention.
- Q: When a scene switch fails to load, should the system offer a retry action, or just show a static error message? → A: Retry action — the error state includes a way to retry loading the same scene without switching away and back.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - GM drags a token directly on the canvas (Priority: P1)

A GM running a live session wants to reposition a token — a monster that moves, an NPC that's dragged into position — without opening a separate panel, breaking the flow of the encounter. They click and drag the token's icon directly on the map canvas, and it moves smoothly to the new spot, exactly like dragging a mini on a physical table.

**Why this priority**: This is the single biggest everyday friction point today: every token move currently requires leaving the canvas for a modal list. This is the MVP — without it, the feature delivers nothing new.

**Independent Test**: On a scene with an existing token, a GM drags it from one grid location to another directly on the canvas (no panel opened) and confirms the token's new position persists after a page reload.

**Acceptance Scenarios**:

1. **Given** a scene with a token placed on it, **When** the GM clicks and drags the token to a new location on the canvas, **Then** the token visually follows the drag in real time and settles at the drop location.
2. **Given** a token has just been dragged to a new position, **When** the move completes, **Then** the token's new position is persisted and still correct after the scene is reloaded.
3. **Given** a GM has dragged a token to a new position, **When** a connected player views the same scene, **Then** the player sees the token at its new position within a few seconds, with no reload required.
4. **Given** the existing TokenPanel is open, **When** the GM instead moves the same token via the canvas drag, **Then** the panel's displayed position for that token updates to match — the two paths never show conflicting state.

---

### User Story 2 - Resize and rotate a token's footprint via canvas handles (Priority: P2)

A GM placing a large creature (e.g. a dragon spanning multiple grid squares) or orienting a directional token (e.g. a cone-shaped creature or a vehicle) needs to adjust its size and facing directly on the canvas, using the same kind of handle-based interaction already available for wall endpoints and shape corners.

**Why this priority**: Builds directly on User Story 1's drag capability and reuses the same interaction paradigm, but is a smaller and less frequently needed adjustment than basic repositioning — most tokens are placed at default size/facing and only occasionally resized or rotated.

**Independent Test**: On a scene with an existing token selected, a GM drags a resize handle to change its footprint and a rotate handle to change its facing, independently of each other, and confirms both persist after reload.

**Acceptance Scenarios**:

1. **Given** a token is selected on the canvas, **When** the GM drags a resize handle, **Then** the token's footprint grows or shrinks in whole grid-cell increments (1×1, 2×2, 3×3, ...), never landing on a fractional-cell size.
2. **Given** a token is selected on the canvas, **When** the GM drags a rotate handle, **Then** the token's facing changes accordingly, independent of its size.
3. **Given** a token has been resized and rotated, **When** the scene is reloaded, **Then** both the new size and new facing are exactly as left.
4. **Given** a token's size or facing has changed, **When** a connected player views the same scene, **Then** the player sees the updated size/facing within a few seconds, with no reload required.

---

### User Story 3 - A player repositions their own token (Priority: P2)

A player wants to move their own character's token during their turn — stepping into a doorway, retreating from melee — directly on the canvas, without asking the GM to do it for them via the panel. Each player has exactly one **primary token** (their default character token, which they can also re-skin with their own photo/avatar), and may additionally be granted control of other tokens by the GM (e.g. a companion or summoned creature) — but players never create tokens themselves; that remains GM-only.

**Why this priority**: Extends the same canvas-drag mechanic to players, which is valuable but depends on User Story 1's drag mechanic already existing, and only applies to tokens a player is permitted to control (not every token on the scene).

**Independent Test**: As a player (non-GM) connected to a scene containing a token assigned to them, drag that token to a new location on the canvas and confirm it moves and persists; then confirm a token *not* assigned to that player cannot be dragged by them; separately, confirm the player can change their primary token's photo but not create a new token.

**Acceptance Scenarios**:

1. **Given** a player is viewing a scene containing a token assigned to them, **When** they drag that token on the canvas, **Then** it moves and the new position is persisted and synced to the GM and other players.
2. **Given** the same scene contains a token not assigned to that player (e.g. an NPC or another player's token), **When** the player attempts to drag it, **Then** the drag has no effect and the token does not move.
3. **Given** a player without GM privileges is viewing the scene, **When** they look at the canvas, **Then** they see no resize/rotate handles on tokens they don't control, even though the GM sees handles on every token.
4. **Given** a GM has granted a player control of an additional token (e.g. a summoned creature) beyond their primary token, **When** that player drags either their primary token or the additionally-granted token, **Then** both moves succeed identically.
5. **Given** a player viewing their own primary token, **When** they change its photo/avatar, **Then** the new image is saved and visible to the GM and other players; **When** the same player attempts to create a brand-new token, **Then** no such control is available to them.

---

### User Story 4 - Clear loading and error feedback when switching scenes (Priority: P2)

A GM switches the active scene mid-session using the existing scene dropdown. Today the transition may appear abrupt or blank while the new scene's background, walls, lights, and tokens load. The GM (and connected players) should instead see an obvious, reassuring loading state, and — if something goes wrong (e.g. the background image can't be fetched) — a clear error message rather than a silently broken or blank canvas.

**Why this priority**: Independent of the token-authoring work in User Stories 1-3; addresses a separate but related "usability of interacting with the canvas" pain point named in the original request. Ranked P2 because it's a polish/trust improvement on an already-functional path (scene switching itself already works), not a missing capability.

**Independent Test**: Trigger a scene switch via the existing SceneSwitcher dropdown and confirm a loading indicator appears immediately and clears once the new scene is fully rendered; separately, simulate a failed background-asset load and confirm a visible error state appears instead of a blank or stuck canvas.

**Acceptance Scenarios**:

1. **Given** a GM selects a different scene from the SceneSwitcher, **When** the new scene's data (background, walls, lights, tokens) is still loading, **Then** a loading indicator is visible on the canvas area for the GM.
2. **Given** the same scene switch, **When** the new scene finishes loading, **Then** the loading indicator disappears and the fully-rendered scene is shown.
3. **Given** a connected player is viewing the previous scene, **When** the GM switches scenes, **Then** the player's view also shows the loading indicator and then the new scene, without requiring a manual page reload.
4. **Given** a scene switch is triggered, **When** the new scene's background image fails to load (e.g. the asset is unreachable), **Then** the GM and connected players see a clear, visible error state instead of a blank or indefinitely-loading canvas, with a retry action available that re-attempts loading the same scene without needing to switch away and back.

---

### Edge Cases

- What happens when a GM drags a token to a position outside the scene's canvas bounds, or on top of another token? (Expectation: the drop is either clamped to bounds or rejected with the token snapping back — exact behavior decided at implementation time, but it must never silently lose the token or leave it in an unrecoverable off-canvas position.)
- What happens if a token is dragged onto a wall segment that blocks movement? (Consistent with existing wall-passability behavior — this spec does not change movement-blocking rules, only how a token gets moved.)
- What happens if two users (e.g. GM and the assigned player) try to drag the same token at the same moment? (Expectation: last-write-wins, consistent with existing move-token mutation behavior; no new conflict-resolution mechanism is introduced by this spec.)
- What happens if a scene switch is triggered again before the previous switch's loading finished? (Expectation: the latest switch wins; the loading indicator continues seamlessly reflecting the most recent target scene, not two overlapping loads.)
- What happens if a resize/rotate handle drag is released outside the canvas area? (Expectation: same clamping/snap-back behavior as an out-of-bounds move.)

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The system MUST allow a GM to reposition an existing token by clicking and dragging it directly on the scene canvas.
- **FR-002**: The system MUST visually move the token in real time as it is dragged, prior to the drop being finalized.
- **FR-003**: The system MUST persist a token's new position, immediately upon drop, through the same mutation path already used by the existing token panel's move action.
- **FR-004**: The system MUST reflect a token's canvas-driven position change to all other connected clients viewing the same scene within a few seconds, without requiring a manual reload.
- **FR-005**: The system MUST keep the existing token panel's displayed state consistent with canvas-driven changes to the same token (position, size, facing) and vice versa.
- **FR-006**: The system MUST allow a GM to resize a selected token's footprint via a canvas-rendered resize handle, constrained to whole grid-cell increments (1×1, 2×2, 3×3, ...) — never a fractional-cell size.
- **FR-007**: The system MUST allow a GM to change a selected token's facing/rotation via a canvas-rendered rotate handle, independent of the resize handle.
- **FR-008**: The system MUST persist a token's size and rotation changes made via canvas handles, and reflect them to other connected clients within a few seconds.
- **FR-009**: The system MUST allow a player (non-GM) to drag a token on the canvas only if that token is their primary token or a token the GM has additionally granted them control of; dragging any other token by that player MUST have no effect.
- **FR-009a**: The system MUST designate exactly one token per player as that player's primary token, and MUST allow that player to change its photo/avatar directly.
- **FR-009b**: The system MUST NOT allow a player to create a new token; token creation remains GM-only, unchanged from today.
- **FR-010**: The system MUST NOT render resize/rotate handles to a non-GM user for tokens they do not control.
- **FR-011**: The system MUST display a loading indicator on the canvas for all connected clients (GM and players) while a newly-selected scene's background, walls, lights, and tokens are being fetched, following a scene switch initiated via the existing scene switcher.
- **FR-012**: The system MUST clear the loading indicator and display the fully-loaded scene once all of the new scene's background, walls, lights, and tokens have loaded successfully.
- **FR-013**: The system MUST display a distinct, visible error state — instead of a blank or indefinitely-loading canvas — if any required part of a newly-selected scene (in particular its background image) fails to load.
- **FR-013a**: The system MUST offer a retry action from the error state that re-attempts loading the same scene, without requiring the GM to switch away to a different scene and back.
- **FR-014**: The system MUST NOT change or degrade existing wall-passability, door, or lighting behavior as a result of this feature's token-authoring changes.
- **FR-015**: The system MUST NOT introduce a new authorization mechanism for token movement — GM-level and player-own-token permissions MUST reuse the existing scene-ownership / token-assignment checks already enforced by the token move mutation.

### Key Entities

- **Token** (existing entity, minimally extended): the on-canvas representation of a creature or object with position, size, and (per this feature) rotation/facing. This feature adds canvas-driven interaction paths to an entity that already exists, and adds a per-player "primary token" designation (one token flagged as a given player's default/profile token) plus player-editable photo/avatar on that token; a future character-sheet feature is expected to build on this designation but is out of scope here.
- **Scene** (existing entity, unchanged shape): the active map a GM switches between; this feature adds client-side loading/error state around an already-existing scene-switch action, not a new persisted concept.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A GM can reposition a token entirely via canvas drag, with zero uses of the token panel, in under 5 seconds per move.
- **SC-002**: A token's position, size, or rotation change made on the canvas is visible to a connected player within 5 seconds, with no manual reload.
- **SC-003**: 100% of token moves attempted by a player on a token not assigned to them are rejected with no visible token movement.
- **SC-004**: 100% of scene switches show a loading indicator that clears automatically once the destination scene is fully rendered, with no persistent blank-canvas period observed during manual verification.
- **SC-005**: A simulated failed scene-asset load produces a visible, distinguishable error state in 100% of manual verification attempts, with no case of an indefinitely-stuck loading indicator.
- **SC-006**: From the error state, using the retry action successfully loads the scene once the underlying failure (e.g. asset availability) is resolved, with no need to switch away and back.

## Assumptions

- Tokens already have a persisted position (x/y) and size; this spec assumes rotation/facing either already exists as a field or is a minimal, no-migration-if-possible addition to the existing token shape — exact persistence detail is a planning-phase decision, not fixed here.
- "Assigned to that player" covers both a player's one designated primary token and any additional tokens the GM has granted them control of; the underlying mechanism reuses whatever existing owner/controller association the token panel or engine token-loading systems use today, extended minimally (a primary-token flag/reference) rather than redesigned.
- The existing TokenPanel modal remains in place unchanged for bulk management, avatar/health-bar editing, and non-canvas token creation — this spec adds a second, canvas-native interaction path alongside it, not a replacement.
- Token type/visual differentiation (NPC/vehicle/player art, per MVP.md Phase 4) is explicitly out of scope, as is all campaign/world lifecycle work (world creation, campaign launch, join URLs, pausing a world) — those remain separate future specs.
- Token resize is constrained to whole grid-cell increments (1×1, 2×2, 3×3, ...), matching standard TTRPG creature-size convention, rather than free continuous pixel resizing.
- "A few seconds" for cross-client sync matches the same live-sync latency bar already established and verified by specs 001-003 for walls, shapes, and lights — no new performance target is introduced.
- **Known dependency, discovered during spec 003's implementation**: no live GraphQL subscription transport currently exists anywhere in the app (`apps/web/src/engine/world/sync/{walls,lights,shapes,tokens}.ts` all only coalesce *outbound* changes; none consume the server's `worldEventsCreated` events). Today, a second connected client only sees another user's canvas change after a manual page reload, for every entity type — not just tokens. FR-004/FR-008's "reflect to other connected clients within a few seconds" and SC-002 therefore depend on a separate, dedicated feature (tracked as its own spec) that wires up that transport. This spec's own-client behavior (drag/resize/rotate/photo changes appearing instantly for the user making them, and correctly on next reload for everyone) does not depend on it and can ship independently; only the *live* (no-reload) cross-client visibility bar does.
