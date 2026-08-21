# Quickstart: Token Authoring Polish

Validation scenarios below map to spec.md's Acceptance Scenarios. Run against a local dev stack (`docker compose up`, server, `apps/web` dev server).

## Prerequisites

- Local dev stack running, a GM test account owning a world with an existing token on a scene
- A second, distinct test account (per spec 005 US4's invite fix) for the non-GM gating check

## Scenario 1 — Real canvas resize/rotate handles (US1)

1. As the GM, select a token on the canvas.
2. Confirm a resize handle and a separate rotate handle are visibly rendered on the token (no keyboard shortcut needed to discover them).
3. Drag the resize handle; confirm the footprint changes only in whole grid-cell increments.
4. Drag the rotate handle; confirm facing changes continuously and independently of size.
5. Reload; confirm both persist. As a connected second session, confirm the change is visible (per spec 004's existing sync path; live no-reload sync remains spec 005's territory).
6. As the second, non-GM test account, view the same token; confirm no resize/rotate handles render for them.

**Expected**: matches FR-001–FR-005, SC-001.

## Scenario 2 — Reliable ownership assignment (US2)

1. As the GM, open a token's ownership popover in TokenPanel.
2. Fill the owner-input field and move focus away (Tab, or click elsewhere in the popover); confirm the popover stays open.
3. Click the primary-token checkbox; confirm it becomes checked within a couple of seconds, with no hang.
4. Repeat steps 1-3 for at least 5 different tokens/attempts; confirm 100% reliability.
5. Run `apps/web/e2e/token-authoring.spec.ts`'s previously-`test.skip`-ed player-owned-token test (now un-skipped) 3 consecutive times; confirm it passes every time.

**Expected**: matches FR-006–FR-008, SC-002, SC-003.

## Scenario 3 — Full suite as connected walkthrough (T039 equivalent)

Run the complete `token-authoring.spec.ts` suite (all tests, none skipped) as one pass, confirming every scenario from spec 004's quickstart.md holds together — not just individually.

**Expected**: matches SC-004; per research.md §4, this run itself satisfies spec 004's original T039 ask.
