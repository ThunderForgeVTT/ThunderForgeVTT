# Specification Quality Checklist: A Dashboard That Says What Is Going On

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

**Two of the owner's panels have nothing behind them, and the spec says so
before it asks for them.** There is no invitation addressed to a person — world
invites are anonymous codes with a use count — and there is no last-played
timestamp anywhere in the schema. The first is Q1, recommended out of this spec
and into its own. The second is FR-012, written as "the product MUST record
this; nothing records it today" so that the plan cannot mistake it for a
read.

**One requirement exists because of something buried on the page being
removed.** `/counter` holds the only user-facing export-my-data and
delete-my-account controls in the app — the contracts of ADR-004, ADR-005,
ADR-011 and ADR-012. FR-051 moves them before the page goes, and FR-064 proves
it. Without that, "replace the preview page" would quietly delete two data
rights.

**The panel-source table is in the spec on purpose.** Most panels have a source
and three are per world rather than per person. Naming which is which, with
references, is what stops the plan from discovering mid-build that "characters I
hold" needs one query per world.

**The empty state is a user story, not a footnote.** Today a new account never
sees the welcome page at all — zero worlds redirects to the create-world form,
and so does a failed load, so a transient error reads as "your worlds are gone".
US5 and FR-042 separate the two.

**Three open questions, none blocking.** Q1 (invitations), Q2 (what counts as
"played", recommended as any live session in the world, since spec 051's
heartbeat is the nearest existing fact) and Q3 (which address survives). A plan
can start on the panels that have sources while these are decided.
