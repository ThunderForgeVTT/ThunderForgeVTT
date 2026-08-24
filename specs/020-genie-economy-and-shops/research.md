# Phase 0 Research: Genie Session Resource Economy

Spec 020's own "Research Summary" section already resolved most technical
unknowns against the current codebase (item-inventory primitive, holdings
key shape, NPC-inventory-as-stock, no generic currency, no quest-tracking
precedent). This document resolves the remaining open implementation
decisions the spec deliberately left to planning.

## R1 — Resource carryover storage: column vs. side table

**Decision**: A single nullable-with-default boolean column,
`worlds.genie_resource_carryover_enabled`, directly on the `worlds` table.
Default `false`.

**Rationale**: `WorldSystemSettingsPage.tsx` was checked (per spec 020's
Configuration section instruction) — it only handles *which* system a world
runs (`gameSystemId`) and where to surface that system's legal notice
(spec 016). There is no existing per-system-settings side-table pattern
anywhere in this codebase to follow; the only comparable precedent
(dnd5e's `CurrencyPurse`) lives inside that pack's own `resource_data` JSON,
not a side table. A single boolean is not enough surface area to justify a
new `world_genie_settings(world_id, ...)` table (YAGNI — one column, one
flag, one pack). If a second Genie-specific world setting is ever needed,
promote to a side table then.

**Alternatives considered**:
- `world_genie_settings` side table — rejected as premature for a single
  boolean; adds a join for every `startGenieSession` call with no present
  second consumer.
- Storing it in Genie's existing per-actor `resource_data` JSON (dnd5e's
  pattern) — rejected because carryover is a *world*-level GM setting, not
  per-actor data; it would need to live redundantly on every actor or on a
  synthetic "world settings actor," both worse than a column.

## R2 — Transaction pattern for `purchaseFromShop` and clock-reward grants

**Decision**: Mirror `accept_resource_trade_impl`'s existing pattern exactly
(`mutations_genie_session.rs`) — a single Diesel `conn.transaction(|conn| { ... })`
closure per mutation, doing all reads/writes/NOTIFY-row-insert inside it, so
a mid-transaction error rolls back every partial write.

**Rationale**: This is already the established in-repo pattern for
multi-step, must-be-atomic Genie session mutations (two-holdings update in
one trade accept). FR-005/FR-005a/FR-006 explicitly require this shape.
Reusing it means no new transaction idiom to review or test.

**Alternatives considered**: Application-level optimistic locking / retry
loop — rejected as unnecessary complexity; Postgres row-level transactions
plus the FR-005a atomic conditional `UPDATE ... WHERE quantity > 0` already
give correct concurrent behavior with less code.

## R3 — `world_events` NOTIFY payload `kind` values

**Decision**: Reuse `EVENT_CODE_GENIE_SESSION_STATE` (15) — no new event
code — with new string `kind` payload values: `"resource_grant"`,
`"purchase"`, `"clock_reward"`, joining the existing kinds already used for
wish/doom-clock/puzzle-clock/trade state changes.

**Rationale**: FR-007 states this explicitly, and it's consistent with
every other Genie session mutation (spec 018/019) — the frontend's
`applyGenieSessionWorldEvent`/`startGenieSessionEventSync` already
dispatches on `kind` inside event code 15's payload, so adding new `kind`
values is additive and needs no new subscription wiring, only new `kind`
branches in that existing dispatcher and in `useGenieSession.ts`'s handlers
(both already call generic `refetch()`/`refetchTrades()` on *any* kind
today, so even an unhandled new `kind` would still trigger a correct
refetch — new branches are for precision/minimizing unnecessary refetches,
not correctness).

**Alternatives considered**: A new dedicated event code per mutation type —
rejected; would require new subscription-side dispatch wiring for no
behavioral gain, and fragments what is conceptually still "Genie session
state changed."

## R4 — Puzzle Clock reward "exactly once" guarantee

**Decision**: `granted_at` (nullable timestamp) on each
`world_genie_puzzle_clock_rewards` row, set the instant that entry's reward
fires. `advancePuzzleClock`'s transaction selects only reward rows for the
newly-reached segment(s) where `granted_at IS NULL`, grants them, and sets
`granted_at` in the same transaction — a second call at the same segment
(if it could ever happen) finds no ungranted rows left and is a no-op for
rewards (segment update behavior unchanged).

**Rationale**: Matches FR-006's "exactly once" requirement and User Story
3's Acceptance Scenario 3. Using a nullable timestamp (vs. a boolean) also
gives free grant-history/audit data for the "Forge Daggers" production-run
case, at no extra cost.

**Alternatives considered**: A separate `granted_reward_ids` array/join
table tracked outside the reward row — rejected, adds a table for no
behavioral difference over a nullable column.

## R5 — Test strategy

**Decision**: Server-side `cargo test` coverage mirrors
`mutations_genie_session.rs`'s existing test module structure — one test
per Functional Requirement's happy path and its explicit rejection paths
(no active session, non-GM caller, insufficient resources/items, concurrent
last-unit purchase via two overlapping transactions). Cross-client
visibility (FR-007's live sync) gets one Playwright e2e spec, mirroring
`genie-resource-trade.spec.ts`'s two-browser-context pattern: GM grants in
context A, player's `SessionResourceTrade` panel updates in context B
without reload.

**Rationale**: Both patterns already exist and are proven in this codebase
for the closely-related spec 018/019 features — no new testing
infrastructure needed.

**Alternatives considered**: None — this is a direct precedent match, not
an open question.

## Outcome

All Technical Context unknowns are resolved; no `NEEDS CLARIFICATION`
markers remain. Proceeding to Phase 1.
