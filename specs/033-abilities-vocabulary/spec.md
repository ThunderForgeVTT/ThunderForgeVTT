# Feature Specification: An Open Ability Vocabulary and a Guarded System Switch

**Feature Branch**: `033-abilities-vocabulary`

**Created**: 2026-09-01

**Status**: Draft

**Input**: User description: "Abilities need a type identifier and the interface should break the tab set out by it (in 5e: Spells, Enchantments, Feats). A system supplies its own umbrella term and its own type names over the same shape underneath — 5e would not call the concept 'abilities'. In 5e/Pathfinder an enchantment binds to an item; a spell binds to a character and has levels. Support those facets without building a custom thing under the hood, so the app stays hyper mobile between systems. Separately: a GM may change a world's game system, but if content already exists the interface must flag it in red and require a double confirmation, warning of data loss / mistranslation."

## User Scenarios & Testing *(mandatory)*

### Why these two halves are one specification

They are specified together because they share a single seam and neither is
complete without the other. Once a game system owns the vocabulary of ability
types, the set of types a world can express becomes a property of the world's
active system — so *changing that system changes what the world's existing
abilities are called and whether they are recognised at all*. The question "what
happens to an Enchantment when the world stops being 5e?" cannot be answered
inside half one, and cannot even be asked inside half two. Splitting them would
put the answer in neither spec.

They remain **independently deliverable**: User Story 2 (the guarded switch)
ships value with none of the vocabulary work done, and User Stories 1, 3 and 4
ship value with the switch left as it is today. Only the orphaned-type behaviour
(FR-026..FR-030) requires both, and it is written so that it degrades correctly
if only one half exists.

---

### User Story 1 - The compendium breaks out ability types, in the system's own words (Priority: P1)

A GM opens their world's compendium. Today they find a single flat list of
abilities with a "Type" column, and to see only spells they must read down the
column. They want what their rulebook gave them: a section per kind of thing.
They also want those sections called what their game calls them — a 5e GM does
not have "Abilities"; they have spells, feats, and (once User Story 3 lands)
enchantments. A Genie GM has Scrolls and Knacks. A Blades GM has neither word.

The compendium's ability area breaks out into one tab per ability type that the
world's active system recognises, in an order the system chooses, each labelled
in the system's own plural term. The umbrella name of the area itself — the word
on the outer compendium tab, the page heading, the create button — is also the
system's word, not "Abilities". Selecting a type tab shows only abilities of
that type; creating from within a tab pre-selects that type.

**Why this priority**: This is the visible complaint, and it is deliverable on
top of what already exists — the four built-in types and per-type labels already
ship (spec 025, FR-009..FR-014), and every shipped pack already loads its
manifest for attributes and status resources. It requires no new contribution
mechanism, no schema change and no data migration, and it makes the value of
Stories 3 and 4 legible before either is built.

**Independent Test**: Seed a world with abilities of at least two types, set the
world's system to one that supplies vocabulary and to one that supplies none,
and confirm in both cases that the ability area is tabbed by type, that every
label comes from the system where it supplies one, and that a system supplying
nothing still produces a correct, fully-labelled tab set. No other story needs
to exist.

**Acceptance Scenarios**:

1. **Given** a world whose active system supplies an umbrella term and plural
   type names, **When** the GM opens the compendium, **Then** the ability area
   is named with the system's umbrella term and presents one tab per recognised
   type, each carrying the system's plural label, in the system's declared
   order.
2. **Given** that world, **When** the GM selects a type tab, **Then** only
   abilities of that type are listed, and the count shown on the tab matches the
   number of rows listed.
3. **Given** a type tab is selected, **When** the GM creates a new ability from
   within it, **Then** the new ability is of that type without the GM choosing
   a type again.
4. **Given** a world whose active system supplies no ability vocabulary at all,
   **When** the GM opens the compendium, **Then** the area is named with the
   application's default umbrella term and every built-in type gets a tab with
   its default label — no blank tab, no missing tab, no error.
5. **Given** a type tab that currently contains no abilities, **When** the GM
   selects it, **Then** they are told the section is empty and offered creation,
   rather than being shown the unfiltered list.
6. **Given** a player (not a GM) viewing the same world, **When** they open the
   ability area, **Then** they see the same vocabulary and the same tab set,
   with GM-only abilities excluded from both the listings and the tab counts.

---

### User Story 2 - Changing a world's system warns with real numbers and asks twice (Priority: P2)

A GM has been running a world in one system for months — dozens of actors,
abilities and items authored against it — and opens System Settings intending to
try another. Today the change applies on a single click with no warning at all.
They want the application to stop them, tell them exactly what is at stake in
numbers, and make them say yes twice.

When a GM selects a different system for a world that already contains authored
content, the interface presents a prominent, visually severe (red) warning that
names the counts of content that will be affected and the systems that content
was authored for, states plainly what will and will not happen to it, and
requires two distinct deliberate confirmations before the change is applied. A
world with no content is switched without ceremony. The server refuses any
switch request for a world with content that does not carry the GM's
acknowledgement, so the guard cannot be bypassed by calling the operation
directly.

**Why this priority**: It closes a live, unguarded data-visibility hazard and is
independent of every other story in this spec. It is P2 rather than P1 only
because the underlying operation is non-destructive (see FR-024) — the harm it
prevents is a GM's confusion and lost session time, not deleted rows.

**Independent Test**: Seed a world with a known number of actors, abilities and
items; attempt a system change; confirm the warning appears, that the numbers it
names match the seeded numbers exactly, that a single confirmation does not
apply the change, that cancelling leaves the world's system unchanged, and that
a content-free world switches with no warning at all.

**Acceptance Scenarios**:

1. **Given** a world containing 12 actors and 30 abilities authored while a
   given system was active, **When** the GM selects a different system, **Then**
   a red warning names those exact counts and that system by its display name,
   and the change is not yet applied.
2. **Given** that warning is shown, **When** the GM confirms once, **Then** the
   change is still not applied and a second, distinct confirmation is required
   that names the system being switched to.
3. **Given** the warning is shown, **When** the GM cancels at either step,
   **Then** the world's active system is unchanged and no content is altered.
4. **Given** a world containing no actors, abilities, items or lore, **When**
   the GM selects a different system, **Then** the change proceeds through the
   existing confirmation flow with no red warning and no second confirmation.
5. **Given** a world with content, **When** a system change is requested without
   the acknowledgement the warning collects, **Then** the request is refused and
   the world is unchanged.
6. **Given** a world with content, **When** a non-GM member requests a system
   change, **Then** it is refused regardless of acknowledgement.
7. **Given** the GM has completed both confirmations, **When** the change
   applies, **Then** the interface states what became hidden and that switching
   back restores it, and the world's content counts are unchanged.
8. **Given** a GM selects the system the world is already using, **When** they
   submit, **Then** no warning is shown and nothing changes.

---

### User Story 3 - A system names its own ability types (Priority: P3)

A 5e GM wants a section for **Enchantments**, and the concept does not exist in
the application's built-in set. A different system wants Cantrips; another wants
Manoeuvres; another wants none of these. The GM should get their section because
their system pack asked for it — not because the application was edited to know
about 5e.

A game system declares the ability types it recognises as part of its own
content. The types a world can use are the union of the application's built-in
types and the types its active system declares. A declared type carries its own
identity, its singular and plural names, its position in the tab order, and
(User Story 4) its binding facets. A type declared by one system is not offered
in a world running another. Adding a type never requires editing anything shared
by other systems.

**Why this priority**: It is the architecturally load-bearing half — it is what
makes the interface "hyper mobile between systems" rather than a growing list of
special cases — but it is worth nothing to a GM until the tabs of User Story 1
exist to hold the new types, and it introduces the orphaned-type problem that
User Story 2's guard makes survivable.

**Independent Test**: Add a type declaration to one shipped system pack and none
of the others; confirm the new type appears as a tab and as a creation option in
a world running that system, is absent in every other world, and that no file
shared between systems was modified to achieve it.

**Acceptance Scenarios**:

1. **Given** a system pack declaring a type not in the built-in set, **When** a
   GM opens a world running that system, **Then** that type appears as its own
   tab and as a choice when creating an ability.
2. **Given** the same pack, **When** a GM opens a world running a *different*
   system, **Then** that type is offered nowhere.
3. **Given** a system declares a type whose identity matches a built-in type,
   **When** a world runs that system, **Then** the system's names and ordering
   are used and exactly one tab appears for it — never two.
4. **Given** two contributors declare the same type identity in a way that
   cannot be resolved as a re-labelling, **When** the application assembles the
   vocabulary, **Then** the conflict is reported at assembly time and is not
   left to surface when a GM happens to author one of them.
5. **Given** a system declares no types at all, **When** a GM opens a world
   running it, **Then** the built-in types are available and nothing is lost.
6. **Given** an ability authored under a type its world's current system does
   not recognise, **When** the GM opens the ability area, **Then** that ability
   is still listed, still opens, still edits and is never deleted or silently
   re-typed (see FR-026..FR-030).

---

### User Story 4 - Types declare what they bind to and how they are graded (Priority: P4)

A 5e enchantment is a property of a *sword*. A 5e spell is a property of a
*character*, and it has a level — 1st through 9th. Pathfinder's ranks, another
system's circles or tiers are the same shape with a different word. A GM should
be able to author an enchantment onto an item and a 3rd-level spell onto a
character, and the application should not contain a bespoke "enchantment
feature" or a bespoke "spell level field" to make that possible.

An ability type declares, generically, **what an ability of that type may be
attached to** — a character, an item, or nothing — and **whether abilities of
that type carry an ordered grade**, along with the system's word for that grade
and its range. The application enforces the binding and presents the grade in
the system's own words. Item-bound abilities appear on the item they are
attached to alongside that item's existing mechanical effects, presented as one
list to the GM.

**Why this priority**: It is the largest piece of new surface — it is the only
story here requiring a place for item-bound abilities to live, and it is the
only story that touches the item sheet. Stories 1 and 3 deliver most of the
felt value without it, and this depends on Story 3 having established who
declares a type in the first place.

**Independent Test**: Declare in one system a character-bound graded type and an
item-bound ungraded type; confirm a graded ability records and displays its grade
in the system's word for it, confirm the item-bound type can be attached to an
item and refuses attachment to a character, and confirm the character-bound type
does the reverse.

**Acceptance Scenarios**:

1. **Given** a type declared as binding to characters and carrying a grade
   called "Level" over 1..9, **When** the GM authors an ability of that type,
   **Then** they are asked for its Level, values outside 1..9 are refused, and
   every surface showing that ability shows the grade using the word "Level".
2. **Given** a type declared as binding to items, **When** the GM opens an
   item, **Then** they can attach an ability of that type to it, and that
   ability appears on the item.
3. **Given** an item-bound type, **When** the GM opens a character sheet,
   **Then** abilities of that type are not offered for attachment to the
   character.
4. **Given** a character-bound type, **When** the GM opens an item, **Then**
   abilities of that type are not offered for attachment to the item.
5. **Given** an item carrying both mechanical effects and an attached ability,
   **When** the GM views the item, **Then** both are presented in one place,
   each identified as what it is, with no duplicated entry.
6. **Given** a type that declares no grade, **When** the GM authors an ability
   of that type, **Then** no grade is asked for and none is displayed.
7. **Given** an item-bound ability is attached to an item and the item is
   deleted, **When** the GM opens the ability, **Then** the ability itself
   still exists in the compendium and reports that it is no longer attached.

---

### Edge Cases

- **An ability whose type the world's system no longer recognises.** This is
  where the two halves meet. The ability is never deleted, never re-typed and
  never hidden: it is grouped under a clearly-marked section for unrecognised
  types, labelled with the type identity it was authored under, and restored to
  its own tab the moment a system recognising that type is active again.
- **A grade whose scale a new system does not define.** The recorded grade value
  is retained and shown as a plain recorded value; it is not clamped, rescaled
  or discarded.
- **An item-bound ability under a system where its type is now character-bound
  (or vice versa).** The existing attachment is preserved and reported as
  inconsistent with the current system; no new attachments of that shape are
  permitted while that system is active.
- **A system pack that is missing, uninstalled, or fails to load.** The world
  falls back to the built-in vocabulary; every ability remains listed, with
  types shown under the unrecognised section rather than lost.
- **A malformed vocabulary declaration** — an empty name, a non-object entry, a
  grade range whose minimum exceeds its maximum. The bad entry is ignored and
  the rest of the system's vocabulary still applies; the application never
  presents a blank label.
- **Two GMs switching a world's system at the same time.** The last write wins
  and each GM is shown the world's actual resulting system, not the one they
  chose.
- **A world switched away and immediately back.** Every previously visible
  ability, actor and item is visible again, with nothing renamed or re-typed.
- **A system change while a session is live.** The warning states that connected
  players will see the world's vocabulary change; the change is not silently
  applied only to the GM.
- **Counts in the warning at the moment of confirmation.** If content was added
  between the warning being drawn and the second confirmation, the applied
  change is still correct; the warning is a disclosure, not a lock.

## Requirements *(mandatory)*

### Functional Requirements

**Ability types and their presentation (User Story 1)**

- **FR-001**: Every ability MUST carry exactly one ability type identity.
- **FR-002**: The application MUST present a world's abilities broken out by
  ability type as a tab set, one tab per type the world's active system
  recognises, rather than as a single undifferentiated list.
- **FR-003**: A game system MUST be able to supply an **umbrella term** — the
  name of the concept itself, singular and plural — that replaces the
  application's default word ("Ability"/"Abilities") on every user-facing
  surface naming the concept, including the compendium tab, page headings,
  creation controls, empty states and confirmation text.
- **FR-004**: A game system MUST be able to supply the singular and plural name
  and the display order for each ability type it recognises.
- **FR-005**: Supplying an umbrella term, names or ordering MUST be optional. A
  system supplying none MUST produce a complete, correctly-labelled tab set
  using the application's built-in vocabulary.
- **FR-006**: Every user-facing surface that names an ability type — tabs,
  badges, filters, creation controls, item and character sheets, share views —
  MUST use the active system's vocabulary, and none MUST show a built-in name
  where the system supplied its own.
- **FR-007**: Each type tab MUST show the number of abilities of that type
  visible to the viewer, and that count MUST equal the number of rows the tab
  lists.
- **FR-008**: Creating an ability from within a type tab MUST default the new
  ability to that type.
- **FR-009**: An empty type tab MUST state that it is empty and offer creation,
  and MUST NOT fall back to showing abilities of other types.
- **FR-010**: The tab set and its labels MUST be identical for GMs and players;
  only the abilities within them differ, per the existing GM-only visibility
  rules.

**Contributed vocabulary (User Story 3)**

- **FR-011**: The set of ability types available in a world MUST be the union of
  the application's built-in types and the types declared by that world's active
  game system.
- **FR-012**: Adding a new ability type for one game system MUST NOT require
  modifying anything shared with other game systems. This MUST be verifiable by
  an automated repository check, in the manner ADR-054 established for
  interaction effects — a violation must be something anyone can see, not a
  matter of judgement.
- **FR-013**: A type declared by one system MUST NOT be offered for authoring in
  a world running a different system.
- **FR-014**: A declaration whose identity matches a built-in type MUST be
  treated as re-labelling that type, producing exactly one tab, and MUST NOT
  create a duplicate.
- **FR-015**: An irreconcilable identity collision between two declarations MUST
  be reported when the vocabulary is assembled, not when a GM first authors one
  of the colliding types.
- **FR-016**: A malformed or unusable declaration MUST be ignored without
  discarding the rest of that system's vocabulary, and MUST never produce a
  blank or missing label.
- **FR-017**: The four ability types that exist today MUST remain permanently
  available as built-ins. Existing worlds and existing abilities MUST require no
  migration, no re-typing and no GM action.

**Binding and grading facets (User Story 4)**

- **FR-018**: An ability type MUST be able to declare what an ability of that
  type may be attached to: a character, an item, or nothing.
- **FR-019**: The application MUST refuse to attach an ability to a subject its
  type does not permit, and MUST enforce that refusal at the data boundary, not
  only in the interface.
- **FR-020**: Abilities attached to an item MUST be visible on that item, listed
  together with that item's existing mechanical effects, each identified as what
  it is, without duplication.
- **FR-021**: An ability type MUST be able to declare an ordered grade — the
  system's own name for it, and its permitted range — and abilities of such a
  type MUST record a value on it.
- **FR-022**: Every surface displaying a graded ability MUST show the grade
  using the system's word for it, and MUST show no grade for ungraded types.
- **FR-023**: Grade values outside a type's declared range MUST be refused at
  authoring time; values already recorded that fall outside a *newly* declared
  range MUST be retained and displayed, never clamped or discarded.

**Changing a world's game system (User Story 2)**

- **FR-024**: Changing a world's active game system MUST NOT delete, rewrite or
  re-tag any authored content. Content authored for the previous system MUST
  remain stored exactly as authored, and MUST become visible again if that
  system is made active again.
- **FR-025**: Before applying a system change to a world that contains authored
  content, the application MUST present a visually severe (red) warning that:
  (a) names the counts of affected content by kind — actors, abilities, items,
  and any other system-tagged content; (b) names the system(s) that content was
  authored for and the system being switched to, by display name; (c) states
  plainly that affected content becomes hidden rather than destroyed and that
  switching back restores it; and (d) names anything that will be presented
  differently rather than hidden.
- **FR-026**: The warning MUST NOT overstate the consequence. It MUST describe
  what actually happens per FR-024, and MUST NOT claim data will be deleted.
- **FR-027**: Applying a system change to a world with content MUST require two
  distinct deliberate confirmations, the second naming the target system. One
  confirmation MUST NOT be sufficient.
- **FR-028**: The server MUST refuse a system change for a world with content
  unless the request carries the acknowledgement the confirmation flow collects,
  so the guard cannot be bypassed by invoking the operation directly.
- **FR-029**: A world containing no authored content MUST be switchable without
  the red warning and without a second confirmation.
- **FR-030**: Selecting the system the world already uses MUST be a no-op with
  no warning.
- **FR-031**: Only a world's DM (Owner or GM) MUST be able to change its system;
  this MUST hold regardless of acknowledgement.
- **FR-032**: Cancelling at any confirmation step MUST leave the world's active
  system and all content unchanged.
- **FR-033**: After a change is applied, the application MUST state what has
  become hidden and how to restore it.

**Where the halves meet: abilities of unrecognised types**

- **FR-034**: An ability whose type is not recognised by the world's active
  system MUST remain listed, viewable, editable and deletable by its GM. It MUST
  NOT be hidden, deleted or silently assigned a different type.
- **FR-035**: Such abilities MUST be grouped under a clearly-marked section for
  unrecognised types, each labelled with the type identity it was authored under.
- **FR-036**: Such abilities MUST return to their own tab, with the system's
  labels, when a system recognising their type is active again.
- **FR-037**: The counts in FR-025's warning MUST include abilities that will
  become unrecognised as a result of the change.
- **FR-038**: A GM MUST be able to re-type such an ability to a type the current
  system recognises, as a deliberate act, and MUST NOT have that done for them.

**Scope guard**

- **FR-039**: This feature MUST NOT make one world's abilities, items or actors
  visible, searchable or copyable from another world. The constitution's DMCA /
  Content Moderation Guardrail is therefore not triggered; existing per-ability
  share links are unchanged in scope by this feature.

### Key Entities

- **Ability**: A named, described, permissioned piece of world content. Gains
  nothing new here except a grade value where its type declares one; its
  existing identity, ownership, effects, permissions and share links are
  unchanged.
- **Ability Type**: The identity an ability is classified by. Has a stable
  identity, a singular and plural name, a display order, and its binding and
  grading facets. Some are built into the application; others are declared by a
  game system. The two are indistinguishable to a GM.
- **Ability Vocabulary**: What a game system says about abilities — its umbrella
  term and the set of types it declares or re-labels. Optional in whole and in
  part; the world falls back to built-ins for anything it omits.
- **Binding Facet**: A type's declaration of what an ability of that type may be
  attached to — character, item, or nothing.
- **Grade Facet**: A type's declaration that its abilities carry an ordered
  value, with the system's name for it and its permitted range. "Level" in 5e,
  "Rank" or "Circle" or "Tier" elsewhere; one shape, many words.
- **Ability Attachment**: The link between an ability and the thing it is
  attached to. One such relationship exists today (ability to character); this
  feature adds the item counterpart and constrains both by the type's binding
  facet.
- **Item Effect**: An item's existing mechanical effect. Unchanged by this
  feature and *not* merged into abilities; the two are reconciled only in
  presentation, on the item, per FR-020.
- **World System Assignment**: A world's active game system. Determines the
  vocabulary in force and therefore which abilities are recognised.
- **Content Inventory**: The counted, per-kind, per-system summary of a world's
  authored content, computed to populate the system-change warning with real
  numbers.
- **Unrecognised-Type Ability**: An ability whose type identity is not in the
  active system's vocabulary. A presentation state, not a stored state — nothing
  about the ability changes when it enters or leaves it.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: In a world holding abilities of two or more types, a GM reaches
  every ability of a chosen type in one click, with no text filtering and no
  scanning of a type column.
- **SC-002**: For a system supplying a full vocabulary, 0 occurrences of the
  application's built-in ability words appear on any ability surface a GM or
  player sees.
- **SC-003**: A new ability type can be added for one game system with 0
  changes to files shared by other game systems, demonstrated by an automated
  repository check that fails if the property is violated.
- **SC-004**: All four game systems already exercised end-to-end (genie, dnd5e,
  pathfinder2e, blades_in_the_dark) render their own tab set from their own
  declarations, verified by end-to-end tests that run per system.
- **SC-005**: A system change alters 0 rows of authored content: actor, ability,
  item and lore counts before and after are identical.
- **SC-006**: The counts shown in the system-change warning match the world's
  actual content counts exactly, in 100% of seeded test cases.
- **SC-007**: A system change on a world with content cannot be completed in
  fewer than two distinct confirming actions, in 100% of attempts, including
  attempts that call the operation directly without the interface.
- **SC-008**: Switching a world away from a system and back restores 100% of
  previously visible content to visibility, with 0 items renamed or re-typed.
- **SC-009**: 100% of abilities whose type is unrecognised under the active
  system remain listed, openable and editable, and 0 are deleted or re-typed
  without a GM's deliberate act.
- **SC-010**: A graded ability displays its grade in the active system's word
  for it on 100% of surfaces that show the ability, and an ungraded ability
  shows no grade on any surface.
- **SC-011**: An attachment forbidden by a type's binding facet is refused in
  100% of attempts, including attempts that bypass the interface.
- **SC-012**: Existing worlds require 0 GM actions and 0 data migration to
  continue working after this feature ships.
- **SC-013**: A system pack that is absent, malformed, or supplies no vocabulary
  produces 0 blank labels, 0 missing tabs and 0 errors.

## Assumptions

- The four ability types shipped today (spell, feat, power, talent) are treated
  as the application's permanent built-in contribution rather than as a legacy
  set to be migrated away from. Grandfathering is therefore automatic: an
  existing world's abilities keep their types and gain system labels if the
  system supplies them.
- Per-type presentation labels already exist and already come from the active
  system's own content (spec 025, FR-010..FR-014, exercised for genie today).
  This feature extends that established mechanism rather than inventing a second
  one; the umbrella term and type membership are new, the labelling idea is not.
- A game system's manifest is already the place a world reads system-specific
  vocabulary from — attributes and status resources are read this way today and
  proven across four systems end-to-end — so the vocabulary declared here has an
  established home and an established loading path.
- Ability types are *content vocabulary*, not *build capabilities*. This is the
  distinction ADR-054 drew when it rejected a manifest for interaction effects:
  a manifest may not declare a capability no code can perform, but it is exactly
  the right place to declare a naming and grouping that varies per ruleset. The
  binding and grading facets stay declarative for the same reason — a type
  declares *that* it binds to items and *that* it is graded, and the application
  performs both generically. Anything a type would need bespoke code to do is
  out of scope here.
- Item-bound abilities need a home that does not exist today: items carry
  mechanical effects, not abilities. This feature assumes that relationship is
  added as a peer of the existing character attachment, and that item effects
  are left entirely alone — they are a different concept at a different layer,
  reconciled only in how the item presents them.
- Content authored for a system remains stored as authored when the world's
  system changes; nothing in the current change operation removes or re-tags it.
  This feature's warning describes that behaviour rather than changing it, and
  FR-024 makes it a requirement so a later change cannot quietly break it.
- The content counts the warning names are computed from the world's own
  content; no cross-world query is involved.
- Ability usage mechanics remain out of scope, as in spec 025 — a grade is a
  recorded, displayed property of an ability, not a slot, charge or resource.

## Decisions

These are recorded rather than left open, because each rules out an alternative
that will otherwise be proposed again.

- **The type vocabulary is contributed, not central.** A closed, central list of
  ability types cannot express "5e also has Enchantments" without the
  application being edited for every system that ever ships — which is precisely
  the coupling ADR-054 rejected for interaction effects, and precisely what
  Constitution Principle II forbids. The precedent is decided, not open: types
  are contributed and the available set is their union, and the property is
  enforced by an automated check rather than by discipline (FR-012, SC-003).
- **The shape under the vocabulary stays one shape.** Every system's types are
  the same entity with different names and declared facets. "Label
  white-washing" is exactly the intent: 5e's Spells and Genie's Scrolls are one
  concept wearing two words, which is what keeps a world's content portable
  across a system change and keeps the application mobile between systems.
- **A system change is non-destructive, and the warning says so.** Content
  authored for another system becomes *hidden and recoverable*, not lost. The
  warning is therefore severe in presentation and honest in wording: red,
  double-confirmed, counted — and it never claims deletion. A warning that names
  "12 actors and 30 abilities authored for genie" gets read; a generic one gets
  clicked through, and a false one teaches GMs to distrust every warning the
  application shows them. The counting query is part of the same work as the
  dialog, not a follow-up.
- **The guard lives at the data boundary, not only in the dialog.** Per
  Constitution Principle III, the refusal is enforced where the change is
  applied (FR-028), because a guard that exists only in the interface is not a
  guard.
- **Abilities and item effects are not merged.** An item effect is a mechanical
  rule the resolution layer consumes; an ability is named, described,
  permissioned, shareable content. Collapsing them would either strip abilities
  of everything that makes them abilities or promote every numeric modifier into
  compendium content. They are reconciled where a GM actually experiences the
  confusion — on the item, as one list — and nowhere else.
- **An unrecognised type is a presentation state, never a data change.** The
  alternative — re-typing an ability to the nearest recognised type on switch —
  is a silent, lossy, irreversible edit performed on a GM's authored content by
  a dialog they clicked through. It is refused. Re-typing is available, as a
  deliberate act, per FR-038.

**Out of scope**: usage tracking (slots, charges, cooldowns, prepared/known);
resolution or adjudication of ability effects; canvas representation of
abilities; cross-world or public browsing of any content (FR-039); GM-authored
ability types outside a system pack; automatic translation of content between
systems.

**Constitution note (Principle IV)**: applying ADR-054's contribution seam to
ability vocabulary, and constraining the system-change operation, are
architecturally significant. An ADR recording that decision is expected to land
with the implementation, not after it.
