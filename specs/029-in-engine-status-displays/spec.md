# Feature Specification: In-Engine Status Displays and the Engine UI SDK

**Feature Branch**: `029-in-engine-status-displays`

**Created**: 2026-08-29

**Status**: Draft

**Input**: User description: "Offer a model for health bars and other types of bars or on-screen counters — some might be a health bar above a token, some might be health, stamina and mana, some might be health and energy. From a selected token we should be able to display that token's bars and counters in a corner, configurable. Eventually we want theming and more advanced UI/UX features around in-engine content and controls. Cover the player's and the GM's perspective, and focus the work on the engine itself so it provides a robust TypeScript interface we can then work into the instantiation."

## Overview

A person at a table needs to read the board, not interrogate it. Right now
every token is a coloured square or a piece of art, and everything else about
it — how hurt it is, how much of its magic is spent, whether it is about to
drop — is invisible until somebody clicks. In a fight with a dozen
combatants, that is a dozen clicks to answer a question the board should
simply be showing.

This feature gives tokens **status displays**: bars and counters drawn by the
engine, above or around a token, showing the resources that matter for the
game being played. It also gives the currently selected token a larger
**status panel** in a screen corner the viewer chooses, so a player can keep
their own character's vitals in view while looking at the map.

Which resources exist is **not the engine's decision**. One system tracks hit
points and nothing else. Another tracks health, stamina and mana. A third
tracks health and energy. The engine renders whatever the active game system
declares, and knows nothing about what any of it means.

The second half of this feature is the surface it is driven through. Today
the engine's TypeScript boundary is a handful of loosely-typed entry points —
`apply_world_command(json)` takes a JSON string, and the shapes it accepts
live in Rust with hand-written mirrors on the TypeScript side. Anything built
on top of that inherits the drift. This feature establishes a **typed,
versioned SDK** for in-engine presentation: the application declares what it
wants shown and the engine draws it, with the contract expressed once and
checked by the compiler on both sides.

### Why this is also the thing that makes existing work real

The engine already contains a derived-statistics subsystem: components for
token data and abilities, a `DerivedStats` type, per-frame systems that
recompute it, and a `GameSystem` trait implemented by a built-in ruleset. It
is registered in the frame loop and **it has never executed on a single real
token**, because the `Token` component it queries is never attached to any
spawned entity. It computes nothing, every frame, for nobody.

That is not a coincidence to be fixed separately. That subsystem has no
consumer, and this feature is the consumer. Wiring the data in without
something that displays it would move the dead end rather than remove it, so
this specification treats "the numbers reach the screen" as the whole job.

## Clarifications

### Session 2026-08-29

- **Who decides which resources a token shows?** The active game system
  declares the set; a world or a Game Master may narrow what is displayed but
  cannot invent a resource the system does not define.
- **Are bars authorization-bearing?** Yes. A bar is a disclosure, and this is
  the sharpest risk in the feature — see User Story 3.
- **Does the engine own theming?** Not in this feature. The SDK is shaped so
  that appearance is data supplied by the application rather than constants
  compiled into the engine, which is what a later theming feature needs — but
  no theming UI, no user-authored themes, and no per-world palette ships here.

### Resolved 2026-08-29

Both questions previously deferred here have been answered by the product
owner, and both answers made the model simpler rather than more complicated.

**A resource is a list of entries, not a current-and-maximum pair.** The
question "what does a value above its maximum mean" turned out to be the
wrong question. Overflow is not a value exceeding a bound; it is a _second
entry_. A boss with three stages is three entries. Temporary hit points or a
shield are an entry stacked above the base one. Damage depletes the topmost
entry first.

This is worth stating plainly because of what it removes: with entries, a
value can never exceed its maximum, so the ambiguity has no way to be
expressed rather than being detected and handled. A resource that declares
`allowStacking: false` has exactly one entry, and a value above that entry's
maximum is then an unambiguous error rather than a judgement call about
clamping.

**The Game Master controls disclosure per token, across four states.** Not
a visible/hidden pair: hiding a bar entirely tells a player something too
("the GM is concealing this one"), and showing an exact figure often tells
them too much. The four states are set out in FR-013a and give the GM a range
between those extremes — and in every one of them the GM continues to see the
true value.

## User Scenarios & Testing _(mandatory)_

### User Story 1 - Reading my own character at a glance (Priority: P1)

As a player, I can see my character's vital resources without clicking
anything: a compact bar above my token on the map, and a fuller panel in a
corner of the screen that stays visible while I look elsewhere.

**Why this priority**: This is the feature. Everything else refines it.

**Independent Test**: Open a world as a player whose character has hit
points. The token carries a bar reflecting current and maximum. Reduce the
character's hit points from another session; the bar shortens without a
reload.

**Acceptance Scenarios**:

1. **Given** a token bound to an actor with a health resource, **When** the
   scene loads, **Then** a bar appears above the token showing current
   against maximum.
2. **Given** that token is selected, **When** I look at my chosen screen
   corner, **Then** the panel shows every resource the system declares for
   it, each labelled.
3. **Given** the value changes server-side, **When** the change arrives,
   **Then** both the bar and the panel update without a reload.
4. **Given** a resource has no maximum (a counter, not a bar), **When** it is
   displayed, **Then** it renders as a labelled count rather than a partially
   filled bar.

---

### User Story 2 - Running a table without opening every sheet (Priority: P1)

As a Game Master, I can see the state of every token I am entitled to see —
across the whole board at once — so I can run an encounter without opening a
character sheet per creature.

**Why this priority**: The GM is the person with a dozen tokens and no time.

**Independent Test**: Place several NPC tokens with differing health. As GM,
every one shows its bar simultaneously; as a player in the same scene, the
same tokens disclose only what the visibility rules permit.

**Acceptance Scenarios**:

1. **Given** several tokens with resources, **When** I view the scene as GM,
   **Then** each shows its own bar concurrently, with no click required.
2. **Given** a token I am not entitled to inspect, **When** I view it,
   **Then** it shows nothing rather than an empty or zeroed bar — absence and
   "at zero" must not look alike.

---

### User Story 3 - Not learning what I was not told (Priority: P1)

As a player, I cannot discover information the Game Master is withholding by
reading a bar — and as a Game Master, I can rely on that.

**Why this priority**: A bar is a disclosure channel. This project has
already shipped one bug of exactly this class, where a hidden scene's art was
reachable by asking for it directly because two call sites answered the same
question differently. A status bar is the same hazard with a smaller surface
and a faster feedback loop: an attacker does not need to read the payload,
they can watch the pixels.

**Independent Test**: As a player, view an NPC whose exact values are not
disclosed to players. Confirm that neither the rendered bar, the panel, nor
any data reaching the client carries the exact figure.

**Acceptance Scenarios**:

1. **Given** a resource whose exact value is not disclosed to my role,
   **When** the token is drawn, **Then** the client never receives the exact
   value — the coarsening happens on the server, not in the renderer.
2. **Given** a coarse band is disclosed instead ("bloodied", "unharmed"),
   **When** it is shown, **Then** it is visually distinct from a precise bar,
   so nobody mistakes an estimate for a reading.
3. **Given** I select a token I may not inspect, **When** the panel would
   open, **Then** it shows what I am entitled to and says the rest is not
   available, rather than showing blanks.

---

### User Story 3a - Choosing what the table gets to know (Priority: P2)

As a Game Master, I set how much each token discloses — exact, present-but-
unknown, a percentage, or a quarter band — and I change it mid-encounter
without stopping play.

**Why this priority**: The disclosure rules are P1 because they must not leak;
the _control_ over them is P2 because a sensible default (players see their
own exactly, NPCs chunked) covers most tables without anyone touching it.

**Independent Test**: Set a boss token to chunked. As a player, confirm only a
quarter band is visible and no exact figure reaches the client. Change it to
visible mid-session; the player's view updates without a reload.

**Acceptance Scenarios**:

1. **Given** a token set to greyed, **When** a player views it, **Then** they
   see that the resource exists and no indication of its value.
2. **Given** a token set to chunked, **When** its value crosses a quarter
   boundary, **Then** the player's display changes; within a quarter, it does
   not.
3. **Given** I change a token's state mid-encounter, **When** the change
   lands, **Then** every player's view updates live and mine continues to show
   the true value throughout.
4. **Given** two tokens of the same creature, **When** I set different states
   on each, **Then** they disclose differently.

---

### User Story 4 - A system that tracks more than hit points (Priority: P2)

As a game-system author, I declare the resources my system tracks and how
each should be presented, and tokens in worlds using my system show them
without any change to the engine.

**Why this priority**: The engine hard-coding "health" would make every
system after the first a special case.

**Independent Test**: Install a system declaring health, stamina and mana.
Tokens show three bars. Install one declaring health and energy. Tokens show
two. No engine change between them.

**Acceptance Scenarios**:

1. **Given** a system declaring three resources, **When** a token using it is
   drawn, **Then** all three appear in the system's stated order.
2. **Given** a system declaring a resource as a counter rather than a bar,
   **When** it is drawn, **Then** it renders as a counter.
3. **Given** a system that declares none, **When** its tokens are drawn,
   **Then** no status furniture appears at all — not an empty container.

---

### User Story 5 - Putting the panel where it does not cover the map (Priority: P2)

As a viewer, I choose which corner the selected-token panel occupies, and my
choice persists.

**Independent Test**: Move the panel to a different corner, reload, and find
it where it was left.

**Acceptance Scenarios**:

1. **Given** the panel is in one corner, **When** I move it to another,
   **Then** it moves immediately and stays after a reload.
2. **Given** no token is selected, **When** I look at the panel, **Then** it
   is absent or empty rather than showing the last token's values.

---

### User Story 6 - Building on the engine without guessing (Priority: P2)

As an application developer, I drive every one of these displays through a
typed TypeScript SDK, and a mistake in what I send is a compile error rather
than a token that silently fails to draw.

**Why this priority**: The engine's current boundary is
`apply_world_command(jsonString)`. Every shape crossing it is hand-mirrored
in TypeScript, so the two drift, and a drifted field fails silently — the
engine ignores what it cannot parse. This feature adds a substantial amount
of new surface, and adding it to an untyped boundary would multiply that
problem rather than contain it.

**Independent Test**: Send a status-display declaration with a misspelled
field. The build fails. Send a well-formed one; it draws.

**Acceptance Scenarios**:

1. **Given** the SDK, **When** I declare a display with a wrong field name or
   type, **Then** the TypeScript compiler rejects it.
2. **Given** a command shape changes in the engine, **When** the SDK is
   regenerated, **Then** call sites that no longer match fail to compile.
3. **Given** the engine receives a declaration it cannot honour, **When** it
   processes it, **Then** it reports the rejection through the existing event
   callback rather than ignoring it silently.

---

### User Story 7 - Appearance that can later be themed (Priority: P3)

As a product owner, I can change how status displays look without changing
the engine, so that a later theming feature has something to configure.

**Acceptance Scenarios**:

1. **Given** appearance values are supplied by the application, **When** they
   change, **Then** the rendering changes with no engine rebuild.
2. **Given** no appearance is supplied, **When** displays are drawn, **Then**
   a documented default is used and stated in one place.

---

### Edge Cases

- **A value above its maximum.** Made unexpressible by FR-002a rather than
  handled: overflow is a further entry, so there is no state to clamp. A value
  that still exceeds its entry's maximum is a defect to report, not a
  presentation problem to solve.
- **An entry list on a resource that forbids stacking.** Rejected outright
  (FR-002b), because silently merging two pools loses which was temporary.
- **A value below zero.** Dying is a state many systems track; a bar must not
  render as negative width.
- **A resource the system stopped declaring** while tokens still carry a
  value for it. The display must follow the declaration, not the stored data.
- **A token bound to no actor.** It has no resources; it shows none.
- **Many tokens at once.** The engine has been measured holding thousands of
  sprites at 60fps; status furniture multiplies the per-token draw cost and
  must not become the thing that ends that.
- **Very long resource labels**, and labels in scripts with different
  metrics. Truncation must not be the only defence.
- **Colour as the only signal.** A red bar and a green bar are the same bar
  to a viewer with a red-green deficiency; this project already tests token
  colours for separation in lightness as well as hue, and status displays
  must meet the same standard.
- **A player owning several tokens.** The corner panel follows selection, not
  ownership, and must not flicker between them.
- **Rapid changes** — a burst of damage — must not queue up a backlog of
  animations that outlives the fight.

## Requirements _(mandatory)_

### Functional Requirements

#### The resource model

- **FR-001**: The active game system MUST declare the set of resources it
  tracks; the engine MUST NOT contain a built-in notion of "health" or any
  other named resource.
- **FR-002**: Each declared resource MUST carry an identifier, a
  human-readable label, and a presentation kind of either _bar_ (has a
  maximum) or _counter_ (does not).
- **FR-002a**: A resource MUST be modelled as an **ordered list of entries**,
  each with its own current value and maximum, rather than as a single
  current-and-maximum pair. One entry is the ordinary case; several express a
  multi-stage boss bar, a shield over health, or any other layered pool.
- **FR-002b**: A declaration MUST state whether the resource allows more than
  one entry (`allowStacking`). A resource that does not MUST be rejected if a
  second entry is supplied, rather than silently merging or dropping it.
- **FR-002c**: Depletion MUST consume the topmost entry first, and an entry
  reduced to zero MUST remain in the list rather than being removed — a boss
  on its last stage should still read as a boss on its _last_ stage, which
  requires the spent ones to still be visible.
- **FR-002d**: Because overflow is a further entry, a current value MUST
  never exceed its own entry's maximum. Any such value is an error to report,
  not a state to render.
- **FR-003**: A declaration MUST specify display order; the engine MUST NOT
  impose one.
- **FR-004**: A token's displayed resources MUST be derived from the system's
  declaration intersected with what the viewer is entitled to see.
- **FR-005**: A resource present in stored data but absent from the current
  declaration MUST NOT be displayed.

#### Per-token displays

- **FR-006**: A token whose actor has displayable resources MUST render them
  attached to the token, positioned so they remain legible as the token moves
  and the camera zooms.
- **FR-007**: A token with no displayable resources MUST render no status
  furniture whatsoever.
- **FR-008**: "Not disclosed" MUST be visually distinguishable from "at
  zero".
- **FR-009**: Status displays MUST update live from the same event path that
  carries other world changes, without a reload.

#### The selected-token panel

- **FR-010**: Selecting a token MUST present its displayable resources in a
  screen-corner panel.
- **FR-011**: The corner MUST be viewer-configurable and MUST persist across
  reloads.
- **FR-012**: With no selection, the panel MUST NOT display stale values from
  a previous selection.

#### Visibility and disclosure

- **FR-013**: Coarsening or withholding a value MUST happen on the server.
  The client MUST NOT receive an exact value it is not entitled to display.
- **FR-013a**: A Game Master MUST be able to set, per token and per resource,
  one of four disclosure states. The Game Master always sees the true value
  regardless of which is set; these govern what everybody else sees:
  - **Visible** — the exact current and maximum, as the GM sees it.
  - **Greyed** — the bar's _presence_ is disclosed and its value is not. The
    viewer learns that the token has this resource and nothing more. This is
    the honest form of "hidden": removing the bar entirely also discloses
    something, because a token conspicuously lacking a bar every other token
    has is itself a signal.
  - **Percentage** — a proportion, without the underlying figures. A viewer
    learns the creature is at 40% and not that it has 400 hit points.
  - **Chunked** — the proportion rounded to quarters. The coarsest disclosure
    that still communicates "hurt" versus "nearly dead".
- **FR-013b**: The server MUST send only what the state permits. Percentage
  MUST arrive as a proportion with no maximum attached; chunked MUST arrive as
  a quarter index, not as a percentage the client rounds. A client that never
  receives the figure cannot leak it.
- **FR-013c**: Percentage discloses more than it appears to and MUST be
  documented as such where a GM chooses it. A viewer who knows the damage they
  dealt can divide it by the percentage change and recover the maximum, and
  from then on reads exact values. Chunked resists this because a quarter
  index rarely moves on a single hit. This is not a reason to withhold the
  option — a GM may well want a readable boss fight — but it is a reason the
  interface must not present the four states as merely four appearances.
- **FR-013d**: Disclosure state MUST be per token, not per actor: two tokens
  of the same creature in one scene may legitimately differ, and the GM sets
  it on the one in front of the players.
- **FR-014**: A coarse disclosure MUST be visually distinct from an exact
  one.
- **FR-015**: Disclosure decisions MUST use the existing world-role and
  per-object permission model rather than introducing a parallel one.
- **FR-016**: A viewer MUST NOT be able to infer a withheld value from
  animation, ordering, sizing, or the presence of the display itself.

#### The TypeScript SDK

- **FR-017**: The engine MUST expose a typed TypeScript interface for
  declaring and updating status displays; callers MUST NOT need to hand-build
  JSON.
- **FR-018**: The types crossing the boundary MUST have a single source of
  truth, with the TypeScript side derived from it rather than maintained in
  parallel.
- **FR-019**: The SDK MUST be versioned, and the engine MUST reject a
  declaration from an incompatible version with a stated error rather than
  partial application.
- **FR-020**: A rejected or unparseable command MUST be reported through the
  engine's existing event callback; silent discard is not acceptable.
- **FR-021**: The SDK MUST expose the current display state for testing, so a
  test can assert what would be drawn without rendering pixels.

#### Presentation values

- **FR-022**: Colours, sizes and spacing MUST be supplied by the application
  rather than compiled into the engine.
- **FR-023**: A documented default set MUST exist in exactly one place.
- **FR-024**: Any default palette MUST meet the same separation standard
  already applied to token kinds: distinguishable in perceived lightness, not
  hue alone.

#### Performance

- **FR-025**: Status displays MUST NOT reduce the engine's measured
  interactive token capacity below its documented figure; the cost MUST be
  measured against the existing capacity sweep rather than assumed.
- **FR-026**: Off-screen tokens MUST NOT pay full display cost.

#### Explicitly out of scope

- Theming UI, user-authored themes, per-world palettes.
- Arbitrary application-authored widgets or scripting inside the engine.
- Editing resource values from the panel (this feature displays; it does not
  mutate).
- Combat tracking, turn order, or any ruleset behaviour beyond presentation.
- Movement gating by a computed speed — related, and belongs to the
  game-system enforcement work rather than here.

### Key Entities

- **ResourceDefinition**: what a system declares — identifier, label,
  presentation kind, order. Owned by the game system package.
- **ResourceEntry**: one layer of a resource — a current value and its own
  maximum. Ordered; depletion consumes the topmost first.
- **ResourceValue**: a token's ordered list of entries for one resource, plus
  the disclosure state it was rendered under.
- **DisclosureState**: visible, greyed, percentage, or chunked. Set by the
  Game Master per token and per resource; applied server-side, so the client
  receives only what the state permits.
- **TokenStatusDisplay**: the resolved set of values the engine draws for one
  token.
- **PanelPlacement**: the viewer's chosen corner, persisted per viewer.
- **DisplayAppearance**: application-supplied presentation values.

## Follow-up: clocks

Raised 2026-08-30 while declaring Blades in the Dark, and deliberately **not**
built here.

Blades runs on progress clocks — a heist's alarm filling segment by segment —
and Genie has puzzle and doom clocks of its own. They look like a bar and are
not one, in a way that matters:

- A clock **fills toward an event**. A resource **depletes toward zero**. The
  same rendering means opposite things, so a red nearly-full bar reads as
  "nearly dead" for health and "about to go badly" for a doom clock — which
  happens to work, and works by accident rather than by design.
- A clock is **not owned by a token**. It belongs to the situation: the
  heist, the scene, the session. Everything in this feature hangs off an
  actor behind a token, and a clock has neither.
- A clock's segments are **discrete and named**, not a proportion. Four of
  six is not 67%; it is four segments, and the count is the point.

So the honest question is whether a clock is a resource with inverted
semantics or a different thing that happens to look like a bar. Declaring one
as a resource to make it fit would be the mistake this feature has twice
avoided — inventing a fixture that suits the model instead of describing a
ruleset as it is.

Worth its own spec. It would need a home that is not a token, a fill
direction, and segment rendering rather than a fraction.

## Success Criteria _(mandatory)_

### Measurable Outcomes

- **SC-001**: A player can determine their character's health without any
  click or navigation.
- **SC-002**: A Game Master can read every entitled token's state in one
  glance, with no per-token interaction.
- **SC-003**: A system declaring three resources and one declaring two both
  render correctly with no engine change between them.
- **SC-004**: No exact value a viewer is not entitled to see is present in
  any payload reaching that viewer's client — verified by inspecting the
  wire, not the screen.
- **SC-005**: A malformed display declaration fails at compile time in the
  application, and an incompatible one is rejected at runtime with a reported
  error.
- **SC-005a**: For every disclosure state other than visible, the exact figure
  is absent from the payload reaching a non-GM client — verified on the wire.
  A token set to chunked yields a quarter index and no percentage; one set to
  percentage yields no maximum.
- **SC-006**: The engine's interactive token capacity with status displays
  enabled is measured and stated; any reduction from the documented baseline
  is a recorded number rather than an unknown.
- **SC-007**: The default palette passes the same lightness-separation test
  applied to token kinds.

## Assumptions

- Game system packages are the right home for resource declarations; the
  packaging and manifest-serving pipeline already exists and this extends its
  manifest rather than adding a second channel.
- The existing world-event path is sufficient to carry status changes; no new
  transport is required.
- Actor resource data already has a storage location
  (`world_actor_system_data`), and this feature reads it rather than
  introducing a parallel store.
- The engine's existing derived-statistics components are the intended
  computation site; making them execute on real tokens is in scope here
  because this feature is their first consumer.
- Persisting the viewer's panel corner is a per-viewer convenience and does
  not need to survive on another device.

## Follow-up: per-viewer visibility and conditions

Raised 2026-08-30 from play-testing the engine, and deliberately **not** built
here. Recorded now because it is the same shape as this spec's disclosure work
and will be tempting to bolt onto it.

Two mechanics, one underlying problem: a token holding a torch, and a token
that some viewers can see and others cannot — a stealth roll against a DC that
one player's passive Perception beats and another's does not.

### Why it belongs with disclosure and not beside it

Disclosure coarsens a token's **values** per viewer. Visibility coarsens its
**existence** per viewer. The rule this spec settled carries over unchanged and
is the non-negotiable part: the server resolves per viewer and never sends what
that viewer may not know. `status-disclosure.spec.ts` asserts on the payload
rather than the screen for exactly this reason — a client that received the
value and chose not to draw it fails the promise, and a modified client sees
everything. Shipping a token with `visible: false` would be that bug, at the
scale of the whole board.

The model wants to be `Disclosed` one level up: `Seen { token }`,
`Inferred { last_known_position }` for "you saw it a beat ago" or "you heard
something", and `Unseen` where nothing is sent at all. The GM always resolves
to `Seen`, with a marker that the table's view differs — the role `configured`
already plays for disclosure.

### What is genuinely new, and harder

- **The live event stream.** Disclosure coarsens a query response. Visibility
  has to coarsen the subscription feed: a token moving where three of five
  players can see it fans out three ways plus the GM's. Today that is one
  broadcast. This lands on the delivery loop, and it is a per-viewer fan-out
  cost rather than a filter that can be added at the edge.
- **Absence is information.** A player whose updates for a token simply stop
  learns something from the silence. "Went invisible" and "left the scene" must
  not be distinguishable, and neither must the timing of when updates ceased.
  This is the part to get wrong on paper rather than at a table.
- **Conditions must be declared by the system, not known by the engine.** The
  FR-001 lesson, which this spec paid for twice. 5e's binary invisible is not
  Blades' hidden is not Pathfinder 2e's observed/concealed/hidden/undetected.
  PF2e is the right target: it is already a graded ladder and maps onto a
  tagged union directly, and 5e is then the degenerate case.
- **A light is not hidden by its bearer.** In 5e, turning invisible does not
  hide your torch. So an attached light's visibility must resolve independently
  of the token carrying it — the light staying put while its carrier vanishes
  is correct behaviour, not a bug. This constraint only appears because both
  features exist, which is the argument for speccing them together.

### Dependency

Contested perception needs passive scores, and `Token.abilities` is spawned
empty because the server does not send them (see `docs/status-displays.md`).
That is the same actor-to-token plumbing MVP Phase 8 requires, so the two want
sequencing together rather than in either order alone.

### Already built

Token-attached light sources are largely in place: `WorldLightPayload` carries
`attachedTokenId` and `systems/lighting.rs` resolves a light's position from
its bearer. What is missing is the interface to attach one and the rules layer
around it — not the engine seam.
