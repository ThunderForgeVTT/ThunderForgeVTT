# Specification Quality Checklist: Moderation Across the Instance

**Purpose**: Validate specification completeness and quality before proceeding to planning
**Created**: 2026-09-15
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

**The defect is on the server, not in the CSS.** The owner named a screen full
of GUIDs. The Context section shows why re-rendering it would not have fixed
anything: the moderation surface has four queries and none of them lists cases
or returns a name. That is recorded with references so the plan does not start
from "improve the presentation".

**Two content-type holes were found while reading, and are requirements rather
than footnotes.** The web client's entity-type union omits abilities, which the
server supports; and collections — the most publishable content type in the
product, readable by a caller with no account — are not a moderation entity type
at all. FR-050 and FR-051 make closing both part of this feature.

**One requirement exists to keep a report out of a public repository.** In-app
feedback (spec 037) delivers to the project's public issue tracker. FR-023 names
that path and forbids it, because the failure would be irreversible and would
happen to the person least able to absorb it.

**Privacy is stated as requirements, not as a principle.** Who may read a
report (FR-040), what a listing must not publish about a claimant (FR-041),
that reads are attributable (FR-042) and how long a report is kept (FR-043),
which the owner settled at two years on 2026-09-15.

**The three questions this spec asked are now decided (owner, 2026-09-15) and
are recorded in Decisions 6 to 8.** A consequence ladder for repeated
misconduct is deliberately not in this feature, so that thresholds are not
invented in a spec about visibility; a moderator who is not an operator is not
built now, and the requirements are worded so the role can be added later
without rewriting them; a resolved conduct report is kept for two years. Each
was decided by taking the recommendation the question carried.

**Scope held against the statute.** Spec 015's counter-notice, restoration and
repeat-infringer counting and spec 039's ladder are read and listed by this
spec, never redefined; FR-044 keeps conduct out of the copyright ladder so the
two cannot contaminate each other.
