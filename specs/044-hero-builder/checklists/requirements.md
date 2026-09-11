# Specification Quality Checklist: The Hero Builder

**Purpose**: Validate specification completeness and quality before proceeding to planning
**Created**: 2026-09-10
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

- **This spec names packages and a mutation on purpose.** The owner asked
  where the builder lives ("a frontend project like engine"), so the
  package/app split is part of the WHAT. It is decided in research R1. The
  existing `uploadActorImage` mutation and `validateHero` are named because
  the security posture rests on reusing them rather than adding new doors.

- **Phase (c) contains a finding, not just a feature.** As read on
  2026-09-10, a claim or a player-created character (spec 017) writes no
  actor-permission row, and a member with no grant resolves to Viewer. The
  imagery mutation requires Editor. So a player most likely can't set their own
  character's art today, even by uploading a file. FR-030 fixes this narrowly,
  with an imagery-only right for the holder and not a general Editor grant.

- **FR-011 exists because the package's safe default is unsafe in a builder.**
  The default id prefix is derived from the hero, so "identical heroes clash
  harmlessly". A builder shows the same hero several times: the main preview
  and the thumbnail of the current choice. A `url(#…)` that resolves to a
  hidden copy draws no gradient. So the builder always sets explicit prefixes.

- **FR-038 is the price of phase (d).** Stored specs turn the catalogue's
  choice names into a compatibility surface. Adding and redrawing choices stays
  free, while renaming or removing one is no longer allowed.

- **Randomise and quick NPC are deterministic tooling.** They pick from closed
  lists with a seeded generator. ADR-051 is respected, and the only AI mention
  is in Out of Scope, framed as optional assistance to a human GM.
