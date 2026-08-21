# Feature Specification: Live Cross-Client Canvas Sync via GraphQL Subscriptions

**Feature Branch**: `005-live-canvas-sync`

**Created**: 2026-08-20

**Status**: Draft

**Input**: User description: "Live cross-client canvas sync via GraphQL subscriptions: Wire up a real, working GraphQL subscription transport in apps/web so that when one connected client changes a wall, light, shape, or token, every other client currently viewing the same scene sees that change appear live, within a few seconds, with no manual page reload required. Today this transport does not exist anywhere in the app — apps/web/src/engine/world/sync/{walls,lights,shapes,tokens}.ts each already have a fully-written inbound event-consumer function (e.g. startWallEventSync) designed to loop over a worldEventsCreated(worldId) GraphQL subscription and apply the change, but nothing in the app ever opens that subscription or feeds it — no apollo-client, no graphql-ws, no equivalent client exists. The server side already emits the underlying NOTIFY-driven world_events for wall/light/shape changes (eventCode-based, e.g. src/server/src/world_events.rs's EVENT_CODE_WALL_CHANGED) and already exposes worldEventsCreated(worldId) as a GraphQL subscription field; the gap is entirely client-side transport wiring, not new server-side eventing. This feature's job is to add the missing subscription client/transport, connect each of the four existing (but currently unused) inbound event-consumer functions to it, and verify — with a real two-browser-context test — that a change made in one session becomes visible in another connected session without a reload, for each of walls, lights, shapes, and tokens. Out of scope: any new event types beyond what world_events already emits; any change to the outbound mutation bridges (create/update/delete calls), which already work correctly today; and spec 004's token-specific canvas authoring work (drag/resize/rotate/ownership), which is being tracked separately and merely depends on this feature's transport existing for its own live-sync success criterion."

## Clarifications

### Session 2026-08-20

- Q: After a client reconnects following a dropped connection, should it fully re-fetch the current scene state, or just resume the subscription and rely on new events going forward? → A: Full re-fetch of scene state on reconnect (the same fetch a manual reload already does) — guarantees correctness for any change that occurred during the outage window, with no new server capability (event replay/backfill) required.
- Q: Should the client retry reconnection indefinitely in the background, or give up after some attempts and require a manual action? → A: Retry indefinitely with backoff — the client keeps attempting reconnection for as long as the tab is open (with increasing delay between attempts), showing a persistent "reconnecting" indicator throughout; there is no dead-end state the user must manually recover from.
- Folded in after planning: two pre-existing invite/world-membership bugs, found during spec 003's live verification and independently re-flagged by both a unit-test gap analysis (zero coverage on invite mutations) and an e2e gap analysis (no test in the entire project exercises a genuine non-owner account — every "second session" test reuses the GM's own login as a workaround), are now in scope as User Story 4 below. Fixing them directly unblocks verifying this feature's own User Story 1 with a real second account instead of a workaround, and is the single blocking issue behind the total absence of non-owner permission-boundary test coverage project-wide.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - A connected player sees a GM's wall/light/shape edit without reloading (Priority: P1)

A GM is running a live session. A player is already viewing the same scene in their own browser tab. The GM adds a wall, toggles a wall's passability, moves a light, or draws a shape. The player's view updates to reflect the change within a few seconds — they never have to refresh the page to see what the GM just did.

**Why this priority**: This is the foundational trust gap named across specs 001-004: every one of those specs' "connected player sees the change live" acceptance scenarios has been quietly relying on infrastructure that doesn't exist. Closing it is the single highest-value fix available, since it retroactively makes prior specs' stated (but never-actually-working) live-sync claims true.

**Independent Test**: With two browser contexts open on the same scene, make a wall, light, or shape change in one context and confirm it appears in the other within a few seconds, with no reload triggered in either context.

**Acceptance Scenarios**:

1. **Given** two clients viewing the same scene, **When** one client creates, updates, or deletes a wall, **Then** the other client's view reflects that change within a few seconds, without a page reload.
2. **Given** two clients viewing the same scene, **When** one client creates, updates, or deletes a light source, **Then** the other client's view reflects that change within a few seconds, without a page reload.
3. **Given** two clients viewing the same scene, **When** one client creates, updates, or deletes a shape annotation, **Then** the other client's view reflects that change within a few seconds, without a page reload.
4. **Given** a client's own tab makes a change, **When** that same change round-trips back via the new subscription transport, **Then** it does not double-apply or flicker — the client recognizes it already has the change (from its own optimistic update) and reconciles cleanly.

---

### User Story 2 - A connected client's live sync survives a brief disconnect (Priority: P2)

A player's network hiccups briefly (laptop sleep, wifi drop, tab backgrounded) while connected to a live session. When their connection is restored, their view catches up to the current state of the scene — they don't end up silently frozen on stale data with no indication anything is wrong.

**Why this priority**: A subscription transport that silently stops delivering events after any disconnect (without any recovery or visible signal) would be worse than today's "always requires a manual reload" baseline in one specific way — today's baseline is at least an consistent, known limitation; a transport that sometimes works and sometimes silently stalls is a harder-to-diagnose regression. Ranked P2 because the common case (User Story 1) delivers the primary value; this hardens it.

**Independent Test**: Establish a live connection, simulate a network interruption (e.g. briefly go offline), restore connectivity, and confirm the client either automatically resumes receiving live events or clearly signals that it is not currently live (rather than silently appearing live while missing events).

**Acceptance Scenarios**:

1. **Given** a client with an active live connection, **When** its network is briefly interrupted and then restored, **Then** the client automatically reestablishes the live connection and performs a full re-fetch of the current scene's walls/lights/shapes/tokens (the same fetch a manual reload already does), guaranteeing it reflects anything that changed during the outage, without requiring a manual page reload.
2. **Given** a live connection could not be reestablished automatically after a longer outage, **When** the client is still open, **Then** the client visibly indicates it is not currently receiving live updates (a persistent "reconnecting" indicator), rather than silently showing stale data as if it were current, and keeps attempting to reconnect with increasing delay between attempts for as long as the tab remains open — there is no dead-end state requiring a manual reload.

---

### User Story 3 - Reused by tokens once spec 004 lands (Priority: P3)

Once spec 004's token-specific canvas authoring (drag/resize/rotate/ownership) is implemented, a token's live position/size/rotation/photo change made by one client should appear to other connected clients the same way walls/lights/shapes already do, using this feature's transport — with no token-specific transport work required.

**Why this priority**: This feature's `tokens.ts` inbound consumer already exists and is structurally identical to the walls/lights/shapes ones; wiring the shared transport should make it work "for free." Ranked P3/lowest because it depends on spec 004's own token-authoring work for there to be anything meaningful to sync, and because it doesn't need a token-specific decision here beyond confirming the same transport applies uniformly.

**Independent Test**: With the transport from User Story 1 in place, confirm a token position change (from spec 004, once available, or from today's existing `upsert_token` path in the interim) is delivered to a second connected client the same way a wall change is, with no separate token-specific transport code required.

**Acceptance Scenarios**:

1. **Given** the subscription transport from User Story 1 is active, **When** a token's position, size, rotation, or photo changes, **Then** a second connected client sees that change within a few seconds, without a reload, via the same transport walls/lights/shapes use — not a separate mechanism.

---

### User Story 4 - A GM can invite a genuine second player into their world (Priority: P2)

A GM wants to test or actually run a session with a real second person — not their own account in a second browser tab. Today, generating an invite fails outright, and even a corrected invite would fail to let anyone join, because the world's own owner was never recorded as a member of their own world in the first place. This story fixes both, so a GM can invite a real second account and have it actually work.

**Why this priority**: Independent of the subscription-transport work in User Stories 1-3, but discovered during this feature's own planning as the reason no test anywhere in the project — including this feature's own User Story 1 — has ever been verified with a genuine second (non-owner) account rather than the GM's own login reused in a second browser context. Ranked P2 because User Stories 1-3 remain independently valuable and testable (with the same-account workaround) without this story, but this story is what upgrades every one of those tests from "simulated" to "real."

**Independent Test**: As a GM, generate an invite code for a world; as a second, genuinely distinct user account, use that code to join the world; confirm the second account is now a member (non-owner) and can view the world's scenes under the same GM-only authoring restrictions already enforced today.

**Acceptance Scenarios**:

1. **Given** a GM viewing their world's settings, **When** they generate an invite code, **Then** the request succeeds and returns a usable code, instead of today's argument-shape error.
2. **Given** a valid invite code, **When** a second, distinct user account redeems it, **Then** that account becomes a member of the world without error.
3. **Given** a newly created world, **When** the world is created, **Then** a world-membership record for the world's own owner already exists at that moment — no separate manual step is needed before invite generation or any other membership check works correctly.
4. **Given** a newly-joined non-owner member, **When** they view a scene in that world, **Then** they see the scene under the exact same GM-only authoring gates already enforced for every other feature (walls, lights, shapes, tokens) — this story introduces no new permission scope, only fixes the broken path to reach the existing one.

---

### Edge Cases

- What happens when a client has been open for a very long session (many hours)? (Expectation: the connection either persists or transparently reconnects; no requirement to close and reopen the tab periodically.)
- What happens if the same change is delivered twice (e.g. a reconnect replays an event already applied)? (Expectation: applying the same change twice must be idempotent — the final displayed state is correct either way, consistent with how the existing outbound optimistic-update path already reconciles.)
- What happens if a client is viewing a scene and the GM deletes that scene entirely while the client is connected? (Expectation: out of scope for this feature to define new behavior here — existing scene-deletion handling, whatever it is today, is unchanged; this feature only concerns already-supported wall/light/shape/token change events.)
- What happens to a client that never successfully establishes the subscription at all (e.g. transport unsupported in an unusual environment)? (Expectation: the client falls back to today's existing behavior — its own changes still work optimistically, and other clients' changes require a manual reload to see — rather than crashing or breaking the page.)

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The system MUST establish a live GraphQL subscription connection from each connected client to the server, scoped to the world/scene the client is currently viewing.
- **FR-002**: The system MUST deliver wall create/update/delete events to all other clients connected to the same scene, causing their view to reflect the change within a few seconds, without a manual reload.
- **FR-003**: The system MUST deliver light-source create/update/delete events to all other clients connected to the same scene, causing their view to reflect the change within a few seconds, without a manual reload.
- **FR-004**: The system MUST deliver shape-annotation create/update/delete events to all other clients connected to the same scene, causing their view to reflect the change within a few seconds, without a manual reload.
- **FR-005**: The system MUST connect the existing `startWallEventSync`, light, and shape inbound event-consumer functions to the new transport, reusing their existing event-handling logic rather than rewriting it.
- **FR-006**: The system MUST NOT change the existing outbound mutation bridges (the create/update/delete GraphQL mutation calls and their optimistic local updates), which already function correctly today.
- **FR-007**: The system MUST reconcile a client's own change (already applied optimistically) against that same change arriving back via the subscription, without visible duplication, flicker, or reversion.
- **FR-008**: The system MUST automatically attempt to reestablish the live connection after a brief network interruption, without requiring the user to reload the page.
- **FR-008a**: The system MUST, upon successfully reestablishing the live connection after any interruption, perform a full re-fetch of the current scene's walls, lights, shapes, and tokens — the same fetch already used for a manual page reload — to guarantee correctness for any change that occurred during the outage window, rather than relying solely on the subscription to have delivered it.
- **FR-009**: The system MUST visibly indicate to the user when the live connection is not currently active (e.g. after a reconnection attempt fails), rather than silently displaying potentially stale data as if it were live.
- **FR-009a**: The system MUST retry reconnection indefinitely, with increasing delay between attempts, for as long as the client remains open — there MUST be no dead-end state that requires a manual reload or other explicit user action to resume reconnection attempts.
- **FR-010**: The system MUST continue to function for a client whose subscription connection cannot be established at all — that client's own changes still work as they do today, only its visibility of *other* clients' changes falls back to requiring a manual reload.
- **FR-011**: The system MUST NOT introduce any new server-side event type beyond what `world_events` already emits for walls/lights/shapes/tokens today.
- **FR-012**: The system MUST fix the invite-generation request so it succeeds (correcting the argument-shape mismatch between the client's invite call and the resolver's expected input) rather than failing outright.
- **FR-013**: The system MUST ensure a world's own owner has a world-membership record from the moment the world is created, rather than requiring a separate, currently-nonexistent manual step.
- **FR-014**: The system MUST allow a second, distinct user account to redeem a valid invite code and become a non-owner member of that world.
- **FR-015**: The system MUST NOT introduce any new permission scope or authorization mechanism as part of this fix — a newly-joined member is subject to exactly the same GM-only authoring gates already enforced today for walls, lights, shapes, and tokens.

### Key Entities

- **World event** (existing entity, unchanged shape): the server-emitted, `eventCode`-based change record already produced for wall/light/shape/token mutations (`src/server/src/world_events.rs`). This feature adds a delivery mechanism for events that already exist; it does not add new event types or change their shape.
- **Subscription connection** (new, client-side, not persisted): the live transport connection a client maintains to receive `worldEventsCreated(worldId)` events for as long as it is viewing a world/scene. Not a database entity — connection state lives only in the browser tab for its lifetime.
- **World membership** (existing entity, corrected lifecycle): the existing `world_members` record associating a user with a world. This feature does not change its shape, only guarantees a row is created for the world's owner at world-creation time (User Story 4), which today never happens.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A wall, light, or shape change made by one connected client is visible to another connected client within 5 seconds, with zero manual reloads, in 100% of manual verification attempts.
- **SC-002**: A client's own change never visibly double-applies, flickers, or reverts when it round-trips back through the new subscription transport, in 100% of manual verification attempts.
- **SC-003**: After a simulated brief network interruption, a client automatically resumes live updates and re-fetches current scene state without a manual reload, correctly reflecting any change made during the outage, in 100% of manual verification attempts.
- **SC-004**: When a client's live connection is not active, that state is visibly indicated to the user rather than silently presented as current, and the client continues attempting to reconnect in the background with no manual action required, in 100% of manual verification attempts.
- **SC-005**: This feature closes the specific gap identified during spec 003 and spec 004 planning: after this feature ships, spec 004's SC-002 (live cross-client token sync) becomes achievable using this feature's transport, with no token-specific transport work.
- **SC-006**: A GM can generate a working invite and a genuinely distinct second account can join their world, in 100% of manual verification attempts — replacing today's 100% failure rate on this path.
- **SC-007**: After this feature ships, at least one existing or new test in the project exercises a genuine non-owner account (not the GM's own login reused), closing the permission-boundary test blind spot flagged by both the unit-test and e2e gap analyses.

## Assumptions

- The server-side `worldEventsCreated(worldId)` GraphQL subscription field and its underlying `world_events`/NOTIFY mechanism already work correctly and need no server-side change — confirmed during spec 003/004 research; this feature is client-transport-only.
- "A few seconds" matches the same latency bar used throughout specs 001-004 for live-sync claims — no new, stricter performance target is introduced here.
- The four existing inbound event-consumer functions (`startWallEventSync` and its light/shape/token equivalents) are correctly written against the `worldEventsCreated` shape and need only to be connected to a real transport, not rewritten — per the codebase's own documentation comments (`walls.ts`/`lights.ts`/`shapes.ts`), confirmed during spec 003's implementation work.
- Choice of subscription client library (e.g. `graphql-ws`, Apollo Client, or a minimal hand-rolled WebSocket client) is an implementation-time decision, not fixed by this spec — whichever is chosen must integrate with the existing world store dispatch pattern these four files already use.
- This feature does not address scene-switch loading/error UX (spec 004 User Story 4) or token-specific canvas authoring (spec 004 User Stories 1-3) — those remain separate, spec 004's territory; this feature only supplies the transport spec 004's own live-sync success criterion depends on.
- User Story 4 (invite/membership fix) is independent of User Stories 1-3's subscription transport work — different files, no shared code path — and can be implemented in either order or in parallel.
- The invite/membership fix is scoped strictly to making the existing, already-designed invite/membership flow actually work (fixing two concrete bugs); it does not redesign invites, add new roles, or change who can invite whom — that authorization model is unchanged.
