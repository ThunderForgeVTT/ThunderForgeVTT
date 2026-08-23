# Implementation Plan: World Lore Wiki

**Branch**: `012-lore-wiki` | **Date**: 2026-08-22 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/012-lore-wiki/spec.md`

## Summary

Add a world-scoped lore wiki: DM-created entries with a GitHub-flavored-Markdown editor/renderer, an ownership block reusing the exact Viewer/Editor/Owner model already built for actors (spec 010), wiki-style `[[...]]` in-text links that resolve to either another lore entry or an actor (auto-maintained "linked from" backlinks on the target), clipboard-paste/drag-drop image upload processed and stored in the existing RustFS object store under UUID keys, a human-readable urlified slug stored alongside the UUID for shareable URLs, and an immutable per-save revision history with restore. The plan reuses four existing subsystems wholesale (actor permission model, RustFS storage, WebP transcode, GraphQL resolver/auth pattern) and adds three genuinely new pieces: Markdown parsing/rendering, slug generation, and revision-history storage — none of which exist anywhere in the codebase today.

## Technical Context

**Language/Version**: Rust 2024 edition (`src/server`), TypeScript 6.0 + React 19.2 (`apps/web`)

**Primary Dependencies**: Axum 0.8.9 + async-graphql 7.2.1 + async-graphql-axum (GraphQL API), Diesel 2.3.9 (postgres, r2d2, chrono, uuid, serde_json), aws-sdk-s3 1.143.0 / aws-sdk-sts 1.112.0 (RustFS object storage, per ADR-039), `image` 0.25.10 (already used for WebP transcode). New: a GitHub-flavored-Markdown parser/renderer crate for the server (`pulldown-cmark` + its `pulldown-cmark-escape`/GFM extension support, or `comrak` which has GFM tables/task-lists/strikethrough built in — see research.md) and a slug-generation crate (`slug` or hand-rolled ASCII-fold + kebab-case, per research.md). Frontend: React 19 + react-router-dom 7.14, hand-rolled `fetch`-based GraphQL client (`apps/web/src/api/*.ts`, no Apollo/urql), Radix-based design system (`@/components/ui/`); new: a Markdown editor with paste/drop image handling and a client-side GFM renderer matching the server's rendering (research.md picks the pairing).

**Storage**: PostgreSQL via Diesel (new tables: `world_lore_entries`, `world_lore_permissions`, `world_lore_links`, `world_lore_revisions`, `world_lore_image_assets`); RustFS (S3-compatible object storage, existing `storage/rustfs.rs` + `transcode.rs`) for lore Markdown revision bodies over a size threshold and all image assets — see data-model.md for the DB-vs-object-store split.

**Testing**: `cargo test` (server, matching existing `#[tokio::test]` resolver tests in `graphql/mutations_actors.rs` etc.), Playwright (`apps/web`, `pnpm e2e`) for browser-level flows; `cargo check --target wasm32-unknown-unknown` is not applicable here (no engine/canvas changes).

**Target Platform**: Linux server (Axum), web browser (React SPA) — no engine/WASM/canvas involvement.

**Project Type**: Web application (existing `src/server` + `apps/web` split; this feature adds no new top-level project).

**Performance Goals**: Pasted-image upload-to-rendered in <10s for a typical image (SC-003, matches existing RustFS transcode path's demonstrated latency).

**Constraints**: 25 MB max per image upload and 25 MB max per entry's Markdown content (FR-010/FR-010a, fixed defaults — no instance-configurable quota system in this pass, per spec Assumptions).

**Scale/Scope**: World-scoped (not scene-scoped); reuses the same per-world membership/DM-role scale as actors — no new scale class introduced.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

- **Principle I (ECS owns simulation, React owns chrome)**: N/A — lore has no canvas/simulation presence; it is pure CRUD + rendering inside the existing React chrome (staging/compendium pages). PASS.
- **Principle II (Plugin-modular engine architecture)**: N/A — no `src/engine` changes. PASS.
- **Principle III (Ownership & authorization at the data boundary)**: Satisfied by design — every lore mutation/query enforces its permission check server-side in the GraphQL resolver layer before touching the DB, generalizing `src/server/src/auth/actor_permissions.rs`'s existing pattern (`is_dm_of_world`, `effective_actor_permission`/`require_*_permission`) rather than trusting client-supplied permission claims. New tables carry `created_by` provenance consistent with existing convention. PASS.
- **Principle IV (Real ADRs and specs before divergent implementation)**: This feature already has a Spec Kit spec (specs/012-lore-wiki/spec.md) and this plan. No new architecturally-significant subsystem is introduced beyond what's already ADR'd (RustFS storage, ADR-039) except the Markdown-rendering and revision-history choices — both are implementation-library selections within existing architecture, not new subsystems/ownership boundaries, so no new ADR is required; the choice and rationale are recorded in research.md for future-agent context instead. PASS.
- **Principle V (Verify before claiming done)**: Implementation phase will run `cargo check`/`cargo test` (native, server crate) and `pnpm --filter @thunderforge/web build`/lint, plus a live dev-server pass exercising paste-image and in-text-link flows in browser, before any task is marked complete. PASS (process commitment, verified at implementation time).

No violations to justify — Complexity Tracking table is empty.

## Project Structure

### Documentation (this feature)

```text
specs/012-lore-wiki/
├── plan.md              # This file (/speckit-plan command output)
├── research.md          # Phase 0 output (/speckit-plan command)
├── data-model.md        # Phase 1 output (/speckit-plan command)
├── quickstart.md         # Phase 1 output (/speckit-plan command)
├── contracts/            # Phase 1 output (/speckit-plan command)
│   └── graphql-lore.md
└── tasks.md              # Phase 2 output (/speckit-tasks command - NOT created here)
```

### Source Code (repository root)

```text
src/server/
├── migrations/
│   ├── <ts>_create_world_lore_entries
│   ├── <ts>_create_world_lore_permissions
│   ├── <ts>_create_world_lore_links
│   ├── <ts>_create_world_lore_revisions
│   └── <ts>_create_world_lore_image_assets
├── src/
│   ├── schema.rs                          # extended: new lore tables
│   ├── models.rs                          # extended: LoreEntry, LoreRevision, LoreLink, LoreImageAsset, LorePermission structs
│   ├── auth/
│   │   └── lore_permissions.rs            # NEW — generalizes auth/actor_permissions.rs for lore entries
│   ├── graphql/
│   │   ├── types.rs                       # extended: LoreEntry/LoreRevision/LoreLink GraphQL types
│   │   ├── input_types.rs                 # extended: create/update/restore inputs
│   │   ├── queries/lore.rs                # NEW — lore index, entry detail, revision history queries
│   │   ├── mutations_lore.rs              # NEW — create/update/delete entry
│   │   ├── mutations_lore_permissions.rs  # NEW — ownership-block edits (mirrors mutations_actor_permissions.rs)
│   │   └── mutations_lore_images.rs       # NEW — paste/drop image upload mutation
│   ├── markdown/
│   │   ├── mod.rs                         # NEW — GFM parse/render (server-authoritative rendering for consistency)
│   │   ├── links.rs                       # NEW — `[[...]]` extraction/resolution against lore entries + actors
│   │   └── slug.rs                        # NEW — title → urlified slug, collision disambiguation
│   └── storage/
│       ├── rustfs.rs                      # unchanged — reused for lore image + oversized revision bodies
│       └── transcode.rs                   # extended — add thumbnail/resize step (currently transcode-only)
└── tests/ (or inline #[cfg(test)] per existing convention)

apps/web/src/
├── routes/AppRoutes.tsx                   # extended: /world/:id/lore, /world/:id/lore/:slug/view|edit, /world/:id/lore/:slug/history
├── pages/world/
│   ├── compendium/
│   │   ├── WorldCompendiumPage.tsx        # extended: new "Lore" tab replacing/alongside existing ComingSoonTab slots
│   │   └── LoreCompendiumTab.tsx          # NEW — lore index list (mirrors NpcCompendiumTab.tsx)
│   └── lore/
│       ├── LoreEntryDetailPage.tsx        # NEW — view/edit (mirrors ActorDetailPage.tsx)
│       ├── LoreOwnershipBlock.tsx         # NEW — mirrors ActorOwnershipBlock.tsx
│       ├── LoreMarkdownEditor.tsx         # NEW — editor with paste/drop image handling + in-text-link autocomplete
│       ├── LoreMarkdownRenderer.tsx       # NEW — client-side GFM render matching server rendering
│       └── LoreRevisionHistory.tsx        # NEW — revision list/viewer/restore
├── api/lore.ts                            # NEW — fetch-based GraphQL calls (mirrors api/actors.ts)
└── types/lore.ts                          # NEW — LoreEntryRecord, LoreRevisionRecord, etc.
```

**Structure Decision**: No new top-level project — this feature extends the existing two-project split (`src/server` Rust GraphQL backend, `apps/web` React frontend) exactly as spec 010 (actors) did, adding new modules/files rather than a new service. Lore images and any oversized revision bodies reuse the existing RustFS-backed `storage/` module; no second storage mechanism is introduced (mirrors constitution's existing single-asset-mechanism precedent from ADR-039).

## Complexity Tracking

*No violations — table intentionally empty.*
