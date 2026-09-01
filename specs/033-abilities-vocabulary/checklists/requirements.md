# Specification Quality Checklist: An Open Ability Vocabulary and a Guarded System Switch

**Purpose**: Validate specification completeness and quality before proceeding to planning
**Created**: 2026-09-01
**Feature**: [spec.md](../spec.md)

## Content Quality

- [x] No implementation details (languages, frameworks, APIs)
- [x] Focused on user value and business needs
- [x] Written for non-technical stakeholders
- [x] All mandatory sections completed

## Requirement Completeness

- [x] No [NEEDS CLARIFICATION] markers remain
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

## Notes

All items pass. Four judgements are recorded in the spec's **Decisions**
section rather than left implicit, because each rules out an alternative that
will otherwise be proposed again.

- **Two halves, one spec.** The spec opens by arguing this rather than assuming
  it: the orphaned-type behaviour (FR-034..FR-038) belongs to neither half
  alone, and splitting would leave it unowned. Each half remains independently
  deliverable — User Story 2 needs none of the vocabulary work, and User
  Stories 1/3/4 need none of the guard.

- **The closed-enum question is settled, not open.** ADR-054 already rejected a
  central enum for the same reason it fails here, and enforced its replacement
  with an automated repository check. FR-012 and SC-003 carry that forward as a
  checkable property rather than an intention.

- **The switch is non-destructive.** Content authored for another system is
  stored per system and is not removed or re-tagged when the world's system
  changes, so "mistranslation" is *hidden and recoverable*, not lost. FR-024
  makes that a requirement so a later change cannot quietly break it, and
  FR-026 forbids the warning from overstating it. Severe presentation, honest
  wording.

- **Item binding has no home today.** Items carry mechanical effects, not
  abilities. The spec confronts this directly: abilities gain an item
  attachment as a peer of the existing character attachment, item effects are
  left alone, and the two are reconciled only in how an item presents them
  (FR-020). This is the largest piece of new surface in the feature and is why
  it is P4 rather than higher.

## Open items for the feature owner

- **Story priority.** The guarded switch is P2 on the grounds that the
  operation it guards is non-destructive. If the intent is to close the
  unguarded hazard before anything else ships, it should be re-prioritised to
  P1 — nothing else in the spec depends on that ordering.
- **Grade scope.** A grade is specified as a recorded, displayed, ordered
  property (FR-021..FR-023) — not a slot, charge or resource. If "spell levels"
  is meant to imply slot consumption, that is usage tracking and is currently
  out of scope here and in spec 025.
