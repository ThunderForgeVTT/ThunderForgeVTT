# Specification Quality Checklist: Instance Setup and Configuration

**Purpose**: Validate specification completeness and quality before proceeding to planning
**Created**: 2026-09-07
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

- **The largest finding is an absence: there is no mail.** No SMTP, no mailer,
  no queue, no template anywhere in the codebase. Meanwhile spec 039 says a
  person is told when a strike lands and when an appeal resolves, spec 037
  tells a submitter what became of their feedback, and spec 035's invitations
  arrive somehow. Those are promises with nothing behind them. That is why
  FR-013 to FR-017 give this feature the *capability* and not only the
  settings: configuring mail without providing mail would repeat the mistake
  at one remove.

- **FR-010 is the one that pays off later.** Spec 007 decided the environment
  wins for OAuth fields; every subsystem since has been free to decide again,
  and one of them will decide differently. Stating it once, for every setting,
  costs a sentence now and prevents a class of "why did my value not take".

- **FR-024 and the Out of Scope entry on migration are deliberately
  conservative.** `SYNC_GITHUB_APP_*` works today and somebody is running it.
  A configuration feature that requires working deployments to be
  reconfigured is a configuration feature nobody upgrades into.

- **FR-026 is a dependency other specs rely on.** Spec 039 requires that an
  instance with no notice contact cannot publish beyond a world. That
  requirement is stated in both places on purpose — 039 states the rule
  because it is a copyright rule, 040 states it because it is the gate — and
  they must not drift.

- **This spec claims ownership of a boundary.** Specs 037 and 039 currently
  carry configuration requirements of their own (`FEEDBACK_GITHUB_APP_*`, the
  operator identity and notice contact collected at setup). Those belong here.
  A pointer has been added to each so the requirement is not written twice and
  answered differently.
