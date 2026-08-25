# Specification Quality Checklist: World Abilities Compendium

**Purpose**: Validate specification completeness and quality before proceeding to planning
**Created**: 2026-08-25
**Feature**: [spec.md](../spec.md)

## Content Quality

- [x] No implementation details (languages, frameworks, APIs)
- [x] Focused on user value and business needs
- [x] Written for non-technical stakeholders
- [x] All mandatory sections completed

## Requirement Completeness

- [x] No [NEEDS CLARIFICATION] markers remain — all 3 resolved with the requester 2026-08-25, recorded in the Clarifications section
- [x] Requirements are testable and unambiguous
- [x] Success criteria are measurable
- [x] Success criteria are technology-agnostic (no implementation details)
- [x] All acceptance scenarios are defined
- [x] Edge cases are identified
- [x] Scope is clearly bounded
- [x] Dependencies and assumptions identified

## Feature Readiness

- [x] All functional requirements have clear acceptance criteria
- [x] User scenarios cover primary flows
- [x] Feature meets measurable outcomes defined in Success Criteria
- [x] No implementation details leak into specification

## Validation Notes

**Iteration 1** — 3 `[NEEDS CLARIFICATION]` markers (FR-022/023/024 in the
draft). All three were genuine scope-driving decisions with no safe default.
Presented to the requester; all three answered.

**Iteration 2** — clarifications folded in, spec restructured:

| Question | Resolution | Spec impact |
|---|---|---|
| Naming collision with the per-system `abilities` manifest block | Neither concept renamed. Systems supply optional **presentation facets** re-labeling shared classifications in their own vocabulary (5E "Spells"/"Feats", Genie "Scrolls") | New FR-009..FR-014; new `Ability Classification` + `Ability Presentation Facet` entities; SC-006 extended; 2 new edge cases |
| Share links / Copy-to-World | **In scope**, mirroring spec 013's item shares | New User Story 6 (P3); FR-032..FR-037; SC-008; **new blocking Guardrail Checkpoint section** |
| Actor attachment ("this NPC knows Fireball") | **In scope**, mirroring item inventory | New User Story 3 (P2); FR-021..FR-023; `Known Ability Entry` entity; SC-007 |

All checklist items pass as of iteration 2.

**Iteration 3 (clarification session, 2026-08-25)** — run post-plan at the
requester's direction, as a readiness pass before implementation. Three questions
asked and answered; all downstream artifacts updated to match.

| Question | Resolution | Spec impact |
|---|---|---|
| Can a GM hide an individual ability, or does the ownership block only control editing? | Add a per-ability `gmOnly` flag, separate from the ownership block, mirroring `scenes.hidden` | FR-024 amended; FR-024a-d + FR-025 rewritten; US5 rewritten as two mechanisms; SC-004a; `Ability` entity gains the flag |
| Does a player viewing an NPC see its GM-only abilities? | No — silently omitted, with no inferable trace | FR-023 amended; US3 scenario 3 amended |
| Which ability does `[[Name]]` resolve to when two share a name? | Oldest wins, deterministic `ORDER BY created_at ASC`; same fix applied to items | FR-030a, FR-030b; 3 new edge cases |

**Why this mattered**: question 1 exposed a genuine contradiction, not a gap.
US5's stated purpose was hiding secret abilities, but the permission model it
inherits *structurally cannot* express "hidden" — `ActorPermissionLevel`'s lowest
value is `Viewer`, which is also the default for a member with no row. Its
acceptance scenario 3 described an unreachable state. Left unresolved, US5 would
have been built in full and then found not to do the thing it exists for.
Question 2 was the direct knock-on (FR-023 vs the new FR-024b), and question 3
covered a latent non-determinism inherited from the item resolver.

Checklist state unchanged at 16/16 — these were correctness fixes to already-
passing items, not newly satisfied criteria.

## Constitution Alignment

- **Principle III** (ownership/authorization at the data boundary,
  `created_by`/`updated_by` provenance) — captured as FR-027, reinforced by
  FR-024..FR-026 and SC-004.
- **Principle IV** (spec before divergent implementation) — satisfied by this
  document; an ADR is likely warranted at `/speckit-plan` time for the
  presentation-facets addition to the manifest contract (ADR-027's territory).
- **Principle I** (ECS owns simulation) — not engaged; abilities are explicitly
  non-canvas this pass (Non-Goals).
- **DMCA / Content Moderation Guardrail** — engaged by User Story 6 only.
  Captured as an explicit blocking Guardrail Checkpoint section, with FR-037
  (no discoverability/enumeration) written specifically to keep the feature on
  the non-repository side of the required determination. User Stories 1-5 are
  unaffected and may proceed independently.

## Notes for the next phase

- User Story 6 must not begin implementation until the Guardrail Checkpoint is
  satisfied and recorded. Stories 1-5 are unblocked.
- FR-010's presentation facets touch the system manifest contract governed by
  ADR-027 — `/speckit-plan` should decide whether that warrants an ADR
  amendment or a new ADR.
- The spec deliberately mirrors spec 013 throughout; the plan phase should
  confirm which of spec 013's tables/modules can be genuinely reused or
  generalized versus duplicated.
