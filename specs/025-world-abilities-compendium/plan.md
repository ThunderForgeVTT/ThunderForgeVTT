# Implementation Plan: World Abilities Compendium

**Branch**: `025-world-abilities-compendium` | **Date**: 2026-08-25 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/025-world-abilities-compendium/spec.md`

## Summary

Graduate the Compendium's last "coming soon" placeholder (Abilities) into a
fully-functional content type, deliberately mirroring spec 013's Item
implementation rather than inventing a parallel design. Delivers: a
world-scoped Ability entity with structured Effects, actor attachment
("known abilities"), per-ability Viewer/Editor/Owner access control, lore
in-text-link cross-referencing in both directions, and share links with
Copy-to-World.

One genuinely new mechanism beyond the Item template: **per-system
presentation facets**. Ability classifications are a fixed, portable,
system-agnostic set of underlying values; each game system may optionally
supply display labels re-expressing them in its own vocabulary (a 5E-style
system showing "Spells"/"Feats", Genie showing "Scrolls"). This resolves the
naming collision with the existing per-system `abilities` manifest block
(which declares ability *scores*) without renaming either concept and without
a breaking manifest change — facets are optional, with built-in defaults.

## Technical Context

**Language/Version**: Rust 2024 edition (`src/server`), TypeScript 6.0 + React 19.2 (`apps/web`)

**Primary Dependencies**: Axum 0.8.9 + async-graphql 7.2.1 + async-graphql-axum (GraphQL API), Diesel 2.3.9 (postgres, r2d2, chrono, uuid, serde_json). **No new server or frontend dependency.** Postgres `pg_trgm` (already enabled by spec 013's migration) is reused for FR-007's "did you mean?" name matching. Frontend reuses the existing hand-rolled `fetch`-based GraphQL client (`apps/web/src/api/*.ts`), the Radix-based design system (`@/components/ui/`), spec 011's `Tabs`/`CompendiumTabDef[]` shell, spec 013's searchable-table-plus-preview-panel pattern, and spec 021's shared CodeMirror Markdown editor.

**Storage**: PostgreSQL via Diesel. New tables mirroring the item set: `world_abilities`, `world_ability_permissions`, `world_ability_effects`, `world_ability_shares`, `world_actor_abilities`. Plus an additive change to `world_lore_links` to accept an ability target (mirroring spec 013's `add_item_target_to_world_lore_links` migration). No object storage (RustFS) — abilities have no image in this pass.

**Testing**: `cargo test` (server; native, matching existing `#[tokio::test]` resolver-test convention and requiring `DATABASE_URL` from the repo-root `.env` plus running Postgres/RustFS containers), Playwright (`apps/web`) for browser-level flows. No WASM/engine involvement, so `cargo check --target wasm32-unknown-unknown` is not applicable.

**Target Platform**: Linux server (Axum), web browser (React SPA) — no engine/WASM/canvas involvement.

**Project Type**: Web application — extends the existing `src/server` + `apps/web` split; adds no new top-level project.

**Performance Goals**: Ability authoring saved and reflected in the Compendium with no full page reload (SC-002). Search narrows a 100+ ability catalog interactively (SC-003).

**Constraints**: No dice rolling, effect resolution, trigger evaluation, or usage/slot tracking (spec Non-Goals) — Effect rows are scaffolded, inert authored data. Ability names are NOT unique per world; the "did you mean?" check is advisory, never blocking. Abilities have no canvas presence. Presentation facets are display-only and never alter stored data.

**Scale/Scope**: World-scoped (not scene-scoped), reusing the same per-world membership/DM-role scale as actors, lore, and items. Share/Copy reuses the cross-world scale already established by Actor Share (spec 010) and Item Share (spec 013).

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

- **Principle I (ECS owns simulation, React owns chrome)**: N/A — Abilities have no canvas/simulation presence (explicit Non-Goal). Pure CRUD + rendering inside existing React chrome (Compendium tab, actor sheet, detail pages). **PASS.**
- **Principle II (Plugin-modular engine architecture)**: N/A — no `src/engine` changes. **PASS.**
- **Principle III (Ownership & authorization at the data boundary)**: Satisfied by design — every Ability mutation/query enforces its check server-side in the resolver layer, generalizing `src/server/src/auth/item_permissions.rs`'s existing pattern. Actor-attachment mutations check the *Actor's* permission, not the Ability's (FR-022), exactly as spec 013 did for inventory. New tables carry `created_by`/`updated_by` provenance per FR-027. **PASS.**
- **Principle IV (Real ADRs and specs before divergent implementation)**: This feature has a spec and this plan. Most of it reuses subsystems already covered by specs 010/012/013 (ownership blocks, share links, in-text links) — no new ADR needed for those. The one novel mechanism, per-system presentation facets, was investigated in Phase 0 on the assumption it would require an ADR-027 amendment: **it does not** (research.md §1). ADR-027's Decision already sanctions "any system-specific blocks", its Alternatives Considered explicitly records that additive optional fields are non-breaking, and `pack_system_spec`'s schema does not set `additionalProperties: false` — so an optional `abilityFacets` key is covered by the existing contract. The `legal` amendment was needed only because it added a *required* field with an *enforcement* rule. **PASS — no ADR required.**
- **Principle V (Verify before claiming done)**: Implementation will run native `cargo check`/`cargo test` (server crate) and `pnpm --filter @thunderforge/web build`/lint, plus a live dev-server pass exercising ability authoring, effects, actor attachment, lore linking, and share/copy in-browser. Note the documented environment limitation: this sandbox cannot render the Bevy canvas, but **this feature has no canvas surface**, so its Playwright coverage is fully runnable here (the same property that let spec 005's `live-sync.spec.ts` pass). **PASS** (process commitment, verified at implementation time).

### DMCA / Content Moderation Guardrail — addressed as a prerequisite

Constitution v1.1.0's guardrail applies to **User Story 6 (share links)**.
Verified during planning:

- **(a) Notice-and-takedown program operational** — **SATISFIED**. Spec 015 is
  complete (41/41 tasks, zero unchecked), including per-entry disable/restore
  and a regression test that a share link stops resolving once its target is
  moderated.
- **(b) On-record "centralized public repository" determination** — **was
  missing for every share-link feature; now drafted as part of this work.**

**The governance finding**: no determination existed anywhere in `specs/` or
`docs/adrs/` for *any* share-link feature. Actor share links (spec 010, FR-023)
and item share links (spec 013, FR-022..FR-027) both shipped without one, and
spec 013's Constitution Check does not mention the guardrail at all. The root
cause is identifiable: spec 015's own Assumptions state the platform "currently
has no public compendium-sharing … feature (confirmed by repository search)"
and that its guardrails are "preventative … not a retrofit of an existing one."
**That was factually wrong when written** — actor sharing had already shipped
and item sharing was being built the same day (specs 013 and 015 were both
created 2026-08-23, the day the constitution was amended to v1.1.0). The
guardrail was authored as purely forward-looking and never applied to the
sharing that already existed.

**Consequence for this plan**: rather than deferring User Story 6, the
determination is pulled in as **task T001** — the first task in the feature,
ahead of all implementation. It is drafted as
`docs/adrs/20260825-049-share_link_dmca_repository_determination.md`, which:

- finds that share links are **not** a centralized public repository, because
  they are non-enumerable, non-discoverable, owner-audience-controlled,
  revocable, and takedown-effective;
- makes that finding **conditional on six named invariants** (FR-037's
  no-enumeration property is one of them), so any future index, browse view, or
  search over shared content re-opens the determination;
- **covers actor and item shares retroactively**, closing the pre-existing gap
  rather than leaving it open;
- requires acceptance by an accountable owner before it takes effect — the
  analysis and recommendation are drafted, the risk acceptance is not something
  this plan can self-certify.

User Stories 1-5 have no dependency on T001 and proceed regardless of its
outcome. If the determination were rejected, only User Story 6 would drop.

## Project Structure

### Documentation (this feature)

```text
specs/025-world-abilities-compendium/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
├── contracts/           # Phase 1 output
├── checklists/
│   └── requirements.md  # From /speckit-specify
└── tasks.md             # Phase 2 output (/speckit-tasks — NOT created here)
```

### Source Code (repository root)

```text
src/server/
├── migrations/
│   ├── <ts>_create_world_abilities
│   ├── <ts>_create_world_ability_permissions
│   ├── <ts>_create_world_ability_effects
│   ├── <ts>_create_world_ability_shares          # US6 — gated
│   ├── <ts>_create_world_actor_abilities
│   └── <ts>_add_ability_target_to_world_lore_links
└── src/
    ├── schema.rs                              # EXTENDED: 5 tables + joinables + allow_tables_…
    ├── models.rs                              # EXTENDED: WorldAbility, AbilityEffect, AbilityPermission,
    │                                          #   AbilityShare, ActorAbilityEntry (+ New* structs)
    ├── auth/
    │   └── ability_permissions.rs             # NEW — mirrors auth/item_permissions.rs
    ├── graphql/
    │   ├── types.rs                           # EXTENDED: GraphQLAbility/Effect/Permission/ShareLink,
    │   │                                      #   AbilityClassification enum,
    │   │                                      #   ModerationEntityType::WorldAbility
    │   ├── queries/
    │   │   ├── ability.rs                     # NEW — worldAbilities, ability, suggestAbilityName
    │   │   └── lore.rs                        # EXTENDED: lore_entries_linking_to_ability,
    │   │                                      #   ABILITY enum variant, 4th lore_link_targets branch
    │   ├── mutations_abilities.rs             # NEW — CRUD + effects (+ validate_formula/_target)
    │   ├── mutations_ability_permissions.rs   # NEW — ownership block
    │   ├── mutations_ability_shares.rs        # NEW — US6, gated
    │   ├── mutations_actor_abilities.rs       # NEW — attach/detach, permissioned against the ACTOR
    │   ├── mutations_lore.rs                  # EXTENDED: pass target_ability_id through
    │   └── mutations_moderation.rs            # EXTENDED: WorldAbility match arm
    └── markdown/
        └── links.rs                           # EXTENDED: 4th resolution target (appended last)

apps/web/src/
├── routes/
│   ├── AppRoutes.tsx                          # EXTENDED: /world/:id/ability/:abilityId/view|edit,
│   │                                          #   /shared/ability/:code (US6)
│   └── pageLoaders.ts                         # EXTENDED: abilityView/abilityEdit/sharedAbility
├── pages/
│   ├── ability-share/
│   │   └── SharedAbilityPage.tsx              # NEW — US6, gated
│   └── world/
│       ├── compendium/
│       │   ├── WorldCompendiumPage.tsx        # EXTENDED: replaces <ComingSoonTab label="Abilities" />
│       │   ├── AbilityCompendiumTab.tsx       # NEW — mirrors ItemCompendiumTab.tsx
│       │   ├── AbilityPreviewPanel.tsx        # NEW — mirrors ItemPreviewPanel.tsx
│       │   └── ComingSoonTab.tsx              # loses its last caller — delete or keep (research.md §7)
│       ├── ability/
│       │   ├── AbilityDetailPage.tsx          # NEW — mode: "view" | "edit"
│       │   ├── AbilityEffectEditor.tsx        # NEW — gated on canEdit (unlike the item version)
│       │   └── AbilityOwnershipBlock.tsx      # NEW — mirrors ItemOwnershipBlock.tsx
│       ├── actor/
│       │   ├── ActorAbilitiesPanel.tsx        # NEW — mirrors ActorInventoryPanel.tsx (no quantity)
│       │   └── ActorDetailPage.tsx            # EXTENDED: mount the panel, canManage={canEdit}
│       └── lore/
│           └── LoreMarkdownEditor.tsx         # EXTENDED: ternary → label map (fixes a shipped bug)
├── api/
│   ├── abilities.ts                           # NEW
│   ├── abilityShares.ts                       # NEW — US6, gated
│   └── actorAbilities.ts                      # NEW
├── types/
│   ├── ability.ts                             # NEW
│   ├── abilityShare.ts                        # NEW — US6, gated
│   └── lore.ts                                # EXTENDED: widen LoreLinkTargetKind (fixes a shipped bug)
├── utils/
│   └── abilityFacets.ts                       # NEW — mirrors utils/sizeCategory.ts
└── e2e/
    └── abilities-compendium.spec.ts           # NEW — runnable here (no canvas surface)

packs/systems/<id>/system.json                 # OPTIONAL per pack: new "abilityFacets" key
```

**Structure Decision**: No new top-level project — extends the existing
`src/server` (Rust GraphQL) + `apps/web` (React SPA) split exactly as specs
010/012/013 did. The Compendium's tabbed shell is extended in place: the
`{ value: "abilities", … content: <ComingSoonTab label="Abilities" /> }` entry in
`WorldCompendiumPage.tsx` becomes the same two-panel
`grid lg:grid-cols-[2fr_1fr]` layout the Items tab already uses. No restructuring
of the other three tabs, and `WorldSidebarNav.tsx` already deep-links
`?tab=abilities` — no nav change needed.

Facets require **no server code at all**: `get_system_manifest` returns
`system.json` verbatim, so a new optional key flows to the frontend resolver
util unaided (research.md §1).

## Post-Design Constitution Re-Check

Re-evaluated after Phase 1 (data-model.md, contracts/, quickstart.md):

- **Principle I / II** — unchanged, still N/A. Phase 1 introduced no canvas or
  engine surface.
- **Principle III** — **strengthened** by design. Every resolver in
  `contracts/graphql-abilities.md` names its authorization helper explicitly;
  `world_abilities` carries both `created_by` and `updated_by` (an improvement
  over `world_items`, which has only `created_by`); and
  `contracts/graphql-actor-abilities.md` pins the subtle rule that attachment
  permission follows the **actor**, not the ability, with a named regression
  test. Phase 1 also surfaced a check no precedent enforces — a cross-world
  guard on `attachAbilityToActor`, since neither the FKs nor the UNIQUE
  constraint prevent attaching an ability from another world. **PASS.**
- **Principle IV** — **improved**. The assumed ADR-027 amendment turned out to be
  unnecessary (research.md §1), so this feature adds no ADR and no governed-
  contract change. **PASS.**
- **Principle V** — **strengthened**. quickstart.md records the exact commands
  and the `set -a && source .env` prerequisite that a bare `cargo test` needs,
  and confirms this feature's e2e is genuinely runnable in this sandbox because
  it has no Bevy canvas surface. **PASS.**
- **DMCA guardrail** — **resolved into a prerequisite** rather than left
  blocking. ADR-049 drafts the determination (covering actor and item shares
  retroactively) and is task T001. `contracts/ability-share.md` now carries the
  six invariants the determination is conditional on, and FR-037's
  no-enumeration property is recorded as *structurally* guaranteed — there is
  simply no list-shares query — rather than merely intended.

No new violations. No Complexity Tracking entries required.

## Complexity Tracking

> No Constitution Check violations requiring justification.

The one deviation worth naming is not a violation: this feature deliberately
**duplicates** spec 013's item module shape rather than generalizing items and
abilities into a shared "world artifact" abstraction. Rationale recorded in
research.md §5 — premature generalization across two content types with
diverging futures (abilities gain usage/slots, items gain quantity/equipping)
would couple them at exactly the point they are expected to diverge.
