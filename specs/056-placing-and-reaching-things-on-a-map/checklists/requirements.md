# Specification Quality Checklist: Placing and Reaching the Things on a Map

**Purpose**: Validate specification completeness and quality before proceeding to planning
**Created**: 2026-09-15
**Feature**: [spec.md](../spec.md)

## Content Quality

- [x] No implementation details (languages, frameworks, APIs)
- [x] Focused on user value and business needs
- [x] Written for non-technical stakeholders
- [x] All mandatory sections completed

## Requirement Completeness

- [ ] No [NEEDS CLARIFICATION] markers remain
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

**Two questions are open, and the first box above is deliberately unticked.**
Q1 (does a player see an interaction point they may not use) and Q2 (which thing
wins when a click could mean a wall or a token) are in the spec's Questions
section, each with a recommendation. Neither blocks the rest: FR-036 already says
the spec does not choose the storage shape, and FR-016 states the requirement —
resolve by a stated rule, and provide a route to the thing that lost — without
naming the rule. Planning can start; both need an answer before the menu's item
list is final.

**Why file and line references appear in a stakeholder document.** The owner's
brief is a set of complaints about what the product does, and four of them turn
out to be *almost* wrong in a way that changes the work:

- A wall **can** be selected by clicking it — but only while the Walls tool is
  armed, and in that mode a click that misses starts drawing
  (`src/engine/src/plugins/wall.rs:84-94`,
  `src/engine/src/systems/wall.rs:359-364`).
- A free-standing interaction point **already exists** as spec 030's `region` —
  and the server withholds it from players entirely
  (`src/server/src/graphql/queries/interactives.rs:270-272`), so it cannot be
  what the owner is asking for without a change.
- `Object` **already exists** as a token kind
  (`crates/thunderforge-canvas-core/src/token_kind.rs:48-49`) and decides nothing
  but a fill colour.
- The panel that does door work **says in its own doc comment** that it is meant
  to be opened by right-clicking a door
  (`apps/web/src/components/InteractionAuthor/DoorControls.tsx:14-18`).

A spec that restated the complaints without showing where each one lands would
scope the work wrongly in both directions. The references are there so a reader
can check the correction.

**Three findings a reader would not expect, and the spec is built on all three.**

1. **A designated door draws no badge.** Interaction badges are spawned only over
   a subject found in `TokenEntities`
   (`src/engine/src/plugins/interaction_marker.rs:179-190`), and a door's
   subject is a wall id. So the most common interactive in any dungeon is the
   one with no sign that it responds. FR-037.
2. **There is no way to give an object hit points.** ADR-102 makes a token with
   no actor a marker — unlinked, `system_data` null — and `changeHitPoints`
   refuses it with "That creature has no hit points recorded"
   (`src/server/src/combat/hit_points.rs:393-399`). Decision 3 asks for exactly
   this path, and it does not exist. FR-045.
3. **An object cannot be hidden from players.** Visibility hangs on
   `world_actors.visible_to_players` (`src/server/src/schema.rs:770`); an object
   has no actor. Shapes carry their own flag (`schema.rs:1087`); tokens do not.
   FR-050.

**One defect is surfaced and deliberately not fixed.**
`interaction::drop_for_subject` exists for cleaning up interactives whose subject
was deleted and has no callers anywhere
(`src/server/src/interaction.rs:276-284`); `subject_ref` is deliberately not a
foreign key, so the orphan is invisible to the database. Making walls reachable
makes this reachable too, but fixing it is its own change and is listed under Out
of Scope.

**Nothing that landed on 2026-09-15 is respecified.** The play-field right-click
menu (`da13758`) — attack, damage, heal, link/copy, hide name, remove, place a
token, add a light, and its whole keyboard and focus contract — is a dependency,
not a subject. FR-023 says so explicitly, and the "gesture this spec builds on"
section names what is being reused.

**The three owner decisions are recorded as decisions, not proposals**, in the
Decisions section, each with the argument from the code that settles it. FR-080
to FR-085 name each place spec 030 is amended, so a reader of 030 can be pointed
at the exact requirement that moved.
