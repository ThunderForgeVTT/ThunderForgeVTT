# Feature Specification: Token Authoring Polish — Real Resize/Rotate Handles & Reliable Ownership Assignment

**Feature Branch**: `006-token-authoring-polish`

**Created**: 2026-08-20

**Status**: Draft

**Input**: Direct follow-up from spec 004 (canvas-native token authoring), closing out its three remaining open items found during implementation: (1) resize/rotate currently ships as undiscoverable-by-default keyboard shortcuts rather than canvas-rendered drag handles, because the engine-side token plugin restructuring needed to build real handles properly (Constitution Principle II) didn't fit in that session; (2) the player-owned-token e2e test (drag own token, GM-granted additional token, primary-photo edit, no create control) is written but `test.skip`-ed — it hangs on a Radix Popover auto-dismissal race in `TokenPanel`'s ownership-assignment UI that two real fixes narrowed but didn't fully resolve; (3) spec 004's quickstart.md walkthrough was never executed as one connected pass confirming all of SC-001 through SC-006 hold together, only per-task via automated suites.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - A GM resizes and rotates a token using real canvas handles (Priority: P1)

A GM selects a token on the canvas and sees the same kind of interactive handles already available on walls and shapes — small draggable grips at the token's corners for resizing and a separate grip for rotating — rather than having to know and use keyboard shortcuts. Dragging a resize handle snaps the token to whole grid-cell increments; dragging the rotate handle changes facing independently.

**Why this priority**: This is the direct completion of spec 004 User Story 2, which shipped a functionally-equivalent but not interaction-consistent stand-in (keyboard shortcuts) as a deliberate, documented time-budget tradeoff. Closing this gap is the single highest-value remaining item — it's the difference between "the capability exists if you know the secret keys" and "the capability is discoverable the same way every other canvas tool already is."

**Independent Test**: On a scene with an existing token selected, a GM drags a corner handle to resize (confirming whole-grid-cell snapping) and drags a separate rotate handle to change facing, independently, with both persisting after reload and syncing to a connected player — with no keyboard shortcut required at any point.

**Acceptance Scenarios**:

1. **Given** a token is selected on the canvas, **When** the GM looks at it, **Then** resize and rotate handles are visibly rendered on the token, the same way wall endpoint handles and shape corner handles already render on selection.
2. **Given** a token's resize handle, **When** the GM drags it, **Then** the token's footprint grows or shrinks in whole grid-cell increments (1×1, 2×2, 3×3...), never a fractional cell, matching spec 004's existing resize behavior.
3. **Given** a token's rotate handle, **When** the GM drags it, **Then** the token's facing changes continuously (not in fixed 30° steps), independent of any resize in progress.
4. **Given** a non-GM player viewing the scene, **When** they look at any token, including one they control, **Then** no resize/rotate handles are rendered for them (unchanged from spec 004's existing gating).
5. **Given** the existing keyboard shortcuts (`]`/`[`/`,`/`.`) from spec 004, **When** this feature ships, **Then** they may remain as a secondary input method or be removed — either is acceptable, but the canvas handles are the primary, discoverable mechanism.

---

### User Story 2 - Assigning token ownership in TokenPanel never hangs the UI (Priority: P1)

A GM assigns a token to a specific player (setting `ownerUserId` and, optionally, `isPrimary`) using TokenPanel's existing owner-input field and primary-token checkbox. This interaction completes reliably every time — the popover showing these controls does not unexpectedly close mid-interaction, and the checkbox visibly reflects the change without hanging or requiring a page reload.

**Why this priority**: This blocks the one meaningful piece of spec 004 that still isn't live-verified (User Story 3's player-owned-token behavior) — the server-side authorization is already correct and tested, but nobody can currently prove the GM-facing assignment UI itself is reliable, because the UI hangs before the test can even finish clicking the checkbox. This is a P1 alongside User Story 1 because it's blocking real usage, not just test coverage — a real GM hitting this popover-closing race in a live session would have the same broken experience the stuck test demonstrates.

**Independent Test**: As a GM, open a token's ownership controls in TokenPanel, set an owner and check "primary," and confirm the checkbox reflects checked state within a couple of seconds with no popover closing unexpectedly and no UI hang — repeated across several tokens/attempts to confirm it isn't just occasionally lucky.

**Acceptance Scenarios**:

1. **Given** a GM has a token's ownership popover open, **When** they fill the owner-input field and move focus away from it (by any means — clicking elsewhere in the popover, pressing Tab, or otherwise), **Then** the popover remains open and the primary-token checkbox becomes interactable.
2. **Given** the primary-token checkbox is interactable, **When** the GM clicks it, **Then** it becomes checked within a couple of seconds and stays checked — no hang, no silent revert, no requirement to reload the page.
3. **Given** this fix, **When** spec 004's `test.skip`-ed player-owned-token end-to-end test is un-skipped, **Then** it passes reliably (not just once) against a live dev stack.

---

### Edge Cases

- What happens if a GM opens the ownership popover, sets an owner, but never touches the primary checkbox? (Expectation: unchanged from today — ownership is set, primary status is whatever it already was; this feature doesn't change that logic, only the interaction reliability.)
- What happens if two rapid resize-handle drags occur in quick succession (e.g., a fast double-drag)? (Expectation: each completes and persists correctly, consistent with how rapid wall-endpoint drags already behave — no new debouncing/queueing mechanism is introduced by this feature.)
- What happens if a GM drags a token's rotate handle to a full 360°+ rotation? (Expectation: normalized to a 0-360° or equivalent range, consistent with how rotation is already stored as a float; no new range validation beyond what already exists.)

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The system MUST render a resize handle and a separate rotate handle on a selected token's canvas representation, visible only to the GM (or a player controlling that specific token, if consistent with spec 004's existing per-token control model — GM-only rendering is the default/simpler choice if this needs to be decided at implementation time).
- **FR-002**: The system MUST allow the GM to drag the resize handle to change the token's footprint in whole grid-cell increments, matching spec 004's existing 1×1-5×5 range and snapping behavior.
- **FR-003**: The system MUST allow the GM to drag the rotate handle to change the token's facing continuously, independent of any concurrent resize.
- **FR-004**: The system MUST persist resize/rotate changes made via canvas handles through the same `update_token` mutation path spec 004 already established — no new mutation.
- **FR-005**: The system MUST continue to hide resize/rotate handles from non-GM players, unchanged from spec 004's existing `IsGameMaster`-gated behavior.
- **FR-006**: The system MUST ensure TokenPanel's ownership-assignment popover (owner-input field, primary-token checkbox) does not close unexpectedly as a result of normal interaction with its own contents (filling a field, moving focus between its own controls).
- **FR-007**: The system MUST ensure the primary-token checkbox reliably reflects a GM's click within a couple of seconds, with no hang and no silent revert requiring a page reload.
- **FR-008**: The system MUST NOT change the underlying `update_token`/ownership-assignment authorization logic — this feature fixes UI interaction reliability only, not server-side behavior, which spec 004 already implemented and tested correctly.

### Key Entities

- **Token** (existing entity, unchanged shape): no new fields — this feature changes interaction mechanism (canvas handles vs. keyboard shortcuts) and UI reliability (popover behavior), not the token data model spec 004 already established.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A GM can resize and rotate a token entirely via canvas-rendered handles, with zero keyboard shortcuts required, in under 10 seconds per adjustment.
- **SC-002**: Assigning a token's ownership and primary status via TokenPanel completes successfully (checkbox reflects the change, popover stays open through the interaction) in 100% of manual verification attempts across at least 5 repeated tries.
- **SC-003**: Spec 004's previously-`test.skip`-ed player-owned-token end-to-end test passes in 3 consecutive live runs against a real dev stack, with no hang.
- **SC-004**: Spec 004's quickstart.md Scenarios 1-3 are executed as one connected walkthrough (not just per-task automated checks) confirming SC-001 through SC-006 from that spec all hold together.

## Assumptions

- The engine-side restructuring needed for User Story 1 (growing `src/engine/src/plugins/token.rs` from its current placeholder into a real plugin, per Constitution Principle II, mirroring `WallPlugin`/`ShapePlugin`) is an implementation-time concern, not a user-facing requirement in itself — it's the enabling work behind FR-001-003, not a separate acceptance scenario.
- User Story 2's root cause (a second, unconfirmed Radix Popover dismissal trigger, per spec 004's tasks.md) may turn out to need a different fix approach than initially guessed once properly instrumented (e.g. React DevTools/render logging) — this spec doesn't prescribe the exact mechanism, only the required outcome (FR-006/FR-007).
- This spec does not include spec 005's subscription-transport work (User Stories 1-3 there, live cross-client sync for walls/lights/shapes/tokens) — that remains its own, already-fully-planned feature, tracked separately and unaffected by this one.
- Keyboard shortcuts from spec 004 may be kept as a secondary/power-user input path or removed entirely; this spec does not mandate either, only that canvas handles become the primary mechanism (per Edge Cases and Acceptance Scenario 5).
