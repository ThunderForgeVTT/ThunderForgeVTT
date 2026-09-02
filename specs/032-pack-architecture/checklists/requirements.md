# Specification Quality Checklist: Pack Architecture

**Purpose**: Validate specification completeness and quality before proceeding to planning
**Created**: 2026-09-01
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

## Feature-Specific Checks

- [x] The two pack types are defined by disjoint responsibility, not by naming
      convention (FR-001, FR-002)
- [x] "Look only" is stated as an enforced boundary with automated validation,
      not as a design preference (FR-003, SC-003)
- [x] The security asymmetry between the two halves is explicit: the interface
      half carries no pack-code risk, the system half cannot avoid it
      (Overview, FR-017, SC-011)
- [x] The blocked half is scoped as blocked, with a stated interim restriction
      rather than an implicit "later" (FR-017, Assumptions)
- [x] Both halves are independently shippable and independently testable
      (User Story 1 and User Story 2 Independent Test sections)
- [x] Missing-pack behaviour is specified for both pack types, including
      restoration and content preservation (FR-018 through FR-021)
- [x] The empty-state wording inconsistency is captured as a testable
      requirement with a measurable outcome (FR-022, FR-023, SC-008)
- [x] The per-world scoping question is resolved with reasoning, not
      deferred (FR-009, FR-010, Decisions)
- [x] The base-pack naming choice is resolved, and the underlying requirement
      is stated so it survives the naming answer — which it had to, the answer
      having gone the other way (FR-007, Decisions)

## Open Items for Requester Confirmation

All three answered 2026-09-02. Recorded in the specification's **Decisions**
section; the answers overrode the draft on two of the three.

- [x] **Base interface pack name.** Answered: **Forge**, overriding the draft's
      "Mithral". FR-007 now states the peer requirement outright rather than
      relying on the name to imply it.
- [x] **Interface pack scoping.** Answered: **per world, set by the Game
      Master**, overriding the draft's per-user preference. FR-009 and FR-010
      rewritten; the per-world *suggestion* (old FR-010) is gone, since there is
      no per-user selection left for it to be advisory to.
- [x] **Accessibility floor for interface packs.** Answered: **rejection at
      validation**. Added as FR-012a and SC-003a. This is a consequence of the
      scoping answer — a table-wide look is one no reader can opt out of.

## Feature-Specific Checks — User Story 4 (added 2026-09-02)

- [x] The declaration vocabulary is stated as a minimum set of kinds, each
      evidenced by a shipping system's sheet, rather than as an open-ended
      "support more types" (FR-031)
- [x] Blank slots and fixed lists are distinguished, because a format modelling
      only the second mislabels the first (FR-032)
- [x] Grouped values are required, so a Fate consequence and a Cypher stat do
      not arrive as unrelated parts (FR-033)
- [x] The base pack's conformance obligation is extended to the new kinds, so
      "every system has a usable sheet" is a property rather than a promise
      (FR-034)
- [x] Unknown kinds degrade to labelled text rather than vanishing, and the
      reason is stated: absence is indistinguishable from not having it
      (FR-035, SC-014)
- [x] The manifests' own incompleteness is a requirement rather than a
      background note, since a format that could express these would still show
      nothing while they stay silent (FR-036)
- [x] The enlargement is named as an enlargement in Decisions, rather than
      folded in silently

## Validation Notes

**Drafting decisions:**

1. *Priority ordering follows the security gate, not the excitement.* The
   system-pack half is the more architecturally interesting work and the half
   with an existing partial implementation, but it is P2 because it depends on
   an unwritten decision of record (runtime pack-code loading and security).
   The interface-pack half is P1 because it is genuinely unblocked and
   independently shippable.

2. *Implementation leakage removed.* Early drafts named the existing static
   sheet map, the build-time alias mechanism, the hooks module, and specific
   ADR file paths inside requirements. All were rewritten as outcomes — "no
   change to shared application code that names the pack", "a written contract
   describing every surface a pack may contribute". The concrete mechanisms and
   the existing code they must replace belong in `plan.md`.

3. *ADR references retained only in Overview and Assumptions*, as dependency
   context for a reader, never inside an FR or SC.
