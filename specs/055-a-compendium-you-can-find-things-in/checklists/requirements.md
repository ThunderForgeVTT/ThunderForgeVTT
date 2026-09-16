# Specification Quality Checklist: A Compendium You Can Find Things In

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

**The owner's belief about the lore tree is nearly right, and the part that is
wrong matters.** They said folders were talked about and never implemented.
What was implemented is the storage (`world_lore_entries.parent_id`), the move
mutation with its cycle guard, the reparenting delete, and a whole client tree
library (`loreTree.ts`) — used by exactly one panel, a parent dropdown on an
entry's own detail page. So this is not a feature to build from nothing; it is
a screen to build over machinery that already works, which is a materially
different plan. The Context section shows the machinery so that the plan starts
from it.

**The bigger lore finding is not the tree.** There is **no way to hide a lore
entry from a player**, and the permission ladder structurally cannot express
one — `Viewer` is both floor and default, and the code that generates the
ladder says so in a comment warning against adding visibility to it. Every
member of a world can read every lore entry today, and the list query does not
consult the ladder at all. "So players and individuals could see it" therefore
needs a new axis before it needs a tree, which is why FR-024 exists and why Q1
is the only question in this spec about existing data.

**Items are further from the request than the request sounds.** There is no
type column, no tag table and no pack-declared item vocabulary anywhere. But
the product already ships the exact pattern for abilities — a classification,
a pack vocabulary, type tabs with counts, an unrecognised bucket — so the item
requirements are written as "the same as that", not as a new design. Q2 and Q3
are the two places where items might legitimately differ from abilities.

**Why file and line references appear in a stakeholder document.** Each of the
owner's four complaints rests on a claim about what the product does, and in
each case the truth is more specific than the complaint: the lore tree exists
but is unreachable, the items tab has nothing to index because the data has no
type, the books tab is mostly manager-only with its one browsing surface
missing a search box, and "the tokens list" names two different panels with two
different defects. A reader should be able to check each of those. The
references are confined to the Context sections and to the small number of
requirements whose subject *is* a specific existing behaviour.

**Books were answered, not restated.** The owner asked what the page needs. The
answer separates three readers: every member of a world may read a switched-on
book's entries and should get a page worth reading (FR-084); a manager gets
what they have today plus a search that works across the whole book (FR-080 to
FR-086); and an administrator gets an instance-wide view that is about books
and never about their contents (FR-087 to FR-089). That last boundary is not
invented here — `require_book_manager` already refuses an admin bypass and says
in a comment why — and FR-089 records it as a requirement so that an
instance-wide view cannot quietly become a back door.

**The play field's token list was added to this spec, not given its own.** It
is the same defect: structure the product stores and does not show, and a search
whose rules about what a player may find must match the compendium's. Two
findings are worth checking. First, the complaint is literal — the tool rail's
Tokens panel returns `null` when nothing is selected, so it renders an empty
flyout. Second, there is a *second* surface called Tokens, a modal that does
list a scene's tokens and labels every row `Token 3f2a1b9c` while the row model
already carries a name. That is spec 053's GUID defect in a different screen,
and FR-070a to FR-070c answer it the same way. Meanwhile every scene-scoped
list the panel would need already exists on the server — tokens, walls, lights,
shapes, interactives — with the player-visibility filtering already applied.

**The boundary with the placing spec is stated as three requirements, not a
sentence.** FR-079a to FR-079c say which spec owns what: spec 056 owns
the gesture and everything after the click — the carry, the snap, the drop, what
is created, whether it is scenery or interactive, the "place here" menu — and
this spec owns finding the thing and deciding who may see it. The seam is one
handover, *this thing, now, on the cursor*, and the gesture behind it already
exists (`beginTokenPlacement`). Written this way so that neither spec
respecifies the other, and so a reviewer can tell immediately which one a
question belongs to. Spec 056 was still being written when this landed; if its
own text draws the line differently, the two must be reconciled before either is
planned.

**Performance is stated in numbers against a named world.** SC-020 defines the
world once — 3,000 lore entries, 2,000 items, 500 NPCs, 500 abilities, an
8,000-entry book — and SC-021 to SC-024 give bounds on it. FR-119 makes missing
one a build failure rather than a note, because every requirement in this spec
is about a list and a correct list that takes eight seconds has not solved the
owner's problem.

**Three open questions, none blocking.** Q1 (what happens to existing lore when
visibility arrives) is the only one that touches data somebody is already
reading, and it is recommended conservatively. Q2 (one item type or several)
and Q3 (may a world add a type) are both about how closely items should copy
abilities, and a plan can start on the lore tree and the books page while they
are decided.
