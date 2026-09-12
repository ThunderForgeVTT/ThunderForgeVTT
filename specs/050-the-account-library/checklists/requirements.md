# Specification Quality Checklist: The Account's Library

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

**Two success criteria are deliberately unfinished, and say so.** SC-004 and
FR-072 defer the acceptable read-latency margin to measurement rather than
inventing a number. A criterion whose threshold is guessed is not measurable,
it is decorative — and the repository has a precedent for stopping at a
measurement gate (spec 043) rather than shipping past one.

**"Base", "delta" and "inheritance" are modelling terms, not implementation.**
They name what a Game Master experiences — a book, their changes to it, and a
world using it — and each has an acceptance scenario stated in those terms.
None names a storage mechanism, and the spec deliberately does not say how the
resolution is performed.

**No questions remain open.** All three were answered on 2026-09-12 and the
"Questions for the owner" section was removed rather than left as a stub.
Decisions 4 to 7 record the answers and what each one settled.

**One answer was "measure it", and that is recorded as a gate, not a
deferral.** Decision 6 keeps kind-plus-name as the provisional identity rule
and puts FR-029's experiment against the real corpus in front of the delta
model shipping, with FR-089c making it a gate. A rule invented at a desk for
re-attaching somebody's month of work to a re-parsed book is exactly the kind
of guess this project has been burned by, and the repository has a precedent
for stopping at a measurement (spec 043) rather than shipping past one.

**The spec grew a second kind of thing, and that is a real scope change.**
Decision 4 puts authored collections on the same shelf as imported
compendiums, which spec 050 had previously scoped out. The machinery is shared
— shelf, book list, base and delta, origin — so what is genuinely new is
sync-back (FR-100 to FR-105), download (FR-009a to FR-009c), and authoring
into a collection at all. This is noted here so the plan sizes it honestly
rather than treating it as a naming change.

**One requirement exists to stop a claim being taken on trust.** FR-086 asks
for evidence that switching a book off leaves no copy behind. The whole
storage argument rests on nothing ever being copied into a world, and that is
exactly the sort of property that quietly stops being true.

**Q2's default is the spec's known weak point and is not hidden.** Identity by
kind and name breaks on renames. That is stated in Edge Cases, in FR-025, and
in the question itself, rather than being discovered during implementation.
