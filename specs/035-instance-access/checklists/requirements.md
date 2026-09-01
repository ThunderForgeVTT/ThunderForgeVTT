# Specification Quality Checklist: Instance Access

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

## Notes

- **16/16 items passing**, with two operator decisions recorded as assumptions
  rather than blocking markers, because the spec is testable under either
  answer:
  1. **Default policy for a new instance** — assumed invite-only. Closed is the
     more conservative reading of the operator's intent. Either satisfies
     FR-013; the choice only changes what an operator must do on day one.
  2. **Retention period for handled access requests** — assumed 90 days. The
     number must be one the operator is willing to print on the public form
     (FR-035), so it is theirs to set, not the spec's.
- **The ADR-007 / ADR-042 conflict is resolved, not deferred**, and that
  resolution is the spec's centre of gravity. The instance access policy is a
  gate *above* both: it decides *whether* a stranger is admitted; ADR-042
  continues to decide *how* an admitted person's account is created. A closed
  instance therefore restores ADR-007's effective outcome for unmatched OAuth
  identities without reverting ADR-042 on an open one. FR-005 through FR-009
  state this in testable form; SC-001 is the test that would have caught the
  silent-admission bug. **Planning must produce an ADR** recording this
  layering — it changes the meaning of an accepted decision and is exactly the
  kind of change Constitution Principle IV requires be written down.
- **Two carve-outs are load-bearing and must survive planning**: first-run
  administrator bootstrap (ADR-008) is never gated (FR-010, SC-003), and
  linking a provider identity to an *existing* account is authentication, not
  signup, so the ADR-006 password-confirmation path is untouched (FR-009).
- **User Story 3 crosses a new data boundary** — personal data from
  unauthenticated non-users. Story 5 (retention, consent notice, abuse) is
  scoped as a separate priority deliberately so it is reviewed on its own
  terms; it must not be dropped as "part of" Story 3. Planning should confirm
  with the DMCA/moderation program owner (ADR-043) that inbound request text
  is covered by existing moderation tooling or gets its own.
- **Not in scope, stated so it stays out**: world invitations (ADR-050) are
  complete and untouched; an instance invitation confers no world membership
  (FR-017). No new role is introduced. No delivery protocol is chosen.
