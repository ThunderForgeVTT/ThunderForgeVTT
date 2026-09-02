# Feature Specification: Pack Architecture — Interface Packs Are Themes, System Packs Drive Function

**Feature Branch**: `032-pack-architecture`

**Created**: 2026-09-01

**Status**: Planned

**Input**: User description: "Interface pack = THEME. Look only. Skins for the engine. User configurable. Call the base one 'Forge' or 'Mithral'. System pack = FUNCTION — each game system decides how the interface *works*: what a character sheet is, how items are presented, whether combat has rounds. A dnd5e sheet and a pathfinder sheet are wildly different and each system should ship its own."

---

## Overview

The product has two words that both end in "pack" and they mean opposite things.
Today the distinction is undeclared, and the code shows it: a world carries an
interface-pack field that nothing consumes and two screens describe as "Unbound
placeholder" and "Not yet assigned" respectively; meanwhile system packs already
ship real functional surfaces, but the application decides which one to mount
from a fixed, build-time list that a pack cannot add itself to.

This specification separates the two concepts and states the rule that makes the
separation worth having:

- An **interface pack** changes only how the product **looks**. It contributes
  no behaviour, runs no logic of its own, and can never change what an action
  does. It is a skin.
- A **system pack** decides how the product **works** for one game system: what
  a character sheet is, how items are presented, whether combat has rounds, what
  a roll means.

The "look only" rule is the load-bearing part, and it is a **safety boundary
before it is an aesthetic preference**. Because an interface pack contributes no
executable behaviour, it never raises the question of running third-party code
inside a player's session — the question recorded as unanswered in the runtime
module loading and security decision (ADR-029, currently an empty stub). A system
pack cannot avoid that question: contributing a character sheet or a rules hook
*is* contributing behaviour.

That asymmetry sets the scope and the order of this feature. **The interface-pack
half is unblocked and ships first.** The system-pack half is blocked on ADR-029
being answered, and this specification treats that dependency as a hard gate
rather than a footnote.

## Clarifications

### Session 2026-09-02

- Q: When an interface pack declares it is built for a particular system, may it also decide how that system's stats are arranged on screen — and where do derived numbers like a 5e ability modifier or spell save DC get computed? (FR-003) → A: The system computes, the interface arranges. System packs declare their derived values alongside their stored ones; interface packs declare layout by referring to declared identifiers. Both remain pure data, and both stay outside ADR-029's gate.

- Q: How should a pack's compatibility with a system be established — does the pack simply state which systems it targets, or is that claim checked against what those systems actually declare? → A: Declared **and** validated. A pack states its target systems; validation reads each named system's manifest and rejects the pack if its layout references an identifier that system does not declare, naming both the identifier and the system.

- Q: Where should a system's derived values actually be computed — in the pack's Rust crate on the server, in the pack's TypeScript module in the browser, or declared as expressions in the manifest? → A: In the pack's Rust crate, **conditional on there being a real plugin API**: one shared Rust contract that every pack implements, so the implementation cannot be fumbled differently in each pack. The contract carries declared `identifier → value` pairs, never a fixed struct.

- Q: If every interface pack is named after a metal, what happens to "Forge" — does the base pack get a metal name too, and which one? → A: The base pack stays **Forge** and is the conformance reference for the pack format; packs bundled with the product are named **Forged &lt;Metal&gt;** — Forged Iron, Forged Steel. The schema, not Forge, remains the authority on what a pack may contain; Forge's obligation is to exercise all of it. Third-party packs are not required to borrow the house name.

- Q: How does Forge manage to work with every system — including one nobody has written a pack for — while still being the pack that exercises every layout construct? → A: Layout addresses declarations **generically as well as specifically**. A construct may target a declaration set by kind and order ("every declared attribute, in declaration order"), or an individual identifier by name. Forge uses only the generic form, so it composes against any system without naming a concept it cannot know; targeted packs use names and are validated under FR-026.

**Evidence behind that answer.** The official D&D 5e character sheet carries 336
form fields: 122 checkboxes (18 skill proficiencies, 6 save proficiencies, 6
death saves, the rest spell-prepared toggles), 100 spell name slots, 18 spell
slot counters across nine levels, 9 attack fields, 5 currency denominations, 6
ability scores — **and six separate ability-modifier fields**, because paper
cannot compute. `packs/systems/dnd5e/system.json` declares roughly thirty
things against those 336, and nothing anywhere in the product derives a value
from another: `AttributeDeclaration` carries id, label, abbreviation, source
and order, and stops there.

So a 5e interface cannot be colours and spacing. Most of a 5e sheet is derived
numbers and structures the generic layer has never heard of — death saves exist
in no other shipping system, and Genie has a Wish Pool and a Doom Clock where
5e has spell slots. The question was never whether interfaces are
system-shaped; it is who computes, and the answer above puts that with the
system, where the ruleset already lives.

**Two contracts and two implementations already exist.** The product declares a
system contract twice — once in the engine's own system module and once,
re-declared, inside the bundled 5e pack with a comment saying it "should match"
the first; it has since drifted, and neither is depended upon by anything. The
engine's version carries a fixed set of derived statistics — armour class,
initiative, proficiency bonus — which is the same mistake the attribute and
resource declarations were each introduced to correct. The 5e presentation
exists twice as well: roughly 1,190 lines in the pack that nothing builds or
loads, beside roughly 1,130 lines in shared application code that do run, with
5e the only system holding a module there. FR-027 through FR-030 are written
against that state.

**A published sheet is read for scope, never for design.** Publishers'
character sheets are copyrighted, and their layout, wording, ornament and trade
dress are exactly the part that is theirs. They are consulted to answer *what
does this system ask a player to track* — a question of fact about the ruleset
— and never to answer *what should ours look like*. Every ThunderForge
interface is ThunderForge's own design: our arrangement, our type, our
ornament, our way of rendering an actor. This is the same line the bundled 5e
pack's own manifest already draws in its trademark restrictions, applied to
presentation rather than to naming.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - A Game Master dresses the table (Priority: P1)

A Game Master who finds the default presentation too dark, too ornate, or too
hard to read opens the world's appearance settings, sees the interface packs
available, previews one, and chooses it for the world. Every screen anyone at
that table visits afterwards is drawn in that pack's look. Nothing anyone can do
in the product behaves any differently than it did before: the same buttons
exist, the same actions are permitted, the same numbers appear. The choice
belongs to the world, so the whole table sees one look, and a different world
can look entirely different.

**Why this priority**: This is the half that is unblocked. It delivers visible,
demonstrable value on its own, it retires a field that is currently a lie on two
screens, and — critically — it establishes "look only" as an enforced boundary
*before* any pack has an incentive to smuggle behaviour across it. It also
settles the base pack's identity as the first pack among peers rather than a
privileged house style, which is much cheaper to establish now than to retrofit.

**Independent Test**: Fully testable with no system-pack work whatsoever. Install
two interface packs, switch the world between them as its Game Master, and
confirm every screen re-skins for every participant while every available action,
permission, and displayed value stays identical. Delivers value even if User
Story 2 is never built.

**Acceptance Scenarios**:

1. **Given** a world with the base interface pack active, **When** its Game
   Master selects a different available interface pack in the world's settings,
   **Then** every product surface anyone subsequently opens in that world is
   presented in the newly chosen pack, and the choice persists across sign-out
   and sign-in.
2. **Given** two players in the same world, **When** both open the same scene and
   the same character, **Then** both see the world's chosen pack, and both see
   identical content, identical available actions, and identical values.
3. **Given** a player in a world whose Game Master has never chosen an interface
   pack, **When** they use the product, **Then** the base interface pack is
   applied, and the world's settings show it as the active choice by name rather
   than as an empty or placeholder value.
4. **Given** an interface pack under review for inclusion, **When** it is
   validated, **Then** any attempt by the pack to contribute behaviour — a new
   action, a changed value, a rule, a data mutation, or executable logic of any
   kind — is rejected before the pack is made available, with the rejected
   contribution named.
5. **Given** an interface pack under review for inclusion, **When** its
   presentation is checked against the legibility floor, **Then** a pack that
   fails it is rejected before it is made available, naming what failed —
   because a table-wide look is not something an individual reader can opt out
   of.
6. **Given** someone viewing a list of interface packs, **When** they inspect
   any one of them, **Then** the base pack is presented on the same footing as
   the others, with no capability, placement, or removability that the others
   lack.
7. **Given** a world bound to a game system, **When** its Game Master chooses a
   pack that targets that system, **Then** the system's own values are
   presented in that pack's arrangement — its declared attributes, resources
   and derived values, laid out as that pack declares — and a pack targeting a
   different system is not offered for this world.
8. **Given** a world bound to a system no pack targets, **When** anyone opens an
   actor, **Then** Forge presents that system's declared values through generic
   arrangement, and nothing is missing, blank, or mislabelled.

---

### User Story 2 - A game system brings its own way of working (Priority: P2)

A game system arrives as a pack. It declares the functional surfaces it provides
— what a character sheet is for its characters, how its items are laid out, how
its rolls and conditions are expressed, whether its combat has rounds at all —
and the product mounts them wherever a player or Game Master encounters that
system's content. A different system pack installed in a different world produces
a different sheet, a different inventory presentation, and different rules
behaviour, with no change made anywhere in the shared application to accommodate
either of them.

**Why this priority**: This is the point of the distinction — a d20-with-stat-
blocks system and a narrative dice-pool system genuinely need different sheets,
and today a pack cannot supply one without an edit to the shared application.
It is P2 rather than P1 because it is **gated on the runtime code-loading and
security decision**: mounting a pack's functional surface means running the
pack's own logic, and there is currently no recorded decision about what that is
permitted to do, what it may reach, or what happens when it misbehaves. That gate
must close before this story can be built, and the gate is not this
specification's to close.

**Independent Test**: Testable by introducing a second system pack that
contributes a character sheet visibly distinct from the existing one, and
confirming both mount correctly for their own worlds — with the shared
application unchanged between the two installations. Delivers value independently
of User Story 1: a system pack's surfaces work identically under any interface
pack.

**Acceptance Scenarios**:

1. **Given** a world bound to a game system whose pack declares a character
   sheet, **When** a player opens an actor in that world, **Then** that system's
   sheet is presented, without the shared application containing any
   system-specific branch or list entry naming that system.
2. **Given** two worlds bound to two different game systems, both of which
   declare character sheets, **When** the same player opens an actor in each,
   **Then** each world presents its own system's sheet, and the two are visibly
   and structurally different.
3. **Given** a world bound to a game system whose pack declares *no* sheet for a
   given surface, **When** a player opens that surface, **Then** a
   system-agnostic default is presented and the player can still read and act on
   the underlying content.
4. **Given** a system pack that declares a functional surface, **When** it is
   validated for installation, **Then** it is accepted only if the security and
   sandboxing terms recorded for pack-supplied executable code are satisfied,
   and rejected with a stated reason otherwise.
5. **Given** a system pack whose contributed surface fails while a player is
   using it, **When** the failure occurs, **Then** the failure is contained to
   that surface, the rest of the session remains usable, and the player is told
   which pack failed rather than shown a blank or broken screen.
6. **Given** an author preparing a system pack, **When** they consult the
   published contract for declaring functional surfaces and hooks, **Then** that
   contract exists as a maintained document and describes every surface a pack
   may contribute, with no dangling references to documents that do not exist.

---

### User Story 3 - A world survives a pack that is not there (Priority: P3)

A world's interface pack is uninstalled, or its system pack is removed,
unavailable, or a version the product cannot use. The Game Master opens the world
anyway. Nothing is silently lost: the world opens, the missing pack is named
plainly, and the product states what is degraded and what is unaffected. Content
belonging to the missing system remains stored and exportable even while its
purpose-built presentation is unavailable.

**Why this priority**: This is a correctness and trust requirement rather than a
new capability, and it can be built after either half above. It also carries the
smallest visible piece of the whole feature: the two screens that currently
disagree about how to describe an unset interface pack must say the same true
thing.

**Independent Test**: Testable by removing each pack type from a populated world
in turn and confirming the world still opens, the degradation is named, and no
content is destroyed or made unexportable.

**Acceptance Scenarios**:

1. **Given** a player whose selected interface pack is no longer available,
   **When** they use the product, **Then** the base interface pack is applied
   automatically, they are told once which pack is missing, and no action is
   blocked.
2. **Given** a world whose bound system pack is missing or unusable, **When** a
   Game Master opens that world, **Then** the world opens in a degraded state
   that names the missing system pack, presents that world's content through
   system-agnostic defaults, and permits reading and exporting it.
3. **Given** a world in that degraded state, **When** the missing system pack is
   restored, **Then** the world returns to full function with no content loss
   and no re-binding step required of the Game Master.
4. **Given** any surface that displays a world's interface pack or system pack,
   **When** that value is unset, **Then** every such surface uses the same
   wording for the unset state, and that wording accurately describes the
   product's actual behaviour rather than a placeholder.
5. **Given** a world in a degraded state, **When** a Game Master attempts an
   action that genuinely requires the missing pack, **Then** the action is
   refused with a message naming the missing pack, rather than failing silently
   or producing incorrect results.

---

### Edge Cases

- A pack claims to be both an interface pack and a system pack. It must be
  rejected: the type is exclusive, because the safety rule attaches to the type.
- A system adds a declaration after a pack targeting it was validated. The pack
  is not thereby wrong — a new identifier it does not lay out is simply not
  laid out — but a system that *removes* one breaks every pack referencing it,
  and that must surface at validation rather than as a blank panel.
- A pack targets two systems whose declarations overlap only partly. Its layout
  must validate against each named system independently, not against their
  union.
- An interface pack's presentation makes a control unreadable, invisible, or
  unreachable. Skinning must not be able to remove an affordance; a pack that
  hides an action has changed behaviour by other means.
- A system pack's contributed surface is slow to become ready. The surface must
  have a defined not-yet-ready presentation rather than an empty region that
  reads as "no data."
- Two system packs claim the same system identity. One must win by a stated rule
  and the conflict must be surfaced, not resolved silently.
- A world's chosen interface pack is available to the server but cannot be
  fetched by one participant's client. That participant must fall back to the
  base pack and still see the same content, actions, and values as everyone
  else — nothing about the shared session may depend on which look loaded.
- A world is bound to a system pack whose declared compatibility does not cover
  the running product version.
- A pack is removed while a session is live, not merely between sessions.
- Content authored under one system pack is viewed after the world is re-bound to
  a different system pack.
- An interface pack passes the legibility floor in one theme and fails it in the
  other, or fails only on one surface. Validation must name the surface and the
  mode, not merely the pack.

## Requirements *(mandatory)*

### Functional Requirements

#### The distinction itself

- **FR-001**: The product MUST define exactly two pack types with disjoint
  responsibilities: an interface pack, which determines presentation only, and a
  system pack, which determines game-system function.
- **FR-002**: Every pack MUST declare its type, and a pack MUST NOT declare or
  behave as more than one type.
- **FR-003**: An interface pack MUST NOT be able to contribute behaviour — no
  action, rule, computed value, data change, or executable logic. This
  prohibition MUST be enforced by an automated validation that runs before a pack
  can be made available, not by reviewer judgement alone.
- **FR-003a**: Declaring *where a value appears* is presentation; declaring
  *what a value is* is behaviour. An interface pack MAY therefore arrange,
  group, order, and emphasise values by referring to identifiers a system pack
  has declared, and MUST NOT compute, transform, or conditionally derive any
  value it displays. A layout referring to an identifier the bound system does
  not declare MUST fail validation rather than render blank.
- **FR-003b**: An interface pack MUST NOT reproduce a publisher's sheet
  layout, ornament, wording, or trade dress. Published sheets inform *what a
  system tracks*; they MUST NOT be the source of *how ThunderForge presents
  it*. Any pack bundled with the product MUST be an original ThunderForge
  design, and a pack's legal metadata MUST NOT claim a licence it does not
  hold for presentation it did not author.
- **FR-004**: A system pack MUST be able to contribute functional surfaces —
  minimally a character sheet, an items/inventory presentation, and rules
  behaviour hooks — without any change to shared application code that names the
  pack or the game system.
- **FR-005**: The set of functional surfaces available for a given world MUST be
  the union of what the installed packs contribute, rather than a central list
  that must be edited whenever a pack is added or removed.
- **FR-006**: The product MUST NOT require a system pack in order to present a
  world's content; a system-agnostic default MUST exist for every surface a
  system pack may contribute.

#### Interface packs

- **FR-007**: A base interface pack, named **Forge**, MUST always be present,
  MUST be the applied default when no other choice is in effect, and MUST have
  no capability or status that other interface packs cannot also have. The name
  is the house's; the standing is not — nothing about being the shipped pack may
  give Forge a capability, a placement, or an exemption another pack cannot have.
- **FR-007a**: Forge MUST be the **conformance reference** for the pack format:
  every token and every layout construct the format offers MUST appear
  somewhere in Forge, and that MUST be enforced by a test rather than by
  inspection. The schema remains the authority on what a pack may contain —
  Forge's role is to exercise it, not to define it. Stated this way on purpose:
  a base pack that *defined* the vocabulary would hold a capability no other
  pack has, which is the accumulated privilege FR-007 exists to prevent, and a
  format construct nothing can actually build would then be discovered by a
  third-party author rather than by Forge failing its own test.
- **FR-007b**: Interface packs bundled with the product MUST be named
  **Forged &lt;Metal&gt;** — Forged Iron, Forged Steel — with Forge itself as the
  base. Third-party packs MUST NOT be required to adopt the house name; the
  convention signals that a pack ships with the product, and requiring it of
  packs the product did not author would make a claim on work that is not the
  product's, which is the same line FR-003b draws from the other direction.
- **FR-008**: A world's Game Master MUST be able to see the interface packs
  available, preview one before committing, and select one for the world.
- **FR-009**: An interface-pack selection MUST be stored against the world and
  MUST apply to every participant in that world, so that the whole table sees
  one look; a different world MUST be able to carry a different selection.
- **FR-010**: Selecting an interface pack MUST be a Game Master authority,
  refused for a participant who does not hold it, and the refusal MUST name the
  authority required rather than failing silently.
- **FR-011**: Switching interface packs MUST NOT change which actions are
  available, which permissions apply, or which values are displayed.
- **FR-012**: An interface pack MUST NOT be able to hide, disable, or make
  unreachable any control that the product presents in the base pack.
- **FR-012a**: An interface pack MUST meet a stated legibility floor —
  minimally text and control contrast — and a pack that fails it MUST be
  rejected at validation, naming what failed. Rejection rather than a warning
  to the reader, because FR-009 makes the look table-wide: a participant who
  cannot read the pack their Game Master chose has no setting of their own to
  escape to, which makes an illegible pack indistinguishable from FR-012's
  unreachable control.

#### System-shaped interfaces

- **FR-024**: A system pack MUST be able to declare its **derived** values —
  a 5e ability modifier, a save total, a passive score, a spell save DC —
  alongside the values it stores, and the product MUST resolve both into the
  same `identifier → value` form before anything presents them. A value that
  only exists on paper because paper cannot compute is a value this product
  computes.
- **FR-025**: An interface pack MUST be able to declare layout — grouping,
  ordering, emphasis, and repeating collections — over identifiers a system has
  declared. The format MUST be able to express the structural shapes the
  shipping systems actually have, which differ in kind and not merely in
  degree: a six-ability block with an eighteen-row skill list; a three-pool
  column with no skills; a skills-only ladder with no abilities at all; a
  nine-level slot grid; a bounded success/failure tracker.
- **FR-025a**: A layout construct MUST be able to address a system's
  declarations **generically** — by kind and declaration order, as in "every
  declared attribute" or "every declared resource" — as well as **specifically**
  by identifier. Generic addressing is what lets one layout compose against a
  system it has never heard of, including one that ships later; specific
  addressing is what lets a targeted pack lay out a nine-level slot grid or a
  death-save tracker.
- **FR-025b**: Forge MUST use generic addressing only, and MUST NOT reference
  any system's identifiers. This is what makes FR-006's system-agnostic default
  a mechanism rather than a promise: the default *is* Forge, and it works for
  every system precisely because it names nothing. It is also what reconciles
  FR-007a with FR-026 — Forge exercises every construct while remaining
  compatible with every system, because the constructs it exercises are the
  generic ones.
- **FR-026**: An interface pack MUST declare the systems it targets, and that
  claim MUST be validated before the pack is made available: a pack whose
  layout references an identifier a named system does not declare MUST be
  rejected, naming both the identifier and the system. A declaration that is
  merely asserted is the failure mode spec 016 already corrected for legal
  metadata, arriving in a second place.
- **FR-027**: There MUST be exactly one contract that every system pack
  implements to supply its values, and it MUST be stated once. Two
  declarations of the same contract, kept in step by convention, MUST NOT
  exist — every pack must be held to the same obligations by construction
  rather than by each author reproducing them.
- **FR-028**: That contract MUST carry values as declared identifier-and-value
  pairs and MUST NOT name any particular system's concepts in its own
  vocabulary. A contract with fixed places for armour class, initiative and
  proficiency bonus is one ruleset's character sheet built into the product:
  it has nowhere to put a system whose resources are stress and trauma, and
  nothing at all to say to one that declares no abilities.
- **FR-029**: A system pack's implementation of that contract MUST be
  discovered rather than listed. Adding a pack MUST NOT require editing a
  central registry, and that property MUST be enforced automatically rather
  than left to convention.
- **FR-030**: A system's presentation MUST live in that system's pack and MUST
  NOT live in shared application code. Where a system's presentation exists in
  both places, exactly one MUST survive, and it MUST be the pack's.

#### System packs

- **FR-013**: A system pack MUST declare the functional surfaces it provides,
  and the product MUST mount a declared surface wherever that system's content is
  encountered.
- **FR-014**: A system pack MUST be able to contribute both the read and the
  edit presentation of a surface, so that a system's authoring experience is as
  system-specific as its viewing experience.
- **FR-015**: The product MUST publish and maintain a written contract
  describing every surface and hook a system pack may contribute, and MUST NOT
  reference contract documents that do not exist.
- **FR-016**: A failure inside a pack-contributed surface MUST be contained to
  that surface, MUST leave the rest of the session usable, and MUST identify the
  responsible pack to the user.
- **FR-017**: A system pack that supplies executable behaviour MUST be accepted
  only under the recorded terms governing pack-supplied code — what it may
  access, what it is denied, and how it is contained. Until those terms exist as
  a decision of record, the product MUST NOT accept system packs from any source
  other than those shipped with the product itself.

#### Absence, degradation, and honesty

- **FR-018**: When a world's selected interface pack is unavailable, the product
  MUST fall back to the base pack, MUST inform each participant once, and MUST
  NOT block any action.
- **FR-019**: When a world's system pack is missing or unusable, the world MUST
  still open, MUST name the missing pack, MUST present its content through
  system-agnostic defaults, and MUST keep that content readable and exportable.
- **FR-020**: Restoring a previously missing system pack MUST restore full
  function with no content loss and no re-binding action required.
- **FR-021**: An action that genuinely requires a missing pack MUST be refused
  with a message naming the pack, never failing silently or producing a result
  computed without it.
- **FR-022**: Every surface that displays a world's pack bindings MUST use
  identical wording for the unset state, and that wording MUST describe the
  product's actual behaviour.
- **FR-023**: The product MUST NOT display a pack binding as a placeholder value
  when a real default is in effect.

### Key Entities *(include if feature involves data)*

- **Pack**: A named, versioned, installable unit that extends the product. Has a
  stable identity, a human-readable title, a version, a compatibility range,
  licensing/attribution information, and exactly one type.
- **Interface Pack**: A pack whose entire contribution is presentation. Declares
  a look; declares no behaviour. Selectable by a user.
- **System Pack**: A pack that defines how the product functions for one game
  system. Declares functional surfaces (character sheet, item presentation,
  rules hooks) and the data shapes its content uses. Bound to a world.
- **Base Interface Pack**: **Forge** — the interface pack that is always present
  and applied by default. A peer of every other interface pack, not a privileged
  one.
- **Pack Contribution**: A single declared thing a pack offers — one surface,
  one hook, one look. The unit of validation, of mounting, and of failure
  containment.
- **Declared Value**: One `identifier → value` pair a system publishes about an
  actor, whether stored (a 5e Strength score) or derived (its modifier). The
  unit everything downstream carries and nothing downstream interprets.
- **System Contract**: The single contract every system pack implements to
  supply its declared values, stated once and shared by everything that reads
  them (FR-027, FR-028).
- **Layout Declaration**: A pack's statement of how declared values are
  arranged — generic when it addresses a declaration set by kind and order,
  specific when it names an identifier (FR-025a).
- **World Appearance Binding**: A world's chosen interface pack, scoped to the
  world and applied to everyone in it. Set by the world's Game Master.
- **World Pack Binding**: A world's association with a system pack and with an
  interface pack. Both are authoritative for the world; they differ only in what
  they govern — one how it works, the other how it looks.
- **Degraded World State**: The state of a world whose bound system pack is
  absent or unusable — content intact and readable, system-specific presentation
  and behaviour unavailable, and the cause named.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A Game Master can change the world's interface pack and see every
  product surface re-skinned in under 30 seconds, without leaving the settings
  surface to do it and without restarting a session; every other participant in
  that world sees the change without reloading.
- **SC-002**: Across a full pass of the product under two different interface
  packs, 100% of available actions, permissions, and displayed values are
  identical; the only differences observed are presentational.
- **SC-003**: A candidate interface pack that attempts to contribute behaviour is
  rejected in 100% of validation runs, with the offending contribution named.
- **SC-003a**: A candidate interface pack that falls below the legibility floor
  is rejected in 100% of validation runs, naming the surface and the mode that
  failed.
- **SC-003b**: A candidate interface pack that names a target system it cannot
  render — its layout references an identifier that system does not declare —
  is rejected in 100% of validation runs, naming the identifier and the system.
- **SC-004**: A new game system can contribute a character sheet, an item
  presentation, and rules behaviour with **zero** lines changed in shared
  application code — measured as: the change set that adds the system touches
  only that system's own pack directory.
- **SC-005**: Adding a second system pack alongside the first does not alter,
  break, or visually change any surface belonging to the first — verified by an
  end-to-end pass over both worlds.
- **SC-006**: 100% of worlds with a missing or unusable system pack still open,
  and 100% of their content remains readable and exportable while degraded.
- **SC-007**: Restoring a missing pack returns the world to full function with
  zero content loss and zero manual re-binding steps.
- **SC-008**: Every surface displaying an unset pack binding uses the same
  wording — measured as zero distinct strings for the unset state across the
  product, down from two today.
- **SC-009**: A failure injected into a pack-contributed surface leaves 100% of
  the surrounding session usable, and the user-facing message names the
  responsible pack in 100% of cases.
- **SC-010**: A pack author can produce a working system pack from the published
  contract alone, without reading shared application source, and the contract has
  zero references to documents that do not exist.
- **SC-011**: The interface-pack half (User Story 1) reaches a shippable,
  demonstrable state without any dependency on the pack-code security decision
  being resolved.

## Decisions

Three questions this specification originally left to the requester have been
answered. They are recorded here as decisions, not assumptions, so that a later
reading does not reopen them.

- **The base interface pack is named Forge** (2026-09-02, requester). The draft
  argued for "Mithral" on the grounds that a house name invites the base pack to
  accumulate privileges. The name is Forge; the concern is answered by FR-007
  stating the peer requirement outright rather than by choosing a word that
  hints at it. If Forge ever acquires a capability another pack cannot have,
  FR-007 is what has been violated — the name was never the guarantee.
  *Extended later the same day:* bundled packs are named **Forged &lt;Metal&gt;**
  with Forge as the base, and Forge is the format's conformance reference
  rather than its authority (FR-007a, FR-007b).
- **An interface pack is chosen per world by its Game Master, not per user**
  (2026-09-02, requester). The draft argued the opposite. The table sees one
  look, chosen by the person who runs the table; there is no per-user override
  and no per-world "suggestion". The accessibility reasoning that motivated the
  per-user reading does not disappear, it *moves*: because a participant cannot
  opt out of the world's look, the legibility floor becomes something validation
  has to enforce rather than something a reader can route around. That is
  FR-012a, and it exists because of this decision.
- **A pack that fails the legibility floor is rejected at validation, not
  shipped with a warning** (2026-09-02, requester). See FR-012a and SC-003a.

## Assumptions

- **The two halves ship in sequence, not together.** The interface-pack half is
  unblocked and is the deliverable of this specification's first increment. The
  system-pack half is gated on the runtime pack-code security decision (ADR-029,
  currently an empty stub); FR-017 states the interim restriction that holds
  until it is answered.
- **This is a pack-*architecture* decision, not a content-sharing feature.** It
  governs how the product is extended, not how users share content between
  worlds. It therefore does not trigger the constitution's DMCA/content-
  moderation guardrail, which attaches to features exposing one world's
  compendium content beyond that world. If a pack marketplace is later proposed,
  that guardrail applies to the marketplace, not to this specification.
- **The functional surfaces named here are the starting set, not the final one.**
  Character sheet, item presentation, and rules hooks are the surfaces with
  evidence of need today. The contribution mechanism is expected to admit further
  surfaces (combat structure, chat presentation, compendium browsing) without
  redesign.
- **Existing decisions of record are inputs, not open questions.** The system
  pack manifest contract and the system-agnostic actor data model are already
  decided and are assumed; this feature builds the presentation and mounting half
  on top of them rather than revisiting either.
- **The subsystem-contributes-its-own-declarations pattern already established
  for interaction effects is the precedent** this feature follows for pack
  contributions, including the expectation that the "no central list" property is
  enforced automatically rather than by convention.
- **Existing bundled system packs are the migration population.** They are
  expected to move to the contribution mechanism without their behaviour changing
  from a user's point of view.
