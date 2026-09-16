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

**One of the owner's panels has nothing behind it, and one had a source under a
name nobody would look under.** There is no invitation addressed to a person —
world invites are anonymous codes with a use count — so FR-035 drops the panel
and FR-037 sends targeted invitations to their own spec. Last played turned out
to exist: `world_live_play.last_beat_at`, the durable per-world mark spec 051's
heartbeat writes. The spec's first draft said nothing records it, which is true
of `worlds` and `world_members` and false of that table; FR-012a now requires
reading the existing record and forbids a second one. This is the correction
most worth checking, because a plan built on the first draft would have added a
column the product already has.

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

**The three questions this spec asked are now decided (owner, 2026-09-15) and
are recorded in Decisions 5 to 8.** Invitations leave this spec; "played" means
any live session in the world by anybody, including a Game Master preparing
alone; and `/welcome` is the address that survives, with `/counter` redirecting
to it. Each was decided by taking the recommendation the question carried.
