# Specification Quality Checklist: Interactive Elements — Props, Doors and Triggers

**Purpose**: Validate specification completeness and quality before proceeding to planning
**Created**: 2026-08-30
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

All items pass. Both open questions were answered and are recorded in the
spec's **Decisions** section rather than removed, because each rules out an
alternative somebody will otherwise propose again later.

- **Secrets are a table concern, not a wire concern.** Secret geometry and its
  metadata travel to clients that do not draw it, consistent with the earlier
  token-visibility decision. The alternative costs per-viewer scene filtering
  permanently, to frustrate somebody who has decided to spoil their own game.
- **Effects are contributed by subsystems, not enumerated here.** This
  dissolved the question about sound and scene transitions rather than
  answering it: an absent subsystem contributes nothing, so nothing dead is
  ever authorable. It also means audio, multi-scene navigation, party tokens
  and space travel each arrive by contributing effects without reopening this
  feature.

The strongest requirement to hold the line on is **FR-039** — this feature's
own logic may not reference any specific effect, target type or subsystem, and
removing every contributor must leave a working feature that offers nothing.
It is the one that will be quietly violated first, most likely by doors, since
doors are the effect it is most tempting to treat as built in. **US7** exists
to catch exactly that.

Two named non-goals are worth re-reading before planning, because they are the
ones a reader is most likely to assume are included: **party tokens** (a
GM-controlled token the whole party sees and follows, for world-map scenes)
and **multi-scene management**. Both are coming; neither is here.
