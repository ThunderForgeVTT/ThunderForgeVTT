# Implementation Plan: Genie Session Resource Economy — Grants, NPC Shops, Quest/Contract Rewards

**Branch**: `020-genie-economy-and-shops` | **Date**: 2026-08-24 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/020-genie-economy-and-shops/spec.md`

## Summary

Session Resources (Insight/Favor/Essence) currently have no way to enter the
economy — holdings can only move via a trade or a Puzzle Clock spend, both of
which require holdings to already exist. This plan adds the three missing
entry points, each reusing an existing primitive rather than inventing new
infrastructure: (1) a GM-only `grantSessionResource` mutation plus reuse of
the existing `addItemToInventory` mutation for item grants, with an optional
per-world resource-carryover setting; (2) a `world_genie_shop_listings` table
and atomic `purchaseFromShop` mutation, reusing NPC `world_actor_inventory`
as shop stock; (3) a `world_genie_puzzle_clock_rewards` table plus an
`actorId`-aware `advancePuzzleClock`, so Puzzle Clock segments can each carry
a configured resource/item payout. All new mutations follow the existing
`mutations_genie_session.rs` transaction and NOTIFY (`EVENT_CODE_GENIE_SESSION_STATE`)
patterns so spec 018's live cross-client sync covers them for free.

## Technical Context

**Language/Version**: Rust 1.75+ (server, native target), TypeScript/React (web)

**Primary Dependencies**: Axum, async-graphql, Diesel/PostgreSQL (server); React, existing `apps/web/src/api/genieSession.ts` + `useGenieSession.ts` (web)

**Storage**: PostgreSQL via Diesel migrations under `src/server/migrations/`

**Testing**: `cargo test` (server-side mutation/authorization/transaction tests, mirroring `mutations_genie_session.rs`'s existing test module and `genie_wish_granted_item_round_trips_through_inventory`), Playwright e2e (`apps/web/e2e/`, mirroring `genie-resource-trade.spec.ts`'s two-context pattern) for cross-client visibility

**Target Platform**: Linux server (native `cargo check`/`cargo test`), browser (web app build/tsc)

**Project Type**: Web application (Rust/Axum/GraphQL backend + React frontend), extending an existing feature area (Genie game system pack) rather than a new project

**Performance Goals**: No new performance targets beyond existing mutation latency norms (single-transaction DB round trip per mutation, consistent with `accept_resource_trade_impl`)

**Constraints**: Shop purchase and puzzle-clock-reward grants MUST be atomic (single DB transaction, no partial state on failure — FR-005, FR-005a, FR-006); all three mutation families MUST be authorized server-side per Constitution Principle III

**Scale/Scope**: Single-world, single-session scope, same order of magnitude as existing Genie session mutations (a handful of players, a handful of shop listings/reward entries per clock) — no scale unknowns

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

- **Principle I (ECS Owns Simulation)**: N/A — this feature touches no canvas/engine state; it's server-side game-economy data plus React panels, matching the rest of the Genie session loop.
- **Principle II (Plugin-Modular Engine)**: N/A — no Bevy engine changes.
- **Principle III (Ownership & Authorization at the Data Boundary)**: Satisfied by design — `grantSessionResource`/`createShopListing`/puzzle-clock-reward configuration are GM-only (`is_dm_of_world`, the established pattern); `purchaseFromShop` is buyer-initiated but server-validates affordability and stock atomically; every new table carries the existing provenance/authorization conventions (session/world scoping, no new ownership model invented). PASS.
- **Principle IV (Real ADRs and Specs Before Divergent Implementation)**: This plan *is* that artifact — spec 020 already exists and is clarified; this plan + its Phase 0/1 outputs land alongside implementation, not retroactively. No new ADR is triggered — this extends an already-ADR'd subsystem (Genie session loop, two-party-consent trade ADR from spec 019) rather than introducing a new architectural boundary. PASS.
- **Principle V (Verify Before Claiming Done)**: Implementation phase (not this plan) will run `cargo check`/`cargo test` (server) and `tsc`/Playwright (web) before claiming done, per existing project convention. N/A to gate at planning time; noted for the tasks phase.
- **DMCA Guardrail**: N/A — this feature does not expose one world's content beyond that world; shops/grants/rewards are entirely within-world.

No violations. Complexity Tracking section not needed.

## Project Structure

### Documentation (this feature)

```text
specs/020-genie-economy-and-shops/
├── plan.md              # This file
├── research.md           # Phase 0 output
├── data-model.md          # Phase 1 output
├── contracts/
│   └── genie-economy.md   # Phase 1 output
└── quickstart.md          # Phase 1 output
```

(`tasks.md` is Phase 2, produced by `/speckit-tasks`, not this command.)

### Source Code (repository root)

Extends the existing Genie system pack + core server GraphQL layer — no new
top-level project. Concretely:

```text
src/server/
├── migrations/
│   ├── <timestamp>_genie_resource_carryover_setting/  # up.sql/down.sql
│   ├── <timestamp>_genie_shop_listings/               # up.sql/down.sql
│   └── <timestamp>_genie_puzzle_clock_rewards/        # up.sql/down.sql
├── src/
│   ├── schema.rs                          # regenerated (diesel migration run)
│   ├── models.rs                          # + GenieShopListing, GeniePuzzleClockReward, etc.
│   └── graphql/
│       └── mutations_genie_session.rs     # + grantSessionResource, createShopListing,
│                                           #   purchaseFromShop, configurePuzzleClockReward,
│                                           #   advancePuzzleClock gains optional actorId

apps/web/src/
├── api/genieSession.ts                    # + grantSessionResource, shop listing/purchase,
│                                           #   puzzle-clock-reward client calls
├── hooks/useGenieSession.ts                # + grant/purchase/reward action callbacks
└── packs/systems/genie/web/src/components/  # + GM grant UI, NPC shop UI, clock reward config UI
```

**Structure Decision**: This is not a new frontend/backend split — it's an
extension of the existing Genie pack + core GraphQL server, following the
exact file layout `mutations_genie_session.rs` / `useGenieSession.ts` /
`packs/systems/genie/web` already establish. No new top-level directories.

## Complexity Tracking

*No Constitution Check violations — this section is not applicable.*
