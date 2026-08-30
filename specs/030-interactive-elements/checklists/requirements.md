# Specification Quality Checklist: Interactive Elements — Props, Doors and Triggers

**Purpose**: Validate specification completeness and quality before proceeding to planning
**Created**: 2026-08-30
**Feature**: [spec.md](../spec.md)

## Content Quality

- [x] No implementation details (languages, frameworks, APIs)
- [x] Focused on user value and business needs
- [x] Written for non-technical stakeholders
- [x] All mandatory sections completed

## Requirement Completeness

- [ ] No [NEEDS CLARIFICATION] markers remain
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

**One item deliberately fails.** Two decisions are open, carried in the spec's
own **Open Questions** section rather than as inline markers so they read as
decisions to make rather than gaps to fill:

- **Q1 — how protected is a prepared secret?** The project already decided
  that a player inspecting their own client is a table problem rather than an
  engineering one. Whether that extends to permanently prepared content is a
  genuine question, not a default: stealth is transient and self-correcting,
  a spoiled secret door is spoiled for good.
- **Q2 — what happens to effects whose subsystem does not exist?** Sound and
  multi-scene navigation are both unbuilt. The spec must not quietly assume
  either, and the three options differ in scope by a lot.

Both change scope materially, which is why they are questions rather than
assumptions. Everything else was defaulted and recorded under Assumptions.

Two named non-goals are worth re-reading before planning, because they are the
ones a reader is most likely to assume are included: **party tokens** (a
GM-controlled token the whole party sees and follows, for world-map scenes)
and **multi-scene management**. Both are coming; neither is here. The region
and approval models are shaped to admit them.
