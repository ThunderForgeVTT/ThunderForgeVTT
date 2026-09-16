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
on it. The references are there so a reader can check the correction.

**Three requirements name a file, deliberately.** FR-004 names the three places
that state a starting mode, because the defect *is* that they disagree and a
requirement to make them agree has to say which three. FR-014 names
`anonymous.rs`, because the requirement is about the exact set of resolvers
that answer without a session and that set is a list the product keeps in one
module. FR-015b names the capability the requirement attaches to. Everywhere
else, a requirement states behaviour and the Context sections carry the
references.

**The three questions this spec asked are now decided (owner, 2026-09-15) and
are recorded in Decisions 5 to 7.** A share link on a non-open instance
resolves only for a signed-in account (FR-014); enabling lore synchronisation
requires the notice contact in every mode (FR-015); readiness renders "not
required in this mode" as a third state (FR-011). The first two change what the
spec claims: with them, "a non-open instance publishes nothing outward" is true
by construction rather than nearly true with a footnote, which is what the
legal argument in the Context needs.

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
