# Tasks: Genie Leveling, Session Resource Trading, and a Populated Demo World

**Input**: spec.md in this directory. Written retrospectively (implemented directly, not run through the full specify→clarify→plan→tasks ceremony), in the same verified-outcome style as spec 018's later tasks (T059-T063).

## Phase 1: Genie leveling (User Story 1)

- [X] T001 Add `trait_data.level` (optional, integer 1-10) to `packs/systems/genie/system.json` — confirmed: no `required` list on `trait_data`, so existing writes (e.g. the condition-toggle mutation) are unaffected.
- [X] T002 Extend `validate_trait_data` in `packs/systems/genie/server/src/validators.rs` to bounds-check `level` (1-10) when present — confirmed via 4 new unit tests (`trait_data_rejects_level_zero`, `trait_data_rejects_level_above_ten`, `trait_data_accepts_level_in_range`, `trait_data_accepts_no_level_at_all`), all passing.
- [X] T003 Add a "Resources" tab to `packs/systems/genie/web/src/components/CharacterSheet.tsx` (Level, Wish Points current/max, Health current/max), with new `GenieResourceData` type and `onLevelChange`/`onResourceChange` callback props — exported from `packs/systems/genie/web/src/index.ts` (including `calculateMaxWishPoints` itself, previously only reachable via the bundled manifest).
- [X] T004 Wire `resource_data`/`level` into `ActorDetailPage.tsx`'s `GenieActorSheet` (previously fetched `ability_data`/`proficiency_data`/`trait_data` but never `resource_data` at all) — level changes write both `trait_data.level` and a recalculated `resource_data.max_wish_points` via `calculateMaxWishPoints`, clamping `current_wish_points`.
- [X] T005 e2e: `apps/web/e2e/genie-leveling.spec.ts` — confirmed a level change from 1→5 recalculates max Wish Points 2→6 and persists across reload, verified via both the real UI and a direct `actorSystemData` GraphQL query.

## Phase 2: Session Resource trading (User Story 2)

- [X] T006 Add `genieTradeProposals(actorId)` query (`src/server/src/graphql/queries/genie_session.rs`), filtering `to_actor_id = actorId AND status = 'pending'`, authorized via `require_caller_controls_actor` (bumped to `pub(crate)` in `mutations_genie_session.rs`) — confirmed via 2 new unit tests (pending-only filtering, rejects a non-controlling caller).
- [X] T007 Add `fetchGenieTradeProposals` to `apps/web/src/api/genieSession.ts`; extend `useGenieSession.ts` with `myActor`/`partyMembers` (derived from `getWorldActors`) and `myHoldings`/`incomingProposals` (fetched once both session and `myActor` are known).
- [X] T008 Mount `SessionResourceTrade` in `GenieSessionPanel.tsx`, sourcing `currentUserId` from `useAuth()` directly (no new prop threaded through `WorldStagingRoutePage`), `resourceTypes` hardcoded from the manifest's stable `sessionResources` block.
- [X] T009 **Real bug found and fixed**: `caller_controls_actor` (`mutations_genie_session.rs`) checked only `world_actors.owned_by`; a player who *claimed* (not created) their character — spec 017's real onboarding path — got "You do not control this actor" for every Session Resource action. Fixed to also check `world_actor_claims` (joined through `world_members`). Confirmed via a new unit test (`a_player_who_only_claimed_their_character_controls_it_for_session_resources`) and, more importantly, a real two-account e2e run that reproduced the failure live before the fix and passed after it.
- [X] T010 e2e: `apps/web/e2e/genie-resource-trade.spec.ts` — two genuinely distinct accounts (GM + a player who joins via the real invite-and-claim flow, mirroring `invite-membership.spec.ts`'s established pattern), GM proposes a trade, player's own page renders it under "Incoming Trade Proposals" with the correct offer text. Found and fixed a second bug along the way: `apps/web/e2e/fixtures/helpers.ts`'s `register()` didn't wait for the post-registration redirect, letting a caller's immediate navigation abort it mid-flight — fixed at the source.
- [ ] T011 Not done: a UI to browse/decline/reject a trade proposal beyond accepting it — no `declineResourceTrade`/`rejectResourceTrade` mutation exists server-side. Out of scope per spec.md.
- [ ] T012 Not verified: Session Resource trading across two real *simultaneously connected* clients seeing each other's actions live — no GraphQL subscription transport exists client-side anywhere in this app (same gap spec 018 already documented for the rest of the session loop). `genie-resource-trade.spec.ts` proves the query/mutation/UI wiring is correct, not live sync.

## Phase 3: Populated demo seed (User Story 3)

- [X] T013 Add a second demo user (`e2edemo2`) + `world_members` row to `src/server/seeds/e2e_demo.sql` — confirmed `require_world_member` only falls back to `worlds.created_by` for the world's *creator*, so a second player genuinely needs an explicit row.
- [X] T014 Seed 2 items (`world_items`, no effects — see spec.md's Out of Scope) and 3 NPCs spanning size categories (diminutive/medium/colossal, one with an active condition) via `world_actors` + `world_actor_system_data`.
- [X] T015 Seed 2 PCs (one level 1 owned by the second demo user, one level 3 owned by the first) plus one `world_actor_inventory` grant.
- [X] T016 Verify: applied against a real Postgres twice in a row — first apply inserts everything (confirmed via `SELECT` against `world_actor_system_data`/`world_items`/`world_members`), second apply is a complete no-op (every `INSERT 0 0`). Also verified via the real UI: logged in as the demo user, opened a seeded PC's sheet (Resources tab showed the seeded level/Wish Points correctly), and drove a real propose→list resource trade between the two seeded demo users.
- [X] T017 Documented in `e2e_demo.sql`'s own comments: which JSONB fields are validator-checked only on the GraphQL mutation path (not by this raw SQL), and the exact allowed-value lists raw inserts must not violate, as a drift canary for future edits.

## Verification Summary

- `cargo test -p thunderforge genie` — 16/16 passing (14 pre-existing/spec-018 + 2 new: `genie_trade_proposals_*`, plus the claim-control regression test brings it to match; see Phase 2).
- `pnpm exec tsc --noEmit` / `pnpm run build` — clean in `apps/web`.
- Full genie e2e suite (12 tests across 9 spec files, `--workers=1` to avoid this dev environment's known parallel-worker resource contention) — 12/12 passing, including the 2 new specs added this pass (`genie-leveling.spec.ts`, `genie-resource-trade.spec.ts`) and no regressions in the 7 carried over from spec 018.
