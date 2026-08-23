# Quickstart: DMCA Notice-and-Takedown Process

## Prerequisites

- Server running locally with the `content_moderation_actions` migration applied.
- A test world with at least one `world_actor`, one `world_item`, and one lore entry, owned by a test account.
- GraphQL playground/client pointed at the local server.

## Scenario 1 — Submit a valid takedown notice and confirm the entry disappears (User Story 1)

1. Note a target entity's `entityType`/`entityId` (e.g. a `world_actor`).
2. Call `submitTakedownNotice` with all required fields populated and both statement booleans `true`.
3. **Expect**: response is a `GraphQLModerationCase` with `currentStatus: CONTENT_DISABLED`.
4. As the world's owning account, query the world's actor list.
   **Expect**: the disabled actor is absent from the list.
5. Query the disabled actor directly by ID.
   **Expect**: a moderation placeholder is returned (`moderated: true`), not the actor's real data — even though the caller is the owner.
6. Confirm every other actor/item/lore entry in the world is still fully readable (SC-002 — blast radius is exactly one entry).

## Scenario 2 — Reject an incomplete notice (Edge case)

1. Call `submitTakedownNotice` omitting `accuracyStatement`.
2. **Expect**: response's `currentStatus: NOTICE_REJECTED_INCOMPLETE`, `missingElements` lists `accuracyStatement`.
3. Confirm the target entity remains fully visible — no disable action occurred.

## Scenario 3 — Counter-notice and automatic restoration (User Story 2)

1. Using the `caseId` from Scenario 1, call `submitCounterNotice` as the owning account with all required fields.
2. **Expect**: `currentStatus: COUNTER_NOTICE_FORWARDED`, `restorationDueAt` set to now + the configured waiting period.
3. Advance the server clock (or use a test hook) past `restorationDueAt` with no further `resolveModerationCase` call from compliance staff.
4. **Expect**: case status auto-transitions to `CONTENT_RESTORED`; the entity is visible again in list and single-entity queries.

## Scenario 4 — Compliance staff blocks restoration (User Story 2, Scenario 3)

1. Repeat steps 1-2 of Scenario 3 to reach `COUNTER_NOTICE_FORWARDED`.
2. Before `restorationDueAt`, as compliance staff, call `resolveModerationCase(caseId, CONTENT_REMAINS_DISABLED)`.
3. **Expect**: the entity stays disabled past what would have been the restoration date.

## Scenario 5 — Repeat-infringer flagging (User Story 3)

1. Create three separate valid takedown cases against the same test account, each reaching `CONTENT_DISABLED` with no counter-notice, within the configured lookback window.
2. Query `repeatInfringerFlags`.
   **Expect**: the test account's ID is present.

## Scenario 6 — Guardrail review gate (User Story 4)

This scenario is a documentation/process check, not a runtime test: confirm `docs/adrs/<next-number>-content-moderation-and-dmca-safe-harbor.md` and this spec are linked from wherever the project's feature/launch-review checklist lives, so that a future "public compendium sharing" proposal is required to reference Scenarios 1-5 above as evidence the moderation program is operational before that proposal can proceed.

## Public-facing surfaces to manually verify (FR-001, FR-002)

- `/legal/dmca` (or equivalent route) renders the designated agent's name, mailing address, and electronic contact, reachable without authentication.
- The takedown intake form is reachable from that same page and submits into `submitTakedownNotice` above.
