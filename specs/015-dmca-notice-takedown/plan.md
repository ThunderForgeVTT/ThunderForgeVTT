# Implementation Plan: DMCA Notice-and-Takedown Process

**Branch**: `015-dmca-notice-takedown` | **Date**: 2026-08-23 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/015-dmca-notice-takedown/spec.md`

## Summary

Give the platform a working DMCA safe-harbor program: a published agent designation, an intake channel that validates notices against the statutory elements, a generic capability to disable one specific piece of user-entered world content (an actor, item, or lore entry) without touching the rest of that world, a counter-notice/restoration flow, and a durable per-account infringement history that drives a repeat-infringer policy. Technically this is a moderation layer bolted onto the existing per-world content tables (`world_actors`, `world_items`, and the lore-wiki entities from spec `012-lore-wiki`) rather than a rewrite of any of them: one new polymorphic `content_moderation_actions` table records notices/counter-notices/resolutions, and read paths for those content types gain a server-side visibility check so disabled content is invisible to world members without being deleted. A launch-review checklist (documentation, not code) implements the FR-012 guardrail against future public-sharing features.

## Technical Context

**Language/Version**: Rust 1.75 (server, Axum + async-graphql + Diesel/PostgreSQL); TypeScript/React (web) for the compliance intake form and moderation-notice UI

**Primary Dependencies**: Existing server stack (Diesel, async-graphql, Axum); no new runtime dependency required — notice validation and record-keeping are plain server logic against a new table

**Storage**: PostgreSQL via Diesel migration, one new table (`content_moderation_actions`) plus a nullable moderation-state check added to the existing read queries for `world_actors`, `world_items`, and lore entries

**Testing**: `cargo test` (server) for notice validation, disable/restore behavior, and repeat-infringer threshold logic; existing Vitest/RTL conventions for the intake form and any moderation-notice display component

**Target Platform**: Linux server (native `cargo check`, not wasm32 — this feature touches `src/server` only, not the engine crate)

**Project Type**: Web application (existing backend + frontend split: `src/server`, `apps/web`)

**Performance Goals**: Not performance-sensitive — notice volume is expected to be low (human-submitted legal correspondence, not a hot path); standard web request latency (sub-second) is sufficient

**Constraints**: Disable action MUST be reversible (counter-notice restoration) and MUST NOT delete underlying data; per FR-013, moderation records MUST survive deletion of the world/account they reference, which rules out a hard foreign-key `ON DELETE CASCADE` from `content_moderation_actions` to world/actor/item/lore tables — the record must outlive its subject

**Scale/Scope**: Single new table + visibility-check additions to 3 existing content read paths (actors, items, lore) + one static legal/compliance page + one intake form + one moderation-notice banner component + one internal compliance-review surface for repeat-infringer tracking

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

- **Principle I (ECS owns simulation)**: N/A — this feature touches only server data and web chrome, never canvas/engine state.
- **Principle II (Plugin-modular engine)**: N/A — no engine crate changes.
- **Principle III (Ownership & authorization at the data boundary)**: PASS by design — the moderation visibility check is enforced at the GraphQL/database read boundary (same layer as existing ownership checks), not client-side. A disabled entry must be unreadable via the API regardless of client trust, consistent with existing convention (ADR-009, ADR-013, ADR-023, ADR-028).
- **Principle IV (Real ADRs and specs before divergent implementation)**: This spec exists; because the moderation table crosses three existing content domains (actors, items, lore) and introduces a new authorization concept (moderation-disabled ≠ deleted, ≠ permission-denied), it is architecturally significant enough to warrant a short ADR alongside implementation — tracked as a Phase 1 output below, not deferred.
- **Principle V (Verify before claiming done)**: Server changes verified via native `cargo check`/`cargo test`; web changes verified via `tsc`/build and a running dev instance for the intake form and notice banner.

**Initial gate result**: PASS — no violations requiring Complexity Tracking.

**Post-design re-check** (after Phase 1 data-model/contracts): Still PASS. The polymorphic `content_moderation_actions` table and resolver-boundary enforcement (data-model.md, contracts/graphql-moderation.md) keep authorization server-side per Principle III, add no engine/canvas surface (Principle I/II N/A confirmed), and the ADR requirement identified in research.md R4 is carried forward as an implementation-phase task rather than silently dropped.

## Project Structure

### Documentation (this feature)

```text
specs/015-dmca-notice-takedown/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md         # Phase 1 output
├── quickstart.md         # Phase 1 output
├── contracts/             # Phase 1 output
│   └── graphql-moderation.md
└── tasks.md              # Phase 2 output (/speckit-tasks, not created here)
```

### Source Code (repository root)

```text
src/server/
├── migrations/
│   └── <timestamp>_create_content_moderation_actions/
│       ├── up.sql
│       └── down.sql
└── src/
    ├── graphql/
    │   ├── mutations/moderation.rs      # submitTakedownNotice, submitCounterNotice, resolveModerationAction
    │   └── queries/moderation.rs        # moderationHistory (per-account, compliance-staff-only)
    └── moderation.rs                    # notice validation, repeat-infringer threshold evaluation

apps/web/
└── src/
    ├── pages/legal/
    │   ├── DmcaPolicyPage.tsx           # public agent designation + policy text
    │   └── TakedownNoticeFormPage.tsx   # public intake form
    └── components/moderation/
        └── ModeratedContentBanner.tsx   # shown in place of a disabled compendium entry, with counter-notice CTA

docs/adrs/
└── <next-number>-content-moderation-and-dmca-safe-harbor.md
```

**Structure Decision**: Standard existing web-application split (`src/server` Rust backend, `apps/web` React frontend). No new top-level project. The moderation table is intentionally generic/polymorphic (`entity_type` + `entity_id`) rather than three separate tables, so it composes cleanly across the existing and future per-world content domains (actors, items, lore, and any future compendium content type) without needing a migration every time a new content type is added.

## Complexity Tracking

*No Constitution Check violations — table intentionally left empty.*
