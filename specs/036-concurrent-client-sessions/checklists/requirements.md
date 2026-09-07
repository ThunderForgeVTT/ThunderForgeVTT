# Specification Quality Checklist: Concurrent Client Sessions

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

- **Content Quality is a deliberate partial pass.** The Context section names
  files, a source line and existing test ids — `auth/sessions.rs:370`,
  `mutations_combat.rs`, `CombatPanel.tsx`'s selectors, the `user_sessions`
  table. That is evidence for *why the behaviour is what it is today*, not a
  design for the fix: no requirement below it names a file, a table or a
  mechanism, and FR-001 through FR-026 would read the same against a different
  implementation. The alternative — restating "signing in ends the previous
  session" without saying where that is decided — would send the planning pass
  to rediscover a one-line policy.

- **Four decisions were defaulted rather than marked.** Each is in Assumptions
  with the reasoning, and each is a fair target for `/speckit-clarify`:
  sign-out scope (one session, not all), whether a password change ends other
  sessions (yes), the concurrent-session bound and what happens at it (evict
  the oldest), and whether presence is per person or per client (per person).

- **US1 and US2 are both P1 on purpose.** US1 is the capability and US2 is the
  only way it stays true; shipping US1 alone would leave the eviction free to
  return unnoticed, which FR-020 exists to prevent.

- **FR-025 and FR-026 are load-bearing.** The eviction being removed was a
  security decision with a stated rationale, and this spec is only honest if
  it replaces that protection rather than deleting it. A plan that satisfies
  FR-001 without FR-025/FR-026 has made the instance weaker.

## Re-validation after clarification (2026-09-07)

Five answers were folded into the spec and all checklist items still pass. The
shape changed enough to be worth recording:

- **The feature narrowed and grew at the same time.** "Several equal clients"
  became "several sessions, one play field, the rest companions" (FR-027 to
  FR-031) — narrower — while gaining companion surfaces, sheet-initiated
  system checks and the peer boundary (FR-032 to FR-041) — larger. US1 and
  US2 are unchanged; US3a, US3b and US3c are new.

- **FR-035 restates an existing decision rather than making one.** ADR-044
  already holds that the server is the only party trusted to produce a roll,
  and the constitution already forbids a system's rules living in shared
  presentation code. The sheet naming a check while the system decides it is
  those two rules meeting a new surface, which is why it reads as a constraint
  and not as a design.

- **FR-038 through FR-041 are the load-bearing ones now**, alongside FR-025
  and FR-026. ADR-052 deliberately separated "may hold", "may continue" and
  "may distribute", and this spec adds a fourth: *which surface* may do so.
  A plan that lets a companion window reach the GM over a peer has crossed the
  line this spec was written to draw.

- **Two answers were defaulted inside the clarification.** Play-field takeover
  resolves in favour of the newest claim with the loser demoted and told
  (rather than refusing the new window), and a claim must not outlive its
  client (FR-030). Both are stated in Clarifications with the reasoning; both
  remain fair to revisit.
