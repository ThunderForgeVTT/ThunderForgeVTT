# Feature Specification: World Abilities Compendium

**Feature Branch**: `025-world-abilities-compendium`

**Created**: 2026-08-25

**Status**: Draft

**Input**: User description: "World Abilities compendium — GM-authored abilities (spells, feats, powers, talents) as first-class world artifacts, closing out the 'Coming Soon' placeholder that spec 011 deliberately left on the Compendium's Abilities tab (FR-008). Model this on spec 013 (items-inventory), which is the direct precedent: it graduated the Items tab out of that same placeholder state with its own tables, GraphQL surface, permissions, detail/edit pages, compendium tab + preview panel, and lore cross-linking. Abilities should follow that established shape rather than inventing a parallel one."

## Clarifications

Three scope questions were resolved with the requester on 2026-08-25:

1. **Naming collision with the per-system `abilities` manifest block** (which
   holds ability *scores* like Might/Cunning/Spirit, not spells): neither
   concept is renamed. Instead, each game system supplies **presentation
   facets** that re-label this feature's ability classifications in
   system-appropriate language — 5E might present them as "Spells" and
   "Feats", Genie as "Scrolls". The underlying concept and data are shared;
   only the labels are system-specific. Captured as FR-009..FR-013.
2. **Share links / Copy-to-World**: **in scope**, mirroring spec 013's item
   shares (User Story 6). This triggers Constitution v1.1.0's DMCA/Content
   Moderation Guardrail as a blocking prerequisite — see Guardrail Checkpoint.
3. **Actor attachment**: **in scope** — actors have known abilities, mirroring
   item inventory (User Story 3).

### Session 2026-08-25

- Q: Should a GM be able to hide an individual ability from players entirely, or does the ownership block only control who can *edit* it? (FR-025, US5) → A: Option C — add a per-ability `gmOnly` flag, separate from the ownership block, mirroring `scenes.hidden`. The ownership block continues to mean edit rights only.
- Q: When a GM attaches a GM-only ability to an NPC, should a player who can view that NPC see it in the NPC's known-abilities list? (FR-023 vs FR-024b, US3/US5) → A: Option A — hidden entirely. Non-DMs get a filtered list with GM-only abilities silently omitted; they cannot tell anything was withheld.
- Q: When two abilities in the same world share a name, which one should `[[That Name]]` in a lore entry link to? (FR-006 vs FR-028, US4) → A: Option A — oldest wins, via a deterministic `ORDER BY created_at ASC`. Apply the same fix to items, which carry the identical latent bug.
- Q: A deleted ability leaves a tombstone entry on any actor that knew it, but the tombstone carries no `gm_only` flag — so deleting a GM-only ability would leak its name. How should a tombstone read to a player? (FR-023, raised during US3 implementation) → A: Redact it. Every tombstone reads `REDACTED` to a non-DM, secret or not, enforced server-side rather than in the UI.

## Context: why this spec exists

Spec 011 (world-compendium) built the Compendium as a tabbed portal and
deliberately shipped Items and Abilities as labeled "coming soon"
placeholders (its FR-008), because neither had a data model yet. Spec 013
(items-inventory) subsequently graduated the Items tab into a real,
fully-functional feature. The Abilities tab was never given the same
treatment and remains the last placeholder in the Compendium — a visible,
tracked gap rather than a broken promise.

This spec closes it, following spec 013's established shape rather than
inventing a parallel one, so a GM curating a world has the same authoring
experience for a spell/feat/power as they already have for an item.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - A GM authors an ability and it appears in the Compendium (Priority: P1) 🎯 MVP

A GM curating their world opens the Compendium's Abilities tab and, instead
of a "coming soon" placeholder, sees a real searchable table of the world's
abilities. They create a new ability — a spell, feat, power, or talent —
giving it a name, a rich description, and a classification. It appears
immediately in the table, and selecting its row opens a preview panel
summarizing it, exactly as the NPCs and Items tabs already behave. The
classification choices they see are labeled in their own system's language.

**Why this priority**: This alone removes the last placeholder in the
Compendium and delivers the core value — a GM can record and find their
world's abilities. Every other story in this spec enriches abilities that
this story makes exist; none of them are reachable without it.

**Independent Test**: Open the Compendium's Abilities tab as a GM, create an
ability, confirm it appears in the table, search for it by name, select its
row, and confirm the preview panel shows what was entered. Delivers a
working ability catalog with no other story implemented.

**Acceptance Scenarios**:

1. **Given** a world whose Compendium previously showed an Abilities
   placeholder, **When** any world member opens the Abilities tab, **Then**
   they see a real, searchable ability table (empty-state messaging if the
   world has no abilities yet) and no "coming soon" text anywhere.
2. **Given** a GM on the Abilities tab, **When** they create an ability with
   a name, description, and classification, **Then** it is saved and appears
   in the table without a page reload.
3. **Given** a world with several abilities, **When** a member types part of
   an ability's name into the tab's search box, **Then** the table narrows to
   matching abilities.
4. **Given** a member viewing the ability table, **When** they select a row,
   **Then** a preview panel shows that ability's summary alongside the table,
   matching the NPCs/Items tabs' layout.
5. **Given** a non-GM world member on the Abilities tab, **When** they view
   it, **Then** they can browse, search, and preview abilities but see no
   create/edit/delete affordances.

---

### User Story 2 - A GM records what an ability actually does (Priority: P2)

Having created an ability, the GM records its mechanical shape: what it does
when used, expressed as structured but system-agnostic data (an effect type,
a dice/formula string, a target resource) rather than as prose buried in the
description. A single ability may carry several effects — a spell that both
deals damage and applies a modifier, for instance.

**Why this priority**: An ability with only a name and prose description is a
note, not a game artifact. Structured effects are what make abilities usable
by a future resolution/rolling system, and they mirror how Items already
work — but the catalog is genuinely useful without them, so this follows P1
rather than blocking it.

**Independent Test**: Add two effects of different types to an existing
ability, confirm both persist independently, edit one without disturbing the
other, and remove one. Delivers mechanical authoring on top of Story 1's
catalog.

**Acceptance Scenarios**:

1. **Given** a GM editing an ability, **When** they add an effect with a type,
   a dice/formula string, and a target resource, **Then** it is saved and
   shown as part of that ability.
2. **Given** an ability with more than one effect, **When** the GM edits or
   removes one, **Then** the other effects are unaffected.
3. **Given** a GM entering an effect, **When** they leave the formula empty or
   structurally invalid, **Then** saving is rejected with a clear validation
   message and nothing is persisted.
4. **Given** an ability with effects, **When** any member previews it, **Then**
   its effects are displayed as part of the preview.

---

### User Story 3 - An actor knows abilities (Priority: P2)

A GM opens an NPC and records which abilities that character knows — the
villain who casts Fireball, the veteran who has the Cleave feat. The actor's
sheet shows their known abilities; the ability itself can show which actors
know it. This mirrors how an actor's inventory already lists items.

**Why this priority**: Without this, the ability catalog is an inert reference
list — abilities exist in the world but no character possesses them. This is
what makes abilities usable at the table. It follows P1 because the catalog
must exist before anything can reference it, and sits alongside P2 effects as
the other half of "abilities are real game artifacts."

**Independent Test**: Attach two abilities to an actor, confirm both appear on
that actor, remove one, and confirm the ability itself still exists in the
Compendium (detaching is not deleting).

**Acceptance Scenarios**:

1. **Given** a user with Editor or Owner access to an actor, **When** they
   attach an ability to that actor, **Then** it appears in that actor's known
   abilities.
2. **Given** an actor who already knows an ability, **When** the same ability
   is attached again, **Then** no duplicate entry is created.
3. **Given** an actor with known abilities, **When** a user with at least
   Viewer access to that actor views it, **Then** they see the list of known
   abilities, with any GM-only ability silently omitted unless the viewer is a
   DM (FR-023, FR-024b).
4. **Given** a user with only Viewer access to an actor, **When** they view
   it, **Then** attach/detach controls are not available to them.
5. **Given** an ability attached to one or more actors, **When** it is deleted
   from the Compendium, **Then** the delete succeeds and those actors' entries
   remain visible but are clearly marked as referencing a deleted ability.
6. **Given** an ability attached to an actor, **When** it is detached, **Then**
   the ability itself remains in the world's Compendium unchanged.

---

### User Story 4 - Abilities cross-link with lore, both directions (Priority: P3)

A GM writing a lore entry references an ability by name using the same
in-text link syntax they already use for lore entries, actors, and items —
and the link resolves. Conversely, opening that ability shows an
automatically-maintained list of every lore entry currently linking to it, so
the GM can see where in their world's fiction the ability is invoked.

**Why this priority**: Cross-linking is what makes the Compendium a connected
world rather than four unrelated tables, and abilities are the last content
type excluded from it. Valuable but strictly additive to Stories 1-3.

**Independent Test**: Write a lore entry linking to an ability, confirm the
link resolves and navigates; open the ability and confirm the lore entry
appears in its "linked from" list; delete the link from the lore body and
confirm the ability's list updates.

**Acceptance Scenarios**:

1. **Given** a lore entry body containing an in-text link whose title matches
   an ability, **When** the entry is rendered, **Then** the link resolves to
   that ability and navigates to it.
2. **Given** an ability referenced by one or more lore entries, **When** a
   member views that ability, **Then** it shows a current list of every lore
   entry linking to it.
3. **Given** a link title matching more than one kind of target (a lore entry,
   an actor, an item, and/or an ability) at once, **When** an author inserts
   it, **Then** all matching candidates are offered as distinct, disambiguated
   choices so the author explicitly picks one.
4. **Given** an ability that lore entries link to, **When** it is deleted,
   **Then** deletion succeeds and those links subsequently render as
   unresolved/broken rather than blocking the delete.

---

### User Story 5 - Per-ability access control (Priority: P3)

Two independent controls. First, a GM can mark an individual ability **GM-only**,
hiding it from players entirely — a villain's signature power, or a secret
technique nobody has discovered yet. Second, each ability carries an ownership
block letting the GM grant specific members Viewer, Editor, or Owner *edit*
rights, with the GM always retaining full control.

These are deliberately separate mechanisms: the ownership block governs **who may
change an ability**, the GM-only flag governs **whether players can see it at
all**.

**Why this priority**: The ownership-block half matches actors (spec 010), lore
(spec 012), and items (spec 013), so abilities being the sole content type
without it is an inconsistency worth closing. The GM-only half is genuinely new
— no compendium content type can be hidden from players today; only scenes can,
via their own `hidden` flag — and it is what makes secret abilities possible at
all. The catalog remains fully usable for open-information worlds without either.

**Independent Test**: As a GM, mark an ability GM-only and confirm from a second
member's session that it is absent from the tab, its detail route, and lore-link
resolution; unmark it and confirm it appears. Separately, grant that member
Editor and confirm they can edit it but cannot reassign permissions.

**Acceptance Scenarios**:

1. **Given** an ability marked GM-only, **When** a non-DM member views the
   Compendium, **Then** it does not appear in the ability table, its detail data
   is inaccessible, and a lore link to it renders unresolved.
2. **Given** an ability marked GM-only, **When** a DM views it, **Then** it is
   fully visible and clearly marked as GM-only in the UI.
3. **Given** a GM-only ability, **When** the GM unmarks it, **Then** it becomes
   visible to members immediately, with no other change to its data.
4. **Given** an ability with no explicit permission entries, **When** a world
   member with no entry views it, **Then** they have Viewer (read-only) access by
   default, consistent with actors/lore/items.
5. **Given** a GM, **When** they view any ability in their world, **Then** they
   have full control regardless of the ownership block's contents.
6. **Given** a member with Editor access to an ability, **When** they edit it,
   **Then** the edit succeeds; **When** they attempt to change its ownership
   block or its GM-only flag, **Then** that is denied (DM-only).

---

### User Story 6 - Sharing an ability with another world (Priority: P3)

A GM who has authored a well-crafted ability wants to share it — with a friend
running a different campaign, or into another of their own worlds. They
generate a share link for that specific ability; anyone opening it sees a
read-only preview. A logged-in viewer with DM access to another world can copy
it into that world as a new, fully independent record.

**Why this priority**: Directly mirrors spec 013's item share links (its
FR-022..FR-027), so abilities reach parity with items. Lowest priority because
it is the only story with an external process prerequisite (see Guardrail
Checkpoint) and delivers no value to a GM working within a single world.

**⚠️ Blocked until the Guardrail Checkpoint below is satisfied.**

**Independent Test**: Generate a share link for an ability, open it from a
session with no membership in the source world, confirm read-only preview;
copy it into a second world; confirm edits to the copy do not affect the
source; revoke the link and confirm it no longer resolves.

**Acceptance Scenarios**:

1. **Given** a member with Owner-level access to an ability (including a GM's
   implicit access), **When** they generate a share link, **Then** a stable,
   shareable reference to that specific ability is produced.
2. **Given** a valid, non-revoked share link, **When** anyone opens it,
   **Then** they see a read-only preview of the ability's full data (name,
   description, classification, effects) with no edit controls and no exposure
   of its ownership block, regardless of their own world membership.
3. **Given** a logged-in viewer of a shared ability, **When** they choose
   "Copy to World", **Then** they are shown the worlds where they hold DM
   access and must select a destination before anything is copied.
4. **Given** a confirmed copy, **When** it completes, **Then** a new,
   independent ability record (including independent copies of all effects)
   exists in the destination world with an empty ownership block and no live
   or referential link back to the source.
5. **Given** a copied ability, **When** the source is edited, **Then** the copy
   is unaffected, and vice versa.
6. **Given** a share link, **When** its creator (or a GM) revokes it, **Then**
   anyone opening it afterward sees a clear "no longer available" state rather
   than the ability's data.

---

### Edge Cases

- Two abilities in the same world share a name — allowed (matching items,
  FR-019 of spec 013), but the authoring UI should surface a non-blocking
  "did you mean [existing ability]?" prompt so duplication is deliberate.
- An ability is deleted while a lore entry links to it or an actor knows it —
  the delete succeeds; lore links render unresolved and actor entries are
  marked as referencing a deleted ability. Deletion is never blocked.
- A world's game system is changed after abilities are authored — abilities
  and their classifications survive intact; only the *labels* shown for those
  classifications change to the new system's facets.
- A world's system supplies no ability facets, or omits a label for one
  classification — the built-in default label is used for that classification.
  A system is never required to supply facets.
- A system's facets label two different classifications identically — the
  authoring UI must still present them as distinct choices, since they remain
  distinct underlying values.
- An effect's target resource names something the world's system doesn't
  define (e.g. "Mana" in a system with no mana) — accepted as authored text;
  this spec validates structure, never ruleset semantics.
- An ability's description is very long or contains rich formatting — handled
  by the same shared Markdown editing/rendering path lore and items already
  use, not a bespoke one.
- A world has no abilities yet — the tab shows a clear empty state with a
  create affordance for GMs, not a blank table or fabricated example rows.
- A GM references a GM-only ability by name in a lore entry players *can* read —
  the link renders unresolved for them, but the **name still appears** as the
  link's own text. This is accepted: the GM typed that name into readable prose
  themselves, and the ability's data stays inaccessible. Hiding the ability does
  not retroactively censor the GM's own writing.
- A GM marks an ability GM-only *after* players have already seen it — the flag
  is evaluated live, so it disappears from their view immediately. Nothing
  attempts to un-remember what a player already read.
- Two abilities share a name and one is GM-only — a non-DM's link resolves to
  the earliest-created **visible** match, skipping the hidden one; a DM's
  resolves to the earliest-created match overall. The same title can therefore
  resolve differently for a DM and a player, which is the intended consequence
  of hiding.

## Requirements *(mandatory)*

### Functional Requirements

#### Core ability entity and Compendium tab (User Story 1)

- **FR-001**: The system MUST provide a world-scoped Ability entity with a
  human-supplied name, a description supporting rich formatting, and a
  classification selected by the author.
- **FR-002**: Only a DM (a world member holding the Owner or GM role, per the
  precedent in specs 010/013) MUST be able to create a new Ability.
- **FR-003**: Every Ability MUST be identified and stored by a UUID-based
  identifier, matching the convention established for actors (spec 010) and
  items (spec 013).
- **FR-004**: The Compendium's Abilities tab MUST be a fully-functional
  searchable table with a row-preview panel, replacing spec 011's placeholder
  and matching the NPCs and Items tabs' existing layout and behavior.
- **FR-005**: Any world member MUST be able to browse, search, and preview
  Abilities in the Compendium, subject to per-ability access control (FR-024);
  create/edit/delete affordances MUST be gated to DMs and to members holding
  sufficient per-ability permission.
- **FR-006**: The system MUST NOT enforce Ability name uniqueness within a
  world — two Abilities may share a name.
- **FR-007**: While an author is entering or editing an Ability's name, the
  system MUST surface a non-blocking "did you mean [existing Ability]?" prompt
  when the entered name closely matches an existing Ability in the same world.
- **FR-008**: An Ability's description MUST be authored and rendered through
  the same shared rich-text/Markdown mechanism already used for lore entries
  and items, not a bespoke editor or renderer.

#### Per-system presentation facets (User Story 1)

- **FR-009**: Ability classifications MUST be a fixed, system-agnostic set of
  underlying values shared by every game system, so that ability data is
  portable across systems and survives a world's system being changed.
- **FR-010**: A game system MUST be able to supply **presentation facets** —
  per-classification display labels that re-express the shared underlying
  classifications in that system's own language (for example, presenting them
  as "Spells" and "Feats" in a 5E-style system, or "Scrolls" in Genie).
- **FR-011**: Supplying facets MUST be optional for a game system. Where a
  system supplies no facets, or omits a label for a particular classification,
  the system MUST fall back to that classification's built-in default label.
- **FR-012**: Every user-facing surface that displays an ability's
  classification — the Compendium table, the preview panel, authoring/edit
  forms, and an actor's known-abilities list — MUST show the active system's
  facet label rather than the raw underlying value.
- **FR-013**: Changing a world's game system MUST change only the labels
  displayed for existing abilities' classifications; it MUST NOT alter,
  migrate, or invalidate any stored ability data.
- **FR-014**: This spec MUST NOT rename or otherwise alter the existing
  per-system `abilities` manifest block (which declares ability *scores* such
  as Might/Cunning/Spirit). The two concepts coexist under the same word and
  are disambiguated by context: ability scores are system configuration;
  Abilities are world-authored compendium content.

#### Structured effects (User Story 2)

- **FR-015**: An Ability MUST support zero or more structured Effects, each
  carrying at minimum an effect type, a dice/formula string, and a target
  resource or attribute name expressed generically.
- **FR-016**: Ability Effect types MUST cover at least the same set already
  established for Item Effects (`heal`, `damage`, `modifier`, `attack-roll`)
  and MUST be extensible to future types without redesign.
- **FR-017**: A user with Editor or Owner access to an Ability MUST be able to
  add, edit, and remove that Ability's Effects independently of one another.
- **FR-018**: The system MUST reject saving an Ability Effect with an empty or
  structurally invalid formula string, with a clear validation error.
- **FR-019**: The system MUST NOT resolve, roll, evaluate, or apply an Ability
  Effect — Effects are authored data intended for a future resolution system
  to consume (see Non-Goals).
- **FR-020**: The Ability Effect data model MUST be scaffolded to anticipate
  (without implementing) a future trigger/activation concept distinguishing an
  on-use effect from a passive/always-active one, mirroring FR-004a of spec 013.

#### Actor attachment (User Story 3)

- **FR-021**: Every Actor MUST have a set of known Abilities — an unordered
  collection of references to Abilities in the same world, with at most one
  entry per distinct Ability (re-attaching an already-known Ability MUST NOT
  create a duplicate).
- **FR-022**: A user with Editor or Owner access to an Actor MUST be able to
  attach and detach Abilities on that Actor. Attach/detach MUST require
  permission on the *Actor*, independent of the acting user's permission on the
  Ability itself, mirroring the inventory rule in spec 013 (FR-013).
- **FR-023**: A user with at least Viewer access to an Actor MUST be able to
  view that Actor's known-abilities list, **excluding any Ability marked GM-only
  when the viewer is not a DM** (FR-024b). The filtering MUST be silent — a
  non-DM MUST NOT be able to infer that entries were withheld, whether from a
  placeholder row, a count, or a gap in ordering. Detaching an Ability MUST NOT
  delete the Ability itself, and deleting an Ability MUST NOT be blocked by
  Actors knowing it — such entries MUST remain visible but be clearly marked as
  referencing a deleted Ability.
- **FR-023a**: A tombstoned known-ability entry (its Ability deleted) MUST have
  its name redacted for any non-DM viewer, enforced server-side. A tombstone
  retains no GM-only flag to consult, so the system cannot tell whether it was
  secret; it therefore fails closed and redacts every tombstone rather than risk
  leaking the name of a deleted GM-only Ability. A DM continues to see the real
  name snapshot.

#### Access control (User Story 5)

- **FR-024**: Every Ability MUST have an ownership block — a per-world-member
  record of Viewer/Editor/Owner permission level — using the same model,
  defaults (Viewer for members with no explicit entry), and DM-always-full-
  control rule established for actors (spec 010), lore (spec 012), and items
  (spec 013). The ownership block governs **edit rights only**; it MUST NOT be
  the mechanism for hiding an Ability (its lowest level, Viewer, is also its
  default, so it cannot express "hidden").
- **FR-024a**: Every Ability MUST carry a GM-only flag, independent of its
  ownership block, defaulting to not-GM-only for a newly created Ability.
- **FR-024b**: An Ability marked GM-only MUST be invisible to every non-DM world
  member across every surface: absent from the Compendium ability list and its
  search results, inaccessible via its detail data, absent from a lore author's
  in-text link candidates, absent from the ability catalog offered when attaching
  an ability to an actor, and rendered as an unresolved link where a lore entry
  references it.
- **FR-024c**: Setting or clearing an Ability's GM-only flag MUST require DM
  status; Owner-level access to the Ability alone MUST NOT be sufficient.
- **FR-024d**: A DM viewing a GM-only Ability MUST see it clearly marked as
  GM-only, so its hidden status is never ambiguous to the person who set it.
- **FR-025**: The system MUST deny access to a GM-only Ability's detail data for
  any non-DM user, enforced server-side at the data boundary rather than by UI
  gating alone.
- **FR-026**: Changing an Ability's ownership block MUST require Owner-level
  access to that Ability (or DM status); Editor access MUST NOT be sufficient.
- **FR-027**: All Ability mutations MUST enforce authorization server-side at
  the data boundary, and Ability records MUST carry `created_by`/`updated_by`
  provenance, per Constitution Principle III.

#### Lore cross-linking (User Story 4)

- **FR-028**: The system MUST support the existing in-text link syntax (spec
  012) resolving to an Ability, in addition to its existing resolution to lore
  entries, actors, and items.
- **FR-029**: Every Ability MUST maintain an automatically-derived,
  always-current "linked from" list of every lore entry whose body currently
  contains a resolved in-text link to it, mirroring actors and items.
- **FR-030**: When an in-text link title matches more than one kind of target
  simultaneously, the authoring UI MUST present all matching candidates —
  including Abilities — as distinct, disambiguated choices.
- **FR-030a**: When an in-text link title matches more than one Ability in the
  same world (permitted by FR-006), resolution MUST be deterministic — the
  earliest-created match wins — so the same title always resolves to the same
  Ability rather than an arbitrary row.
- **FR-030b**: An in-text link candidate list and link resolution MUST exclude
  GM-only Abilities for a non-DM author or reader (FR-024b). A lore entry that
  references a GM-only Ability MUST render that link as unresolved for a non-DM
  reader.
- **FR-031**: Deleting an Ability MUST NOT be blocked by lore entries linking
  to it; such links MUST subsequently render as unresolved/broken.

#### Sharing and copying (User Story 6)

- **FR-032**: A world member with Owner-level access to an Ability (including a
  DM's implicit access) MUST be able to generate a shareable link for that
  specific Ability, mirroring the item share mechanism (spec 013, FR-022).
- **FR-033**: Opening a valid, non-revoked Ability share link MUST show a
  read-only preview of the Ability's full data (name, description,
  classification, effects) without edit controls and without exposing its
  ownership block, regardless of the viewer's own world membership.
- **FR-034**: A logged-in viewer of a shared Ability MUST be able to choose
  "Copy to World", see the worlds where they hold DM-level access, and select a
  destination before anything is copied.
- **FR-035**: Confirming a copy MUST create a new, independent Ability record
  (including independent copies of all its Effects) in the destination world,
  with a new identity, an empty ownership block, and no live or referential
  link back to the source; subsequent edits to either MUST NOT affect the other.
- **FR-036**: The Ability-level Owner (or DM) who generated a share link MUST
  be able to revoke it; a revoked link MUST show a clear "no longer available"
  state rather than the Ability's data.
- **FR-037**: A shared Ability MUST NOT be discoverable by browsing, searching,
  or enumeration — a share link is reachable only by possessing the link
  itself. This feature provides no cross-world ability directory of any kind
  (see Guardrail Checkpoint).

### Key Entities *(include if feature involves data)*

- **Ability**: A world-scoped entity with a human-supplied name, a
  rich-formatted description, a classification drawn from a fixed
  system-agnostic set, a GM-only visibility flag, an ownership block
  (Viewer/Editor/Owner per world member, same model as Actor, Lore Entry, and
  Item — governing edit rights, not visibility), and zero or more Ability
  Effects. A valid in-text link target from lore, alongside lore entries,
  actors, and items.
- **Ability Effect**: A structured, system-agnostic building block attached to
  an Ability, with an effect type (`heal`, `damage`, `modifier`,
  `attack-roll`, extensible), a dice/formula string, and a generically-named
  target resource or attribute. Authored data only — it never resolves or
  applies anything itself.
- **Ability Classification**: One of a fixed, shared set of underlying values
  categorizing an Ability. Portable across systems; never renamed or migrated
  by a system change.
- **Ability Presentation Facet**: A per-game-system display label for one
  Ability Classification, letting each system express the shared
  classifications in its own vocabulary. Optional per system, with built-in
  default labels as fallback. Presentation only — carries no mechanical meaning
  and never affects stored ability data.
- **Known Ability Entry**: A link between one Actor and one Ability in the same
  world. An Actor has zero or more, at most one per distinct Ability.
  Visibility and edit rights follow the *Actor's* permission model, not the
  Ability's.
- **Ability Share Link**: A stable, shareable, non-enumerable reference to one
  specific Ability, created by a member with Owner-level access, mirroring the
  Item Share Link (spec 013). Carries a revoked/active state. A "Copy to World"
  action through it produces a brand-new, independent Ability (with cloned
  effects) — a one-time deep copy, not an ongoing link.
- **Actor** (existing, spec 010): Gains a set of Known Ability Entries; no
  other change to the entity.
- **Lore Entry** (existing, spec 012): Gains Abilities as a valid in-text link
  target; no other change to the entity.
- **World Member / World** (existing, spec 010): Supplies the pool of
  assignable subjects for an Ability's ownership block and the "DM"
  authorization concept (Owner or GM role).
- **System Manifest `abilities` block** (existing, ADR-027): Declares per-system
  ability *scores* — a distinct concept from this spec's Ability, deliberately
  left unrenamed per FR-014.

## Guardrail Checkpoint (prerequisite, addressed in-feature)

Constitution v1.1.0's DMCA / Content Moderation Guardrail applies to User
Story 6, because share links make one world's compendium content accessible
outside that world. It requires, before implementation begins, both:

- **(a)** The notice-and-takedown program (spec 015: intake, disable,
  counter-notice/restoration, repeat-infringer tracking) is operational —
  **satisfied**, spec 015 is complete (41/41).
- **(b)** An explicit, on-record determination of whether Ability share links
  constitute "a centralized public repository" for user-shared,
  potentially-copyrighted content — **satisfied**, ADR-049 Accepted 2026-08-25.

**Both prerequisites are now met; User Story 6 is unblocked.**
Planning found that (b) had never been recorded for *any* share-link feature —
actor shares (spec 010, FR-023) and item shares (spec 013, FR-022..FR-027) both
shipped without one, because spec 015's own Assumptions incorrectly stated that
no sharing feature existed. The determination is therefore drafted as
`docs/adrs/20260825-049-share_link_dmca_repository_determination.md`, covering
all three share-link features, and is **task T001** — the first task in this
feature, ahead of all implementation.

FR-037 (no discoverability, link-possession only) is one of the six invariants
ADR-049's determination is conditional on. Violating any of them re-opens the
determination.

User Stories 1-5 do not touch this checkpoint and may proceed regardless of its
outcome.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: The Compendium contains zero "coming soon" placeholder tabs —
  all four tabs (NPCs, Lore, Items, Abilities) are fully functional.
- **SC-002**: A GM can create a fully-specified ability (name, description,
  classification, and at least one effect) in under 3 minutes without leaving
  the Compendium.
- **SC-003**: A world member can locate a specific ability among at least 100
  abilities in under 15 seconds using the tab's search.
- **SC-004**: 100% of ability mutations reject unauthorized callers
  server-side, verifiable independently of the UI.
- **SC-004a**: A GM-only ability is absent from 100% of non-DM-reachable
  surfaces — list, search, detail, name suggestions, actor known-abilities,
  lore link candidates, and lore link resolution — verified server-side rather
  than by UI gating.
- **SC-005**: An ability referenced from lore is reachable in one interaction
  from the lore entry, and the reverse ("what links here") is visible on the
  ability without any manual bookkeeping by the GM.
- **SC-006**: Abilities authored under one game system remain intact and
  viewable after the world's system is changed — zero data loss, with only
  classification labels changing.
- **SC-007**: A GM can see every ability a given actor knows in one view, and
  attach or detach an ability in under 15 seconds.
- **SC-008**: An ability copied to another world is fully independent —
  verifiable by editing either copy and observing zero effect on the other.

## Non-Goals

Explicitly out of scope for this spec, recorded so a later spec can pick them
up deliberately rather than by accident:

- **Dice rolling / effect resolution**: Ability Effects are authored data.
  Integrating them with the `thunderforge-dice` crate (spec 014) or any
  resolution engine is a separate feature.
- **Server-authoritative adjudication**: No involvement of the
  `thunderforge-crucible` crate / `SessionAdjudicator` (spec 024). The
  project's 24-spec audit confirmed no existing spec scoped server-authoritative
  movement/manipulation resolution; that remains new scope belonging to spec
  024 or a follow-up, not something this spec owes.
- **Canvas integration**: Abilities do not appear on, target, or affect the
  Bevy canvas in this pass.
- **Ability usage tracking**: No spell slots, charges, cooldowns, prepared/known
  distinction, or per-session consumption. An actor either knows an ability or
  does not.
- **Cross-world ability browsing or a public repository**: Explicitly excluded
  by FR-037 — see the Guardrail Checkpoint above.
- **GM-extensible classification taxonomy**: The underlying classification set
  is fixed (FR-009). Systems re-label it via facets (FR-010) but cannot add new
  classifications; a world needing a bespoke one uses the closest fit.

## Assumptions

- The Compendium tab shell (spec 011) is genuinely extensible as designed —
  its `CompendiumTabDef[]` array was built so a new tab is a small addition
  rather than a restructuring, and replacing the Abilities placeholder with
  real content requires no change to the other three tabs.
- Spec 013's Item implementation is the correct template for Ability's
  permissions, effects, table/preview, actor attachment, lore-linking, and
  share-link shape; where this spec is silent on a detail, the Item precedent
  governs rather than a new invention.
- The shared Markdown authoring/rendering path (spec 021's unified CodeMirror
  editor, plus lore's existing renderer) is reused as-is for ability
  descriptions; this spec introduces no new editor.
- The existing Viewer/Editor/Owner ownership-block model needs no change to
  accommodate a fourth content type.
- The system manifest contract can carry optional presentation facets without a
  breaking change to existing packs, since FR-011 makes facets optional and
  every currently-shipped pack simply gets default labels until it opts in.
- No migration of existing data is required — no abilities exist today.
