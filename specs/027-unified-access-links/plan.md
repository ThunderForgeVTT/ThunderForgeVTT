# Implementation Plan: Unified World Access Links & Consolidated Permission Resolution

**Branch**: `027-unified-access-links` | **Date**: 2026-08-25 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/027-unified-access-links/spec.md`

## Summary

Two changes with one root cause. **Part B** gives world invite links the
lifecycle controls they lack — explicit revocation and GM-initiated rotation —
so a leaked link can actually be killed, and raises invite codes to the entropy
already used for content share links. **Part A** collapses four near-verbatim
permission modules into one declarative source, moves the world-level DM check
out of the actor module it accidentally lives in, and fixes the live privilege
leak that duplication hid: member removal never cleans up ability grants.

The technical shape is settled in [research.md](./research.md): a declarative
macro over all permissioned content types (Diesel's typed tables make a generic
function impractical, and a polymorphic table would lose the `ON DELETE
CASCADE` that keeps grants from outliving their content); an additive migration
extending `world_invites` rather than a new table, so no live code is
invalidated; and a single conditional `UPDATE` that validates and consumes a
use atomically — which both fixes an existing lost-update race and delivers
uniform failure for free.

## Technical Context

**Language/Version**: Rust (edition 2024) server; TypeScript 6 / React 19 web

**Primary Dependencies**: Axum, async-graphql, Diesel + PostgreSQL (server);
Vite, React Router, the in-house fantasy design system (web)

**Storage**: PostgreSQL. One additive migration on `world_invites`
(`revoked`, `rotated_from`). No changes to the four permission tables.

**Testing**: `cargo test -p thunderforge` (server, real DB via `DATABASE_URL`);
`vitest` (web unit); Playwright (e2e)

**Target Platform**: Linux server + browser. No engine/WASM surface in this
feature.

**Project Type**: Web application — Rust backend, React frontend

**Performance Goals**: No new hot paths. The consolidated resolver must not
add a query round-trip versus today's per-type functions; join remains a single
transaction.

**Constraints**: Behaviour-preserving for all existing authorization (FR-021 /
SC-003). No live invite code may be invalidated (FR-007 / SC-006). Rotation
must be atomic (FR-004).

**Scale/Scope**: 4 permission modules consolidated, ~49 call sites for the DM
check move, 1 migration, ~6 GraphQL operations added or changed, 1 frontend
panel extended.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

Assessed against constitution v1.1.0.

| Principle | Verdict | Notes |
|---|---|---|
| **I. ECS owns simulation, React owns chrome** | ✅ N/A | No canvas surface. Invite panel is chrome; no engine change. |
| **II. Plugin-modular engine architecture** | ✅ N/A | No engine crate change. |
| **III. Ownership & authorization at the data boundary** | ✅ **Strengthened** | This feature's substance. Authorization stays server-side at the GraphQL/DB boundary; the consolidation makes it harder to omit. New `world_invites` columns are additive; the table already carries `created_by`. |
| **IV. Real ADRs and specs before divergent implementation** | ⚠️ **ADR required** | See below. |
| **V. Verify before claiming done** | ✅ Planned | `cargo test -p thunderforge` + `tsc` + `vite build`; e2e for the rotation path. No WASM target involved. |
| **DMCA / content-moderation guardrail** | ✅ Not triggered | The guardrail governs features exposing one world's *content* beyond that world. An access link admits a person *into* a world; it exposes no compendium content across worlds and creates no new sharing surface. ADR-049's existing determination for share links is unchanged and no new determination is required. Recorded in spec Assumptions. |

**Principle IV — ADR obligation.** Two decisions here are architecturally
significant and MUST land as an ADR in the same change set, not retroactively:

1. **Permission resolution becomes macro-generated from a single declaration**,
   and the polymorphic-table alternative is rejected on `ON DELETE CASCADE`
   grounds (research §1). This changes how every future permissioned content
   type is added.
2. **`world_invites` becomes a revocable, rotatable access link**, and remains
   distinct from content share links at the storage level (research §4).

Planned as **ADR-050**, authored during Phase 1 of implementation. This is a
gate condition, not a nice-to-have: without it the next contributor sees a
macro with no recorded reason and the polymorphic option looks unexplored.

**No violations requiring justification.** Complexity Tracking is therefore
omitted.

## Project Structure

### Documentation (this feature)

```text
specs/027-unified-access-links/
├── plan.md              # This file
├── research.md          # Phase 0 output ✅
├── data-model.md        # Phase 1 output ✅
├── quickstart.md        # Phase 1 output ✅
├── contracts/           # Phase 1 output ✅
│   ├── graphql-access-links.md
│   └── permission-resolution.md
├── checklists/
│   └── requirements.md  # 16/16 passing
└── tasks.md             # Phase 2 (/speckit-tasks — NOT created here)
```

### Source Code (repository root)

```text
src/server/
├── migrations/
│   └── 2026-08-26-*-add_revocation_to_world_invites/   # NEW: up.sql + down.sql
└── src/
    ├── auth/
    │   ├── world_membership.rs          # CHANGED: gains is_dm_of_world (from actor_permissions)
    │   ├── permissioned_entities.rs     # NEW: the macro + the single declaration
    │   ├── actor_permissions.rs         # SHRINKS: resolution now macro-generated
    │   ├── item_permissions.rs          # SHRINKS: ditto
    │   ├── lore_permissions.rs          # SHRINKS: ditto; loses the `pub use` shim
    │   └── ability_permissions.rs       # SHRINKS: keeps is_ability_visible_to by hand
    ├── graphql/
    │   ├── mutations_invites.rs         # CHANGED: revoke + rotate; atomic join
    │   ├── queries/invite.rs            # CHANGED: link state in the payload
    │   └── share_codes.rs               # NEW: the one code generator
    └── schema.rs                        # REGENERATED by diesel

apps/web/src/
├── api/world.ts                          # CHANGED: revoke/rotate operations
├── hooks/useWorldInvites.ts              # CHANGED: state derivation + refetch on rotate
└── components/
    ├── campaign/CampaignSettingsPanel.tsx    # CHANGED: refresh/revoke controls, state badges
    └── world/SessionSetupInviteLink.tsx      # CHANGED: reflects link state

docs/adrs/
└── 2026????-050-*.md                     # NEW: see Constitution Check
```

**Structure Decision**: The existing web-application layout is used unchanged —
Rust server under `src/server/`, React app under `apps/web/`. The one new
server module (`auth/permissioned_entities.rs`) sits beside the four modules it
generates, so the declaration and its consumers are read together. No new
top-level directories.

## Implementation Sequencing

Ordered so the correctness fixes land before the behaviour-preserving refactor,
per the spec's story priorities. Each phase is independently shippable.

| Phase | Delivers | Rationale |
|---|---|---|
| 1 | Migration + ADR-050 + shared code generator | Additive; unblocks everything; no behaviour change |
| 2 | **US1** — revoke + rotate, atomic join | The headline capability. Includes the §5 race fix. |
| 3 | **US2** — ability-grant cleanup on member removal | The live privilege leak. Deliberately fixed by hand here, *before* the macro exists, so it ships even if Phase 5 slips. |
| 4 | **US3 + US4** — link state surfacing, uniform failure | Makes Phase 2's control legible |
| 5 | **US5** — macro consolidation; hand-written cleanup from Phase 3 replaced by generated | Largest change, least visible effect, lands last |

**Phase 3 before Phase 5 is deliberate and worth stating plainly**: the
consolidation is what makes the bug *unrepeatable*, but a hand-written fourth
cleanup block is what makes it *fixed*. Shipping the fix first means a slip in
the refactor does not leave the privilege leak open.

## Risks

| Risk | Mitigation |
|---|---|
| Macro consolidation silently changes an authorization outcome | SC-003 is the guard: the entire existing authorization suite must pass **unmodified**. Any test edited to accommodate the change is a failure signal, not a fix. |
| The DM-check move touches 49 sites | Mechanical and compiler-verified — a missed site is a build error, not a runtime bug. |
| Migration invalidates a live invite | `DEFAULT FALSE` on `revoked` makes every existing row read active. Asserted by SC-006 with a test over pre-migration rows. |
| Rotation loophole (cap reset) misread as a security control | Recorded in spec Edge Cases; GM-facing copy must not describe the cap as enforcement. |
| `useWorldInvites` has no live push | Known and documented; rotation refetches explicitly rather than awaiting a subscription. |

## Post-Design Constitution Re-check

Re-evaluated after Phase 1 artifacts.

- **Principle III** — the design keeps every check server-side; the macro emits
  the same `FORBIDDEN` extension the hand-written functions return today, and
  visibility (`gm_only`) stays a separate axis per FR-019. ✅
- **Principle IV** — ADR-050 is scoped above and scheduled in Phase 1. ✅
- **Principle V** — quickstart.md defines the runnable checks per story. ✅
- **DMCA guardrail** — design introduces no cross-world content exposure;
  `world_invites` remains world-scoped and the no-enumeration invariant is
  preserved (a GM lists only their own world's links, as today). ✅

No new violations. Gate passes.
