# Specification Quality Checklist: The Game Master's Board

**Purpose**: Validate specification completeness and quality before proceeding to planning
**Created**: 2026-09-15
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

**Three of the owner's five items are already built and unreachable, and saying
so is most of this spec's value.** Fog of war has a table, a mutation with an
ownership guard, a pause gate, a query and a render layer, and a grep of
`apps/web/src` for `fogMask` returns nothing. Grid snapping and the room and
door primitives are implemented in the engine and `set_wall_primitive` is
exported to wasm with no caller. The colour channel a time-of-day tint needs
runs unbroken from `AmbientLight.color` through `SetAmbientLight` to the
darkness shader's `ambient.rgb`, and nothing has ever populated it — which is
why every dark scene in the product is the same hard-coded moonlight blue. A
plan that reads this spec as five new features would budget several times what
the work costs.

**The spec supersedes a decision of spec 045, deliberately and in one place.**
Spec 045 decision 3 built explored areas and wrote "There is no fog of war" in
its own problem statement, putting Game-Master-painted fog in Out of Scope.
Decision 1 here reverses only that: fog becomes a shared, server-authoritative
layer, and explored areas stay exactly as 045 built them — per player, in that
player's browser, never on the server (FR-033). The hierarchy the owner asked
for is what makes both correct at once: lights, then fog, then each player's
own memory.

**The most consequential correction is about the shape of the stored fog.**
The existing `fog_masks` row holds a bitmap. Spec 045's exploration module
documents, in its own header, why a bitmap is the wrong shape for exactly this
data — large, needs rescaling when the grid changes, meaningless outside a
shader — and stores cells instead. FR-028 and Decision 3 retire the bitmap. A
plan that adopts the existing column because it is there would build the one
design the codebase has already argued against, and would end with fog and
exploration speaking two vocabularies about the same board.

**Two defects were found that the owner did not ask about and would not
expect.** Every wall the wall tool draws is emitted with `blocksMovement:
false` — only a drawn door blocks movement — so since spec 045 gave that flag
teeth, a Game Master who draws a room watches players walk through it. And
`updateFogMask` writes without the `world_events` broadcast every other scene
mutation carries, so a reveal would have reached nobody live. FR-006 and FR-026
exist because of those two.

**"Right side of the screen" is a genuine conflict, so it is a question rather
than an assumption.** The owner asked for time of day on the right. The right
side is the play dock (Chat, Actors, Combat, Settings); the scene's light, the
lamps, the walls and the fog are all on the left tool rail. Question 5 states
the trade honestly and recommends against his literal words, because splitting
the scene's light from the time of day across two rails is the same failure
that made him unable to find the scene's light in the first place.

**The seam with spec 056 is drawn in four requirements, not left to a
planner.** 056 owns the *gesture* — right-clicking a wall, selecting one by
clicking it, the one "place here" menu, interaction points, what an object is —
and it explicitly keeps the rail panel (its FR-006) and defers where a placed
thing lands to "the scene's snapping rule" (its FR-003). 057 owns what the
panel *means*, and what that snapping rule *is*. FR-002 makes arming a drawing
action and 056's *start a wall here* two ways into one behaviour; FR-005 makes
the panel and 056's wall menu read one state; FR-005b forbids either from
reimplementing the other; FR-018 makes 056 FR-003's deferral point at FR-010 to
FR-017. FR-035 keeps fog from being offered as the answer to 056 FR-050's
hide-one-object, which it is not: fog hides an area and everything in it.

**Five questions are left open, four of them genuinely reversible.** Fog shared
versus per player (Q1) is the one the brief predicted and the one with the
largest cost difference. Named times versus a continuum (Q2), scene versus
world clock (Q3), whether turning fog off keeps it (Q4) and which rail holds it
all (Q5) follow. Each carries a recommendation. Everything the owner settled in
his own words on 2026-09-15 is in Decisions 1 to 8 instead, including the two
this spec decided on his behalf and said why: a creature view lets a Game
Master act rather than only look (Decision 6), and it does not survive a reload
(Decision 7).
