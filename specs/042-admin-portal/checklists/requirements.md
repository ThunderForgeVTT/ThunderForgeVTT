# Specification Quality Checklist: The Admin Portal

**Purpose**: Validate specification completeness and quality before proceeding to planning
**Created**: 2026-09-07
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

- **US8 is a restriction and is P1 anyway.** "The portal cannot read the
  table" constrains every other story in the spec: US1 through US7 built
  without it quietly become a window into everybody's game. It is placed among
  the P1s deliberately so it cannot be the thing that slips to a second pass,
  which is exactly what happens to restrictions when they are ranked by
  visible value.

- **FR-010 is the requirement most likely to be built wrong**, and the numbers
  are in the Context so nobody has to take it on faith. `storage/dedupe.rs`
  measured 4,387 canvas assets holding 61 distinct images — 2,695 MB stored
  for 116 MB of content, with 3,815 rows sharing bytes across worlds. So
  "storage for this user" genuinely has two answers, they differ by orders of
  magnitude, and an operator deciding whether to remove an account needs the
  second one. A single unlabelled number would be worse than no number,
  because it would be acted on.

- **FR-012 exists because of a gap in another subsystem.** Nothing in this
  product deletes stored objects — `storage/rustfs.rs` has no delete operation
  and `dedupe.rs` says reference counting is the prerequisite — so
  "reclaimable" is arithmetic, not a button. Labelling it theoretical is the
  honest presentation until that changes.

- **FR-018 fixes a live defect rather than adding a feature.** Today
  `/admin/moderation` reaches a case only through `repeatInfringerFlags`, so a
  case leaves the list the moment its owner files a counter-notice — at
  exactly the point a human has 10 to 14 days to decide it. The e2e written
  for spec 015's counter-notice half had to create four cases to keep one
  reachable, which is the shape of the bug seen from the outside.

- **The spec deliberately does not rebuild the eleven existing panels.** They
  work. What they lack is an instance around them, which is what US1 to US3
  add.

- **The two hardest boundaries are both in Out of Scope on purpose**: no
  impersonation, and no editing a person's content. An administrator may take
  content down through the moderation process; they may not become somebody or
  rewrite what they wrote. Both are the sort of thing that gets added later
  "just for support" and is very hard to remove afterwards.
