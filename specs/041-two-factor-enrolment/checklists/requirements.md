# Specification Quality Checklist: Two-Factor Enrolment, Recovery and Removal

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

- **`totp-rs` is named in the Context and in no requirement.** That is the
  line this spec walks deliberately: the crate, the algorithm, the digits, the
  period and the skew are all already chosen and already correct, so naming
  them is *evidence that the engine is done*, not a design. Every FR below
  would read identically against a different verifier — which is the test for
  whether an implementation detail has leaked.

- **The spec is smaller than the request and that is the point.** The ask was
  to spec TOTP with database backing; TOTP with database backing is built,
  tested since today, and the secret is already encrypted at rest. Rebuilding
  it would have produced a week of work whose output already exists. What did
  not exist is the human path, and that is what this covers.

- **FR-013 is the security fix, and it is one sentence.** Starting a new
  enrolment must not disable a confirmed factor. Today `two_factor_setup_start`
  clears `two_factor_enabled` on the password alone, so the *absence* of a
  removal feature is currently doing the job of one — an accident standing in
  for a decision. FR-012 and FR-014 complete it by giving removal a front door
  that costs what the front door should cost.

- **FR-019 is what makes the instance-wide switch usable at all.** It is
  enforced today and has no enrolment path, so turning it on bars every
  account that has not enrolled — which, since there is no enrolment
  interface, is all of them. A requirement that cannot be satisfied is not a
  policy; it is an outage with a checkbox.

- **US2 is the one that must not be cut.** Everything else degrades
  gracefully if it ships late. Shipping enrolment without recovery codes
  creates a new way for people to permanently lose their accounts, and the
  person it happens to did nothing wrong. If this feature ships in pieces,
  US1 and US2 ship together or neither ships.

- **FR-016 comes from a comment in the source.** `totp.rs` says the matched
  step exists "so a caller can refuse to accept the same step twice", and then
  drops it. The code already knew; nothing acted on it.

## Re-validation after the enrolment and administrator clarification (2026-09-07)

All items still pass. The spec grew from six stories to seven and from 26
requirements to 36.

- **US1 is now written as the build-out it needs to be**, not a sentence about
  settings. One flow with three entrances — account settings, first-run setup,
  and a sign-in that requires enrolment — identical in steps and wording,
  differing only in where somebody arrives afterwards (FR-001a). Three
  entrances and three implementations is how the second one ends up subtly
  worse than the first.

- **US3 makes the administrator rule a property of the role, not a policy**
  (FR-027). Nothing switches it off. The reasoning is that an administrator
  holds the instance — members, content, access policy, legal contacts — and
  the first one is created at the exact moment nobody has thought about
  security yet, which is the only moment when adding it is free.

- **FR-001b is the requirement that keeps US3 from being a trap.** A fresh
  instance may have no working mail server when its first administrator
  enrols, so enrolment must not depend on mail. Paired with FR-029: the
  recovery codes are issued and saveable *before setup ends*, because at that
  moment there is no mail, no second administrator, and nobody to ask.

- **FR-031 is the upgrade path, and it is easy to forget.** An instance that
  upgrades into this rule has administrators without a second factor. They
  must be walked through enrolment at next sign-in, not refused — otherwise
  the rule locks the existing world out of itself the day it ships.

- **The split is now explicit**: absolute for administrators, optional for
  everybody else, with the instance-wide requirement still an operator's
  choice (FR-033). Two different kinds of rule that would otherwise be easy to
  collapse into one setting.

- **Spec 040 gained a matching FR-002a** so its own definition of "setup is
  finished" agrees with this one. Two specs stating the same gate is
  deliberate; them disagreeing later would not be.
