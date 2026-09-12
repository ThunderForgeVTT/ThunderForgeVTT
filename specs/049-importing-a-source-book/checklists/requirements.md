# Specification Quality Checklist: Importing a Source Book

**Purpose**: Validate specification completeness and quality before proceeding to planning
**Created**: 2026-09-12
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

Two items needed a second pass, and one is a deliberate, recorded exception.

**Named existing artefacts are not implementation details.** The spec names
`crates/thunderforge-pdf`, `packs/systems/dnd5e/server/src/statblock.rs` and
`scripts/check-system-registry.mjs`. Each is a thing that already exists and a
constraint on the work — "this must be reached through the reader that is
already there", "this must not need an exemption from the check that already
fails the build". Removing them would make the spec shorter and the
constraints unenforceable. This matches the house pattern in specs 045, 047
and 048.

**The three open questions carry stated defaults.** Rather than leaving
[NEEDS CLARIFICATION] markers, each undecided point has a default written into
the requirements (FR-051 commercial-by-default, the Assumptions entry refusing
image-only books, removal-without-versioning) and a table under "Questions for
the owner". The spec is therefore complete and testable as written, and the
owner's answers narrow it rather than unblock it. This is the pattern specs
046, 047 and 048 follow.

**Numbers in Success Criteria are not yet all measured.** SC-001 and SC-003
are stated against the 246-book library that already exists and has already
been measured. SC-002's 90% is a target, not an observation; FR-060 requires
the measurement run that will either meet it or correct it. It is marked as a
target in the spec rather than presented as a finding.
