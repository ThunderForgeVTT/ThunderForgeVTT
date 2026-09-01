# Feature Specification: Pack Architecture — Interface Packs Are Themes, System Packs Drive Function

**Feature Branch**: `032-pack-architecture`

**Created**: 2026-09-01

**Status**: Draft

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

## User Scenarios & Testing *(mandatory)*

### User Story 1 - A player dresses the product to taste (Priority: P1)

A player who finds the default presentation too dark, too ornate, or too hard to
read opens their appearance settings, sees the interface packs available to them,
previews one, and chooses it. Every screen they visit afterwards is drawn in that
pack's look. Nothing they can do in the product behaves any differently than it
did before: the same buttons exist, the same actions are permitted, the same
numbers appear. Their choice follows them from world to world and from session to
session, and it is theirs alone — nobody else at the table sees their pick.

**Why this priority**: This is the half that is unblocked. It delivers visible,
demonstrable value on its own, it retires a field that is currently a lie on two
screens, and — critically — it establishes "look only" as an enforced boundary
*before* any pack has an incentive to smuggle behaviour across it. It also
settles the base pack's identity as the first pack among peers rather than a
privileged house style, which is much cheaper to establish now than to retrofit.

**Independent Test**: Fully testable with no system-pack work whatsoever. Install
two interface packs, switch between them as a player, and confirm every screen
re-skins while every available action, permission, and displayed value stays
identical. Delivers value even if User Story 2 is never built.

**Acceptance Scenarios**:

1. **Given** a signed-in player with the base interface pack active, **When**
   they select a different available interface pack in their settings, **Then**
   every product surface they subsequently open is presented in the newly chosen
   pack, and the choice persists across sign-out and sign-in.
2. **Given** two players in the same world with different interface packs
   selected, **When** both open the same scene and the same character, **Then**
   each sees their own pack's presentation, and both see identical content,
   identical available actions, and identical values.
3. **Given** a player who has never chosen an interface pack, **When** they use
   the product, **Then** the base interface pack is applied, and their settings
   show it as the active choice by name rather than as an empty or placeholder
   value.
4. **Given** an interface pack under review for inclusion, **When** it is
   validated, **Then** any attempt by the pack to contribute behaviour — a new
   action, a changed value, a rule, a data mutation, or executable logic of any
   kind — is rejected before the pack is made available, with the rejected
   contribution named.
5. **Given** a player viewing a list of interface packs, **When** they inspect
   any one of them, **Then** the base pack is presented on the same footing as
   the others, with no capability, placement, or removability that the others
   lack.

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
- An interface pack's presentation makes a control unreadable, invisible, or
  unreachable. Skinning must not be able to remove an affordance; a pack that
  hides an action has changed behaviour by other means.
- A system pack's contributed surface is slow to become ready. The surface must
  have a defined not-yet-ready presentation rather than an empty region that
  reads as "no data."
- Two system packs claim the same system identity. One must win by a stated rule
  and the conflict must be surfaced, not resolved silently.
- A player's chosen interface pack is available to them but not to another
  player in the same world. Nothing about the shared session may depend on it.
- A world is bound to a system pack whose declared compatibility does not cover
  the running product version.
- A pack is removed while a session is live, not merely between sessions.
- Content authored under one system pack is viewed after the world is re-bound to
  a different system pack.
- An interface pack ships a look that fails contrast or legibility expectations —
  is that a rejection at validation time or a warning to the player?

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

- **FR-007**: A base interface pack MUST always be present, MUST be the applied
  default when no other choice is in effect, and MUST have no capability or
  status that other interface packs cannot also have.
- **FR-008**: Users MUST be able to see the interface packs available to them,
  preview one before committing, and select one.
- **FR-009**: An interface-pack selection MUST be stored as a per-user
  preference that applies across every world that user enters, and MUST NOT
  change what any other user sees.
- **FR-010**: A world MAY record a *suggested* interface pack, which MUST be
  presented to users as a suggestion they can accept or decline, and MUST NOT
  override a user's own selection.
- **FR-011**: Switching interface packs MUST NOT change which actions are
  available, which permissions apply, or which values are displayed.
- **FR-012**: An interface pack MUST NOT be able to hide, disable, or make
  unreachable any control that the product presents in the base pack.

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

- **FR-018**: When a user's selected interface pack is unavailable, the product
  MUST fall back to the base pack, MUST inform the user once, and MUST NOT block
  any action.
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
- **Base Interface Pack**: The interface pack that is always present and applied
  by default. A peer of every other interface pack, not a privileged one.
- **Pack Contribution**: A single declared thing a pack offers — one surface,
  one hook, one look. The unit of validation, of mounting, and of failure
  containment.
- **User Appearance Preference**: A user's chosen interface pack, scoped to the
  user and not to any world.
- **World Pack Binding**: A world's association with a system pack, and its
  optional *suggested* interface pack. Distinct in kind: the system binding is
  authoritative for the world; the interface suggestion is advisory to each user.
- **Degraded World State**: The state of a world whose bound system pack is
  absent or unusable — content intact and readable, system-specific presentation
  and behaviour unavailable, and the cause named.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A user can change their interface pack and see every product
  surface re-skinned in under 30 seconds, without leaving the settings surface to
  do it and without restarting a session.
- **SC-002**: Across a full pass of the product under two different interface
  packs, 100% of available actions, permissions, and displayed values are
  identical; the only differences observed are presentational.
- **SC-003**: A candidate interface pack that attempts to contribute behaviour is
  rejected in 100% of validation runs, with the offending contribution named.
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

## Assumptions

- **Naming.** The base interface pack is named **"Mithral"**, not "Forge". The
  two candidate names encode different architectures: "Forge" reads as the
  house style, which invites the base pack to accumulate privileges other packs
  cannot have; "Mithral" reads as the first pack among peers, which is the
  architecture FR-007 requires. This is a decision the requester should confirm —
  the requirement (peer, not privileged) stands regardless of which word wins.
- **Interface pack scope is per user, not per world.** The requester said
  "probably user configurable," and this specification resolves the ambiguity in
  favour of a per-user preference (FR-009) with an optional per-world
  *suggestion* (FR-010). The alternative — a Game Master imposing a look on the
  whole table — was rejected because a skin that carries no behaviour has no
  reason to be table-wide, and because accessibility needs (contrast, size,
  legibility) belong to the person looking at the screen. This is the second
  decision worth confirming.
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
