# Specification Quality Checklist: Sound at the Table

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

- **Bevy is named nowhere in the spec, deliberately.** The description
  mentioned it; the requirements do not, because none of them would change if
  the engine's audio came from somewhere else. The one place a mechanism is
  named — the seam check in FR-028 — is there because *not changing a specific
  file* is the acceptance criterion, and a requirement to leave something
  alone has to say what.

- **FR-018 is what makes FR-017's ambiguity tenable.** The product taking no
  position on what a GM shares is only defensible while it keeps nothing: a
  pipe that retains is a library, and a library invites exactly the question
  the ambiguity is meant to leave alone. A plan that caches shared audio "for
  performance" has changed what this feature is, not how it works.

- **FR-007 through FR-012 are not UX polish.** Audio is the one medium a
  person cannot avert their eyes from, and an unmutable track is something
  done *to* somebody. These sit at P1 for that reason, not because they are
  easy.

- **FR-031 exists because of a specific past failure.** Spec 031's `bevy_state`
  delta was supposed to be recorded when the flag landed and was not; by the
  time anyone looked, the engine no longer compiled without it and the number
  was gone for good. Requiring a generated figure at the moment audio lands is
  that lesson written down where it will be read.

- **The riskiest assumption is "one table plays one thing at a time."** It is
  the simplest honest behaviour and it is very likely the first thing a GM
  asks to change — a music bed *plus* a door creak is the obvious next want.
  Mixing is in Out of Scope on purpose, and that boundary deserves the first
  question in `/speckit-clarify`.

- **US4 could ship before US1** if the tab share is what is wanted soonest: it
  needs no content pipeline at all. It is ordered second because it inherits
  the mute, the indicator and the first-sound prompt from US1–US3, and
  shipping it first would mean building those under a different name.

## Re-validation after clarification (2026-09-07)

Four answers folded in; all checklist items still pass. What changed:

- **The ambiguity moved to where it belongs.** It was written as one blanket
  position over the whole feature; it is now specific to the *share* path, and
  the reason is stated rather than asserted: a shared tab produces no copy, so
  there is nothing to file a notice against. Uploaded media is content and is
  moderated as content (FR-026a–FR-026g). That is a stronger position than the
  first draft, not a weaker one — it is defensible in both halves for
  different reasons.

- **FR-018a is new and is the load-bearing line.** "Nothing is cached" was too
  absolute to be true: playback needs a buffer, and a plan that discovered
  this later would have had to either break the requirement or quietly
  redefine it. Buffering and recording are now distinguished explicitly —
  bounded in seconds, memory only, discarded as played, never addressable —
  so the distinction is a requirement rather than a plan's private judgement.

- **FR-026c is the one that will be forgotten if it is not insisted on.**
  `collections::moderation_entity_type` returns `None` for `"scene"` today,
  so a takedown against a scene lands on its images instead — documented at
  the callsite, and still a gap. Audio must not ship the same way. Being a
  moderated entity from day one costs almost nothing now and is awkward to
  retrofit.

- **The constitution's DMCA guardrail is engaged, and half of it is
  outstanding.** Condition (a) — the notice-and-takedown program is
  operational — was verified in the codebase, not assumed: `moderation/`
  implements intake, disable, counter-notice with waiting period and lazy
  auto-restoration, and repeat-infringer tracking, and spec 015's 41 tasks are
  complete. Condition (b) — the on-record determination of whether this
  constitutes "a centralized public repository" — **is not something a spec can
  give itself.** The reading written into the spec is that uploading audio adds
  a content type to an exposure that already exists rather than creating a new
  repository. It needs the accountable owner's confirmation before
  implementation begins, and the spec says so in those words.
