# Implementation Plan: Players Section

**Branch**: `023-players-section` | **Date**: 2026-08-25 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/023-players-section/spec.md`

## Summary

Give every world a dedicated "Players" section (sidebar nav, alongside Scenes/NPCs/Lore/Items/Abilities) showing every member paired with the character they've claimed — a real roster, not a bare username list. For a GM, the same page is also where role changes and member removal happen, superseding the world dashboard's Campaign Settings panel (which keeps only invite generation and the player-created-actors toggle). Reuses the existing `updateMemberRole`/`removeMember` mutations and role-hierarchy authorization as-is, plus one bundled fix (an Owner-fallback gap those two mutations currently lack), and adds a single additive `claimedActor` field to the existing `worldMembers` query rather than a new data shape.

## Technical Context

**Language/Version**: Rust 2024 edition (server, `src/server`); TypeScript 6.0 / React (frontend, `apps/web`) — no engine-crate involvement, this feature has no canvas/simulation surface

**Primary Dependencies**: Axum + async-graphql + Diesel/PostgreSQL (server); React + the existing fantasy design system, `WorldSectionShell`/`WorldSidebarNav` (frontend) — both already established, no new dependency

**Storage**: PostgreSQL via Diesel — no schema migration needed; `world_actor_claims` and `world_members` already exist and already carry everything this feature reads

**Testing**: `cargo test` (server, resolver-level, mirrors the existing convention every prior spec in this repo used) + Playwright e2e (`apps/web/e2e/*.spec.ts`)

**Target Platform**: Server-side web (Linux) + browser client — no engine/WASM change

**Project Type**: Web application — existing monorepo layout (`src/server` GraphQL backend, `apps/web` React frontend), no new project

**Performance Goals**: Roster + claim data for a typical world (single-digit-to-low-tens of members) loads in one round trip via the extended `worldMembers` query — no pagination or additional performance work needed at this scale

**Constraints**: Server remains authoritative for role/removal authorization (Constitution Principle III) — the existing role-hierarchy check in `updateMemberRole`/`removeMember` is reused unchanged, not reimplemented client-side

**Scale/Scope**: Single-world-scoped feature; no cross-world concerns

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-checked after Phase 1 design below.*

| Principle | Assessment |
|---|---|
| I. ECS Owns Simulation, React Owns Chrome | **N/A / Pass.** No canvas or simulation surface — this is pure world-management chrome (roster, role, removal), same category as Compendium/System Settings. |
| II. Plugin-Modular Engine Architecture | **N/A.** No engine-crate change. |
| III. Ownership & Authorization at the Data Boundary | **Pass.** Role-change/removal stay server-enforced via the existing, unmodified role-hierarchy check; the one change (Owner-fallback in the caller lookup) *strengthens* this principle by closing a gap where a legitimate Owner could be wrongly rejected, not by loosening any check. |
| IV. Real ADRs and Specs Before Divergent Implementation | **Pass, no ADR needed.** This spec is the required Spec Kit artifact; nothing here is architecturally significant (no new subsystem, no new ownership boundary, no dependency swap) — it's additive UI plus a one-field query extension and a bug-fix bundled into existing, already-reviewed authorization code. |
| V. Verify Before Claiming Done | **Pass, by process.** `cargo check`/`cargo test` for server changes; `tsc`/build + a running dev instance for the new page; no engine crate touched, so no wasm32 check needed. |

No unjustified violations — Complexity Tracking section is not needed.

## Project Structure

### Documentation (this feature)

```text
specs/023-players-section/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
├── contracts/
│   └── players-section.graphql.md
├── checklists/
│   └── requirements.md
└── tasks.md             # Phase 2 output (/speckit-tasks — not created here)
```

### Source Code (repository root)

```text
src/server/src/
├── graphql/
│   ├── mutations_invites.rs   # update_member_role/remove_member: add require_world_member
│   │                          # Owner-fallback to the caller lookup (research.md §3);
│   │                          # authorization logic itself unchanged
│   └── queries/invite.rs      # world_members_impl / WorldMembershipPayload: add
│                               # claimedActor field, resolved via world_actor_claims join
│                               # (research.md §2)
└── graphql.rs                  # GraphQLWorldMember-adjacent type wiring for the new field

apps/web/src/
├── layouts/world-layout/
│   └── WorldSidebarNav.tsx     # add "Players" to `categories`
├── pages/world/players/        # NEW
│   ├── PlayersRoutePage.tsx    # mirrors ScenesRoutePage.tsx
│   └── PlayersPage.tsx         # mirrors ScenesPage.tsx; isGm branch adds role/remove controls
├── routes/
│   ├── pageLoaders.ts          # add worldPlayers loader
│   └── AppRoutes.tsx           # add /world/:id/players route
├── api/
│   └── worldMembers.ts         # extend WorldMemberRecord with claimedActor; add
│                                # updateMemberRole/removeMember wrappers (currently only
│                                # inlined ad hoc in CampaignSettingsPanel.tsx)
└── components/campaign/
    └── CampaignSettingsPanel.tsx  # remove Player Roster block + role/remove controls
                                    # (FR-011); keep invites + allow-player-created-actors
```

**Structure Decision**: Follows the existing monorepo layout exactly — no new project, no new subsystem. New backend logic lands in the existing `mutations_invites.rs`/`queries/invite.rs` modules that already own this domain, not a new module. New frontend code follows the Scenes-section precedent file-for-file.
