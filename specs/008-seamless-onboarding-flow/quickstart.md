# Quickstart: Seamless Sign-Up-to-Canvas Onboarding Flow

Validation scenarios for this feature, run against a live local dev stack (`pnpm dev` / `make dev`). Each scenario maps to an acceptance scenario in spec.md.

## Prerequisites

- Local dev stack running, migrations applied.
- A way to create a fresh account per scenario (no prior worlds).

## Scenario 1 — Zero-world user reaches the canvas in 2 forms, 0 modals (US1, FR-001, SC-001)

1. Register a brand-new account.
2. **Expect**: you land directly on the create-world form (`/worlds/create`) — no `/welcome` hub content is ever rendered, no extra click.
3. Fill in only a world name, submit.
4. **Expect**: you land directly on `/world/:id/play` — never the dashboard.
5. **Expect**: while the engine loads, a visible status indicator is shown continuously (see Scenario 3) — never a flat, unindicated background.
6. **Expect**: once the engine is ready, the canvas already shows the world's default scene rendered — no "New scene" modal appears first.

Total count check: 2 forms (register, create-world), 0 modals, 0 dashboard stop — matches SC-001's pinned target.

## Scenario 2 — Nothing looks configurable when it isn't (US2, FR-005, SC-003)

1. On the create-world form, confirm only name and description fields are present — no game-system or interface-pack selector.
2. Log in as an existing user with at least one world; open that world's dashboard (via the hub or worlds list, not at creation time).
3. **Expect**: every panel shown reflects real data for that world (or the panel isn't there at all) — no empty placeholder panel for a feature that doesn't exist yet.

## Scenario 3 — Honest engine-load feedback (US1 AC2/AC4, FR-002/FR-003, SC-002)

1. Enter any world's canvas.
2. **Expect**: from the moment the page renders until the engine signals ready, a status indicator is visible continuously — e.g. "Loading engine…" transitioning to a ready state, never a silent gap.
3. Simulate an engine load failure (e.g. block the WASM asset request in devtools).
4. **Expect**: a clear error state renders in the same location, not a blank/static screen indistinguishable from "still loading."

## Scenario 4 — Returning users always see the hub, one-click shortcuts (US3, FR-001a, FR-009, SC-005)

1. As a user with exactly one existing world, log in.
2. **Expect**: you land on the hub screen (not auto-entered into that world), showing that world as a one-click shortcut.
3. Repeat with a user who has multiple worlds. **Expect**: the same hub, all worlds shown as shortcuts.
4. Repeat with a returning user whose worlds were all since deleted (zero accessible worlds). **Expect**: routed to the same zero-worlds create-world path as Scenario 1 — no hub, no empty "your worlds" section.

## Scenario 5 — Invite-code path works for both existing and brand-new accounts (US2 AC3, FR-007, FR-012, SC-004)

1. As a logged-in user with at least one world, use the hub's invite-code entry to enter a valid code for a world you're not yet a member of. **Expect**: taken directly into that world.
2. Log out. Visit a `/join/:code` link directly while unauthenticated. **Expect**: redirected to login with the code preserved.
3. From that login screen, click through to registration instead. **Expect**: the invite code is still preserved after registering, and you land in the target world without re-entering the code.

## Scenario 6 — Error states preserve user input (FR-011, SC-006)

1. On the create-world form, enter a name that will collide or trigger a validation error (per existing world-name validation rules), submit.
2. **Expect**: the form re-renders with your entered name/description intact and a specific error message — not a blank form or a generic failure.
