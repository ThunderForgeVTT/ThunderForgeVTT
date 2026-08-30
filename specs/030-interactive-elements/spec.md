# Feature Specification: Interactive Elements — Props, Doors and Triggers

**Feature Branch**: `030-interactive-elements`

**Created**: 2026-08-30

**Status**: Draft

**Input**: User description: "The ability for the GM to place a token that's not an actor — a book, a shield, a pile of items, a chest, a table — and trigger things. A book that when clicked opens a lore page in a new browser tab. A switch on a wall that turns lighting in a region of the scene on or off. A threshold the players cross that plays a sound effect (we haven't wired up sound effects or music yet). We have a wall tool but we need a tool for designating doors, and what does open or close mean — imagine a room with one door; on click it should open or close, and the GM can right-click to shut and lock it. A region or token that transitions a load of another scene: Player A has requested to transition to scene B and the GM can approve it (multi-scene management is future work). A click on an item on screen can open a secret door that the GM has locked. Spec this out as if it's not a way to build a game without a GM, but a way for the GM to make the game sparkle."

## Overview

A scene today is furniture that cannot be touched. Walls stop movement, lights
cast shadows, tokens represent creatures — and nothing on the map answers when
a player reaches for it. The door in a room is a wall segment that either
blocks or does not; there is no way to open it, no way to lock it, and no
agreed meaning for either word.

This feature gives the Game Master a way to place things that respond. A book
that opens a lore page. A lever that lights a corridor. A door that a player
can open but only the GM can lock. A chest, a table, a pile of gear — objects
that are part of the fiction rather than decoration.

### The framing this whole spec answers to

**This is not a way to run a game without a Game Master.** It is a way for a
Game Master to make a prepared scene feel alive between the moments they are
speaking.

That distinction is a design constraint, not a slogan, and it decides
arguments throughout:

- Every interactive exists because a GM placed it and said what it does.
  Nothing infers, suggests, or generates one.
- An interactive performs an **authored effect**, never an adjudication. It
  can open a door the GM designated as openable. It cannot decide whether a
  character notices the door, succeeds at picking its lock, or is allowed
  through — those are the GM's, and the table's.
- Anything with consequences beyond the immediate object stays **gated on GM
  approval**. A player may ask to leave for another scene; only the GM says
  yes.
- The effect vocabulary is a **closed, declared set**, not a scripting
  language. A GM composes from things the product can explain; they do not
  program. This is the difference between a tool that makes preparation
  faster and one that makes preparation a second job.

## The Interaction Plugin Model

The architectural heart of the feature, and the reason it is worth building as
one thing rather than six.

### Effects are contributed, not enumerated

A fixed list would mean editing this feature every time a subsystem gains
something worth triggering — precisely the coupling the project's plugin
principle exists to prevent. Instead each subsystem **declares the effects it
can perform**, and the authorable vocabulary is the union of what is present.

A declaration carries what a Game Master needs in order to choose it, and what
the product needs in order to store it: a stable identifier, a label and
description in the GM's language, what kind of thing it targets, and what it
needs configured.

### What this feature is not allowed to know

It must be possible to build the interaction feature with no lighting, no
doors and no audio present and have it work — offering nothing, because
nothing has been contributed. Concretely:

- The interaction feature's own logic MUST NOT reference lights, doors or
  sounds.
- Subsystems MUST NOT call into its internals, nor it into theirs. An
  activation is announced; whichever subsystem owns that effect responds.
- Removing a subsystem MUST NOT break this feature, the scene, or any
  interactive that does not use it.

This is the discipline spec 029 applied to game systems, one level up: there
the engine renders resources it cannot name; here it dispatches effects it
cannot name.

### What happens to an interactive whose subsystem is gone

An authored interactive outlives its provider — a scene saved with a lighting
trigger, opened in a build without lighting. It MUST become **unavailable and
visibly so to the Game Master**, and MUST NOT be deleted, silently skipped, or
surfaced to the table as an error. A prepared scene is work; losing part of it
because a capability was absent for one session is not acceptable.

### Why this is the right shape for what is coming

Each named non-goal below — audio, multi-scene navigation, party tokens, space
travel — is a subsystem that will want to be triggerable. Under this model each
arrives by contributing effects, without reopening this feature. That is the
return on building the seam now instead of a fixed list of six effects.

## User Scenarios & Testing _(mandatory)_

### User Story 1 - A prop that opens something (Priority: P1)

A Game Master preparing a library scene places a book on a reading desk. It is
not a creature, has no character sheet, and takes no turn. They attach a link
to a lore entry they have already written. During play a player clicks the
book and the lore page opens in a new tab, leaving the table's view of the
scene undisturbed.

**Why this priority**: It is the smallest complete slice through the whole
idea — placing a non-actor object, authoring behaviour on it, and having a
click produce that behaviour for the right people. Every later story reuses
that spine. It is also immediately useful on its own: handouts and lore
currently reach players by being pasted into chat.

**Independent Test**: Place a prop, attach a link, click it as a player,
observe the page open. Delivers usable handout-in-the-scene behaviour with no
other story built.

**Acceptance Scenarios**:

1. **Given** a GM in scene edit mode, **When** they place a prop and give it a
   link to a lore entry, **Then** the prop appears on the map for the table and
   is marked as interactive.
2. **Given** a player viewing that scene, **When** they click the prop, **Then**
   the linked page opens in a new tab and the scene stays exactly as it was.
3. **Given** a prop with no link attached, **When** a player clicks it,
   **Then** nothing happens and no error is shown — an object that does nothing
   is a legitimate piece of scenery.
4. **Given** a player who is not a member of the world, **When** the scene is
   viewed, **Then** no interactive is offered to them at all.

---

### User Story 2 - Doors that open, close and lock (Priority: P2)

The wall tool can draw a wall. It cannot say "this part is a door". A GM
designates a wall segment as a door; during play a player clicks it to open or
close it, and the GM can right-click to shut and lock it. A locked door does
not answer a player's click.

**Why this priority**: The largest gap between what a table expects and what
exists, and the most-used interactive in any dungeon. It is P2 rather than P1
only because it needs the click handling, permission model and state
propagation that Story 1 establishes — not because it matters less. In table
terms it probably matters most.

**Independent Test**: Designate a door, open and close it as a player, watch
vision and movement change, lock it as the GM and confirm the player's click
no longer works.

**Acceptance Scenarios**:

1. **Given** a wall segment, **When** the GM designates it a door, **Then** it
   is drawn distinguishably from a plain wall and starts closed.
2. **Given** a closed door, **When** a player clicks it, **Then** it opens: it
   stops blocking vision and movement, and every connected client sees the
   change without reloading.
3. **Given** an open door, **When** a player clicks it, **Then** it closes and
   resumes blocking exactly what its wall was drawn to block.
4. **Given** any door, **When** the GM right-clicks it, **Then** they are
   offered shut and lock, and locking takes effect immediately.
5. **Given** a locked door, **When** a player clicks it, **Then** it does not
   open, and the player is told it is locked rather than being ignored.
6. **Given** a locked door, **When** the GM clicks it, **Then** it opens — the
   lock governs the table, not the person running it.

---

### User Story 3 - A switch that changes the lighting (Priority: P2)

A GM places a lever on a corridor wall and associates it with the lights in
that corridor. A player pulls the lever and the corridor goes dark, or comes
alight.

**Why this priority**: It is the first effect that changes the _scene_ rather
than opening something beside it, and lighting is the most atmospheric thing
the engine already does. It proves the effect vocabulary extends past
navigation.

**Independent Test**: Place a switch, associate lights, click as a player, see
the lights change for everybody.

**Acceptance Scenarios**:

1. **Given** a GM editing a scene, **When** they place a switch and associate
   one or more lights with it, **Then** the association is visible to them while
   editing and invisible to players.
2. **Given** an associated switch, **When** a player activates it, **Then**
   those lights toggle for every viewer of the scene, and shadows re-resolve.
3. **Given** a switch whose associated light has since been deleted, **When**
   a player activates it, **Then** the remaining lights still toggle and the GM
   is told about the missing one — a broken association must not silently make a
   switch dead.

---

### User Story 4 - A secret the GM chooses to reveal (Priority: P3)

A GM prepares a study with a bookshelf. Behind it is a passage the players do
not know about. Pulling a specific candlestick opens it. Until then, the
passage is not a door as far as the table is concerned.

**Why this priority**: It is the case that makes interactives feel like a
prepared dungeon rather than a diagram, but it depends on doors existing and
on a decision about how secrets are protected.

**Independent Test**: Prepare a secret door, confirm it is not presented to
players, trigger it from another interactive, confirm it becomes usable.

**Acceptance Scenarios**:

1. **Given** a door marked secret, **When** a player views the scene, **Then**
   it is not drawn or offered as a door.
2. **Given** a secret door, **When** the GM views the scene, **Then** it is
   drawn distinguishably as a secret they have placed.
3. **Given** an interactive configured to reveal it, **When** a player
   activates that interactive, **Then** the passage becomes a normal door for
   everybody from that moment.
4. **Given** a revealed secret door, **When** the scene is reopened later,
   **Then** it is still revealed — a revelation is a fact about the world, not a
   display state.

---

### User Story 5 - Something that happens when players arrive (Priority: P3)

A GM marks the threshold of a chamber. When a player token crosses it,
something happens — the GM is notified, or an authored effect fires.

**Why this priority**: Regions generalise interactives from "things you click"
to "places that matter", which is what the future world-map and travel work
needs. It is P3 because every effect it can fire is already reachable by
clicking, so it adds reach rather than capability.

**Independent Test**: Define a region, move a token across its boundary,
observe the effect fire exactly once.

**Acceptance Scenarios**:

1. **Given** a region with an effect, **When** a player token crosses into it,
   **Then** the effect fires once.
2. **Given** a token already inside a region, **When** it moves within the
   region, **Then** the effect does not fire again.
3. **Given** a region set to fire once ever, **When** a second token enters
   after the first, **Then** it does not fire again and the GM can reset it.
4. **Given** a GM moving a token during preparation, **When** it crosses a
   region, **Then** the effect does not fire — arranging a scene is not playing
   it.

---

### User Story 6 - A player asks, the GM decides (Priority: P3)

A player's token reaches a staircase leading out of the map. They ask to take
it. The GM sees the request and approves or refuses it. Nothing moves the
table anywhere until they do.

**Why this priority**: It establishes the approval pattern the framing
demands, and it is the shape the future scene-transition and travel work will
use. It is P3 because the thing being requested — loading another scene for
everybody — does not exist yet, so this story delivers the request and the
decision, not the destination.

**Independent Test**: Trigger a request as a player, observe it reach the GM,
approve and refuse it, and confirm neither outcome moves anybody until the GM
acts.

**Acceptance Scenarios**:

1. **Given** an interactive marked as requiring approval, **When** a player
   activates it, **Then** the GM receives a request naming the player, the
   interactive and what is being asked.
2. **Given** a pending request, **When** the GM refuses it, **Then** the player
   is told and nothing changes.
3. **Given** a pending request, **When** the GM approves it, **Then** the
   authored effect runs.
4. **Given** a pending request, **When** the GM does nothing, **Then** nothing
   happens — silence is not consent, and a request must never time out into
   approval.

---

### User Story 7 - A new subsystem becomes triggerable (Priority: P3)

Audio does not exist yet. When it is built, a Game Master should be able to
attach a sound to a threshold without this feature being reopened, and without
the audio work having to understand how interactives are placed, permitted or
approved.

**Why this priority**: It delivers no table-visible behaviour on its own,
which is why it is last. It is nonetheless the reason the feature is shaped
this way, and leaving it untested would mean discovering the seam does not
work at the moment a second subsystem arrives — when it is most expensive to
find out.

**Independent Test**: Add a trivial effect provider that does something
observable and nothing else. Confirm it becomes authorable, runs when
triggered, and that removing it leaves every other interactive working.

**Acceptance Scenarios**:

1. **Given** a build with a new subsystem present, **When** a Game Master
   authors an interactive, **Then** that subsystem's effects are offered
   alongside the existing ones, with no change to this feature.
2. **Given** a build with lighting absent, **When** a Game Master authors an
   interactive, **Then** no lighting effect is offered, and everything else
   works.
3. **Given** a scene containing a lighting trigger, **When** it is opened in a
   build without lighting, **Then** the Game Master is shown that the
   interactive is unavailable, and it is neither deleted nor silently ignored.
4. **Given** any subsystem, **When** its effect runs, **Then** it did not
   require this feature to know what the effect does.

---

### Edge Cases

- **Two players click the same door in the same instant.** The door must end
  in one state both of them can see, not oscillate or diverge per client.
- **A GM locks a door while a player's click is in flight.** The lock decides;
  a click that raced it must not open the door.
- **An interactive points at something that no longer exists** — a deleted
  lore entry, a removed light, a scene that was deleted. The interactive must
  fail visibly for the GM and harmlessly for the player.
- **A prop is placed on top of a creature token.** Clicking must resolve
  predictably rather than depending on draw order.
- **An interactive links to an external address.** Opening arbitrary
  destinations from a shared table is a way to send players somewhere hostile;
  what a link may point at needs an explicit answer rather than a default.
- **A player leaves mid-request.** A pending approval whose requester has gone
  must not sit in the GM's queue forever.
- **A region is drawn overlapping another region.** A token crossing both must
  fire both, in a defined order, rather than one arbitrarily winning.
- **A door is designated on a wall that is later deleted.** The door goes with
  it, and any interactive that targeted it fails visibly for the GM.

## Requirements _(mandatory)_

### Functional Requirements

#### Authoring

- **FR-001**: A Game Master MUST be able to place an object on a scene that is
  not a creature — it has no character sheet, takes no turn, and appears in no
  initiative or party list.
- **FR-002**: A Game Master MUST be able to give such an object an appearance
  drawn from the scene's available artwork, and a name.
- **FR-003**: A Game Master MUST be able to attach at most one effect to an
  interactive, chosen from a closed set the product defines.
- **FR-004**: The product MUST NOT offer a general-purpose scripting or
  expression language for effects. Authoring is composition from named effects.
- **FR-005**: Only a Game Master MUST be able to create, edit or delete an
  interactive or its effect.
- **FR-006**: A Game Master MUST be able to see, while editing, which objects
  are interactive and what each one targets; players MUST NOT see that
  authoring view.

#### Doors

Doors are a contributing subsystem under the model above, not part of the
interaction core. What open, closed and locked _mean_ belongs with doors; the
interaction feature only knows that something contributed an effect and that a
Game Master chose it.

- **FR-007**: A Game Master MUST be able to designate a segment of an existing
  wall as a door, without redrawing the wall.
- **FR-008**: The product MUST define **open** as: the segment blocks neither
  vision nor movement, whatever the wall was drawn to block.
- **FR-009**: The product MUST define **closed** as: the segment blocks
  exactly what the wall was drawn to block. A closed window that blocks
  movement but not vision therefore remains see-through, and a closed stone
  door blocks both.
- **FR-010**: **Locked** MUST be a separate property from open and closed,
  governing _who may change the state_ rather than being a third state. This
  keeps "a door that is open and cannot be closed by players" expressible, which
  a three-state model cannot say.
- **FR-011**: A player MUST be able to open and close an unlocked door by
  clicking it.
- **FR-012**: A player MUST NOT be able to open, close, lock or unlock a
  locked door.
- **FR-013**: A Game Master MUST be able to lock and unlock a door, and MUST be
  able to change a locked door's state themselves.
- **FR-014**: A player attempting a locked door MUST be told it is locked.
  Silence is indistinguishable from the product being broken.
- **FR-015**: A door's state MUST persist with the scene, so a dungeon
  reopened next session is as the table left it.

#### Effects

- **FR-016**: The effect set MUST include opening a linked page belonging to
  this world, without navigating the table away from the scene.
- **FR-017**: The effect set MUST include toggling one or more named lights in
  the scene.
- **FR-018**: The effect set MUST include changing a door's state, including
  revealing a secret door.
- **FR-019**: An effect whose target no longer exists MUST report that to the
  Game Master and MUST NOT break the interactive that carries it or the scene
  around it.
- **FR-020**: Every effect that changes the scene MUST reach every connected
  viewer without a reload.
- **FR-021**: The effect vocabulary MUST be the set of effects contributed by
  subsystems present in the build, not a list held by this feature.

#### The effect plugin seam

- **FR-036**: A subsystem MUST be able to contribute one or more effects by
  declaring them, without any change to this feature.
- **FR-037**: An effect declaration MUST carry a stable identifier, a label
  and description in a Game Master's language, what kind of thing it targets,
  and what it needs configured.
- **FR-038**: A Game Master MUST be offered exactly the effects contributed by
  the current build — no effect that nothing can perform, and no effect hidden
  behind a flag.
- **FR-039**: This feature's own logic MUST NOT reference any specific effect,
  target type or subsystem. Removing every contributing subsystem MUST leave a
  feature that places interactives offering no effects, rather than a broken
  one.
- **FR-040**: Activation MUST be announced to whichever subsystem owns the
  effect, rather than performed by this feature. Neither side may call into the
  other's internals.
- **FR-041**: An interactive whose contributing subsystem is absent MUST be
  reported as unavailable to the Game Master, MUST NOT be deleted or rewritten,
  and MUST NOT surface as an error to players.
- **FR-042**: Two subsystems MUST NOT be able to contribute the same effect
  identifier; a collision MUST be detected rather than silently resolved by
  load order.
- **FR-043**: An effect that fails while running MUST report to the Game
  Master and MUST NOT prevent other interactives in the scene from working.

#### Triggers and permission

- **FR-022**: An interactive MUST be activatable by clicking it, where the
  viewer is permitted.
- **FR-023**: A Game Master MUST have a distinct secondary interaction on an
  interactive offering the authoring and override actions — at minimum, for a
  door, shut and lock.
- **FR-024**: A Game Master MUST be able to restrict an interactive to
  themselves, so a prepared trigger is not available to the table.
- **FR-025**: A Game Master MUST be able to mark an interactive as requiring
  their approval; activating it then raises a request rather than running the
  effect.
- **FR-026**: A pending request MUST identify the requesting player, the
  interactive, and what would happen.
- **FR-027**: A request MUST NOT expire into approval, and MUST NOT run its
  effect until the Game Master approves it.
- **FR-028**: A Game Master MUST be able to refuse a request, and the
  requesting player MUST be told.

#### Regions

- **FR-029**: A Game Master MUST be able to define an area of a scene as a
  region and attach an effect to it.
- **FR-030**: A region effect MUST fire when a token crosses into the region,
  once per entry rather than continuously while inside.
- **FR-031**: A Game Master MUST be able to set a region to fire once ever, and
  to reset it.
- **FR-032**: Token movement performed by a Game Master while preparing a
  scene MUST NOT fire region effects.

#### The framing, as testable requirements

- **FR-033**: Every interactive MUST originate from an explicit Game Master
  action. The product MUST NOT create, infer or suggest interactives.
- **FR-034**: No effect may resolve an outcome that belongs to the table —
  whether a character perceives, succeeds, is permitted, or is affected. Effects
  change scene state that a Game Master has already decided upon.
- **FR-035**: A Game Master MUST be able to perform, by hand, everything an
  interactive does, so that no prepared trigger becomes the only route to a
  state.

### Key Entities

- **Interactive**: Something on a scene that responds. Carries what it is
  attached to, who may activate it, whether it needs approval, and at most one
  effect. Belongs to a scene.
- **Prop**: A placed object that is not a creature. Has a position, an
  appearance and a name; no sheet, no turn, no owner playing it.
- **Door**: A designated segment of a wall with a state (open or closed), a
  lock, and whether it is secret. Derives what it blocks from the wall it is
  part of.
- **Region**: A bounded area of a scene that can carry an effect and fires on
  entry.
- **Effect**: A named, closed-vocabulary action — open a page, toggle lights,
  set a door's state, reveal a secret.
- **Approval Request**: A player's activation of a gated interactive, awaiting
  a Game Master's decision. Carries requester, interactive, proposed effect and
  outcome.

## Success Criteria _(mandatory)_

### Measurable Outcomes

- **SC-001**: A Game Master can place an interactive object and attach a
  working effect to it in under two minutes, without consulting documentation.
- **SC-002**: A Game Master can designate a door on an existing wall in under
  thirty seconds.
- **SC-003**: A player activating an interactive sees the result, and every
  other viewer of the scene sees it, within one second, with no reload.
- **SC-004**: No sequence of player actions can change a locked door's state,
  reveal a door the Game Master has marked secret and not revealed, or run a
  gated effect without approval. Verified by attempting each.
- **SC-005**: Concurrent activation of the same interactive by two players
  leaves every viewer seeing the same state.
- **SC-006**: An interactive whose target has been deleted produces a message
  the Game Master can act on, and leaves the scene usable.
- **SC-007**: A scene with 50 interactive elements loads and remains as
  responsive as the same scene without them, measured against the documented
  engine baseline.
- **SC-008**: A Game Master reopening a session finds every door, secret and
  region in the state the table left it.
- **SC-009**: A new effect can be contributed by a subsystem, and appear as
  authorable, with no edit to the interaction feature. Demonstrated by adding
  one.
- **SC-010**: With every contributing subsystem removed, scenes still load,
  interactives can still be placed, and no error reaches a player.
- **SC-011**: A scene authored against a subsystem that is then removed loses
  no authored data, and the Game Master can see exactly which interactives are
  unavailable.

## Assumptions

- **Interactives are scene-scoped.** An interactive belongs to one scene and
  does not follow anything between scenes.
- **One effect per interactive.** Sequences of effects are a scripting language
  in disguise; a GM wanting two things can place two objects. Revisited only if
  play shows it is genuinely limiting.
- **Props reuse the existing object token kind** rather than introducing a
  parallel kind of thing to place, keeping one placement and artwork pipeline.
- **Doors extend the existing wall model** rather than becoming separate
  geometry. Walls already carry a door state of none, open or closed; locking
  and secrecy are additions to that, not a replacement for it.
- **Approval requests are transient.** They live for the session and are not
  history the product needs to retain.
- **A right-click already means "GM options" elsewhere in the canvas**, and
  this follows that convention rather than inventing a gesture.
- **Existing per-viewer decisions carry over, including for prepared
  secrets.** A player inspecting their own client to find a secret door is a
  table problem, not an engineering one — see Decisions. Secret geometry and
  its metadata may travel to clients that do not draw it.

## Dependencies

- **Walls** exist, carry what they block, and already have a door state of
  none, open or closed. This feature extends that model; it does not introduce
  a second one.
- **Lights** exist and can already be attached to a token.
- **Lore entries** exist and have their own pages, which is what a linked prop
  opens.
- **Live scene updates** exist — scene changes already reach connected clients
  without a reload, which FR-020 depends on.
- **Sound and music do not exist anywhere in this project.** No audio is
  loaded, stored, mixed or played. Under the plugin model this is not a
  blocker: no audio subsystem means no sound effect is contributed, so none is
  offered. The threshold trigger itself is built here; what it fires when audio
  exists is contributed then.
- **Multi-scene navigation does not exist.** There is no way to move a table
  from one scene to another. The same applies: this feature builds the request
  and the Game Master's decision, and whatever performs the journey contributes
  its effect when it exists.

## Out of Scope

Named explicitly, because each is a thing a reader of this spec might
reasonably expect to find in it:

- **An audio subsystem.** Loading, storing, mixing or playing sound and music.
  See the open question below on how the trigger is treated in the meantime.
- **Multi-scene management.** Moving a table between scenes, scene ordering,
  or what a "current scene" means for a party. This spec covers a player asking
  and a Game Master deciding; it does not cover the journey.
- **Party tokens.** A token the Game Master controls that the whole party sees
  and follows — a ship, a caravan — for scenes that are world maps rather than
  battle maps. This is coming, and the region and travel model here is shaped
  to admit it, but it is not built here.
- **Space travel and hex-grid world maps.** A future game system will want a
  hex of open space where a party token triggers travel to planets. Regions and
  gated requests are the pieces that will serve it; the system itself is
  separate work.
- **Scripting, conditionals, variables, or chained effects.** See FR-004.
- **Anything that adjudicates.** Perception, lockpicking, saving throws,
  whether a character may pass. See FR-034.
- **Inventory and loot.** A chest is an object that can be interactive. What is
  inside it, and moving that to a character, is a separate feature.

## Decisions

Both questions this spec opened have been answered.

### Secrets are protected by the table, not by the wire

A secret door is sent to every client and hidden by the client that should not
draw it, exactly as token visibility was decided. Coordinates and metadata
reaching a player who could dig them out is acceptable: if somebody opens
their developer tools to announce "there's a secret door here", that is a
problem for that table to have, not one for this product to engineer against.

The cost of the alternative is what settles it. Withholding prepared content
per viewer means filtering scene data per viewer, which is the fan-out expense
the visibility decision was taken to avoid — paid permanently, on every scene
load, to frustrate somebody who has already decided to spoil their own game.

This applies to every secret here: unrevealed doors, GM-only interactives, and
the authoring view of what an interactive targets.

### The effect vocabulary is a plugin registry, not a fixed list

The question was what to do about effects whose subsystem does not exist —
sound has no audio subsystem, scene transitions have no multi-scene
navigation. Ship them dead, build the subsystem here, or defer them.

The answer removes the question rather than choosing between those. **Effects
are contributed by the subsystems that perform them.** This feature owns
placing, triggering, permission and dispatch; it owns no effect at all.
Lighting contributes the effect that toggles lights. Doors contribute open,
close, lock and reveal. When audio is built it contributes a play-sound
effect, and nothing here changes.

So a Game Master can only ever author an effect that something is present to
perform. Sound is not authorable today because nothing offers it — not because
it is disabled, and not because it silently does nothing. That last one is the
failure this project keeps finding, and this shape does not have it.
