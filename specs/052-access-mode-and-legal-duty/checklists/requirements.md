# Specification Quality Checklist: What an Access Mode Obliges

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

**Why file and line references appear in a stakeholder document.** The owner's
decision rests on a claim about what the product does today — "on closed there's
no real sharing" — and two parts of that claim are not true as stated. A spec
that asserted the claim without showing where it fails would launch a plan built
on it. The references are there so a reader can check the correction, and they
are confined to the Context sections; no requirement names a function or a file.

**Three open questions, all with recommendations.** None of them blocks
planning. Q1 (whether a share link opens for a stranger on a non-open instance)
is the one that decides how literally true the spec's own legal sentence is, and
it is recommended rather than decided because it changes what an already-issued
link does. Q2 (lore sync) and Q3 (how readiness words a requirement that does
not apply) are smaller. The owner's four decisions of 2026-09-15 settle
everything else.

**The legal reasoning is stated as an argument, not as advice.** "The legal
reasoning, stated plainly" lists what is claimed and four things that are not,
including that copyright still applies to material in a private world. That
section exists because the failure mode of this feature is somebody reading
"closed instances don't need DMCA information" as "closed instances are exempt
from copyright".

**One requirement contradicts the feature's own headline, on purpose.** FR-012
keeps the existing refusal to mint a share link without a notice contact in
force in every mode. The relaxation is about *when the question is asked*, not
about what an unanswered instance may do, and writing that as a requirement is
what keeps the two apart.

**Three stated defaults disagree in the code today** (a fresh install seeds
`invite_only`, an upgrade seeds `open`, and both the registry default and the
repair path say `closed`). FR-004 makes reconciling them part of the work
rather than leaving the plan to discover it.
