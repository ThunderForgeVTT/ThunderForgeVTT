# Implementation Plan: Genie House System

**Branch**: `018-genie-house-system` | **Date**: 2026-08-23 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/018-genie-house-system/spec.md`

## Summary

Ship `packs/systems/genie/` as a new, wholly-original system pack that deliberately routes through every existing generic subsystem rather than inventing bespoke storage: character/NPC data rides the existing actor `data_types` contract, the Manifestation roll is a formula string resolved by `crates/thunderforge-dice` (spec 014), Wish-Granted Items are ordinary `world_items` rows with a formula-bearing effect (spec 013), and the Patron/lineage link is an ordinary `world_lore_entries` reference (spec 012). The one genuine new capability this pack requires — a scene actually usable in gridless/zone mode — is *not* new engine work: `GridType::Gridless` already exists in `src/engine/src/resources/scene_data.rs` and is already handled (as a no-op) in `src/engine/src/plugins/grid.rs`; the only real gap found during planning is that the `scenes.grid_type` database CHECK constraint only permits `'square'`/`'hex'` today, so `'gridless'` is unreachable from the data layer. Closing that one-column gap, plus wiring real (non-no-op) zone-based token interaction, is in scope for this plan precisely because surfacing exactly this kind of gap is Genie's stated purpose.

Following the 2026-08-23 clarification session, Genie also ships its actual **playable co-op loop** (User Story 7): a shared Session Wish Pool (3 wishes), a session-wide Doom Clock, one or more Puzzle Clocks, and tradeable Session Resources — all session-scoped, shared, live-synced state. This is genuinely new persisted data (three small tables), but the *transport* for keeping it live-synced across every connected player is not new: `world_events` + `worldEventsCreated(worldId)` (spec 005) already broadcasts generic, discriminated JSON payloads to every connected client scoped to a world, currently for codes 10-14 (walls/lights/shapes/map-import/tokens). Genie's session state rides that exact same pipeline under one new `event_code` (15), rather than inventing new sync infrastructure — the second concrete instance in this plan of the engine already having anticipated a need Genie is the first system to actually exercise.

## Technical Context

**Language/Version**: Rust (server: manifest/validators via `crates/pack_system_spec`, actor system data; engine: `src/engine/src/plugins/grid.rs` gridless interaction) — same toolchain as existing system packs; TypeScript/React (`apps/web`) for the character sheet, token size rendering, and condition indicators

**Primary Dependencies**: `crates/pack_system_spec` (`SystemManifest`/`SystemManifestLegal`, already implemented per spec 016 — confirmed present at `crates/pack_system_spec/src/lib.rs`), `crates/thunderforge-dice` (spec 014, for the Manifestation roll formula and `rollDice` mutation), the existing `world_items`/`world_item_effects` tables (spec 013), `world_lore_entries` (spec 012), the existing actor `data_types` validation path (`packs/systems/dnd5e/server/src/validators.rs` is the precedent to follow), `GameSystemContext`/`SystemManifest` (`apps/web/src/contexts/GameSystemContext.tsx`) for web-side manifest loading, and — new for the session loop — `world_events`/`record_world_event` (`src/server/src/world_events.rs`) plus the existing `worldEventsCreated(worldId)` subscription and its client-side sync pattern (`apps/web/src/engine/world/sync/*.ts`, spec 005) for broadcasting Session Wish Pool/clock/resource-trade changes live

**Storage**: Genie's character/NPC/item/lore data rides the generic tables from specs 011-014 with zero new columns, as before. Two narrow schema additions are now in scope: (1) extending `scenes.grid_type`'s CHECK constraint (`src/server/migrations/2026-05-05-010000-0001_create_scenes_table/up.sql`) to permit `'gridless'` alongside `'square'`/`'hex'`, via a new migration; (2) three small new tables for the session loop's genuinely new state — `world_genie_sessions` (wish pool + Doom Clock), `world_genie_puzzle_clocks`, `world_genie_resource_holdings` (data-model.md) — since no existing table models session-scoped shared party state. Both are additive migrations; neither edits an already-applied migration file, per existing Diesel convention.

**Testing**: `cargo test` (native) for manifest validation (`packs/systems/genie/server`, mirroring `packs/systems/dnd5e/server/src/validators.test.rs`), for the Manifestation roll formula resolving correctly through `crates/thunderforge-dice`, and for the new session-state mutations (`spendWish`, `advanceDoomClock`, `advancePuzzleClock`, `tradeSessionResource`) emitting correctly-shaped `event_code = 15` events; `cargo check --target wasm32-unknown-unknown` for the engine-crate gridless-interaction change; Vitest/RTL for the character sheet, token size/footprint rendering, condition indicators, and the new session-loop components (wish pool, Doom Clock, Puzzle Clocks, resource trading); a running dev instance exercised for the scene-topology-switch UI and a full multi-client session-loop playthrough per Constitution Principle V

**Target Platform**: Web (React) + native server (pack loading, manifest/data validation) + WASM engine (canvas/token rendering, gridless zone interaction)

**Project Type**: Web application — new `packs/systems/genie/` following the existing `packs/systems/dnd5e/` three-package layout (`engine/`, `server/`, `web/`)

**Performance Goals**: Not performance-sensitive beyond existing per-system-pack norms — Genie introduces no new hot path; the Manifestation roll's exploding-dice chain is bounded by the dice engine's own existing limits (spec 014), not a new constraint

**Constraints**: FR-011 (no external SRD dependency) means Genie's `system.json` `legal` object declares original content with empty `requiredNotice`/`trademarkRestrictions` and no `sourceUrl` — verified against the already-implemented `SystemManifestLegal` struct's optional fields, no new manifest-contract work needed. FR-002's dual-topology requirement is bounded to *using* the existing `GridType::Gridless` variant, not inventing a third topology concept. FR-013/FR-015/FR-018's live-sync requirement is bounded to *reusing* the existing `world_events` broadcast pipeline under a new event code, not building a second real-time transport.

**Scale/Scope**: One system pack (`packs/systems/genie/engine`, `/server`, `/web`), two schema migrations (grid_type CHECK constraint; three new session-state tables), one engine-plugin change (`plugins/grid.rs`'s gridless match arm gains real interaction logic instead of `()`), one new `world_events` event code (15) and four new GraphQL mutations for the session loop.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

- **Principle I (ECS owns simulation)**: PASS — gridless token interaction is implemented inside the existing `grid.rs` Bevy plugin (already the ECS-owning location for grid logic), not in React. No new canvas-state ownership is introduced; Genie's topology switch is a per-scene `grid_type` value the engine already models. The Session Wish Pool/Doom Clock/Puzzle Clocks/Session Resources are explicitly *not* canvas simulation state (no scene geometry, no tokens, no spatial data) — they belong in the server/React layer, same as any other world-scoped data, not in the engine crate.
- **Principle II (Plugin-modular engine)**: PASS — the gridless-interaction change lands inside the existing `grid` plugin module (`src/engine/src/plugins/grid.rs`), which is already a self-contained, independently-addable plugin per the existing `plugins/mod.rs` structure. No new plugin is created; this is a real interaction body replacing a documented `()` no-op placeholder, not new plugin architecture. The session-loop work introduces no engine/Bevy code at all, per Principle I above.
- **Principle III (Ownership & authorization at the data boundary)**: PASS, with one real new mutation surface to enforce correctly. Character/item/lore data still reuses existing ownership-enforced tables unchanged (specs 010/012/013). The four new session-loop mutations get an explicit authorization split (data-model.md/research.md R8, following the repo's existing "DM-only" mutation convention already used in specs 011/012): `spendWish`, `advanceDoomClock`, and `advancePuzzleClock` are GM-only (they represent GM-adjudicated consequences per FR-014/spec.md Edge Cases, which explicitly says party agreement is a social convention the GM enforces, not a system-enforced vote); `tradeSessionResource` requires both participating players' consent (an offer from one player, confirmed by the other), enforced server-side, with neither the GM nor a single player able to force a trade unilaterally.
- **Principle IV (Real ADRs and specs before divergent implementation)**: This spec+plan pair satisfies the requirement directly. The session-loop's new authorization split (GM-only clock/wish mutations vs. two-party-consent trades) is a genuinely new cross-cutting pattern (the first two-party-consent mutation in the codebase, distinct from the existing single-actor-permission model) — significant enough to warrant a short ADR alongside implementation, tracked as a Phase 2/implementation task rather than deferred silently, consistent with how spec 015's moderation-boundary decision was handled.
- **Principle V (Verify before claiming done)**: Server/manifest changes verified via native `cargo check`/`cargo test`; the engine-crate gridless-interaction change verified via `cargo check --target wasm32-unknown-unknown` specifically (the engine crate never compiles natively, per the constitution's own warning); web changes verified via `tsc`/build and exercised in a running dev instance; per FR-012/SC-002/SC-005, verification MUST include actually playing a full Genie session end-to-end (a combat encounter AND a full win/loss session loop with multiple connected clients), not just unit-testing individual mechanics in isolation.

**Initial gate result**: PASS — no violations requiring Complexity Tracking. One new ADR flagged (Principle IV) for the two-party-consent authorization pattern, tracked for the implementation phase.

**Post-design re-check** (after Phase 1 data-model/contracts): Still PASS. data-model.md's three new session tables and one new `event_code` are additive, reuse the existing `world_events` broadcast transport (spec 005) rather than inventing a new one, and the GM-only vs. two-party-consent authorization split is fully specified in contracts/ — no unresolved ownership ambiguity remains.

## Project Structure

### Documentation (this feature)

```text
specs/018-genie-house-system/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md         # Phase 1 output
├── quickstart.md         # Phase 1 output
├── contracts/             # Phase 1 output
│   ├── genie-manifest-and-rolls.md
│   └── genie-session-loop.md          # NEW: Session Wish Pool, Doom/Puzzle Clocks, Session Resource trading
└── tasks.md              # Phase 2 output (/speckit-tasks, not created here)
```

### Source Code (repository root)

```text
packs/systems/genie/
├── system.json                              # manifest: abilities, skills, Wish Points table, legal (original/no attribution)
├── engine/
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       └── plugin.rs                        # registers Genie-specific token/size-footprint behavior; mirrors dnd5e/engine/src/plugin.rs
├── server/
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── loader.rs                        # mirrors packs/systems/dnd5e/server/src/loader.rs
│       ├── models.rs                        # GenieCharacter / GenieNpc data_types shapes
│       └── validators.rs                    # data_types validation, mirrors dnd5e/server/src/validators.rs
└── web/
    ├── package.json
    └── src/
        ├── index.ts                          # GenieSystemManifest export, mirrors dnd5e/web/src/index.ts
        ├── components/
        │   ├── CharacterSheet.tsx
        │   ├── ManifestationRollButton.tsx    # triggers the rollDice mutation (spec 014) with Genie's formula
        │   ├── ConditionTrack.tsx
        │   ├── SizeCategoryBadge.tsx
        │   ├── SessionWishPool.tsx             # NEW: shared wish pool display + spend-wish request UI
        │   ├── SessionClocks.tsx               # NEW: Doom Clock + Puzzle Clocks, live-synced display
        │   └── SessionResourceTrade.tsx        # NEW: resource holdings + two-party trade offer/accept UI
        └── schema.ts

src/server/
├── migrations/
│   ├── <timestamp>_widen_scene_grid_type_gridless/
│   │   ├── up.sql                            # ALTER ... CHECK (grid_type IN ('square','hex','gridless'))
│   │   └── down.sql
│   └── <timestamp>_create_genie_session_tables/       # NEW
│       ├── up.sql                            # world_genie_sessions, world_genie_puzzle_clocks, world_genie_resource_holdings
│       └── down.sql
└── src/
    ├── world_events.rs                        # NEW event_code = 15 ("genie_session_state") documented alongside existing 10-14
    └── graphql/mutations/
        └── genie_session.rs                   # NEW: spendWish, advanceDoomClock, advancePuzzleClock, tradeSessionResource (propose/accept)

src/engine/src/plugins/
└── grid.rs                                   # `GridType::Gridless` match arm gains real zone-based token interaction (currently `()`)

docs/adrs/
└── <next-number>-genie-session-state-two-party-consent.md   # NEW: the two-party-consent authorization pattern (Constitution Principle IV)
```

**Structure Decision**: `packs/systems/genie/` follows the exact three-package layout already established by `packs/systems/dnd5e/` (`engine/`, `server/`, `web/`) — no new packaging pattern. Outside `packs/systems/genie/`, changes are: the `scenes.grid_type` migration and `grid.rs` match arm (both narrowly scoped to the gridless-scene gap), and the new session-loop tables/mutations/event code (narrowly scoped to the one genuinely new state category this feature introduces — shared, live-synced party session state).

## Complexity Tracking

*No Constitution Check violations — table intentionally left empty.*
