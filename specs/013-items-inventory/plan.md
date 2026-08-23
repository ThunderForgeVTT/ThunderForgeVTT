# Implementation Plan: Items & Inventory System

**Branch**: `013-items-inventory` | **Date**: 2026-08-23 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/013-items-inventory/spec.md`

## Summary

Add a world-scoped Item entity that mirrors the Actor (spec 010) and Lore Entry (spec 012) patterns end to end: DM-created, ownership-blocked (Viewer/Editor/Owner, reused verbatim), UUID-identified, shareable-and-copyable across worlds (reusing the actor share-link mechanism), browsable/searchable in a fully-implemented Compendium "Items" tab (replacing spec 011's placeholder), and a valid `[[...]]` in-text link target from Lore alongside actors. Each Item carries zero or more structured Item Effects (`heal`/`damage`/`modifier`/`attack-roll`, formula + generic target) — authored data only, with no dice-rolling or trigger execution in this pass, but the schema is scaffolded (a `trigger_kind` column) so a future dice-roller spec can add real resolution without a redesign. Actors gain an inventory: a `world_actor_inventory` join table of `(actor, item, quantity)`, permissioned by the *Actor's* ownership block, not the Item's. The plan reuses four existing subsystems wholesale (actor permission model, actor share-link/copy mechanism, lore in-text-link resolution, Compendium tab shell) and adds two genuinely new pieces: the Item/Item Effect/Inventory data model and a name-similarity "did you mean?" check on Item creation — nothing else in the codebase does fuzzy title matching today.

## Technical Context

**Language/Version**: Rust 2024 edition (`src/server`), TypeScript 6.0 + React 19.2 (`apps/web`)

**Primary Dependencies**: Axum 0.8.9 + async-graphql 7.2.1 + async-graphql-axum (GraphQL API), Diesel 2.3.9 (postgres, r2d2, chrono, uuid, serde_json). No new server dependency for the core Item/inventory CRUD. One new dependency for name-similarity matching: Postgres `pg_trgm` extension (trigram similarity via `%`/`similarity()`), enabled through a migration — see research.md §3; this avoids pulling in a Rust-side fuzzy-matching crate for a single "did you mean?" query. Frontend: React 19 + react-router-dom 7.14, hand-rolled `fetch`-based GraphQL client (`apps/web/src/api/*.ts`), Radix-based design system (`@/components/ui/`) — no new frontend dependency; reuses the `Tabs`/`ComingSoonTab`-replacement pattern from spec 011 and the searchable-table-plus-preview-panel pattern from `NpcCompendiumTab.tsx`/`ActorPreviewPanel.tsx`.

**Storage**: PostgreSQL via Diesel (new tables: `world_items`, `world_item_permissions`, `world_item_shares`, `world_item_effects`, `world_actor_inventory`). No object storage (RustFS) is required for this pass since Item icon/image is optional (per Clarifications) and, when present, reuses the existing image upload/transcode path already used for actors — see research.md §5.

**Testing**: `cargo test` (server, matching existing `#[tokio::test]` resolver tests in `graphql/mutations_actors.rs`/`mutations_actor_shares.rs`), Playwright (`apps/web`, `pnpm e2e`) for browser-level flows; no WASM/engine involvement, so `cargo check --target wasm32-unknown-unknown` is not applicable.

**Target Platform**: Linux server (Axum), web browser (React SPA) — no engine/WASM/canvas involvement.

**Project Type**: Web application (existing `src/server` + `apps/web` split; this feature adds no new top-level project).

**Performance Goals**: Item authoring (name/description/effects) saved and reflected in the Compendium catalog with no full page reload (SC-001/SC-002, matches the existing NPC-catalog add-without-reload behavior from spec 011).

**Constraints**: No dice-rolling, effect triggering, or automatic inventory consumption is implemented in this pass (Clarifications) — Item Effect rows are scaffolded, inert data. Item names are NOT unique per world (Clarifications) — the "did you mean?" check is advisory only, never blocking.

**Scale/Scope**: World-scoped (not scene-scoped); reuses the same per-world membership/DM-role scale as actors and lore — no new scale class introduced. Item Share/Copy reuses the cross-world scale already established by Actor Share/Copy (spec 010, User Story 5).

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

- **Principle I (ECS owns simulation, React owns chrome)**: N/A — Items have no canvas/simulation presence; pure CRUD + rendering inside the existing React chrome (Compendium, Actor sheet inventory panel). PASS.
- **Principle II (Plugin-modular engine architecture)**: N/A — no `src/engine` changes. PASS.
- **Principle III (Ownership & authorization at the data boundary)**: Satisfied by design — every Item/inventory mutation and query enforces its permission check server-side in the GraphQL resolver layer, generalizing `src/server/src/auth/actor_permissions.rs`'s existing pattern; inventory mutations check the *Actor's* permission (not the Item's), per FR-013/the spec's explicit Assumption. New tables carry `created_by` provenance consistent with existing convention. PASS.
- **Principle IV (Real ADRs and specs before divergent implementation)**: This feature already has a Spec Kit spec (specs/013-items-inventory/spec.md) and this plan. No new architecturally-significant subsystem is introduced — Item/inventory reuse the same ownership-block, share-link, and in-text-link subsystems already ADR-eligible-and-covered by specs 010/012; the one new technical choice (Postgres `pg_trgm` for "did you mean?") is an implementation-library selection within existing architecture (Postgres is already the DB), not a new subsystem, so no new ADR is required — recorded in research.md instead. PASS.
- **Principle V (Verify before claiming done)**: Implementation phase will run `cargo check`/`cargo test` (native, server crate) and `pnpm --filter @thunderforge/web build`/lint, plus a live dev-server pass exercising Item authoring, inventory add/adjust/remove, lore in-text-linking, and share/copy flows in browser, before any task is marked complete. PASS (process commitment, verified at implementation time).

No violations to justify — Complexity Tracking table is empty.

## Project Structure

### Documentation (this feature)

```text
specs/013-items-inventory/
├── plan.md                        # This file (/speckit-plan command output)
├── research.md                    # Phase 0 output (/speckit-plan command)
├── data-model.md                  # Phase 1 output (/speckit-plan command)
├── quickstart.md                  # Phase 1 output (/speckit-plan command)
├── contracts/                     # Phase 1 output (/speckit-plan command)
│   ├── graphql-items.md
│   ├── graphql-inventory.md
│   ├── item-share.md
│   └── item-lore-links.md
└── tasks.md                       # Phase 2 output (/speckit-tasks command - NOT created here)
```

### Source Code (repository root)

```text
src/server/
├── migrations/
│   ├── <ts>_enable_pg_trgm
│   ├── <ts>_create_world_items
│   ├── <ts>_create_world_item_permissions
│   ├── <ts>_create_world_item_shares
│   ├── <ts>_create_world_item_effects
│   └── <ts>_create_world_actor_inventory
├── src/
│   ├── schema.rs                          # extended: new item/inventory tables
│   ├── models.rs                          # extended: WorldItem, ItemEffect, ItemShare, ItemPermission, ActorInventoryEntry structs
│   ├── auth/
│   │   └── item_permissions.rs            # NEW — generalizes auth/actor_permissions.rs for items
│   ├── graphql/
│   │   ├── types.rs                       # extended: GraphQLItem/ItemEffect/GraphQLItemShareLink/InventoryEntry types
│   │   ├── input_types.rs                 # extended: create/update item, effect, inventory-adjust inputs
│   │   ├── queries/item.rs                # NEW — item catalog, item detail, "did you mean?" name-suggest queries
│   │   ├── mutations_items.rs             # NEW — create/update/delete item, add/edit/remove effect
│   │   ├── mutations_item_permissions.rs  # NEW — ownership-block edits (mirrors mutations_actor_permissions.rs)
│   │   ├── mutations_item_shares.rs       # NEW — createItemShareLink/revokeItemShareLink/copySharedItemToWorld (mirrors mutations_actor_shares.rs)
│   │   └── mutations_inventory.rs         # NEW — addItemToInventory/adjustInventoryQuantity/removeInventoryEntry, permissioned against the Actor
│   └── markdown/
│       └── links.rs                       # EXTENDED (per spec 012's plan.md) — add `world_items` as a third resolution target alongside lore_entry/actor
└── tests/ (or inline #[cfg(test)] per existing convention)

apps/web/src/
├── routes/AppRoutes.tsx                   # extended: /world/:id/item/:id/view|edit, /item-share/:shareCode
├── pages/
│   ├── item-share/
│   │   └── SharedItemPage.tsx             # NEW — mirrors pages/actor-share/SharedActorPage.tsx
│   └── world/
│       ├── compendium/
│       │   ├── WorldCompendiumPage.tsx    # extended: "items" tab content replaces <ComingSoonTab label="Items" />
│       │   ├── ItemCompendiumTab.tsx      # NEW — item catalog list (mirrors NpcCompendiumTab.tsx)
│       │   └── ItemPreviewPanel.tsx       # NEW — mirrors ActorPreviewPanel.tsx
│       ├── item/
│       │   ├── ItemDetailPage.tsx         # NEW — view/edit (mirrors ActorDetailPage.tsx)
│       │   ├── ItemOwnershipBlock.tsx     # NEW — mirrors ActorOwnershipBlock.tsx
│       │   └── ItemEffectEditor.tsx       # NEW — add/edit/remove structured effects (type + formula + target)
│       └── actor/
│           └── ActorInventoryPanel.tsx    # NEW — Item + quantity list on the Actor sheet, add/adjust/remove controls
├── api/
│   ├── items.ts                           # NEW — fetch-based GraphQL calls (mirrors api/actors.ts)
│   ├── itemShares.ts                      # NEW — mirrors api/actorShares.ts
│   └── inventory.ts                       # NEW — addItemToInventory/adjustInventoryQuantity/removeInventoryEntry calls
└── types/
    ├── item.ts                            # NEW — WorldItemRecord, ItemEffectRecord, etc.
    ├── itemShare.ts                       # NEW — mirrors types/actorShare.ts
    └── inventory.ts                       # NEW — ActorInventoryEntryRecord
```

**Structure Decision**: No new top-level project — this feature extends the existing two-project split (`src/server` Rust GraphQL backend, `apps/web` React frontend) exactly as specs 010/011/012 did, adding new modules/files rather than a new service. The Compendium's existing tabbed shell (spec 011) is extended in place — `ItemCompendiumTab.tsx` replaces the `<ComingSoonTab label="Items" />` line in `WorldCompendiumPage.tsx` (apps/web/src/pages/world/compendium/WorldCompendiumPage.tsx:75), no restructuring of the NPCs tab. If spec 012 (lore) has not yet landed when this feature is implemented, `mutations_items`/`queries/item` still ship standalone (Items are useful via the Compendium and Actor inventory on their own) and the lore-link integration (FR-014/015/016) becomes a small follow-up wired into whichever module spec 012 actually lands its link resolver in.

## Complexity Tracking

*No violations — table intentionally empty.*
