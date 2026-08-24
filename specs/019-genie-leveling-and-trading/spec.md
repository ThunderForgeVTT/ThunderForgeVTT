# Feature Specification: Genie Leveling, Session Resource Trading, and a Populated Demo World

**Feature Branch**: `019-genie-leveling-and-trading`

**Created**: 2026-08-24

**Status**: Implemented

**Input**: Continuation of spec 018 (Genie house system). Two of the nine quickstart scenarios were still open after 018's UI-wiring pass — Scenario 6 (Wish Points scale on level-up) and the resource-trading half of Scenario 8 — and the e2e demo world (`src/server/seeds/e2e_demo.sql`) had no items, NPCs, or second player to exercise movement/trading/inventory by hand. This spec closes all three.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - A character's Wish Points scale with level (Priority: P1, spec 018 Scenario 6)

Genie had no `level` concept anywhere in its data model — not in `system.json`, not in any validator, not in any UI — despite already having a level-keyed `wishPoints` table in its manifest and a `calculateMaxWishPoints` calculator that nothing ever called. A player should be able to set their character's level and see their maximum Wish Points recalculate accordingly, matching the manifest's table.

**Why this priority**: This was the last of spec 018's nine quickstart scenarios blocked on more than "just mount an existing component" — level genuinely didn't exist yet, unlike conditions/session-loop which just needed wiring.

**Independent Test**: On a Genie character's sheet, change the level field; confirm the displayed max Wish Points updates to match the manifest's `wishPoints` table for that level, and confirm the change persists across a reload.

**Acceptance Scenarios**:

1. **Given** a Genie character with no level set, **When** their sheet loads, **Then** they default to level 1 with max Wish Points 2 (the manifest's level-1 entry).
2. **Given** a character at level 1, **When** a GM/owner sets their level to 5, **Then** their max Wish Points updates to 6 (the manifest's level-5 entry) and this is persisted server-side (`trait_data.level` and `resource_data.max_wish_points` both survive a reload).
3. **Given** a level change, **When** the new max Wish Points is lower than the character's current Wish Points, **Then** current Wish Points is clamped down to the new max rather than left inconsistent.

---

### User Story 2 - Players discover and act on a proposed Session Resource trade (Priority: P1, spec 018 Scenario 8 remainder)

Spec 018 wired the Session Wish Pool and Doom/Puzzle Clocks, but not Session Resource trading — the backend had `proposeResourceTrade`/`acceptResourceTrade` mutations with no way for the *recipient* of a proposal to ever discover it existed. A player should see a trade proposed to them and be able to accept it.

**Why this priority**: Without this, Session Resources (Insight/Favor/Essence) — one of the three pillars of Genie's design (dice, topology, the escape-room resource loop) — has no usable player-facing path at all, even though the backend fully implements it and has its own test coverage.

**Independent Test**: As a GM, propose a trade to a party member's character; as that party member, confirm the proposal appears listed with the correct offer, and that accepting it is possible when funded.

**Acceptance Scenarios**:

1. **Given** a GM proposes a trade naming another party member's character as the recipient, **When** that character's controller views the session panel, **Then** the proposal appears under "Incoming Trade Proposals" with the correct resource types/quantities and proposer name.
2. **Given** a player who *claimed* their character (the normal onboarding path — a GM creates the character, the player claims it) rather than one they created themselves, **When** they propose or are offered a Session Resource trade, **Then** they are recognized as controlling that character (not rejected with "you do not control this actor").

---

### User Story 3 - A populated demo world for manual exploration (Priority: P2)

The e2e demo world (`src/server/seeds/e2e_demo.sql`) had one empty scene and no items, NPCs, or second player — nothing to click around and exercise by hand. A developer should be able to bring the stack up and immediately have NPCs of varying size, a couple of items, two player characters (one leveled), and a second real account to test multiplayer-shaped features (trading, session membership) against.

**Why this priority**: Lower priority than the two real feature gaps above — this is a developer-experience improvement, not a missing capability — but was the original trigger for this pass ("add some items and npc and players to the demo game so we can exercise them").

**Independent Test**: Apply the seed against a fresh database; confirm (via the real GraphQL API, not just direct SQL inspection) that the items, NPCs, PCs, and second player/membership are all queryable and correctly shaped.

**Acceptance Scenarios**:

1. **Given** a freshly migrated database, **When** `e2e_demo.sql` is applied, **Then** it idempotently seeds 2 items, 3 NPCs spanning size categories (diminutive/medium/colossal), 2 PCs (one level 1, one level 3) with an inventory item on one, and a second demo user with `world_members` access to the demo world.
2. **Given** the seed has already been applied once, **When** it is applied again, **Then** every insert is a no-op (`ON CONFLICT DO NOTHING`) and the DELETE-then-reseed Genie session reset still leaves the rest of the world data untouched.

## Functional Requirements

- **FR-001**: `packs/systems/genie/system.json`'s `trait_data` block MUST declare an optional `level` field (integer, 1-10, matching the `wishPoints` table's range).
- **FR-002**: `packs/systems/genie/server`'s trait_data validator MUST reject a `level` outside 1-10 when present, and MUST NOT require `level` to be present (existing actors/mutations must not break).
- **FR-003**: `packs/systems/genie/web`'s `CharacterSheet` MUST expose a "Resources" tab showing Level, Wish Points (current/max), and Health (current/max), editable when the sheet is editable.
- **FR-004**: A level change MUST update both `trait_data.level` and `resource_data.max_wish_points` (via `calculateMaxWishPoints`), clamping `current_wish_points` down if it now exceeds the new max.
- **FR-005**: The backend MUST expose a `genieTradeProposals(actorId)` query returning that actor's still-pending incoming trade proposals, authorized the same way `acceptResourceTrade` already is.
- **FR-006**: The Session Resource "who controls this actor" check (`caller_controls_actor`) MUST recognize both direct ownership (`world_actors.owned_by`) and a claim (`world_actor_claims` via `world_members`) — not ownership alone.
- **FR-007**: `GenieSessionPanel` MUST mount `SessionResourceTrade`, deriving the viewer's own character and party roster from real world-actor data (accounting for claims, not just ownership).
- **FR-008**: `src/server/seeds/e2e_demo.sql` MUST remain a single idempotent file, safe to re-apply, adding items/NPCs/PCs/a second player without requiring any new tooling.

## Out of Scope

- Live cross-client sync for the session loop (Wish Pool/Doom Clock/trades) — no GraphQL subscription transport exists client-side anywhere in this app yet; this was true before this spec and remains true after it.
- Item effects (`world_item_effects`) on the seeded demo items — left effect-less to avoid a raw-SQL validator-drift risk; addable by hand through the real Items UI.
- A UI to browse/decline a trade proposal beyond accept (no "decline" mutation exists server-side).

## Notes

Two real, non-trivial bugs were found and fixed while implementing and e2e-testing this spec (not scope creep — both directly blocked User Story 2 from working for a real player):

1. `caller_controls_actor` (`src/server/src/graphql/mutations_genie_session.rs`) checked only `world_actors.owned_by`, which never changes when a player claims a GM-created character (spec 017's actual onboarding path) — every Session Resource action was silently broken for any player who joined a world normally rather than creating their own actor.
2. `apps/web/e2e/fixtures/helpers.ts`'s shared `register()` test helper didn't wait for the post-registration redirect before returning, which could abort the register mutation mid-flight if a caller navigated immediately after (as a new two-real-account e2e spec for this feature did) — fixed at the source so future specs don't hit the same race.
